//! Helpers for the singleton `local` provider rows — one per capability
//! (spec 008: chat and transcription are now two distinct `kind = local`
//! rows, disambiguated by `capability`).
//!
//! Downloads and imports need a `provider_id` for the `models` table.
//! The local runtime is a singleton per instance (there is only one
//! mistralrs pipeline live at a time), so we auto-create the row on
//! first use rather than making the operator do it during onboarding.

use haex_crdt::rusqlite::{Connection, Result};
use uuid::Uuid;

use crate::storage::providers::{
    find_provider_by_kind_and_capability, insert_provider, Provider, ProviderCapability,
    ProviderKind,
};

const LOCAL_PROVIDER_NAME: &str = "Local (mistral.rs)";
const LOCAL_TRANSCRIPTION_PROVIDER_NAME: &str = "Gebündelt (offline)";
const LOCAL_TRANSCRIPTION_ADAPTER: &str = "whisper-local";

/// Returns the local chat provider's id, creating the row if missing.
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
        capability: ProviderCapability::Chat,
    };
    insert_provider(conn, &provider)?;
    Ok(provider.id)
}

fn find_local_provider(conn: &Connection) -> Result<Option<Uuid>> {
    find_provider_by_kind_and_capability(conn, ProviderKind::Local, ProviderCapability::Chat)
}

/// Returns the bundled local transcription provider's id, creating the row
/// if missing (spec 008 data-model.md §"Neue Runtime-/Storage-Entität").
/// Idempotent, singleton, mirrors [`ensure_local_provider`] — the only
/// differences are the capability and the `"whisper-local"` adapter
/// discriminator that [`super::super::stt::local::LocalWhisperAdapter`]
/// dispatch matches on.
pub fn ensure_local_transcription_provider(conn: &Connection) -> Result<Uuid> {
    if let Some(id) = find_local_transcription_provider(conn)? {
        return Ok(id);
    }
    let provider = Provider {
        id: Uuid::new_v4(),
        kind: ProviderKind::Local,
        adapter: Some(LOCAL_TRANSCRIPTION_ADAPTER.to_string()),
        name: LOCAL_TRANSCRIPTION_PROVIDER_NAME.to_string(),
        base_url: None,
        credentials: None,
        created_at: now_ms(),
        capability: ProviderCapability::Transcription,
    };
    insert_provider(conn, &provider)?;
    Ok(provider.id)
}

fn find_local_transcription_provider(conn: &Connection) -> Result<Option<Uuid>> {
    find_provider_by_kind_and_capability(
        conn,
        ProviderKind::Local,
        ProviderCapability::Transcription,
    )
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
