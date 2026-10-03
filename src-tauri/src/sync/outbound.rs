//! Serving a pull (spec 024, FR-019, research R4, contracts/sync-protocol.md
//! §2).
//!
//! The sender scans every synced table and the delete log once from the
//! smallest cursor the receiver named and keeps, per cell, what lies beyond
//! the receiver's cursor for the cell's origin. It serves nothing beyond
//! its own progress read before the scan (see [`crate::sync::replica`]), so
//! the receiver can take that progress over once it applied every page.
//! Cells are sorted by HLC across all origins and packed into pages that
//! never share a transaction group.

use std::cmp::Ordering;
use std::collections::VecDeque;

use haex_crdt::{compare_hlc_strings, ColumnChange, ScanFilters};

use crate::storage::query;
use crate::sync::change::{Change, Page, PAGE_BUDGET};
use crate::sync::progress::{self, Vector};
use crate::sync::replica::{synced_tables, Replica};

/// The pages of one pull, handed out one at a time.
#[derive(Debug)]
pub struct Outbox {
    groups: VecDeque<VecDeque<Change>>,
    served: Vector,
    budget: usize,
    finished: bool,
}

/// The answer to a pull request.
#[derive(Debug)]
pub enum Served {
    Pages(Outbox),
    /// The puller is too far behind for an incremental pull
    /// ([`crate::sync::resync`]).
    Resync,
}

/// Answers a `Pull`: a snapshot (served as a pull from nothing) when
/// `replace` is set, `Resync` when the puller is too far behind, otherwise
/// what `theirs` lacks.
pub fn serve(replica: &Replica, theirs: &Vector, replace: bool) -> haex_crdt::Result<Served> {
    if replace {
        return Ok(Served::Pages(serve_pull(replica, &Vector::new())?));
    }
    let served = replica.progress()?;
    if crate::sync::resync::is_stale(theirs, &served, std::time::SystemTime::now()) {
        return Ok(Served::Resync);
    }
    Ok(Served::Pages(serve_pull(replica, theirs)?))
}

/// Collects what `theirs` lacks from this device.
pub fn serve_pull(replica: &Replica, theirs: &Vector) -> haex_crdt::Result<Outbox> {
    serve_pull_with_budget(replica, theirs, PAGE_BUDGET)
}

/// [`serve_pull`] with a page size of `budget` bytes.
pub fn serve_pull_with_budget(
    replica: &Replica,
    theirs: &Vector,
    budget: usize,
) -> haex_crdt::Result<Outbox> {
    // Reading a large pull takes a while; the close waits for it (see [`Replica::hold`]).
    let _held = replica.hold().map_err(haex_crdt::Error::consumer)?;
    let db = replica.db();
    let (served, changes) = {
        let _exchange = replica.exchange();
        let served = replica.progress()?;
        let tables = query::read(db, |r| synced_tables(r))?;
        let after = scan_cursor(theirs, &served);
        let mut changes = Vec::new();
        for table in tables {
            let scanned =
                db.scan_table_for_local_changes(&table, after.as_deref(), ScanFilters::default())?;
            changes.extend(scanned.into_iter().filter(|c| wanted(c, theirs, &served)));
        }
        (served, changes)
    };
    Ok(Outbox::new(changes, served, budget))
}

/// Where the scan starts: a safe SQL lower bound for the smallest cursor the
/// receiver named, or the beginning when it lacks an origin this device knows.
///
/// haex-crdt's scanner uses the row HLC in a SQL text comparison as a cheap
/// pre-filter, then compares each column HLC numerically. HLC node suffixes
/// are variable-width hexadecimal, so using the exact cursor there could
/// hide a numerically newer `time/100` row behind `time/ff`. Dropping the
/// node suffix keeps every HLC at the cursor's physical time in the scan;
/// the scanner's per-column comparison still applies the exact lower bound.
fn scan_cursor(theirs: &Vector, served: &Vector) -> Option<String> {
    if served.keys().any(|origin| !theirs.contains_key(origin)) {
        return None;
    }
    theirs
        .values()
        .min_by(|a, b| compare_hlc_strings(a, b))
        .and_then(|cursor| cursor.split_once('/').map(|(time, _)| format!("{time}/")))
}

/// Whether the receiver lacks `change` and this pull may serve it.
fn wanted(change: &ColumnChange, theirs: &Vector, served: &Vector) -> bool {
    let Some(origin) = progress::origin_of(&change.hlc_timestamp) else {
        return false;
    };
    let hlc = change.hlc_timestamp.as_str();
    progress::is_beyond(hlc, theirs.get(&origin))
        && served
            .get(&origin)
            .is_none_or(|cap| compare_hlc_strings(hlc, cap) != Ordering::Greater)
}

impl Outbox {
    fn new(changes: Vec<ColumnChange>, served: Vector, budget: usize) -> Self {
        let mut changes: Vec<Change> = changes.iter().map(Change::from_column).collect();
        changes.sort_by(|a, b| {
            compare_hlc_strings(&a.hlc, &b.hlc)
                .then_with(|| a.table.cmp(&b.table))
                .then_with(|| a.row_pks.cmp(&b.row_pks))
                .then_with(|| a.column.cmp(&b.column))
        });
        let mut groups: VecDeque<VecDeque<Change>> = VecDeque::new();
        for change in changes {
            let parts = change.split(budget);
            match groups.back_mut() {
                Some(group) if group.front().is_some_and(|c| c.hlc == parts[0].hlc) => {
                    group.extend(parts)
                }
                _ => groups.push_back(parts.into()),
            }
        }
        Self {
            groups,
            served,
            budget,
            finished: false,
        }
    }

    /// Whether the pull has changes at all.
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    /// The next page; `None` after the last one.
    pub fn next_page(&mut self) -> Option<Page> {
        if self.finished {
            return None;
        }
        let mut changes = Vec::new();
        let mut size = 0;
        let mut group_continues = false;
        while let Some(group) = self.groups.front_mut() {
            let group_size: usize = group.iter().map(Change::wire_size).sum();
            if size + group_size <= self.budget {
                size += group_size;
                changes.extend(self.groups.pop_front().into_iter().flatten());
                continue;
            }
            // A group that does not fit starts on a fresh page and, if it
            // does not fit a page either, fills pages part by part. A group
            // begun on an earlier page is always the first on this one.
            if !changes.is_empty() {
                break;
            }
            while let Some(part) = group.front() {
                if !changes.is_empty() && size + part.wire_size() > self.budget {
                    break;
                }
                size += part.wire_size();
                changes.extend(group.pop_front());
            }
            if group.is_empty() {
                self.groups.pop_front();
                continue;
            }
            group_continues = true;
            break;
        }
        let more = !self.groups.is_empty();
        self.finished = !more;
        Some(Page {
            changes,
            group_continues,
            more,
            served: if more {
                Vector::new()
            } else {
                self.served.clone()
            },
        })
    }
}

#[cfg(test)]
#[path = "outbound_tests.rs"]
mod tests;
