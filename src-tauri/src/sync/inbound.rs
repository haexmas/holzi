//! Receiving a pull (spec 024, FR-013, FR-014, FR-019, research R4, R5).
//!
//! Pages are grouped by transaction HLC; a group has exactly one origin. A
//! group still continuing on the next page stays buffered and is applied
//! once complete. Per complete group:
//!
//! - a table this device does not sync aborts the pull without progress (a
//!   schema mismatch the handshake should have caught, or a device-local
//!   table another device must never send);
//! - a group larger than `max_transaction_bytes` by haex-crdt's own rule
//!   aborts the pull;
//! - a group whose origin a known valid device list names as removed, with
//!   an HLC beyond its limit, is rejected (R5);
//! - a group whose rows cannot be created yet, because the cells they lack
//!   come in a later group, waits ([`hold`]);
//! - the rest goes to `Database::apply_remote_changes` in one transaction.
//!
//! Only after that commit does progress rise, per origin to the highest
//! group applied or rejected that lies below every group held, and after the
//! last page to the progress the sender served against. An origin on no list is accepted: an own device
//! vouches for what it delivers (FR-021).

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap, HashSet};

use haex_crdt::{compare_hlc_strings, ColumnChange};
use uuid::Uuid;

use crate::storage::query;
use crate::sync::change::{group_bytes, join_parts, Change, ChangeError, Page};
use crate::sync::device_list;
use crate::sync::keys;
use crate::sync::progress::{self, Vector};
use crate::sync::replica::{synced_tables, Replica};
use crate::sync::resync::RowKey;

#[path = "inbound_hold.rs"]
mod hold;
use hold::{settle, Group, Settled};

/// haex-crdt's delete log, a synced table like any other.
const DELETED_ROWS_TABLE: &str = "haex_deleted_rows";

/// Slack on top of `max_transaction_bytes` for a buffered group's encoded
/// size, which exceeds its SQL size by quoting and hex.
const BUFFER_SLACK: usize = 1024 * 1024;

