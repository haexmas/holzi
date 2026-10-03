//! Resync of a device that is too far behind (spec 024, research R20,
//! contracts/sync-protocol.md §2).
//!
//! Delete markers older than 90 days are pruned
//! ([`crate::storage::maintenance::DELETE_MARKER_RETENTION_DAYS`]), so a
//! device that missed a deletion for longer would keep the deleted row and
//! bring it back. A sender therefore answers a `Pull` with `Resync` when the
//! puller is behind for an origin *and* its cursor for that origin lies
//! before the prune horizon: changes beyond that cursor may include
//! deletions whose markers are gone. The puller then asks for a snapshot
//! (`Pull` with `replace`), which is served as a pull from nothing.
//!
//! Replacing merges first and prunes second, so that a crash in between
//! leaves a union of both states, never a hole. After the snapshot's last
//! page, a local row is removed when
//! - the snapshot did not carry it, and
//! - every cell of it is at or before the progress the sender served for
//!   that cell's origin, i.e. the sender had seen everything this device
//!   knows of the row and still does not have it (deleted there, or never
//!   accepted under research R5).
//!
//! What the sender had not seen yet, as this device's own newer changes,
//! stays and travels the normal way afterwards. Insert-only security tables
//! are never pruned.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::{Duration, SystemTime};

use haex_crdt::rusqlite::types::Value;
use haex_crdt::{compare_hlc_strings, ScanFilters};

use crate::storage::maintenance::DELETE_MARKER_RETENTION_DAYS;
use crate::storage::query::{self, Query};
use crate::sync::progress::{self, Vector};
use crate::sync::replica::{synced_tables, Replica};

/// haex-crdt's delete log, whose rows the snapshot merges like any others.
const DELETED_ROWS_TABLE: &str = "haex_deleted_rows";

/// Tables that only ever grow and are never pruned: the device list and the
/// key material decide who belongs to the vault.
const NEVER_PRUNED: &[&str] = &[
    "device_lists",
    "vault_key_generations",
    "vault_key_envelopes",
];

/// The time part of an HLC (an NTP64 count), 0 when malformed.
fn hlc_time(hlc: &str) -> u64 {
    hlc.split_once('/')
        .and_then(|(time, _)| time.parse().ok())
        .unwrap_or(0)
}

/// The NTP64 time before which delete markers may be gone at `now`.
fn prune_horizon(now: SystemTime) -> u64 {
    let horizon = now
        .checked_sub(Duration::from_secs(
            u64::from(DELETE_MARKER_RETENTION_DAYS) * 24 * 60 * 60,
        ))
        .unwrap_or(SystemTime::UNIX_EPOCH);
    haex_crdt::uhlc::NTP64::from(
        horizon
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default(),
    )
    .as_u64()
}

/// Whether a puller with the cursors `theirs` must resync instead of pulling
/// from a sender at `served`. An origin the puller lacks entirely is no
/// reason: it holds nothing of it that a lost deletion could keep alive.
pub fn is_stale(theirs: &Vector, served: &Vector, now: SystemTime) -> bool {
    let horizon = prune_horizon(now);
    theirs.iter().any(|(origin, cursor)| {
        hlc_time(cursor) < horizon
            && served
                .get(origin)
                .is_some_and(|latest| compare_hlc_strings(cursor, latest) == Ordering::Less)
    })
}

/// A row of a synced table as `(table, row_pks)`, in the form changes carry
/// it.
pub type RowKey = (String, String);

/// Removes the local rows the snapshot did not carry, under the rule in the
/// module docs; returns the tables that lost rows.
pub fn prune_absent(
    replica: &Replica,
    kept: &HashSet<RowKey>,
    served: &Vector,
) -> haex_crdt::Result<Vec<String>> {
    let db = replica.db();
    let _exchange = replica.exchange();
    let tables = query::read(db, |r| synced_tables(r))?;
    let mut doomed: Vec<RowKey> = Vec::new();
    for table in tables {
        if table == DELETED_ROWS_TABLE || NEVER_PRUNED.contains(&table.as_str()) {
            continue;
        }
        let mut rows: HashMap<String, bool> = HashMap::new();
        for cell in db.scan_table_for_local_changes(&table, None, ScanFilters::default())? {
            let covered = progress::origin_of(&cell.hlc_timestamp)
                .and_then(|origin| served.get(&origin))
                .is_some_and(|cap| {
                    compare_hlc_strings(&cell.hlc_timestamp, cap) != Ordering::Greater
                });
            let row = rows.entry(cell.row_pks).or_insert(true);
            *row = *row && covered;
        }
        doomed.extend(
            rows.into_iter()
                .filter(|(pks, covered)| *covered && !kept.contains(&(table.clone(), pks.clone())))
                .map(|(pks, _)| (table.clone(), pks)),
        );
    }
    if doomed.is_empty() {
        return Ok(Vec::new());
    }
    let deleted = db.write(|tx| {
        // Children may go before their parents, or the other way round.
        tx.execute("PRAGMA defer_foreign_keys = 1", &[])?;
        let mut deleted = Vec::new();
        for (table, pks) in &doomed {
            // The candidate scan runs before this transaction. Re-read the current
            // per-column HLCs while the write transaction is open so a local write
            // that landed during the scan keeps its row and is never deleted as a
            // stale snapshot candidate.
            if !row_is_still_covered(tx, table, pks, served)? {
                continue;
            }
            delete_row(tx, table, pks)?;
            deleted.push((table.clone(), pks.clone()));
        }
        Ok(deleted)
    })?;
    let mut touched: Vec<String> = deleted.into_iter().map(|(table, _)| table).collect();
    touched.sort();
    touched.dedup();
    Ok(touched)
}

