//! Groups of removed devices (spec 024, research R5). Part of [`super`].

use std::cmp::Ordering;
use std::collections::HashMap;

use haex_crdt::compare_hlc_strings;
use uuid::Uuid;

use crate::storage::query;
use crate::sync::device_list;
use crate::sync::keys;

/// Per removed origin its limit in the effective list. Only that list's
/// removals count (FR-043, FR-005): it carries forward every removal of the
/// lists it builds on, and a same-generation fork that lost the tie-break has
/// no say. Two main devices that removed each other each hold such a fork;
/// counting the losing one would reject everything the remaining main device
/// writes from then on.
pub(super) fn removal_limits(
    q: &mut impl query::Query,
) -> haex_crdt::Result<HashMap<Uuid, String>> {
    let Some(vault) = keys::vault_pubkey(q)? else {
        return Ok(HashMap::new());
    };
    let valid = device_list::valid_lists(&device_list::load_all(q)?, &vault);
    let Some(effective) = device_list::effective(&valid) else {
        return Ok(HashMap::new());
    };
    let mut limits: HashMap<Uuid, String> = HashMap::new();
    for removed in &effective.list.removed {
        let limit = limits
            .entry(removed.vault_device_uuid)
            .or_insert_with(|| removed.limit_hlc.clone());
        if compare_hlc_strings(&removed.limit_hlc, limit) == Ordering::Less {
            *limit = removed.limit_hlc.clone();
        }
    }
    Ok(limits)
}

/// Whether a group of `origin` at `hlc` lies beyond its removal limit.
pub(super) fn is_removed_at(limits: &HashMap<Uuid, String>, origin: Uuid, hlc: &str) -> bool {
    limits
        .get(&origin)
        .is_some_and(|limit| compare_hlc_strings(hlc, limit) == Ordering::Greater)
}
