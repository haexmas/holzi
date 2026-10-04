//! The one checkpoint for calls from extension frames (ADR-0004, research R14, FR-060): the frame
//! session says who calls, the allowlist [`METHODS`] says what can be called. A method that is not
//! listed answers "not supported" (8000), one of a later delivery "not available" (8001), every
//! call of a disabled extension "disabled" (8002).

use std::sync::Arc;

use serde_json::{json, Value};
use uuid::Uuid;

use super::frames::FrameSession;
use super::{database, methods, permissions};
use crate::extensions::error::{BridgeError, ExtensionErrorCode};
use crate::extensions::host::ExtensionHost;
use crate::extensions::{kv, logs, web};
use crate::storage::query::Query;
use crate::vault_gate::VaultDb;

/// Sends an event to holzi's own window.
pub trait Emit: Send + Sync {
    fn emit(&self, event: &str, payload: Value);
}

/// Everything a bridge method may use: the session it was called from, never anything the
/// extension says about itself.
pub struct CallContext {
    pub db: VaultDb,
    pub host: Arc<ExtensionHost>,
    pub session: Arc<FrameSession>,
    /// This device's `vault_device_uuid`.
    pub device: Uuid,
    pub emitter: Arc<dyn Emit>,
}

pub type Handler = fn(&CallContext, &Value) -> Result<Value, BridgeError>;

/// One method of the allowlist; `module` is where its handler lives (contract test, FR-009).
pub struct Method {
    pub name: &'static str,
    pub handler: Handler,
    pub module: &'static str,
}

/// The allowlist (contracts/bridge.md §Methoden).
pub static METHODS: &[Method] = &[
    Method {
        name: "extension_context_get",
        handler: methods::context_get,
        module: methods::MODULE,
    },
    Method {
        name: "extension_get_info",
        handler: methods::get_info,
        module: methods::MODULE,
    },
    Method {
        name: "extension_tab_attention",
        handler: methods::tab_attention,
        module: methods::MODULE,
    },
    Method {
        name: "extension_dialog_confirm",
        handler: methods::dialog_confirm,
        module: methods::MODULE,
    },
    Method {
        name: "extension_database_query",
        handler: database::query,
        module: database::MODULE,
    },
    Method {
        name: "extension_database_execute",
        handler: database::execute,
        module: database::MODULE,
    },
    Method {
        name: "extension_database_transaction",
        handler: database::transaction,
        module: database::MODULE,
    },
    Method {
        name: "extension_permissions_check_database",
        handler: permissions::check_database,
        module: permissions::MODULE,
    },
    Method {
        name: "extension_database_register_migrations",
        handler: database::register_migrations,
        module: database::MODULE,
    },
    Method {
        name: "extension_web_storage_get_item",
        handler: kv::get_item,
        module: kv::MODULE,
    },
    Method {
        name: "extension_web_storage_set_item",
        handler: kv::set_item,
        module: kv::MODULE,
    },
    Method {
        name: "extension_web_storage_remove_item",
        handler: kv::remove_item,
        module: kv::MODULE,
    },
    Method {
        name: "extension_web_storage_clear",
        handler: kv::clear,
        module: kv::MODULE,
    },
    Method {
        name: "extension_web_storage_keys",
        handler: kv::keys,
        module: kv::MODULE,
    },
    Method {
        name: "extension_logging_write",
        handler: logs::write,
        module: logs::MODULE,
    },
    Method {
        name: "extension_logging_read",
        handler: logs::read_own,
        module: logs::MODULE,
    },
    Method {
        name: "extension_web_fetch",
        handler: web::fetch,
        module: web::MODULE,
    },
    Method {
        name: "extension_web_open",
        handler: web::open,
        module: web::MODULE,
    },
    Method {
        name: "extension_permissions_check_web",
        handler: web::check_web,
        module: web::MODULE,
    },
];

/// Methods of later deliveries (research R1): they answer 8001 until they land.
const LATER: &[&str] = &[
    "extension_permissions_check_filesystem",
    "extension_notifications_",
    "extension_filesystem_",
    "extension_password_",
    "extension_remote_storage_",
    "extension_mail_",
    "extension_shell_",
];

fn is_enabled(ctx: &CallContext) -> Result<bool, BridgeError> {
    let id = ctx.session.extension_id.to_string();
    ctx.db
        .read_blocking(move |q| {
            q.query_row(
                "SELECT enabled FROM extensions WHERE id = ?1 AND state = 'installed'",
                &[&id],
                |r| r.get::<_, i64>(0),
            )
        })
        .map(|enabled| enabled == Some(1))
        .map_err(|_| BridgeError::new(ExtensionErrorCode::Database, "database unavailable"))
}

/// Runs one call. Blocking (methods read the vault). A call that needs a permission (1004) puts
/// its question before the user (`permissions::ask`).
pub fn call(ctx: &CallContext, method: &str, params: &Value) -> Result<Value, BridgeError> {
    if !is_enabled(ctx)? {
        return Err(BridgeError::disabled());
    }
    if let Some(found) = METHODS.iter().find(|m| m.name == method) {
        return (found.handler)(ctx, params).inspect_err(|error| {
            if error.code == ExtensionErrorCode::PermissionPromptRequired {
                super::permissions::ask(ctx, error);
            }
        });
    }
    if LATER.iter().any(|prefix| method.starts_with(prefix)) {
        return Err(BridgeError::not_available());
    }
    Err(BridgeError::not_supported())
}

/// The answer in the form the SDK reads: `{id, result}` or `{id, error}`.
pub fn answer(id: Value, outcome: Result<Value, BridgeError>) -> Value {
    match outcome {
        Ok(result) => json!({ "id": id, "result": result }),
        Err(error) => json!({ "id": id, "error": error }),
    }
}

#[cfg(test)]
#[path = "dispatch_tests.rs"]
mod tests;
