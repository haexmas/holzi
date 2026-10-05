//! What this device has for the parking decisions of one page (spec 017, research R10, R11): its
//! synced and extension tables, the groups it parked, and the removals it knows of. Part of
//! [`super`].

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use haex_crdt::{compare_hlc_strings, ColumnChange};

use super::{columns_of, prefix_of, PARKED_TABLE};
use crate::extensions::ids::ExtensionTable;
use crate::storage::query::Query;
use crate::sync::replica::synced_tables;

/// The registry table whose `purge_hlc` column records a removal.
const EXTENSIONS_TABLE: &str = "extensions";

/// What this device has, read once per page.
#[derive(Debug, Default)]
pub(in crate::sync::inbound) struct Context {
    /// Every synced table, by its name.
    pub(super) synced: HashSet<String>,
    /// The extension tables with their columns, lower case; `None` when the table's SQL cannot be
    /// read (its columns are then not checked).
    pub(super) extension_tables: HashMap<String, Option<HashSet<String>>>,
    /// Parked bytes and the earliest parked HLC per extension prefix.
    pub(super) parked: HashMap<String, (usize, String)>,
    /// The highest `purge_hlc` per prefix of an extension removed with "delete data": changes
    /// older than it are dropped, also after a later "keep data" removal.
    pub(super) purges: HashMap<String, String>,
    /// The `purge_hlc` of the newest removal per prefix, of either kind.
    latest: HashMap<String, String>,
    /// The prefixes whose newest removal deletes data and has not been cleared up for here yet.
    pub(super) pending: HashSet<String>,
    /// Prefix and "delete data" per registered extension id, to read a removal from the wire.
    registry: HashMap<String, (String, bool)>,
    /// The last `purge_hlc` cleared up for, per extension id.
    applied: HashMap<String, String>,
    /// Prefixes of development versions on this device: their tables are not the synced ones.
    pub(super) dev: HashSet<String>,
}

/// A removal read from a group that was applied: the `purge_hlc` of `prefix` and whether it
/// deletes data.
#[derive(Debug, Clone)]
pub(in crate::sync::inbound) struct Noted {
    prefix: String,
    extension_id: String,
    purge_hlc: String,
    purge_data: bool,
}

impl Context {
    pub(in crate::sync::inbound) fn read(q: &mut impl Query) -> haex_crdt::Result<Self> {
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
        let cleared: Vec<(String, String, Option<String>)> = q.query_map(
            "SELECT extension_id, purge_hlc, data_purge_hlc FROM extension_purges_applied_no_sync",
            &[],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        let applied: HashMap<String, String> = cleared
            .iter()
            .map(|(id, purge_hlc, _)| (id.clone(), purge_hlc.clone()))
            .collect();
        let mut context = Self {
            synced,
            extension_tables,
            parked,
            applied,
            dev: crate::extensions::dev::prefixes(q)?,
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
        // A "delete data" removal this device cleared up for keeps filtering after the registry
        // row moved on to a later "keep data" removal.
        for (id, _, data_purge_hlc) in cleared {
            if let (Some((prefix, _)), Some(data_purge_hlc)) =
                (context.registry.get(&id).cloned(), data_purge_hlc)
            {
                context.raise_purge(&prefix, &data_purge_hlc);
            }
        }
        Ok(context)
    }

    /// Records a removal of an extension. The filter keeps the highest "delete data" removal;
    /// only the newest removal of either kind decides whether a clear-up is still pending.
    fn set_purge(&mut self, removal: Noted) {
        if removal.purge_data {
            self.raise_purge(&removal.prefix, &removal.purge_hlc);
        }
        let newest = self.latest.get(&removal.prefix).is_none_or(|latest| {
            compare_hlc_strings(&removal.purge_hlc, latest) == Ordering::Greater
        });
        if !newest {
            return;
        }
        self.latest
            .insert(removal.prefix.clone(), removal.purge_hlc.clone());
        self.pending.remove(&removal.prefix);
        let cleared = self
            .applied
            .get(&removal.extension_id)
            .is_some_and(|applied| {
                compare_hlc_strings(&removal.purge_hlc, applied) != Ordering::Greater
            });
        if removal.purge_data && !cleared {
            self.pending.insert(removal.prefix);
        }
    }

    /// Raises the "delete data" filter of `prefix` to `purge_hlc` if that is higher.
    fn raise_purge(&mut self, prefix: &str, purge_hlc: &str) {
        let higher = self
            .purges
            .get(prefix)
            .is_none_or(|current| compare_hlc_strings(purge_hlc, current) == Ordering::Greater);
        if higher {
            self.purges.insert(prefix.to_owned(), purge_hlc.to_owned());
        }
    }

    /// Reads the removals an applied group writes, so later groups of the same pull already obey
    /// them (the clear-up runs only after the pull); returns them for the pages still to come.
    pub(in crate::sync::inbound) fn note(&mut self, columns: &[ColumnChange]) -> Vec<Noted> {
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
    pub(in crate::sync::inbound) fn extend_noted(&mut self, noted: &[Noted]) {
        for removal in noted {
            self.set_purge(removal.clone());
        }
    }

    /// Whether `prefix` has parked groups.
    pub(in crate::sync::inbound) fn has_parked(&self, prefix: &str) -> bool {
        self.parked.contains_key(prefix)
    }

    /// Whether any extension has parked groups.
    pub(in crate::sync::inbound) fn has_any_parked(&self) -> bool {
        !self.parked.is_empty()
    }

    /// Parked bytes of `prefix`.
    pub(in crate::sync::inbound) fn parked_bytes(&self, prefix: &str) -> usize {
        self.parked.get(prefix).map_or(0, |(bytes, _)| *bytes)
    }

    /// Records a group parked in this page.
    pub(in crate::sync::inbound) fn add_parked(&mut self, prefix: &str, hlc: &str, bytes: usize) {
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
