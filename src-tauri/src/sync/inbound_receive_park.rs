//! Parking while a page is received (spec 017, research R10): the limit per extension and the
//! origin it blocks, and keeping a row whole when its cells came split across an applicable and a
//! parked group. Part of [`super`].

use std::cmp::Ordering;

use haex_crdt::compare_hlc_strings;
use uuid::Uuid;

use super::hold::Group;
use super::park::{self, Context, Parked, Sorted};
use super::{InboundError, Inbox};
use crate::sync::change::group_bytes;

/// The groups a page parks, and the extensions that reached their limit in it.
#[derive(Default)]
pub(super) struct Parking {
    pub parked: Vec<(String, Parked, usize)>,
    pub newly_full: Vec<String>,
}

impl Inbox {
    /// Parks `group` of transaction `hlc` from `origin`; with its extension at the limit it is left
    /// for a later pull instead, and the progress of `origin` stays below it.
    pub(super) fn park_group(
        &mut self,
        context: &mut Context,
        parking: &mut Parking,
        origin: Uuid,
        hlc: String,
        group: Parked,
        bytes: usize,
    ) {
        if let Some(snapshot) = &mut self.snapshot {
            snapshot.rows.extend(
                group
                    .columns
                    .iter()
                    .map(|c| (c.table_name.clone(), c.row_pks.clone())),
            );
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
            if self
                .blocked
                .get(&origin)
                .is_none_or(|lowest| compare_hlc_strings(&hlc, lowest) == Ordering::Less)
            {
                self.blocked.insert(origin, hlc);
            }
            return;
        }
        log::info!(
            "sync: parked a group for extension {} ({})",
            group.prefix,
            group.reason
        );
        context.add_parked(&group.prefix, &hlc, bytes);
        parking.parked.push((hlc, group, bytes));
    }

    /// Parks every group of `candidates` whose extension has parked groups, earlier ones of this
    /// page and groups held from earlier pages too. A row created in one group and completed in a
    /// later, parked one would otherwise be written without the parked cells (a NOT NULL column)
    /// and fail every pull. Returns the groups that still apply.
    pub(super) fn park_with_parked(
        &mut self,
        context: &mut Context,
        parking: &mut Parking,
        candidates: Vec<Group>,
    ) -> Result<Vec<Group>, InboundError> {
        let mut kept = Vec::with_capacity(candidates.len());
        for group in candidates {
            let sorted = park::sort(context, &group.hlc, group.columns, |prefix| {
                context.has_parked(prefix) || self.full_prefixes.contains(prefix)
            })?;
            match sorted {
                Sorted::Apply(columns) => kept.push(Group { columns, ..group }),
                Sorted::Park(parked) => {
                    let bytes = group_bytes(&parked.columns)?;
                    self.park_group(context, parking, group.origin, group.hlc, parked, bytes);
                }
            }
        }
        Ok(kept)
    }
}
