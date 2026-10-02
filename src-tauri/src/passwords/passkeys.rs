//! Passkeys as data (spec 034, FR-004): list, rename and delete, and the insert for the import.
//! The window never creates a passkey and nothing here signs; keys are never listed.

use haex_crdt::rusqlite::params;

use super::model::PasskeyView;
use crate::error::Result;
use crate::storage::query::Query;

/// The passkeys of an entry without their keys.
pub fn list_for_item(q: &mut impl Query, item_id: &str) -> Result<Vec<PasskeyView>> {
    Ok(q.query_map(
        "SELECT id, relying_party_id, relying_party_name, user_name, nickname, algorithm, \
                created_at, last_used_at \
         FROM haex_passwords_passkeys WHERE item_id = ?1 ORDER BY rowid",
        params![item_id],
        |r| {
            Ok(PasskeyView {
                id: r.get(0)?,
                relying_party_id: r.get(1)?,
                relying_party_name: r.get(2)?,
                user_name: r.get(3)?,
                nickname: r.get(4)?,
                algorithm: r.get(5)?,
                created_at: r.get(6)?,
                last_used_at: r.get(7)?,
            })
        },
    )?)
}
