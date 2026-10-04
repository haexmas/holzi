//! Groups for extension tables this device does not have yet (spec 017, FR-037, research R10,
//! R11). Part of [`super`].
//!
//! An extension's tables come from its migrations, which every device runs itself once it has
//! verified the bundle. Rows can arrive before that. A group that touches an extension table this
//! device lacks, or a column such a table lacks, is parked whole in `sync_parked_groups_no_sync`
//! and counts as received, so everything else keeps syncing. Once an extension has a parked
//! group, its later groups are parked behind it, so they are replayed in HLC order.
//! [`replay_ready`] applies parked groups as soon as their tables and columns exist.
//!
//! Changes to the tables of an extension removed with "delete data" that are older than its
//! `purge_hlc` are dropped (R11). Newer ones are parked until this device has cleared up for that
//! removal, so the clear-up cannot drop them: they belong to a reinstall. An unknown table without
//! an extension prefix still aborts the pull, and an extension's device-local (`_no_sync`) table on
//! the wire is a protocol error.

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap, HashSet};

use haex_crdt::rusqlite::params;
use haex_crdt::sqlparser::ast::Statement;
use haex_crdt::{compare_hlc_strings, ColumnChange, CrdtTransaction};

use super::{InboundError, DELETED_ROWS_TABLE};
use crate::extensions::ids::{ExtensionName, ExtensionTable, PublicKey, TablePrefix};
use crate::storage::query::Query;
use crate::sync::replica::synced_tables;

/// At most this many parked bytes per extension. At the limit no group is dropped: the progress
/// of its origin stops before it, so it is fetched again later (R10).
pub(super) const PARKED_LIMIT_BYTES: usize = 256 * 1024 * 1024;

const PARKED_TABLE: &str = "sync_parked_groups_no_sync";

/// Why a group was parked.
pub(super) const MISSING_TABLE: &str = "missing_table";
pub(super) const MISSING_COLUMN: &str = "missing_column";
pub(super) const AFTER_PARKED: &str = "after_parked";
pub(super) const AWAITING_PURGE: &str = "awaiting_purge";

/// The registry table whose `purge_hlc` column records a removal.
const EXTENSIONS_TABLE: &str = "extensions";

/// What this device has, read once per page.
#[derive(Debug, Default)]
pub(super) struct Context {
    /// Every synced table, by its name.
    synced: HashSet<String>,
    /// The extension tables with their columns, lower case; `None` when the table's SQL cannot be
    /// read (its columns are then not checked).
    extension_tables: HashMap<String, Option<HashSet<String>>>,
    /// Parked bytes and the earliest parked HLC per extension prefix.
    parked: HashMap<String, (usize, String)>,
    /// `purge_hlc` per prefix of an extension removed with "delete data".
    purges: HashMap<String, String>,
    /// The prefixes of [`Self::purges`] this device has not cleared up for yet.
    pending: HashSet<String>,
    /// Prefix and "delete data" per registered extension id, to read a removal from the wire.
    registry: HashMap<String, (String, bool)>,
    /// The last `purge_hlc` cleared up for, per extension id.
    applied: HashMap<String, String>,
}

/// A removal read from a group that was applied: the `purge_hlc` of `prefix` and whether it
/// deletes data.
#[derive(Debug, Clone)]
pub(super) struct Noted {
    prefix: String,
    extension_id: String,
    purge_hlc: String,
    purge_data: bool,
}

