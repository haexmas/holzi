//! Groups that wait for the other cells of their rows (spec 024, SC-014,
//! research R4). Part of [`super`].
//!
//! A row's cells can come from several transaction groups: a row created in
//! one group and changed in another has, in the first, only the cells the
//! change did not overwrite. The pull carries the groups in HLC order, so when
//! the pages split the two, the first page would create the row without a
//! column that must not be empty, and haex-crdt cannot insert it. A group
//! whose rows cannot be created yet is therefore held until the cells they
//! lack arrive, and progress stays below it ([`super::Inbox::receive`]), so
//! a pull that breaks off holds back nothing the next one will not send
//! again. The last page applies what is left: whatever still cannot be
//! created is a real inconsistency and fails the pull as before.

use std::collections::{HashMap, HashSet};
use std::ops::Range;

use haex_crdt::rusqlite::ToSql;
use haex_crdt::{ColumnChange, Database};
use serde_json::Value;
use uuid::Uuid;

use super::DELETED_ROWS_TABLE;
use crate::storage::query::{self, Query};

/// One complete transaction group, from one origin.
#[derive(Debug, Clone)]
pub(super) struct Group {
    pub origin: Uuid,
    pub hlc: String,
    pub columns: Vec<ColumnChange>,
}

/// What [`settle`] decided.
pub(super) struct Settled {
    /// The groups to apply now, in the order they came.
    pub apply: Vec<Group>,
    /// The groups that wait for more cells.
    pub held: Vec<Group>,
}

type RowKey = (String, String);

/// Splits `groups` into those whose rows can be created or updated now and
/// those that wait. A row is ready if it exists, or if the cells of the
/// groups applied together cover every column that must have a value. On the
/// `last` page everything is applied.
pub(super) fn settle(db: &Database, groups: Vec<Group>, last: bool) -> haex_crdt::Result<Settled> {
    let mut apply = groups;
    let mut held = Vec::new();
    if last {
        return Ok(Settled { apply, held });
    }
    let mut required: HashMap<String, Vec<String>> = HashMap::new();
    let mut exists: HashMap<RowKey, bool> = HashMap::new();
    loop {
        let mut provided: HashMap<RowKey, HashSet<String>> = HashMap::new();
        for column in apply.iter().flat_map(|g| &g.columns) {
            if column.table_name == DELETED_ROWS_TABLE {
                continue;
            }
            provided
                .entry((column.table_name.clone(), column.row_pks.clone()))
                .or_default()
                .insert(column.column_name.clone());
        }
        let mut incomplete: HashSet<RowKey> = HashSet::new();
        for (key, cells) in provided {
            if !required.contains_key(&key.0) {
                required.insert(key.0.clone(), required_columns(db, &key.0)?);
            }
            if required[&key.0].iter().all(|c| cells.contains(c)) {
                continue;
            }
            if !exists.contains_key(&key) {
                let present = query::read(db, |r| row_exists(r, &key.0, &key.1))?;
                exists.insert(key.clone(), present);
            }
            if !exists[&key] {
                incomplete.insert(key);
            }
        }
        if incomplete.is_empty() {
            return Ok(Settled { apply, held });
        }
        let (blocked, rest): (Vec<Group>, Vec<Group>) = apply.into_iter().partition(|g| {
            g.columns
                .iter()
                .any(|c| incomplete.contains(&(c.table_name.clone(), c.row_pks.clone())))
        });
        held.extend(blocked);
        apply = rest;
    }
}

