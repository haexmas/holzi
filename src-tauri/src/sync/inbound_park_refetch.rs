//! Parked groups to fetch again (spec 017, research R10). Part of [`super`].
//!
//! A group parked while this device was behind can be overtaken: this device then applies a
//! migration that drops or renames the very column the group writes, and the group can never
//! apply. Its origin wrote it with an older version; once that device updated, a fresh copy has
//! the form its own migration gave it. Replay marks such a group `refetch`. Every pull then asks
//! for its origin from right before it ([`floors`]) without lowering the stored progress, and the
//! fresh copy is sorted like any group. The mark goes once a pull that asked from below it got
//! past it ([`forget`]): the fresh copy is applied or parked again by then, or the group is gone
//! because later changes overwrote all of it. A pull that started before the mark leaves it alone.

use haex_crdt::rusqlite::params;
use haex_crdt::{compare_hlc_strings, CrdtTransaction};
use uuid::Uuid;

use super::PARKED_TABLE;
use crate::storage::query::Query;
use crate::sync::progress::{self, Vector};

/// The reason of a parked group to fetch again.
pub(in crate::sync::inbound) const REFETCH: &str = "refetch";

/// Origin and HLC of a group to fetch again.
pub type Floor = (Uuid, String);

/// Marks parked group `id` to be fetched again; `true` when it was not marked yet.
pub(super) fn mark(tx: &mut CrdtTransaction<'_>, id: i64) -> haex_crdt::Result<bool> {
    let changed = tx.execute(
        &format!("UPDATE {PARKED_TABLE} SET reason = ?2 WHERE id = ?1 AND reason != ?2"),
        params![id, REFETCH],
    )?;
    Ok(changed > 0)
}

/// The groups to fetch again, and `vector` lowered to right before each.
pub fn floors(q: &mut impl Query, vector: &mut Vector) -> haex_crdt::Result<Vec<Floor>> {
    let rows: Vec<(String, String)> = q.query_map(
        &format!("SELECT origin, hlc FROM {PARKED_TABLE} WHERE reason = ?1"),
        params![REFETCH],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let mut floors = Vec::new();
    for (origin, hlc) in rows {
        let Ok(origin) = Uuid::parse_str(&origin) else {
            continue;
        };
        match progress::just_below(&hlc) {
            Some(below) => {
                if vector
                    .get(&origin)
                    .is_some_and(|cursor| compare_hlc_strings(cursor, &below).is_gt())
                {
                    vector.insert(origin, below);
                }
            }
            None => {
                vector.remove(&origin);
            }
        }
        floors.push((origin, hlc));
    }
    Ok(floors)
}

/// Removes the marks of `floors`, which a finished pull asked for from below.
pub(in crate::sync::inbound) fn forget(
    tx: &mut CrdtTransaction<'_>,
    floors: &[Floor],
) -> haex_crdt::Result<()> {
    for (origin, hlc) in floors {
        tx.execute(
            &format!("DELETE FROM {PARKED_TABLE} WHERE origin = ?1 AND hlc = ?2 AND reason = ?3"),
            params![origin.to_string(), hlc, REFETCH],
        )?;
    }
    Ok(())
}
