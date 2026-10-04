//! Replaying parked groups once their tables and columns exist (spec 017, research R10). Part of
//! [`super`].

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap};
use std::sync::PoisonError;

use haex_crdt::rusqlite::params;
use haex_crdt::{compare_hlc_strings, ColumnChange, Database};

use super::{sort, Context, Sorted, PARKED_TABLE};
use crate::storage::query::{self, Query};

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
        // Up to the parking limit per extension: each group's changes are read when it is tried.
        let rows: Vec<(i64, String, String)> = query::read(db, |r| {
            r.query_map(
                &format!("SELECT id, extension_prefix, hlc FROM {PARKED_TABLE}"),
                &[],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
        })?;
        let mut by_prefix: HashMap<String, Vec<(i64, String)>> = HashMap::new();
        for (id, prefix, hlc) in rows {
            by_prefix.entry(prefix).or_default().push((id, hlc));
        }
        let mut progressed = false;
        for (prefix, mut groups) in by_prefix {
            groups.sort_by(|a, b| compare_hlc_strings(&a.1, &b.1));
            for (id, hlc) in groups {
                let blob: Option<Vec<u8>> = query::read(db, |r| {
                    r.query_row(
                        &format!("SELECT group_blob FROM {PARKED_TABLE} WHERE id = ?1"),
                        params![id],
                        |row| row.get(0),
                    )
                })?;
                // Gone meanwhile: replayed by a concurrent run, or discarded.
                let Some(blob) = blob else {
                    continue;
                };
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
                            .extend(columns.iter().filter_map(super::super::changed_table));
                        if !columns.is_empty() {
                            let outcome = db.apply_remote_changes(columns)?;
                            super::super::report_unknown_columns(&outcome);
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