impl Context {
    pub(super) fn read(q: &mut impl Query) -> haex_crdt::Result<Self> {
        let synced = synced_tables(q)?.into_iter().collect();
        // The read connection allows no PRAGMA; SQLite keeps the CREATE statement current
        // (an added column is appended to it).
        let tables: Vec<(String, Option<String>)> = q.query_map(
            "SELECT name, sql FROM sqlite_master WHERE type = 'table'",
            &[],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let extension_tables = tables
            .into_iter()
            .filter(|(name, _)| ExtensionTable::parse(name).is_ok())
            .map(|(name, sql)| {
                (
                    name.to_ascii_lowercase(),
                    sql.as_deref().and_then(columns_of),
                )
            })
            .collect();
        let mut parked: HashMap<String, (usize, String)> = HashMap::new();
        let rows: Vec<(String, String, i64)> = q.query_map(
            &format!("SELECT extension_prefix, hlc, bytes FROM {PARKED_TABLE}"),
            &[],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        for (prefix, hlc, bytes) in rows {
            let entry = parked.entry(prefix).or_insert((0, hlc.clone()));
            entry.0 = entry.0.saturating_add(usize::try_from(bytes).unwrap_or(0));
            if compare_hlc_strings(&hlc, &entry.1) == Ordering::Less {
                entry.1 = hlc;
            }
        }
        type Row = (String, String, String, bool, Option<String>);
        let registered: Vec<Row> = q.query_map(
            "SELECT id, public_key, name, purge_data, purge_hlc FROM extensions",
            &[],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )?;
        let applied: HashMap<String, String> = q
            .query_map(
                "SELECT extension_id, purge_hlc FROM extension_purges_applied_no_sync",
                &[],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?
            .into_iter()
            .collect();
        let mut context = Self {
            synced,
            extension_tables,
            parked,
            applied,
            ..Self::default()
        };
        for (id, key, name, purge_data, purge_hlc) in registered {
            let Some(prefix) = prefix_of(&key, &name) else {
                continue;
            };
            context
                .registry
                .insert(id.clone(), (prefix.clone(), purge_data));
            if let Some(purge_hlc) = purge_hlc {
                context.set_purge(Noted {
                    prefix,
                    extension_id: id,
                    purge_hlc,
                    purge_data,
                });
            }
        }
        Ok(context)
    }

    /// Records the current removal of an extension.
    fn set_purge(&mut self, removal: Noted) {
        self.purges.remove(&removal.prefix);
        self.pending.remove(&removal.prefix);
        if !removal.purge_data {
            return;
        }
        let cleared = self
            .applied
            .get(&removal.extension_id)
            .is_some_and(|applied| {
                compare_hlc_strings(&removal.purge_hlc, applied) != Ordering::Greater
            });
        if !cleared {
            self.pending.insert(removal.prefix.clone());
        }
        self.purges.insert(removal.prefix, removal.purge_hlc);
    }

    /// Reads the removals an applied group writes, so later groups of the same pull already obey
    /// them (the clear-up runs only after the pull); returns them for the pages still to come.
    pub(super) fn note(&mut self, columns: &[ColumnChange]) -> Vec<Noted> {
        let cell = |row: &str, column: &str| {
            columns.iter().find(|c| {
                c.table_name == EXTENSIONS_TABLE && c.row_pks == row && c.column_name == column
            })
        };
        let mut noted = Vec::new();
        for change in columns {
            if change.table_name != EXTENSIONS_TABLE || change.column_name != "purge_hlc" {
                continue;
            }
            let Some(purge_hlc) = change.value.as_str() else {
                continue;
            };
            let Some(id) = serde_json::from_str::<serde_json::Value>(&change.row_pks)
                .ok()
                .and_then(|pks| pks.get("id")?.as_str().map(str::to_owned))
            else {
                continue;
            };
            let known = self.registry.get(&id).cloned();
            let text = |column| cell(&change.row_pks, column).and_then(|c| c.value.as_str());
            let prefix = match (text("public_key"), text("name")) {
                (Some(key), Some(name)) => prefix_of(key, name),
                _ => known.as_ref().map(|(prefix, _)| prefix.clone()),
            };
            let purge_data = cell(&change.row_pks, "purge_data")
                .and_then(|c| c.value.as_i64().or(c.value.as_bool().map(i64::from)))
                .map(|value| value != 0)
                .or(known.map(|(_, purge_data)| purge_data));
            let (Some(prefix), Some(purge_data)) = (prefix, purge_data) else {
                continue;
            };
            let removal = Noted {
                prefix,
                extension_id: id,
                purge_hlc: purge_hlc.to_owned(),
                purge_data,
            };
            self.set_purge(removal.clone());
            noted.push(removal);
        }
        noted
    }

    /// Takes over the removals earlier pages of the same pull noted.
    pub(super) fn extend_noted(&mut self, noted: &[Noted]) {
        for removal in noted {
            let newer = self.purges.get(&removal.prefix).is_none_or(|current| {
                compare_hlc_strings(&removal.purge_hlc, current) == Ordering::Greater
            });
            if newer || !removal.purge_data {
                self.set_purge(removal.clone());
            }
        }
    }

    /// Whether `prefix` has parked groups.
    pub(super) fn has_parked(&self, prefix: &str) -> bool {
        self.parked.contains_key(prefix)
    }

    /// Whether any extension has parked groups.
    pub(super) fn has_any_parked(&self) -> bool {
        !self.parked.is_empty()
    }

    /// Parked bytes of `prefix`.
    pub(super) fn parked_bytes(&self, prefix: &str) -> usize {
        self.parked.get(prefix).map_or(0, |(bytes, _)| *bytes)
    }

    /// Records a group parked in this page.
    pub(super) fn add_parked(&mut self, prefix: &str, hlc: &str, bytes: usize) {
        let entry = self
            .parked
            .entry(prefix.to_owned())
            .or_insert((0, hlc.to_owned()));
        entry.0 = entry.0.saturating_add(bytes);
        if compare_hlc_strings(hlc, &entry.1) == Ordering::Less {
            entry.1 = hlc.to_owned();
        }
    }
}

/// A group to park.
#[derive(Debug, Clone)]
pub(super) struct Parked {
    pub prefix: String,
    pub tables: BTreeSet<String>,
    pub reason: &'static str,
    pub columns: Vec<ColumnChange>,
}

/// What to do with one group.
#[derive(Debug)]
pub(super) enum Sorted {
    /// Apply these changes (purged ones are gone; maybe none is left).
    Apply(Vec<ColumnChange>),
    Park(Parked),
}

/// The table prefix of an extension's key and name, as the registry stores them.
fn prefix_of(key: &str, name: &str) -> Option<String> {
    let prefix = TablePrefix {
        public_key: PublicKey::parse_case_insensitive(key).ok()?,
        name: ExtensionName::parse(name).ok()?,
    };
    Some(prefix.to_string())
}

/// The lower-case column names of a `CREATE TABLE` statement.
fn columns_of(create_sql: &str) -> Option<HashSet<String>> {
    match crate::extensions::sql::parse::parse_one(create_sql).ok()? {
        Statement::CreateTable(create) => Some(
            create
                .columns
                .iter()
                .map(|column| column.name.value.to_ascii_lowercase())
                .collect(),
        ),
        _ => None,
    }
}

/// The tables the delete markers of a group delete from, by marker row.
fn marker_targets(columns: &[ColumnChange]) -> HashMap<String, String> {
    columns
        .iter()
        .filter(|c| c.table_name == DELETED_ROWS_TABLE && c.column_name == "table_name")
        .filter_map(|c| Some((c.row_pks.clone(), c.value.as_str()?.to_owned())))
        .collect()
}

/// The table a change writes to as far as its extension is concerned: for a delete marker the
/// table whose row it deletes.
fn effective<'a>(
    change: &'a ColumnChange,
    targets: &'a HashMap<String, String>,
) -> Option<&'a str> {
    if change.table_name == DELETED_ROWS_TABLE {
        targets.get(&change.row_pks).map(String::as_str)
    } else {
        Some(&change.table_name)
    }
}

fn extension_table(name: &str) -> Result<Option<ExtensionTable>, InboundError> {
    match ExtensionTable::parse(name) {
        Ok(table) if table.table.ends_with("_no_sync") => {
            Err(InboundError::DeviceLocalTable(name.to_owned()))
        }
        Ok(table) => Ok(Some(table)),
        Err(_) => Ok(None),
    }
}

/// Decides one group of transaction HLC `hlc`. `waits(prefix)` says whether an extension already
/// has parked groups this one must queue behind.
pub(super) fn sort(
    context: &Context,
    hlc: &str,
    columns: Vec<ColumnChange>,
    waits: impl Fn(&str) -> bool,
) -> Result<Sorted, InboundError> {
    let targets = marker_targets(&columns);
    for change in &columns {
        if !context.synced.contains(&change.table_name)
            && extension_table(&change.table_name)?.is_none()
        {
            return Err(InboundError::UnknownTable(change.table_name.clone()));
        }
        if let Some(target) = effective(change, &targets) {
            extension_table(target)?;
        }
    }
    let purged = |change: &ColumnChange| {
        effective(change, &targets)
            .and_then(|table| ExtensionTable::parse(table).ok())
            .and_then(|table| context.purges.get(&table.prefix.to_string()))
            .is_some_and(|purge| compare_hlc_strings(hlc, purge) == Ordering::Less)
    };
    let columns: Vec<ColumnChange> = columns.into_iter().filter(|c| !purged(c)).collect();

    let mut missing: Option<(String, &'static str)> = None;
    let mut queued: Option<String> = None;
    let mut tables = BTreeSet::new();
    for change in &columns {
        let Some(name) = effective(change, &targets) else {
            continue;
        };
        let Some(table) = ExtensionTable::parse(name).ok() else {
            continue;
        };
        let prefix = table.prefix.to_string();
        let reason = match context.extension_tables.get(&name.to_ascii_lowercase()) {
            // Newer than a removal this device has not cleared up for: the clear-up would drop it.
            _ if context.pending.contains(&prefix) => Some(AWAITING_PURGE),
            None => Some(MISSING_TABLE),
            Some(Some(known))
                if change.table_name != DELETED_ROWS_TABLE
                    && !known.contains(&change.column_name.to_ascii_lowercase()) =>
            {
                Some(MISSING_COLUMN)
            }
            Some(_) => None,
        };
        if let Some(reason) = reason {
            tables.insert(name.to_ascii_lowercase());
            missing.get_or_insert((prefix, reason));
        } else if waits(&prefix) {
            queued.get_or_insert(prefix);
        }
    }
    let Some((prefix, reason)) = missing.or_else(|| queued.map(|prefix| (prefix, AFTER_PARKED)))
    else {
        return Ok(Sorted::Apply(columns));
    };
    Ok(Sorted::Park(Parked {
        prefix,
        tables,
        reason,
        columns,
    }))
}

/// Stores parked groups; one already parked (the same origin and HLC, fetched again) is skipped.
pub(super) fn store(
    tx: &mut CrdtTransaction<'_>,
    parked: &[(String, Parked, usize)],
    now_ms: i64,
) -> haex_crdt::Result<()> {
    for (hlc, group, bytes) in parked {
        let origin = crate::sync::progress::origin_of(hlc)
            .map(|o| o.to_string())
            .unwrap_or_default();
        let known = tx
            .query_row(
                &format!("SELECT COUNT(*) FROM {PARKED_TABLE} WHERE origin = ?1 AND hlc = ?2"),
                params![origin, hlc],
                |r| r.get::<_, i64>(0),
            )?
            .unwrap_or(0)
            > 0;
        if known {
            continue;
        }
        let blob = serde_json::to_vec(&group.columns)
            .map_err(|e| haex_crdt::Error::consumer(format!("parked group: {e}")))?;
        let tables = serde_json::to_string(&group.tables)
            .map_err(|e| haex_crdt::Error::consumer(format!("parked tables: {e}")))?;
        tx.execute(
            &format!(
                "INSERT INTO {PARKED_TABLE} (origin, hlc, extension_prefix, tables, group_blob, \
                 bytes, reason, parked_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
            ),
            params![
                origin,
                hlc,
                group.prefix,
                tables,
                blob,
                i64::try_from(*bytes).unwrap_or(i64::MAX),
                group.reason,
                now_ms
            ],
        )?;
    }
    Ok(())
}

/// Records on `device` that the parked groups of the extensions with `prefixes` reached their
/// limit (R10): its state there says so until it is installed there or removed with "delete
/// data". An extension that is not registered here has no state to show it in.
pub(super) fn note_full(
    tx: &mut CrdtTransaction<'_>,
    prefixes: &[String],
    device: uuid::Uuid,
    now_ms: i64,
) -> haex_crdt::Result<()> {
    if prefixes.is_empty() {
        return Ok(());
    }
    let registered: Vec<(String, String, String)> = tx.query_map(
        "SELECT id, public_key, name FROM extensions WHERE state = 'installed'",
        &[],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    for (id, key, name) in registered {
        let Ok(id) = uuid::Uuid::parse_str(&id) else {
            continue;
        };
        if prefix_of(&key, &name).is_some_and(|prefix| prefixes.contains(&prefix)) {
            crate::extensions::registry::status::mark_parked_limit(tx, id, device, now_ms)
                .map_err(haex_crdt::Error::from)?;
        }
    }
    Ok(())
}

/// Removes the parked groups of `prefix` that are not newer than `purge_hlc` ("delete data",
/// R11); newer ones belong to a reinstall and stay.
pub fn discard(
    tx: &mut CrdtTransaction<'_>,
    prefix: &TablePrefix,
    purge_hlc: &str,
) -> haex_crdt::Result<()> {
    let groups: Vec<(i64, String)> = tx.query_map(
        &format!("SELECT id, hlc FROM {PARKED_TABLE} WHERE extension_prefix = ?1"),
        params![prefix.to_string()],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    for (id, hlc) in groups {
        if compare_hlc_strings(&hlc, purge_hlc) != Ordering::Greater {
            tx.execute(
                &format!("DELETE FROM {PARKED_TABLE} WHERE id = ?1"),
                params![id],
            )?;
        }
    }
    Ok(())
}

#[path = "inbound_park_replay.rs"]
mod replay;
pub use replay::{replay_ready, Replayed};

#[cfg(test)]
#[path = "inbound_park_tests.rs"]
mod tests;
