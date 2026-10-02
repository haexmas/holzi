//! Tags of the entries (spec 034, FR-011, research R2). This part holds what the entries need:
//! creating a tag by name and setting the tags of an entry. Renaming, deleting and the bulk
//! operations come with the folders and tags story.
//!
//! There is no UNIQUE constraint (a conflict halts the sync): a tag's id is derived from its name
//! (`ids::tag_id`), and the application keeps names unique by [`fold`] — case, surrounding space and
//! the spelling of umlauts do not make a second tag.

use std::collections::BTreeSet;

use haex_crdt::rusqlite::params;
use haex_crdt::CrdtTransaction;

use super::ids::{fold, item_tag_id, tag_id};
use crate::error::{HolziError, Result};
use crate::storage::query::Query;

/// The longest tag name, in characters.
pub const MAX_TAG_NAME_CHARS: usize = 64;

/// The trimmed name, or `InvalidInput` with the reason `empty` or `too_long`.
pub fn validate_name(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(HolziError::InvalidInput {
            reason: "empty".to_string(),
        });
    }
    if trimmed.chars().count() > MAX_TAG_NAME_CHARS {
        return Err(HolziError::InvalidInput {
            reason: "too_long".to_string(),
        });
    }
    Ok(trimmed.to_string())
}

/// The id of the tag with this name (by [`fold`]), created when there is none. The stored spelling
/// of an existing tag stays, the spelling of the first creator wins. A renamed tag keeps its old id,
/// so the lookup goes by the folded name, not by the derived id.
pub fn get_or_create(tx: &mut CrdtTransaction<'_>, name: &str) -> Result<String> {
    let name = validate_name(name)?;
    let wanted = fold(&name);
    let existing: Vec<(String, String)> =
        tx.query_map("SELECT id, name FROM haex_passwords_tags", &[], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?;
    if let Some((id, _)) = existing.iter().find(|(_, stored)| fold(stored) == wanted) {
        return Ok(id.clone());
    }
    let id = tag_id(&name).to_string();
    tx.execute(
        "INSERT INTO haex_passwords_tags (id, name, created_at) VALUES (?1, ?2, ?3)",
        params![id, name, super::clock::now()],
    )?;
    Ok(id)
}

/// Makes the tags of the entry exactly `names` (by [`fold`]): links that are no longer wanted go,
/// missing links come, existing links stay untouched so a concurrent change of another link merges.
pub fn set_item_tags(tx: &mut CrdtTransaction<'_>, item_id: &str, names: &[String]) -> Result<()> {
    let mut wanted = BTreeSet::new();
    for name in names {
        wanted.insert(get_or_create(tx, name)?);
    }
    let current: Vec<(String, String)> = tx.query_map(
        "SELECT id, tag_id FROM haex_passwords_item_tags WHERE item_id = ?1",
        params![item_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    for (link_id, tag) in &current {
        if !wanted.contains(tag) {
            tx.execute(
                "DELETE FROM haex_passwords_item_tags WHERE id = ?1",
                params![link_id],
            )?;
        }
    }
    let have: BTreeSet<&String> = current.iter().map(|(_, tag)| tag).collect();
    for tag in wanted.iter().filter(|tag| !have.contains(tag)) {
        tx.execute(
            "INSERT INTO haex_passwords_item_tags (id, item_id, tag_id) VALUES (?1, ?2, ?3)",
            params![item_tag_id(item_id, tag).to_string(), item_id, tag],
        )?;
    }
    Ok(())
}

/// The tag names of an entry, in their stored spelling.
pub fn names_of_item(q: &mut impl Query, item_id: &str) -> Result<Vec<String>> {
    Ok(q.query_map(
        "SELECT t.name FROM haex_passwords_item_tags it \
         JOIN haex_passwords_tags t ON t.id = it.tag_id \
         WHERE it.item_id = ?1 ORDER BY t.name",
        params![item_id],
        |r| r.get::<_, String>(0),
    )?)
}
