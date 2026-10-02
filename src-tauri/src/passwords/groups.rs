//! Folders (spec 034, US2, FR-009, FR-010): nested folders, moving entries and folders, the order
//! of siblings. An entry lies in at most one folder (a single `group_items` row); a folder never
//! goes into itself or below itself; nothing is moved into the trash (that is `trash`), and the
//! trash and what lies in it cannot be reordered.

use haex_crdt::rusqlite::params;
use haex_crdt::CrdtTransaction;
use uuid::Uuid;

use super::model::{GroupPatch, GroupRow, Patch, Target, TargetKind};
use super::sets::Sets;
use super::{clock, TRASH_GROUP_ID};
use crate::error::{HolziError, Result};
use crate::storage::query::Query;

fn invalid(reason: &str) -> HolziError {
    HolziError::InvalidInput {
        reason: reason.to_string(),
    }
}

fn exists(q: &mut impl Query, sql: &str, id: &str) -> Result<bool> {
    Ok(q.query_row(sql, params![id], |r| r.get::<_, i64>(0))?
        .unwrap_or(0)
        > 0)
}

/// Whether `group_id` is the trash or lies below it.
pub fn is_in_trash(q: &mut impl Query, group_id: &str) -> Result<bool> {
    path_contains(q, group_id, TRASH_GROUP_ID)
}

/// Whether the folder `ancestor` is `group_id` itself or one of the folders above it.
fn path_contains(q: &mut impl Query, group_id: &str, ancestor: &str) -> Result<bool> {
    // `UNION` (not `UNION ALL`) drops repeated rows, so even a corrupt loop in the data ends.
    Ok(q.query_row(
        "WITH RECURSIVE up(id, parent_id) AS ( \
           SELECT id, parent_id FROM haex_passwords_groups WHERE id = ?1 \
           UNION \
           SELECT g.id, g.parent_id FROM haex_passwords_groups g JOIN up ON g.id = up.parent_id) \
         SELECT COUNT(*) FROM up WHERE id = ?2",
        params![group_id, ancestor],
        |r| r.get::<_, i64>(0),
    )?
    .unwrap_or(0)
        > 0)
}

/// The sibling folders of a level in display order: by `sort_order` (empty counts as 0), then by
/// name ignoring case. The top level does not list the trash.
pub fn siblings(q: &mut impl Query, parent: Option<&str>) -> Result<Vec<GroupRow>> {
    let (filter, bound): (&str, Vec<&dyn haex_crdt::rusqlite::ToSql>) = match &parent {
        Some(p) => ("parent_id = ?1", vec![p as &dyn haex_crdt::rusqlite::ToSql]),
        None => ("parent_id IS NULL AND id <> 'trash'", vec![]),
    };
    Ok(q.query_map(
        &format!(
            "SELECT id, name, description, icon, color, sort_order, parent_id, \
                    trashed_from_parent_id FROM haex_passwords_groups WHERE {filter} \
             ORDER BY COALESCE(sort_order, 0), name COLLATE NOCASE, id"
        ),
        &bound,
        |r| {
            Ok(GroupRow {
                id: r.get(0)?,
                name: r.get(1)?,
                description: r.get(2)?,
                icon: r.get(3)?,
                color: r.get(4)?,
                sort_order: r.get(5)?,
                parent_id: r.get(6)?,
                trashed_from_parent_id: r.get(7)?,
            })
        },
    )?)
}

/// Creates a folder under `parent` (or at the top). The name may not be empty; the parent must
/// exist and must not be the trash or below it.
pub fn create_group(
    tx: &mut CrdtTransaction<'_>,
    name: &str,
    description: Option<&str>,
    icon: Option<&str>,
    color: Option<&str>,
    parent: Option<&str>,
) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(invalid("name"));
    }
    if let Some(parent) = parent {
        if !exists(
            tx,
            "SELECT COUNT(*) FROM haex_passwords_groups WHERE id = ?1",
            parent,
        )? {
            return Err(invalid("parent"));
        }
        if is_in_trash(tx, parent)? {
            return Err(invalid("target_in_trash"));
        }
    }
    let id = Uuid::new_v4().to_string();
    let now = clock::now();
    tx.execute(
        "INSERT INTO haex_passwords_groups \
         (id, name, description, icon, color, parent_id, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        params![id, name, description, icon, color, parent, now],
    )?;
    Ok(id)
}

/// Changes the given fields of a folder. A folder keeps a non-empty name.
pub fn update_group(tx: &mut CrdtTransaction<'_>, id: &str, patch: &GroupPatch) -> Result<()> {
    if id == TRASH_GROUP_ID {
        return Err(invalid("trash"));
    }
    let stored = tx
        .query_row(
            "SELECT name, description, icon, color, sort_order FROM haex_passwords_groups \
             WHERE id = ?1",
            params![id],
            |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<i64>>(4)?,
                ))
            },
        )?
        .ok_or(HolziError::PasswordsNotFound)?;
    let name = match &patch.name {
        Patch::Keep => Patch::Keep,
        Patch::Clear => return Err(invalid("name")),
        Patch::Set(name) if name.trim().is_empty() => return Err(invalid("name")),
        Patch::Set(name) => Patch::Set(name.trim().to_string()),
    };
    let mut sets = Sets::default();
    sets.text("name", &stored.0, &name);
    sets.text("description", &stored.1, &patch.description);
    sets.text("icon", &stored.2, &patch.icon);
    sets.text("color", &stored.3, &patch.color);
    let order = match &patch.sort_order {
        Patch::Keep => Patch::Keep,
        Patch::Clear => Patch::Clear,
        Patch::Set(value) => Patch::Set(i64::from(*value)),
    };
    sets.number("sort_order", stored.4, &order);
    if sets.is_empty() {
        return Ok(());
    }
    sets.push("updated_at", clock::now());
    sets.execute(tx, "haex_passwords_groups", "id", id)
}

