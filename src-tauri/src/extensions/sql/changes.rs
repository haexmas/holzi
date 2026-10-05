//! Change notifications for extension frames (spec 017, US5, T083, FR-040, research R9).
//!
//! Every window of committed tables (`vault_events`: local writes of any writer, sync, resync)
//! reaches each extension with an open frame as `haextension:sync:tables-updated {tables}`, cut
//! down with the same [`SqlPolicy::allows`] the authorizer uses: its own tables, `_no_sync` ones
//! included, and tables of other extensions it may read; never a core table, and never the name
//! of a table it may not read. An extension with nothing left gets nothing.
//!
//! A migration changes a schema without writing rows, which the commit report does not show. Its
//! journal row does: then the tables of extensions are compared with the last snapshot of their
//! `CREATE` statements, and every table created, altered or dropped is reported as well.
//!
//! A written permission, from this device or another one, holds at once for running folder
//! watches too: those no permission allows any more end (FR-020).

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use serde_json::json;
use tauri::AppHandle;
use tokio::sync::broadcast::error::{RecvError, TryRecvError};
use tokio::sync::broadcast::Receiver;
use uuid::Uuid;

use super::authorizer::MIGRATION_JOURNAL;
use super::policy::SqlPolicy;
use crate::extensions::bridge::database::policy_for;
use crate::extensions::bridge::dispatch::Emit;
use crate::extensions::bridge::events::emit_to_frames;
use crate::extensions::commands::frames::WindowEmitter;
use crate::extensions::fs::end_revoked_watches;
use crate::extensions::host::ExtensionHost;
use crate::extensions::ids::ExtensionTable;
use crate::extensions::mail::watch::end_revoked as end_revoked_mail_watches;
use crate::extensions::permissions::store::TABLES as PERMISSION_TABLES;
use crate::state::AppState;
use crate::storage::query::Query;
use crate::storage::wm_session_commands::current_device_uuid;
use crate::vault_gate::VaultDb;

/// The SDK's event type for changed tables.
pub const TABLES_UPDATED: &str = "haextension:sync:tables-updated";

/// The `CREATE` statement of every extension table, by name.
pub type Schema = HashMap<String, Option<String>>;

/// The tables of `changed` an extension with `policy` may read, sorted.
pub fn readable(policy: &SqlPolicy, changed: &BTreeSet<String>) -> Vec<String> {
    changed
        .iter()
        .filter(|table| policy.allows(table, false))
        .cloned()
        .collect()
}

