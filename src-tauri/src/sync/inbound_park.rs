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
//! `purge_hlc` are dropped (R11). An unknown table without an extension prefix still aborts the
//! pull, and an extension's device-local (`_no_sync`) table on the wire is a protocol error.

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::PoisonError;

use haex_crdt::rusqlite::params;
use haex_crdt::sqlparser::ast::Statement;
use haex_crdt::{compare_hlc_strings, ColumnChange, CrdtTransaction, Database};

use super::{InboundError, DELETED_ROWS_TABLE};
use crate::extensions::ids::{ExtensionName, ExtensionTable, PublicKey, TablePrefix};
use crate::storage::query::{self, Query};
use crate::sync::replica::synced_tables;

/// At most this many parked bytes per extension. At the limit no group is dropped: the progress
/// of its origin stops before it, so it is fetched again later (R10).
pub(super) const PARKED_LIMIT_BYTES: usize = 256 * 1024 * 1024;

const PARKED_TABLE: &str = "sync_parked_groups_no_sync";

/// Why a group was parked.
pub(super) const MISSING_TABLE: &str = "missing_table";
pub(super) const MISSING_COLUMN: &str = "missing_column";
pub(super) const AFTER_PARKED: &str = "after_parked";

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
        let removed: Vec<(String, String, String)> = q.query_map(
            "SELECT public_key, name, purge_hlc FROM extensions \
             WHERE purge_data = 1 AND purge_hlc IS NOT NULL",
            &[],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        let purges = removed
            .into_iter()
            .filter_map(|(key, name, hlc)| {
                let prefix = TablePrefix {
                    public_key: PublicKey::parse_case_insensitive(&key).ok()?,
                    name: ExtensionName::parse(&name).ok()?,
                };
                Some((prefix.to_string(), hlc))
            })
            .collect();
        Ok(Self {
            synced,
            extension_tables,
            parked,
            purges,
        })
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

/// Removes the parked groups of `prefix` ("delete data", R11).
pub fn discard(tx: &mut CrdtTransaction<'_>, prefix: &TablePrefix) -> haex_crdt::Result<()> {
    tx.execute(
        &format!("DELETE FROM {PARKED_TABLE} WHERE extension_prefix = ?1"),
        params![prefix.to_string()],
    )?;
    Ok(())
}

/// What [`replay_ready`] did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Replayed {
    pub groups: usize,
    pub tables: BTreeSet<String>,
}

/// Applies, per extension in HLC order, the parked groups whose tables and columns now exist, and
/// deletes them; an extension's groups stop at the first that still cannot apply. Waits for
/// running migrations, so no group lands between two of them. Safe to run concurrently: applying
/// a group twice changes nothing.
pub fn replay_ready(db: &Database) -> haex_crdt::Result<Replayed> {
    let _migrations = crate::extensions::sql::migrate::applying()
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let mut replayed = Replayed::default();
    loop {
        let context = query::read(db, |r| Context::read(r))?;
        let rows: Vec<(i64, String, String, Vec<u8>)> = query::read(db, |r| {
            r.query_map(
                &format!("SELECT id, extension_prefix, hlc, group_blob FROM {PARKED_TABLE}"),
                &[],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
        })?;
        let mut by_prefix: HashMap<String, Vec<(i64, String, Vec<u8>)>> = HashMap::new();
        for (id, prefix, hlc, blob) in rows {
            by_prefix.entry(prefix).or_default().push((id, hlc, blob));
        }
        let mut progressed = false;
        for (prefix, mut groups) in by_prefix {
            groups.sort_by(|a, b| compare_hlc_strings(&a.1, &b.1));
            for (id, hlc, blob) in groups {
                let columns: Vec<ColumnChange> = serde_json::from_slice(&blob)
                    .map_err(|e| haex_crdt::Error::consumer(format!("parked group: {e}")))?;
                // Another extension's earlier parked group comes first.
                let waits = |other: &str| {
                    other != prefix
                        && context.parked.get(other).is_some_and(|(_, earliest)| {
                            compare_hlc_strings(earliest, &hlc) == Ordering::Less
                        })
                };
                let sorted = sort(&context, &hlc, columns, waits)
                    .map_err(|e| haex_crdt::Error::consumer(format!("parked group {hlc}: {e}")))?;
                match sorted {
                    // Still missing something, or behind another extension's earlier group: the
                    // globally earliest waiting group never waits, so this cannot cycle.
                    Sorted::Park(_) => break,
                    Sorted::Apply(columns) => {
                        replayed
                            .tables
                            .extend(columns.iter().filter_map(super::changed_table));
                        if !columns.is_empty() {
                            let outcome = db.apply_remote_changes(columns)?;
                            super::report_unknown_columns(&outcome);
                        }
                        db.write(|tx| {
                            tx.execute(
                                &format!("DELETE FROM {PARKED_TABLE} WHERE id = ?1"),
                                params![id],
                            )
                            .map(drop)
                        })?;
                        replayed.groups += 1;
                        progressed = true;
                    }
                }
            }
        }
        if !progressed {
            if replayed.groups > 0 {
                log::info!("sync: replayed {} parked groups", replayed.groups);
            }
            return Ok(replayed);
        }
    }
}

#[cfg(test)]
#[path = "inbound_park_tests.rs"]
mod tests;