/// Re-checks a prune candidate against the row's current per-column HLC map.
/// The initial scan is intentionally cheap and happens outside the write
/// transaction; this second read is the safety boundary against a local write
/// racing the scan (R02).
fn row_is_still_covered(
    tx: &mut impl Query,
    table: &str,
    row_pks: &str,
    served: &Vector,
) -> haex_crdt::Result<bool> {
    let pks: BTreeMap<String, serde_json::Value> = serde_json::from_str(row_pks)
        .map_err(|e| haex_crdt::Error::consumer(format!("row key of {table}: {e}")))?;
    if pks.is_empty() {
        return Ok(false);
    }
    let values: Vec<Value> = pks
        .values()
        .map(sql_value)
        .collect::<haex_crdt::Result<_>>()?;
    let filter = pks
        .keys()
        .enumerate()
        .map(|(i, column)| format!("\"{column}\" = ?{}", i + 1))
        .collect::<Vec<_>>()
        .join(" AND ");
    let query = format!("SELECT haex_column_hlcs_no_sync FROM \"{table}\" WHERE {filter}");
    let sql_params: Vec<&dyn haex_crdt::rusqlite::ToSql> = values
        .iter()
        .map(|value| value as &dyn haex_crdt::rusqlite::ToSql)
        .collect();
    let Some(raw): Option<String> = tx.query_row(&query, &sql_params, |row| row.get(0))? else {
        return Ok(false);
    };
    let cells: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&raw)
        .map_err(|e| haex_crdt::Error::consumer(format!("column HLCs of {table}: {e}")))?;
    Ok(cells.values().all(|value| {
        let Some(hlc) = value.as_str() else {
            return false;
        };
        progress::origin_of(hlc)
            .and_then(|origin| served.get(&origin))
            .is_some_and(|cap| compare_hlc_strings(hlc, cap) != Ordering::Greater)
    }))
}

/// Prunes a completed snapshot before exposing the progress it covered.
/// Keeping both operations in one blocking task means cancellation cannot
/// leave the snapshot progress advanced while its prune is still pending.
pub fn prune_absent_and_advance(
    replica: &Replica,
    kept: &HashSet<RowKey>,
    served: &Vector,
) -> haex_crdt::Result<Vec<String>> {
    // Scanning and deleting every synced table takes a while; the close waits for it (see
    // [`Replica::hold`]).
    let _held = replica.hold().map_err(haex_crdt::Error::consumer)?;
    let touched = prune_absent(replica, kept, served)?;
    replica.db().write(|tx| progress::advance(tx, served))?;
    Ok(touched)
}

/// Deletes the row `row_pks` (a JSON object of primary key columns) of
/// `table`.
fn delete_row(
    tx: &mut haex_crdt::CrdtTransaction<'_>,
    table: &str,
    row_pks: &str,
) -> haex_crdt::Result<()> {
    let pks: BTreeMap<String, serde_json::Value> = serde_json::from_str(row_pks)
        .map_err(|e| haex_crdt::Error::consumer(format!("row key of {table}: {e}")))?;
    if pks.is_empty() {
        return Ok(());
    }
    let values: Vec<Value> = pks
        .values()
        .map(sql_value)
        .collect::<haex_crdt::Result<_>>()?;
    let filter = pks
        .keys()
        .enumerate()
        .map(|(i, column)| format!("\"{column}\" = ?{}", i + 1))
        .collect::<Vec<_>>()
        .join(" AND ");
    let params: Vec<&dyn haex_crdt::rusqlite::ToSql> = values
        .iter()
        .map(|v| v as &dyn haex_crdt::rusqlite::ToSql)
        .collect();
    tx.execute(&format!("DELETE FROM \"{table}\" WHERE {filter}"), &params)?;
    Ok(())
}

/// A primary key value as SQLite takes it.
fn sql_value(value: &serde_json::Value) -> haex_crdt::Result<Value> {
    match value {
        serde_json::Value::String(text) => Ok(Value::Text(text.clone())),
        serde_json::Value::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => Ok(Value::Integer(i)),
            (None, Some(f)) => Ok(Value::Real(f)),
            _ => Err(haex_crdt::Error::consumer("a row key number out of range")),
        },
        serde_json::Value::Object(map) => map
            .get("$blob_hex")
            .and_then(|hex| hex.as_str())
            .and_then(|hex| crate::sync::presence::decode_hex(hex).ok())
            .map(Value::Blob)
            .ok_or_else(|| haex_crdt::Error::consumer("a row key of an unknown form")),
        _ => Err(haex_crdt::Error::consumer("a row key of an unknown form")),
    }
}

#[cfg(test)]
#[path = "resync_tests.rs"]
mod tests;