/// Moves entries and folders to `to` (`None` is the top level), all or none (the caller's
/// transaction rolls back on an error). A folder cannot go into itself or below itself (`cycle`),
/// nothing goes into the trash (`target_in_trash`), a target that does not exist is
/// `target_missing`. Returns the number of targets moved.
pub fn move_targets(
    tx: &mut CrdtTransaction<'_>,
    targets: &[Target],
    to: Option<&str>,
) -> Result<u32> {
    if let Some(to) = to {
        if !exists(
            tx,
            "SELECT COUNT(*) FROM haex_passwords_groups WHERE id = ?1",
            to,
        )? {
            return Err(invalid("target_missing"));
        }
        if is_in_trash(tx, to)? {
            return Err(invalid("target_in_trash"));
        }
    }
    for target in targets {
        match target.kind {
            TargetKind::Item => move_item(tx, &target.id, to)?,
            TargetKind::Group => move_group(tx, &target.id, to)?,
        }
    }
    Ok(u32::try_from(targets.len()).unwrap_or(u32::MAX))
}

fn move_item(tx: &mut CrdtTransaction<'_>, item_id: &str, to: Option<&str>) -> Result<()> {
    if !exists(
        tx,
        "SELECT COUNT(*) FROM haex_passwords_item_details WHERE id = ?1",
        item_id,
    )? {
        return Err(invalid("target_missing"));
    }
    let current = tx.query_row(
        "SELECT group_id, trashed_from_group_id FROM haex_passwords_group_items WHERE item_id = ?1",
        params![item_id],
        |r| {
            Ok((
                r.get::<_, Option<String>>(0)?,
                r.get::<_, Option<String>>(1)?,
            ))
        },
    )?;
    match current {
        None => {
            tx.execute(
                "INSERT INTO haex_passwords_group_items (item_id, group_id) VALUES (?1, ?2)",
                params![item_id, to],
            )?;
        }
        Some((group, from)) => {
            let mut sets = Sets::default();
            if group.as_deref() != to {
                sets.push("group_id", to.map(str::to_string));
            }
            if from.is_some() {
                sets.push("trashed_from_group_id", Option::<String>::None);
            }
            sets.execute(tx, "haex_passwords_group_items", "item_id", item_id)?;
        }
    }
    Ok(())
}

fn move_group(tx: &mut CrdtTransaction<'_>, group_id: &str, to: Option<&str>) -> Result<()> {
    if group_id == TRASH_GROUP_ID {
        return Err(invalid("target_in_trash"));
    }
    let current = tx
        .query_row(
            "SELECT parent_id, trashed_from_parent_id FROM haex_passwords_groups WHERE id = ?1",
            params![group_id],
            |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?,
                    r.get::<_, Option<String>>(1)?,
                ))
            },
        )?
        .ok_or_else(|| invalid("target_missing"))?;
    if let Some(to) = to {
        if path_contains(tx, to, group_id)? {
            return Err(invalid("cycle"));
        }
    }
    let mut sets = Sets::default();
    if current.0.as_deref() != to {
        sets.push("parent_id", to.map(str::to_string));
    }
    if current.1.is_some() {
        sets.push("trashed_from_parent_id", Option::<String>::None);
    }
    if sets.is_empty() {
        return Ok(());
    }
    sets.push("updated_at", clock::now());
    sets.execute(tx, "haex_passwords_groups", "id", group_id)
}

/// Gives the sibling folders of one level the order 0, 1, 2, … in the order of `ordered_ids`,
/// which must be exactly the siblings of that level (`not_siblings` otherwise); the trash subtree
/// is `in_trash`. Only folders whose position changes are written.
pub fn reorder_groups(
    tx: &mut CrdtTransaction<'_>,
    parent: Option<&str>,
    ordered_ids: &[String],
) -> Result<()> {
    if let Some(parent) = parent {
        if is_in_trash(tx, parent)? {
            return Err(invalid("in_trash"));
        }
    }
    let current = siblings(tx, parent)?;
    let mut have: Vec<&str> = current.iter().map(|g| g.id.as_str()).collect();
    let mut want: Vec<&str> = ordered_ids.iter().map(String::as_str).collect();
    have.sort_unstable();
    want.sort_unstable();
    if have != want {
        return Err(invalid("not_siblings"));
    }
    for (position, id) in ordered_ids.iter().enumerate() {
        let position = i32::try_from(position).unwrap_or(i32::MAX);
        let stored = current
            .iter()
            .find(|g| &g.id == id)
            .and_then(|g| g.sort_order);
        if stored != Some(position) {
            tx.execute(
                "UPDATE haex_passwords_groups SET sort_order = ?1, updated_at = ?2 WHERE id = ?3",
                params![position, clock::now(), id],
            )?;
        }
    }
    Ok(())
}
