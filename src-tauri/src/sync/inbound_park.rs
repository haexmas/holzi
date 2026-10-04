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

/// At most this many parked bytes per extension. At the limit no group is dropped: the progress
/// of its origin stops before it, so it is fetched again later (R10).
pub(super) const PARKED_LIMIT_BYTES: usize = 256 * 1024 * 1024;

pub(super) const PARKED_TABLE: &str = "sync_parked_groups_no_sync";

/// Why a group was parked.
pub(super) const MISSING_TABLE: &str = "missing_table";
pub(super) const MISSING_COLUMN: &str = "missing_column";
pub(super) const AFTER_PARKED: &str = "after_parked";
pub(super) const AWAITING_PURGE: &str = "awaiting_purge";

#[path = "inbound_park_context.rs"]
mod context;
pub(super) use context::{Context, Noted};

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
        let blob = serde_json::to_vec(&group.columns)
            .map_err(|e| haex_crdt::Error::consumer(format!("parked group: {e}")))?;
        let tables = serde_json::to_string(&group.tables)
            .map_err(|e| haex_crdt::Error::consumer(format!("parked tables: {e}")))?;
        tx.execute(
            &format!(
                "INSERT OR IGNORE INTO {PARKED_TABLE} (origin, hlc, extension_prefix, tables, \
                 group_blob, bytes, reason, parked_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
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

/// Whether the group of `origin` at `hlc` is parked already (fetched again).
pub(super) fn is_stored(
    q: &mut impl crate::storage::query::Query,
    origin: &str,
    hlc: &str,
) -> haex_crdt::Result<bool> {
    Ok(q.query_row(
        &format!("SELECT COUNT(*) FROM {PARKED_TABLE} WHERE origin = ?1 AND hlc = ?2"),
        params![origin, hlc],
        |r| r.get::<_, i64>(0),
    )?
    .unwrap_or(0)
        > 0)
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