/// Why a pull stops; nothing of the failing page is applied.
#[derive(Debug, thiserror::Error)]
pub enum InboundError {
    #[error("the pull names table {0}, which this device does not sync")]
    UnknownTable(String),
    #[error("a transaction group of {bytes} bytes exceeds the limit of {limit} bytes")]
    GroupTooLarge { bytes: usize, limit: usize },
    #[error("malformed page: {0}")]
    Malformed(&'static str),
    #[error(transparent)]
    Change(#[from] ChangeError),
    #[error(transparent)]
    Crdt(#[from] haex_crdt::Error),
}

/// What one page did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Received {
    /// Tables with at least one applied change, for the follow-up work after a pull.
    pub tables: BTreeSet<String>,
    /// Groups rejected under research R5.
    pub rejected_groups: usize,
    /// This was the last page of the pull.
    pub done: bool,
}

/// The receiving side of one pull.
#[derive(Debug, Default)]
pub struct Inbox {
    /// The start of a group that continues on the next page.
    pending: Vec<Change>,
    /// Complete groups waiting for the cells their rows lack ([`hold`]).
    held: Vec<Group>,
    /// The last complete group received for each origin in this pull.
    /// Retaining this across pages prevents a malformed pull from advancing
    /// an origin past a later group and then applying an older group that can
    /// never be requested again.
    last_received: Vector,
    finished: bool,
    /// Set for the pull of a resync snapshot: what it carried.
    snapshot: Option<Snapshot>,
}

/// What a snapshot pull carried, for pruning what it did not
/// ([`crate::sync::resync`]).
#[derive(Debug, Default)]
struct Snapshot {
    rows: HashSet<RowKey>,
    served: Vector,
}

impl Inbox {
    /// Creates an empty receiver for an ordinary pull.
    pub fn new() -> Self {
        Self::default()
    }

    /// An inbox for the pull of a resync snapshot, which remembers the rows
    /// it applied.
    pub fn for_snapshot() -> Self {
        Self {
            snapshot: Some(Snapshot::default()),
            ..Self::default()
        }
    }

    /// The rows a finished snapshot carried and the progress its sender
    /// served against; `None` for an ordinary pull or an unfinished one.
    pub fn into_snapshot(self) -> Option<(HashSet<RowKey>, Vector)> {
        self.snapshot
            .filter(|_| self.finished)
            .map(|s| (s.rows, s.served))
    }

    /// Checks and applies the complete groups of `page`.
    pub fn receive(&mut self, replica: &Replica, page: Page) -> Result<Received, InboundError> {
        if self.finished {
            return Err(InboundError::Malformed("a page after the last one"));
        }
        if page.group_continues && (!page.more || page.changes.is_empty()) {
            return Err(InboundError::Malformed(
                "a continuing group on the last page or on an empty page",
            ));
        }
        let db = replica.db();
        let limit = db.max_transaction_bytes();
        // Device removal publishes its limit under the same exchange lock. Read the
        // current limits only after acquiring that lock and keep it until the changes
        // and their progress are committed, so a removal cannot race this admission
        // decision (FR-027, research R5).
        let _exchange = replica.exchange();

        let mut changes = std::mem::take(&mut self.pending);
        changes.extend(page.changes);
        if page.group_continues {
            let open_hlc = changes.last().map(|c| c.hlc.clone());
            let split = changes
                .iter()
                .rposition(|c| Some(&c.hlc) != open_hlc.as_ref())
                .map_or(0, |i| i + 1);
            self.pending = changes.split_off(split);
            let buffered: usize = self.pending.iter().map(Change::wire_size).sum();
            if buffered > limit.saturating_mul(2).saturating_add(BUFFER_SLACK) {
                return Err(InboundError::GroupTooLarge {
                    bytes: buffered,
                    limit,
                });
            }
        }

        let groups = groups(join_parts(changes)?)?;
        let tables: HashSet<String> = query::read(db, |r| synced_tables(r))?.into_iter().collect();
        let limits = query::read(db, |r| removal_limits(r))?;

        let mut received = Received {
            done: !page.more,
            ..Received::default()
        };
        let mut arrived: Vec<Group> = Vec::new();
        let mut rejected: Vec<(Uuid, String)> = Vec::new();
        let mut group_updates = Vector::new();
        for (hlc, group) in groups {
            let origin = progress::origin_of(&hlc)
                .ok_or(InboundError::Malformed("an HLC without origin"))?;
            if self
                .last_received
                .get(&origin)
                .is_some_and(|last| compare_hlc_strings(&hlc, last) != Ordering::Greater)
            {
                return Err(InboundError::Malformed(
                    "groups out of HLC order across pages",
                ));
            }
            let columns = group
                .iter()
                .map(|change| {
                    if !tables.contains(&change.table) {
                        return Err(InboundError::UnknownTable(change.table.clone()));
                    }
                    Ok(change.to_column()?)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let bytes = group_bytes(&columns)?;
            if bytes > limit {
                return Err(InboundError::GroupTooLarge { bytes, limit });
            }
            progress::raise(&mut group_updates, origin, hlc.clone());
            if is_removed_at(&limits, origin, &hlc) {
                received.rejected_groups += 1;
                rejected.push((origin, hlc));
                continue;
            }
            arrived.push(Group {
                origin,
                hlc,
                columns,
            });
        }
        let mut candidates = std::mem::take(&mut self.held);
        candidates.extend(arrived);
        let Settled { apply, held } = settle(db, candidates, !page.more)?;
        let held_bytes = held.iter().try_fold(0usize, |total, group| {
            group_bytes(&group.columns).map(|bytes| total.saturating_add(bytes))
        })?;
        if held_bytes > limit.saturating_mul(2).saturating_add(BUFFER_SLACK) {
            return Err(InboundError::GroupTooLarge {
                bytes: held_bytes,
                limit,
            });
        }
        received.tables.extend(
            apply
                .iter()
                .flat_map(|g| g.columns.iter().filter_map(changed_table)),
        );
        let accepted: Vec<ColumnChange> = apply
            .iter()
            .flat_map(|g| g.columns.iter().cloned())
            .collect();
        // Progress rises only below every group still held: those come
        // again in a pull that starts from it.
        let mut lowest_held: HashMap<Uuid, String> = HashMap::new();
        for group in &held {
            let lowest = lowest_held
                .entry(group.origin)
                .or_insert_with(|| group.hlc.clone());
            if compare_hlc_strings(&group.hlc, lowest) == Ordering::Less {
                *lowest = group.hlc.clone();
            }
        }
        let mut updates = Vector::new();
        for (origin, hlc) in rejected
            .into_iter()
            .chain(apply.iter().map(|g| (g.origin, g.hlc.clone())))
        {
            if lowest_held
                .get(&origin)
                .is_none_or(|lowest| compare_hlc_strings(&hlc, lowest) == Ordering::Less)
            {
                progress::raise(&mut updates, origin, hlc);
            }
        }
        self.held = held;
        if let Some(snapshot) = &mut self.snapshot {
            snapshot.rows.extend(
                accepted
                    .iter()
                    .map(|c| (c.table_name.clone(), c.row_pks.clone())),
            );
        }
        if !page.more {
            if let Some(snapshot) = &mut self.snapshot {
                snapshot.served = page.served;
            } else {
                for (origin, hlc) in page.served {
                    progress::raise(&mut updates, origin, hlc);
                }
            }
            self.finished = true;
        }

        if !accepted.is_empty() {
            let outcome = db.apply_remote_changes(accepted)?;
            if !outcome.skipped.is_empty() {
                log::debug!(
                    "sync: {} received cells kept the local value",
                    outcome.skipped.len()
                );
            }
        }
        if !updates.is_empty() {
            db.write(|tx| progress::advance(tx, &updates))?;
        }
        for (origin, hlc) in group_updates {
            progress::raise(&mut self.last_received, origin, hlc);
        }
        Ok(received)
    }
}

/// The table a change touches as the views see it: for a delete marker the
/// table whose row it deletes.
fn changed_table(change: &ColumnChange) -> Option<String> {
    if change.table_name != DELETED_ROWS_TABLE {
        return Some(change.table_name.clone());
    }
    (change.column_name == "table_name")
        .then(|| change.value.as_str().map(str::to_string))
        .flatten()
}

/// Splits `changes` into groups of one transaction HLC, which must follow
/// in ascending order.
fn groups(changes: Vec<Change>) -> Result<Vec<(String, Vec<Change>)>, InboundError> {
    let mut groups: Vec<(String, Vec<Change>)> = Vec::new();
    for change in changes {
        match groups.last_mut() {
            Some((hlc, group)) if *hlc == change.hlc => group.push(change),
            Some((hlc, _)) if compare_hlc_strings(&change.hlc, hlc) != Ordering::Greater => {
                return Err(InboundError::Malformed("groups out of HLC order"));
            }
            _ => groups.push((change.hlc.clone(), vec![change])),
        }
    }
    Ok(groups)
}

/// Per removed origin the smallest limit any known valid list sets.
fn removal_limits(q: &mut impl query::Query) -> haex_crdt::Result<HashMap<Uuid, String>> {
    let Some(vault) = keys::vault_pubkey(q)? else {
        return Ok(HashMap::new());
    };
    let valid = device_list::valid_lists(&device_list::load_all(q)?, &vault);
    let mut limits: HashMap<Uuid, String> = HashMap::new();
    for removed in valid.values().flat_map(|signed| &signed.list.removed) {
        let limit = limits
            .entry(removed.vault_device_uuid)
            .or_insert_with(|| removed.limit_hlc.clone());
        if compare_hlc_strings(&removed.limit_hlc, limit) == Ordering::Less {
            *limit = removed.limit_hlc.clone();
        }
    }
    Ok(limits)
}

/// Whether a group of `origin` at `hlc` lies beyond its removal limit.
fn is_removed_at(limits: &HashMap<Uuid, String>, origin: Uuid, hlc: &str) -> bool {
    limits
        .get(&origin)
        .is_some_and(|limit| compare_hlc_strings(hlc, limit) == Ordering::Greater)
}

#[cfg(test)]
#[path = "inbound_tests.rs"]
mod tests;
