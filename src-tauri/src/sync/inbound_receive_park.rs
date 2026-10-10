//! Parking while a page is received (spec 017, research R10): the limit per extension and the
//! origin it blocks, and keeping a row whole when its cells came split across an applicable and a
//! parked group. Part of [`super`].

use std::cmp::Ordering;

use haex_crdt::{compare_hlc_strings, Database};
use uuid::Uuid;

use super::hold::Group;
use super::park::{self, Context, Parked, Sorted};
use super::{InboundError, Inbox};
use crate::storage::query;
use crate::sync::change::group_bytes;
use crate::sync::progress::Vector;

/// The groups a page parks, and the extensions that reached their limit in it.
pub(super) struct Parking<'a> {
    db: &'a Database,
    pub parked: Vec<(String, Parked, usize)>,
    pub newly_full: Vec<String>,
}

impl<'a> Parking<'a> {
    pub(super) fn new(db: &'a Database) -> Self {
        Self {
            db,
            parked: Vec::new(),
            newly_full: Vec::new(),
        }
    }
}

impl Inbox {
    /// This pull asks for the parked groups `floors` again ([`park::refetch`]).
    pub fn refetching(mut self, floors: Vec<park::refetch::Floor>) -> Self {
        self.refetch = floors;
        self
    }

    /// On the last page, the groups asked for again that this pull got past: the sender served
    /// their origin up to them at least, and no group of it waits.
    pub(super) fn refetched(&self, last: bool, served: &Vector) -> Vec<park::refetch::Floor> {
        if !last {
            return Vec::new();
        }
        self.refetch
            .iter()
            .filter(|(origin, hlc)| {
                !self.blocked.contains_key(origin)
                    && served
                        .get(origin)
                        .is_some_and(|up_to| compare_hlc_strings(up_to, hlc) != Ordering::Less)
            })
            .cloned()
            .collect()
    }

    /// Parks `group` of transaction `hlc` from `origin`; with its extension at the limit, or waiting
    /// for a device to update it, it is left for a later pull instead, and the progress of `origin`
    /// stays below it.
    pub(super) fn park_group(
        &mut self,
        context: &mut Context,
        parking: &mut Parking<'_>,
        origin: Uuid,
        hlc: String,
        group: Parked,
        bytes: usize,
    ) -> Result<(), InboundError> {
        if let Some(snapshot) = &mut self.snapshot {
            snapshot.rows.extend(
                group
                    .columns
                    .iter()
                    .map(|c| (c.table_name.clone(), c.row_pks.clone())),
            );
        }
        // Fetched again (progress stayed below it): stored and counted already.
        let origin_text = origin.to_string();
        if query::read(parking.db, |r| park::is_stored(r, &origin_text, &hlc))? {
            parking.parked.push((hlc, group, bytes));
            return Ok(());
        }
        // Written by a device with an older version of the extension: no use parking it. It and
        // the later groups of the extension in this pull come again until that device updated.
        if group.reason == park::OUTDATED_ORIGIN || self.waiting.contains(&group.prefix) {
            if self.waiting.insert(group.prefix.clone()) {
                log::info!(
                    "sync: extension {} waits for device {origin} to update it",
                    group.prefix
                );
            }
            self.block(origin, hlc);
            return Ok(());
        }
        let limit = self.park_limit.unwrap_or(park::PARKED_LIMIT_BYTES);
        let full = self.full_prefixes.contains(&group.prefix)
            || context.parked_bytes(&group.prefix).saturating_add(bytes) > limit;
        if full {
            if self.full_prefixes.insert(group.prefix.clone()) {
                parking.newly_full.push(group.prefix.clone());
                log::warn!(
                    "sync: parked groups of extension {} reached {limit} bytes; \
                     its origin's progress waits until it is installed or removed",
                    group.prefix
                );
            }
            self.block(origin, hlc);
            return Ok(());
        }
        log::info!(
            "sync: parked a group for extension {} ({})",
            group.prefix,
            group.reason
        );
        context.add_parked(&group.prefix, &hlc, bytes);
        parking.parked.push((hlc, group, bytes));
        Ok(())
    }

    /// Keeps the progress of `origin` below its group `hlc`, which comes again in a later pull.
    fn block(&mut self, origin: Uuid, hlc: String) {
        if self
            .blocked
            .get(&origin)
            .is_none_or(|lowest| compare_hlc_strings(&hlc, lowest) == Ordering::Less)
        {
            self.blocked.insert(origin, hlc);
        }
    }

    /// Whether a group of `prefix` has to queue behind a parked or waiting one.
    pub(super) fn queues(&self, context: &Context, prefix: &str) -> bool {
        context.has_parked(prefix)
            || self.full_prefixes.contains(prefix)
            || self.waiting.contains(prefix)
    }

    /// Parks every group of `candidates` whose extension has parked groups, earlier ones of this
    /// page and groups held from earlier pages too. A row created in one group and completed in a
    /// later, parked one would otherwise be written without the parked cells (a NOT NULL column)
    /// and fail every pull. Returns the groups that still apply.
    pub(super) fn park_with_parked(
        &mut self,
        context: &mut Context,
        parking: &mut Parking<'_>,
        candidates: Vec<Group>,
    ) -> Result<Vec<Group>, InboundError> {
        let mut kept = Vec::with_capacity(candidates.len());
        for group in candidates {
            let sorted = park::sort(context, &group.hlc, group.columns, |prefix| {
                self.queues(context, prefix)
            })?;
            match sorted {
                Sorted::Apply(columns) => kept.push(Group { columns, ..group }),
                Sorted::Park(parked) => {
                    let bytes = group_bytes(&parked.columns)?;
                    self.park_group(context, parking, group.origin, group.hlc, parked, bytes)?;
                }
            }
        }
        Ok(kept)
    }
}
