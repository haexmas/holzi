//! Helpers for the singleton `local` provider row.
//!
//! Downloads and imports need a `provider_id` for the `models` table.
//! The local runtime is a singleton per instance (there is only one
//! mistralrs pipeline live at a time), so we auto-create the row on
//! first use rather than making the operator do it during onboarding.

use haex_crdt::rusqlite::{params, Connection, OptionalExtension, Result};
use uuid::Uuid;

use crate::storage::providers::{insert_provider, Provider, ProviderKind};

const LOCAL_PROVIDER_NAME: &str = "Local (mistral.rs)";

/// Returns the local provider's id, creating the row if missing.
/// Idempotent: safe to call from every download / import command.
pub fn ensure_local_provider(conn: &Connection) -> Result<Uuid> {
    if let Some(id) = find_local_provider(conn)? {
        return Ok(id);
    }
    let provider = Provider {
        id: Uuid::new_v4(),
        kind: ProviderKind::Local,
        adapter: None,
        name: LOCAL_PROVIDER_NAME.to_string(),
        base_url: None,
        credentials: None,
        created_at: now_ms(),
    };
    insert_provider(conn, &provider)?;
    Ok(provider.id)
}

fn find_local_provider(conn: &Connection) -> Result<Option<Uuid>> {
    let mut stmt =
        conn.prepare("SELECT id FROM providers WHERE kind = ?1 ORDER BY created_at ASC LIMIT 1")?;
    let raw: Option<String> = stmt
        .query_row(params![ProviderKind::Local.as_str()], |r| r.get(0))
        .optional()?;
    Ok(raw.and_then(|s| Uuid::parse_str(&s).ok()))
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
