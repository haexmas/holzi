//! Steps that run once a vault is open, before the frontend sees it (spec
//! 022-session-restore, research R5/R6, contracts/wm-session.md §2).
//!
//! 1. `PRAGMA secure_delete = ON`, so deleted rows (a saved session turned
//!    off) are overwritten instead of lingering in free pages. haex-crdt
//!    keeps one connection per database, so setting it once is enough.
//! 2. Delete the spec 015 preference `shell.active_workspace_id` for every
//!    device. It is a synced table, so each delete leaves a delete marker
//!    with the device id and the key, no content (FR-011). Idempotent.
//! 3. Run queued tasks from `holzi_maintenance_no_sync`: after migration
//!    0020 dropped the spec 015 tables, one `VACUUM` rewrites the file so
//!    their content does not survive in free pages, then the WAL is
//!    truncated.
//! 4. Fold preferences into their scope (spec 023-settings-app, FR-024,
//!    research R14): settings apply to the vault, so this device's value of
//!    a vault setting becomes the vault value when the vault has none; the
//!    default model applies to the device, so a vault value becomes this
//!    device's when it has none. The old value is deleted either way.
//!    Idempotent; other devices fold their own values when they open.
//!
//! Every failure is logged and never stops the vault from opening (FR-012);
//! a failed task stays queued for the next open.

use haex_crdt::rusqlite::{params, Connection};
use haex_crdt::Database;
use uuid::Uuid;

use super::preferences::{self, PrefScope};

/// Maintenance task queued by migration `0020_wm_session_no_sync`.
pub const VACUUM_TASK: &str = "vacuum_after_legacy_wm_drop";

/// Device preference that stored the active workspace in spec 015.
pub const LEGACY_ACTIVE_WORKSPACE_KEY: &str = "shell.active_workspace_id";

/// Preferences that apply to the whole vault since spec 023 (FR-024).
pub const VAULT_PREFERENCE_KEYS: &[&str] = &[
    "appearance.color_scheme",
    "wm.session_restore",
    "chat.autonomy_mode",
    "chat.permission_mode",
    "cli_delegate.deny_rules",
];

/// Key prefixes of vault preferences with one key per model.
pub const VAULT_PREFERENCE_PREFIXES: &[&str] = &["chat.reasoning_option."];

/// Preferences that apply to one device only: the model files live there.
pub const DEVICE_PREFERENCE_KEYS: &[&str] = &["chat.default_model_id"];

fn is_vault_preference(key: &str) -> bool {
    VAULT_PREFERENCE_KEYS.contains(&key)
        || VAULT_PREFERENCE_PREFIXES
            .iter()
            .any(|prefix| key.starts_with(prefix))
}

/// Step 4 above, in one transaction.
pub fn fold_scoped_preferences(conn: &Connection, device: Uuid) -> haex_crdt::Result<()> {
    let tx = conn.unchecked_transaction()?;
    for row in preferences::list_by_scope(&tx, PrefScope::Device(device))? {
        if !is_vault_preference(&row.key) {
            continue;
        }
        if let Some(value) = row.value.as_deref() {
            if preferences::get(&tx, PrefScope::Vault, &row.key)?.is_none() {
                preferences::insert_or_update(&tx, PrefScope::Vault, &row.key, value)?;
            }
        }
        preferences::delete(&tx, PrefScope::Device(device), &row.key)?;
    }
    for key in DEVICE_PREFERENCE_KEYS {
        let Some(value) = preferences::get(&tx, PrefScope::Vault, key)? else {
            continue;
        };
        if preferences::get(&tx, PrefScope::Device(device), key)?.is_none() {
            preferences::insert_or_update(&tx, PrefScope::Device(device), key, &value)?;
        }
        preferences::delete(&tx, PrefScope::Vault, key)?;
    }
    tx.commit()?;
    Ok(())
}

/// Runs the steps above; logs failures and always returns.
pub fn run_after_open(db: &Database) {
    if let Err(e) = db.with_connection(|conn| Ok(conn.execute_batch("PRAGMA secure_delete = ON;")?))
    {
        log::warn!("maintenance: could not turn on secure_delete: {e}");
    }
    if let Err(e) = db.with_connection(|conn| {
        Ok(conn.execute(
            "DELETE FROM preferences WHERE key = ?1",
            params![LEGACY_ACTIVE_WORKSPACE_KEY],
        )?)
    }) {
        log::warn!("maintenance: could not delete {LEGACY_ACTIVE_WORKSPACE_KEY}: {e}");
    }
    match db.with_connection(|conn| Ok(run_pending_tasks(conn))) {
        Ok(Ok(())) => {}
        Ok(Err(e)) | Err(e) => {
            log::warn!("maintenance: a queued task failed, retrying on the next open: {e}")
        }
    }
    let device = db.device_id();
    if let Err(e) = db.with_connection(|conn| fold_scoped_preferences(conn, device)) {
        log::warn!("maintenance: could not fold preferences into their scope: {e}");
    }
}

/// Runs every queued task and removes it once it succeeded.
pub fn run_pending_tasks(conn: &Connection) -> haex_crdt::Result<()> {
    let queued: i64 = conn.query_row(
        "SELECT COUNT(*) FROM holzi_maintenance_no_sync WHERE task = ?1",
        params![VACUUM_TASK],
        |r| r.get(0),
    )?;
    if queued > 0 {
        conn.execute_batch("VACUUM;")?;
        conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
        conn.execute(
            "DELETE FROM holzi_maintenance_no_sync WHERE task = ?1",
            params![VACUUM_TASK],
        )?;
    }
    Ok(())
}