/// Splits `groups` into steps of at least `size` groups, each applied in a
/// transaction of its own ([`super::Inbox::receive`]). A step never ends
/// between groups that only together create a row: [`settle`] lets a row in
/// whose cells come from several groups applied together, so a row that does
/// not exist yet stays in one step from its first group to the one that
/// completes it. A row that is never completed keeps every later group with
/// it, as the single transaction did.
pub(super) fn steps(
    db: &Database,
    groups: &[Group],
    size: usize,
) -> haex_crdt::Result<Vec<Range<usize>>> {
    struct Seen {
        first: usize,
        cells: HashSet<String>,
        complete: Option<usize>,
    }
    let mut required: HashMap<String, Vec<String>> = HashMap::new();
    let mut rows: HashMap<RowKey, Seen> = HashMap::new();
    for (i, group) in groups.iter().enumerate() {
        let mut touched: HashSet<RowKey> = HashSet::new();
        for column in &group.columns {
            if column.table_name == DELETED_ROWS_TABLE {
                continue;
            }
            let key = (column.table_name.clone(), column.row_pks.clone());
            let seen = rows.entry(key.clone()).or_insert_with(|| Seen {
                first: i,
                cells: HashSet::new(),
                complete: None,
            });
            if seen.complete.is_none() {
                seen.cells.insert(column.column_name.clone());
                touched.insert(key);
            }
        }
        for key in touched {
            if !required.contains_key(&key.0) {
                required.insert(key.0.clone(), required_columns(db, &key.0)?);
            }
            let Some(seen) = rows.get_mut(&key) else {
                continue;
            };
            if required[&key.0].iter().all(|c| seen.cells.contains(c)) {
                seen.complete = Some(i);
                seen.cells = HashSet::new();
            }
        }
    }
    // The last group each group's step must reach.
    let mut until: Vec<usize> = (0..groups.len()).collect();
    for (key, seen) in rows {
        if seen.complete == Some(seen.first) || query::read(db, |r| row_exists(r, &key.0, &key.1))?
        {
            continue;
        }
        let end = seen.complete.unwrap_or(groups.len().saturating_sub(1));
        until[seen.first] = until[seen.first].max(end);
    }
    let mut steps = Vec::new();
    let (mut start, mut reach) = (0, 0);
    for (i, end) in until.into_iter().enumerate() {
        reach = reach.max(end);
        if reach == i && i + 1 - start >= size {
            steps.push(start..i + 1);
            start = i + 1;
        }
    }
    if start < groups.len() {
        steps.push(start..groups.len());
    }
    Ok(steps)
}

/// The columns of `table` an INSERT must give a value: not empty, no default,
/// not part of the key, not haex-crdt's own.
fn required_columns(db: &Database, table: &str) -> haex_crdt::Result<Vec<String>> {
    // The read-only view does not allow `pragma_table_info` (see
    // `replica::synced_tables`), and this only reads the table definition.
    #[allow(clippy::disallowed_methods)]
    let rows: Vec<(String, i64, Option<String>, i64)> = db.with_connection(|conn| {
        let mut statement =
            conn.prepare("SELECT name, \"notnull\", dflt_value, pk FROM pragma_table_info(?1)")?;
        let rows = statement
            .query_map([table], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })?;
    Ok(rows
        .into_iter()
        .filter(|(name, not_null, default, pk)| {
            *not_null == 1 && default.is_none() && *pk == 0 && !name.starts_with("haex_")
        })
        .map(|(name, ..)| name)
        .collect())
}

/// Whether the row exists. A key this cannot read (a blob in the key) counts
/// as existing, so the group is left to haex-crdt.
fn row_exists(q: &mut impl Query, table: &str, row_pks: &str) -> haex_crdt::Result<bool> {
    let Ok(keys) = serde_json::from_str::<serde_json::Map<String, Value>>(row_pks) else {
        return Ok(true);
    };
    let mut clauses = Vec::new();
    let mut values: Vec<Box<dyn ToSql>> = Vec::new();
    for (i, (column, value)) in keys.iter().enumerate() {
        clauses.push(format!("\"{}\" = ?{}", column.replace('"', "\"\""), i + 1));
        values.push(match value {
            Value::String(text) => Box::new(text.clone()),
            Value::Number(n) if n.is_i64() => Box::new(n.as_i64().unwrap_or_default()),
            Value::Number(n) => Box::new(n.as_f64().unwrap_or_default()),
            Value::Bool(flag) => Box::new(i64::from(*flag)),
            _ => return Ok(true),
        });
    }
    if clauses.is_empty() {
        return Ok(true);
    }
    let params: Vec<&dyn ToSql> = values.iter().map(|v| v.as_ref()).collect();
    let found: Option<i64> = q.query_row(
        &format!(
            "SELECT 1 FROM \"{}\" WHERE {}",
            table.replace('"', "\"\""),
            clauses.join(" AND ")
        ),
        &params,
        |r| r.get(0),
    )?;
    Ok(found.is_some())
}
