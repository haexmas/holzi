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
//!
//! Every failure is logged and never stops the vault from opening (FR-012);
//! a failed task stays queued for the next open.

use haex_crdt::rusqlite::{params, Connection};
use haex_crdt::Database;

/// Maintenance task queued by migration `0020_wm_session_no_sync`.
pub const VACUUM_TASK: &str = "vacuum_after_legacy_wm_drop";

/// Device preference that stored the active workspace in spec 015.
pub const LEGACY_ACTIVE_WORKSPACE_KEY: &str = "shell.active_workspace_id";

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