/// The tables of every extension as SQLite keeps them.
pub fn schema(q: &mut impl Query) -> haex_crdt::Result<Schema> {
    let tables: Vec<(String, Option<String>)> = q.query_map(
        "SELECT name, sql FROM sqlite_master WHERE type = 'table'",
        &[],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok(tables
        .into_iter()
        .filter(|(name, _)| ExtensionTable::parse(name).is_ok())
        .collect())
}

/// The tables created, altered or dropped between two snapshots.
pub fn schema_changes(before: &Schema, after: &Schema) -> BTreeSet<String> {
    let altered = after
        .iter()
        .filter(|(name, sql)| before.get(*name) != Some(sql))
        .map(|(name, _)| name.clone());
    let dropped = before
        .keys()
        .filter(|name| !after.contains_key(*name))
        .cloned();
    altered.chain(dropped).collect()
}

/// Sends `changed` to the open frames of every extension, each cut down to what it may read.
/// Blocking: builds each extension's policy from the vault.
pub fn notify(
    db: &VaultDb,
    host: &ExtensionHost,
    emitter: &dyn Emit,
    device: Uuid,
    changed: &BTreeSet<String>,
) {
    if changed.is_empty() {
        return;
    }
    for extension_id in host.frames.extensions() {
        let policy = match policy_for(db, host, extension_id, device) {
            Ok(policy) => policy,
            Err(error) => {
                log::warn!("extension {extension_id}: no change notification: {error:?}");
                continue;
            }
        };
        let tables = readable(&policy, changed);
        if !tables.is_empty() {
            emit_to_frames(
                emitter,
                host,
                extension_id,
                TABLES_UPDATED,
                &json!({ "tables": tables }),
            );
        }
    }
}

/// What one or more windows of committed tables say, after draining what is waiting.
struct Batch {
    tables: BTreeSet<String>,
    /// A report was skipped: anything may have changed.
    lagged: bool,
}

fn drain(
    first: Result<Arc<Vec<String>>, RecvError>,
    changes: &mut Receiver<Arc<Vec<String>>>,
) -> Option<Batch> {
    let mut batch = Batch {
        tables: BTreeSet::new(),
        lagged: false,
    };
    let mut next = match first {
        Ok(tables) => Ok(tables),
        Err(RecvError::Lagged(_)) => Err(TryRecvError::Lagged(0)),
        Err(RecvError::Closed) => return None,
    };
    loop {
        match next {
            Ok(tables) => batch.tables.extend(tables.iter().cloned()),
            Err(TryRecvError::Lagged(_)) => batch.lagged = true,
            Err(TryRecvError::Empty) => return Some(batch),
            Err(TryRecvError::Closed) => return None,
        }
        next = changes.try_recv();
    }
}

/// Handles one batch: ends the watches a changed permission no longer allows, adds the schema
/// changes a migration made, then notifies. Blocking.
fn handle(
    db: &VaultDb,
    host: &ExtensionHost,
    emitter: &dyn Emit,
    device: Uuid,
    known: &mut Schema,
    mut batch: Batch,
) {
    if batch.lagged
        || PERMISSION_TABLES
            .iter()
            .any(|table| batch.tables.contains(*table))
    {
        end_revoked_watches(db, host, device);
        end_revoked_mail_watches(db, host, device);
    }
    if batch.lagged || batch.tables.contains(MIGRATION_JOURNAL) {
        match db.read_blocking(|q| schema(q)) {
            Ok(now) => {
                batch.tables.extend(schema_changes(known, &now));
                if batch.lagged {
                    batch.tables.extend(now.keys().cloned());
                }
                *known = now;
            }
            Err(error) => log::warn!("extensions: reading the schema failed: {error}"),
        }
    }
    // No extension may read a core table: a write to one alone (chat, wm, the store or log of an
    // extension) builds no policy at all.
    batch
        .tables
        .retain(|table| ExtensionTable::parse(table).is_ok());
    notify(db, host, emitter, device, &batch.tables);
}

/// Follows the committed tables of the vault session `state` just published, until it ends.
pub fn start_for_active_instance(app: &AppHandle, state: &AppState) {
    let db = match state.database() {
        Ok(db) => db,
        Err(error) => {
            log::warn!("extensions: no open vault, no change notifications: {error}");
            return;
        }
    };
    let device = match current_device_uuid(app, &db) {
        Ok(device) => device,
        Err(error) => {
            log::warn!("extensions: this device is unknown, no change notifications: {error}");
            return;
        }
    };
    let mut changes = state.vault_changes().subscribe();
    let host = state.extensions();
    let gate = state.gate().clone();
    let token = gate.token();
    let emitter: Arc<dyn Emit> = Arc::new(WindowEmitter(app.clone()));
    let follow = async move {
        let start = {
            let db = db.clone();
            gate.spawn_blocking(move || db.read_blocking(|q| schema(q)).unwrap_or_default())
        };
        let mut known = match start {
            Ok(handle) => handle.await.unwrap_or_default(),
            Err(_) => return,
        };
        loop {
            let first = tokio::select! {
                () = token.cancelled() => return,
                received = changes.recv() => received,
            };
            let Some(batch) = drain(first, &mut changes) else {
                return;
            };
            let (db, host, emitter) = (db.clone(), host.clone(), emitter.clone());
            let run = gate.spawn_blocking(move || {
                let mut known = known;
                handle(&db, &host, emitter.as_ref(), device, &mut known, batch);
                known
            });
            known = match run {
                Ok(handle) => match handle.await {
                    Ok(known) => known,
                    Err(error) => {
                        log::warn!("extensions: change notifications stopped: {error}");
                        return;
                    }
                },
                Err(_) => return,
            };
        }
    };
    if let Err(error) = state.gate().spawn(follow) {
        log::warn!("extensions: change notifications did not start: {error}");
    }
}

#[cfg(test)]
#[path = "changes_tests.rs"]
mod tests;
