//! Receiving a pull (spec 024, FR-013, FR-014, FR-019, research R4, R5).
//!
//! Pages are grouped by transaction HLC; a group has exactly one origin. A
//! group still continuing on the next page stays buffered and is applied
//! once complete. Per complete group:
//!
//! - a table this device does not sync aborts the pull without progress (a
//!   schema mismatch the handshake should have caught, or a device-local
//!   table another device must never send), unless it is an extension's
//!   table this device has not created yet: then the group is parked
//!   ([`park`], spec 017 research R10), or, when a migration this device
//!   applied dropped that table or column, it waits for its origin to update;
//! - a group larger than `max_transaction_bytes` by haex-crdt's own rule
//!   aborts the pull;
//! - a group whose origin a known valid device list names as removed, with
//!   an HLC beyond its limit, is rejected (R5);
//! - a group whose rows cannot be created yet, because the cells they lack
//!   come in a later group, waits ([`hold`]);
//! - the rest goes to `Database::apply_remote_changes`, a few hundred groups per transaction,
//!   so a close waits for one such step at most and then stops the pull.
//!
//! Only after the last commit does progress rise, per origin to the highest
//! group applied or rejected that lies below every group held, and after the
//! last page to the progress the sender served against. An origin on no list is accepted: an own device
//! vouches for what it delivers (FR-021).

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap, HashSet};

use haex_crdt::{compare_hlc_strings, ColumnChange, SkipReason};
use uuid::Uuid;

use crate::storage::query;
use crate::sync::change::{group_bytes, join_parts, Change, ChangeError, Page};
use crate::sync::device_list;
use crate::sync::keys;
use crate::sync::progress::{self, Vector};
use crate::sync::replica::Replica;
use crate::sync::resync::RowKey;

#[path = "inbound_hold.rs"]
mod hold;
use hold::{settle, steps, Group, Settled};

#[path = "inbound_park.rs"]
pub mod park;
use park::{Context, Sorted};

#[path = "inbound_receive_park.rs"]
mod receive_park;
use receive_park::Parking;

/// haex-crdt's delete log, a synced table like any other.
const DELETED_ROWS_TABLE: &str = "haex_deleted_rows";

// ponytail: 256 groups per transaction. Ceiling: a close waits for one such step, which grows with
// the size of the rows and with groups that only together create a row. Upgrade path: count bytes
// instead of groups.
/// How many groups one transaction applies at least before a close may stop the pull
/// ([`hold::steps`]).
const APPLY_CHUNK: usize = 256;

#[cfg(test)]
thread_local! {
    /// Runs after every applied step on this thread, so a test can close the vault between two.
    pub(crate) static AFTER_STEP: std::cell::RefCell<Option<Box<dyn FnMut()>>> =
        const { std::cell::RefCell::new(None) };
}

/// Slack on top of `max_transaction_bytes` for a buffered group's encoded
/// size, which exceeds its SQL size by quoting and hex.
const BUFFER_SLACK: usize = 1024 * 1024;

/// Why a pull stops; nothing of the failing page is applied.
#[derive(Debug, thiserror::Error)]
pub enum InboundError {
    #[error("the pull names table {0}, which this device does not sync")]
    UnknownTable(String),
    #[error("the pull carries {0}, a device-local table of an extension")]
    DeviceLocalTable(String),
    #[error("a transaction group of {bytes} bytes exceeds the limit of {limit} bytes")]
    GroupTooLarge { bytes: usize, limit: usize },
    #[error("malformed page: {0}")]
    Malformed(&'static str),
    #[error(transparent)]
    Change(#[from] ChangeError),
    #[error(transparent)]
    Crdt(#[from] haex_crdt::Error),
    /// The vault started to close; what was applied stays, progress does not rise, and the
    /// next pull fetches the rest.
    #[error(transparent)]
    Closing(#[from] crate::sync::replica::Closing),
}

/// What one page did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Received {
    /// Tables with at least one applied change, for the follow-up work after a pull.
    pub tables: BTreeSet<String>,
    /// Groups rejected under research R5.
    pub rejected_groups: usize,
    /// Groups parked for extension tables this device lacks (spec 017, R10).
    pub parked_groups: usize,
    /// Received cells haex-crdt skipped for a column the table lacks; parking keeps this at 0.
    pub skipped_unknown_columns: usize,
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
    /// Extensions at their parking limit in this pull, and per origin the
    /// lowest group not parked because of it: progress stays below it.
    full_prefixes: HashSet<String>,
    /// Extensions with a group in this pull from a device that still has an older version
    /// ([`park`]): their groups are not applied, and their origins' progress stays below them.
    waiting: HashSet<String>,
    blocked: Vector,
    /// Parking limit per extension; `None` is [`park::PARKED_LIMIT_BYTES`].
    park_limit: Option<usize>,
    /// Removals applied in earlier pages; a held group may not be written yet.
    noted: Vec<park::Noted>,
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

    /// An inbox with a parking limit of `bytes` per extension.
    #[cfg(test)]
    pub fn with_park_limit(bytes: usize) -> Self {
        Self {
            park_limit: Some(bytes),
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
        let _held = replica.hold()?;
        let db = replica.db();
        let limit = db.max_transaction_bytes();
        // Device removal publishes its limit under the same exchange lock. Read the
        // current limits only after acquiring that lock and keep it until the changes
        // and their progress are committed, so a removal cannot race this admission
        // decision (FR-027, research R5).
        let exchange = replica.exchange();

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
        let mut context = query::read(db, |r| Context::read(r, db.device_id()))?;
        context.extend_noted(&self.noted);
        let limits = query::read(db, |r| removal_limits(r))?;

        let mut received = Received {
            done: !page.more,
            ..Received::default()
        };
        let mut arrived: Vec<Group> = Vec::new();
        let mut rejected: Vec<(Uuid, String)> = Vec::new();
        let mut parking = Parking::new(db);
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
                .map(Change::to_column)
                .collect::<Result<Vec<_>, _>>()?;
            let bytes = group_bytes(&columns)?;
            if bytes > limit {
                return Err(InboundError::GroupTooLarge { bytes, limit });
            }
            let sorted = park::sort(&context, &hlc, columns, |prefix| {
                self.queues(&context, prefix)
            })?;
            progress::raise(&mut group_updates, origin, hlc.clone());
            if is_removed_at(&limits, origin, &hlc) {
                received.rejected_groups += 1;
                rejected.push((origin, hlc));
                continue;
            }
            match sorted {
                Sorted::Apply(columns) => {
                    self.noted.extend(context.note(&columns));
                    arrived.push(Group {
                        origin,
                        hlc,
                        columns,
                    });
                }
                Sorted::Park(group) => {
                    self.park_group(&mut context, &mut parking, origin, hlc, group, bytes)?;
                }
            }
        }
        let mut candidates = std::mem::take(&mut self.held);
        candidates.extend(arrived);
        let candidates = self.park_with_parked(&mut context, &mut parking, candidates)?;
        received.parked_groups = parking.parked.len();
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
        let steps = steps(db, &apply, APPLY_CHUNK)?;
        received.tables.extend(
            apply
                .iter()
                .flat_map(|g| g.columns.iter().filter_map(changed_table)),
        );
        // Progress rises only below every group still held or not parked at
        // the limit: those come again in a pull that starts from it.
        let mut lowest_held: HashMap<Uuid, String> = self.blocked.clone().into_iter().collect();
        for group in &held {
            let lowest = lowest_held
                .entry(group.origin)
                .or_insert_with(|| group.hlc.clone());
            if compare_hlc_strings(&group.hlc, lowest) == Ordering::Less {
                *lowest = group.hlc.clone();
            }
        }
        let mut updates = Vector::new();
        let parked_at = parking
            .parked
            .iter()
            .filter_map(|(hlc, _, _)| Some((progress::origin_of(hlc)?, hlc.clone())));
        for (origin, hlc) in rejected
            .into_iter()
            .chain(parked_at)
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
            snapshot.rows.extend(apply.iter().flat_map(|g| {
                g.columns
                    .iter()
                    .map(|c| (c.table_name.clone(), c.row_pks.clone()))
            }));
        }
        if !page.more {
            // An origin with a group not parked at the limit is not served
            // completely: its progress stays below that group.
            let served = page
                .served
                .into_iter()
                .filter(|(origin, _)| !self.blocked.contains_key(origin));
            if let Some(snapshot) = &mut self.snapshot {
                snapshot.served = served.collect();
            } else {
                for (origin, hlc) in served {
                    progress::raise(&mut updates, origin, hlc);
                }
            }
            self.finished = true;
        }

        // Origins with cells haex-crdt skipped for a table or column that is gone now (a clear-up
        // ran after the page was sorted): neither parked nor applied, so their progress stays and
        // the next pull brings them again.
        let mut again: HashSet<Uuid> = HashSet::new();
        for step in steps {
            if replica.closing() {
                return Err(crate::sync::replica::Closing.into());
            }
            let (origins, accepted): (Vec<Uuid>, Vec<ColumnChange>) = apply[step]
                .iter()
                .flat_map(|g| g.columns.iter().map(move |c| (g.origin, c.clone())))
                .unzip();
            let outcome = db.apply_remote_changes(accepted)?;
            received.skipped_unknown_columns += report_unknown_columns(&outcome);
            again.extend(outcome.skipped.iter().filter_map(|skipped| {
                matches!(
                    skipped.reason,
                    SkipReason::MissingTable | SkipReason::UnknownColumn
                )
                .then(|| origins.get(skipped.input_index).copied())
                .flatten()
            }));
            if !outcome.skipped.is_empty() {
                log::debug!(
                    "sync: {} received cells kept the local value",
                    outcome.skipped.len()
                );
            }
            #[cfg(test)]
            AFTER_STEP.with_borrow_mut(|hook| {
                if let Some(hook) = hook {
                    hook();
                }
            });
        }
        for origin in &again {
            updates.remove(origin);
            if let Some(snapshot) = &mut self.snapshot {
                snapshot.served.remove(origin);
            }
        }
        // Parked groups count as received: stored in the write that moves
        // progress past them, with the state of an extension at its limit.
        if !updates.is_empty()
            || !parking.parked.is_empty()
            || !parking.newly_full.is_empty()
            || !self.full_prefixes.is_empty()
        {
            let now_ms = crate::passwords::clock::unix_millis(std::time::SystemTime::now());
            let full_prefixes: Vec<String> = self.full_prefixes.iter().cloned().collect();
            db.write(|tx| {
                park::store(tx, &parking.parked, now_ms)?;
                // A registry row can arrive on a later page than the group that filled the
                // parking limit. Re-check every prefix already full in this Inbox so that page
                // boundaries do not lose the status update.
                park::note_full(tx, &full_prefixes, db.device_id(), now_ms)?;
                progress::advance(tx, &updates)
            })?;
        }
        // An extension that became ready while this pull ran gets what it
        // parked; also catches a group parked behind one replayed meanwhile.
        // The exchange is free again: a replay that waits for a running migration does not hold
        // up the other sessions, and a closing vault stops it between extensions.
        drop(exchange);
        if !page.more && context.has_any_parked() {
            match park::replay_ready(db, &|| replica.closing()) {
                Ok(replayed) => received.tables.extend(replayed.tables),
                Err(error) => log::warn!("sync: replaying parked groups failed: {error}"),
            }
        }
        for (origin, hlc) in group_updates {
            progress::raise(&mut self.last_received, origin, hlc);
        }
        Ok(received)
    }
}

/// Logs and counts received cells haex-crdt skipped for a table or column this device lacks.
/// Parking catches those before; any left means a check missed one, or a clear-up dropped the
/// table meanwhile.
fn report_unknown_columns(outcome: &haex_crdt::ApplyOutcome) -> usize {
    let (columns, tables) = (
        outcome.report.skipped_unknown_column,
        outcome.report.skipped_unknown_table,
    );
    if columns > 0 {
        log::error!("sync: {columns} received cells named a column this device lacks");
    }
    if tables > 0 {
        log::error!("sync: {tables} received cells named a table this device lacks");
    }
    columns + tables
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

/// Per removed origin its limit in the effective list. Only that list's
/// removals count (FR-043, FR-005): it carries forward every removal of the
/// lists it builds on, and a same-generation fork that lost the tie-break has
/// no say. Two main devices that removed each other each hold such a fork;
/// counting the losing one would reject everything the remaining main device
/// writes from then on.
fn removal_limits(q: &mut impl query::Query) -> haex_crdt::Result<HashMap<Uuid, String>> {
    let Some(vault) = keys::vault_pubkey(q)? else {
        return Ok(HashMap::new());
    };
    let valid = device_list::valid_lists(&device_list::load_all(q)?, &vault);
    let Some(effective) = device_list::effective(&valid) else {
        return Ok(HashMap::new());
    };
    let mut limits: HashMap<Uuid, String> = HashMap::new();
    for removed in &effective.list.removed {
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
