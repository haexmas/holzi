//! Watching folders for an extension (spec 017, US9, FR-046, research R19 step 6): one watch per
//! (extension, `ruleId`), so an extension can only end or ask about its own. Every path of a
//! debounced batch reaches only the frames of that extension as `filesync:file-changed` with
//! `ruleId`, `changeType` and the path relative to the watched folder (flat, as the SDK reads it).
//! A watch ends with `unwatch` or when the last frame of its extension closes. Android and iOS
//! offer no watching (FR-066).

// Without a watcher (Android, iOS) only the "not available" answers remain.
#![cfg_attr(
    any(target_os = "android", target_os = "ios"),
    allow(unused_imports, dead_code)
)]

use serde_json::{json, Value};

use super::{authorize, Access, Reach};
use crate::extensions::bridge::dispatch::CallContext;
use crate::extensions::error::{BridgeError, ExtensionErrorCode};

pub const MODULE: &str = module_path!();

/// The event type of a changed path.
pub const FILE_CHANGED: &str = "filesync:file-changed";

/// Longest `ruleId` an extension may name.
const MAX_RULE_ID: usize = 256;

fn rule_id(params: &Value) -> Result<String, BridgeError> {
    params
        .get("ruleId")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty() && id.len() <= MAX_RULE_ID)
        .map(str::to_owned)
        .ok_or_else(|| BridgeError::new(ExtensionErrorCode::Validation, "ruleId is missing"))
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod desktop {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
    use std::time::Duration;

    use notify::{EventKind, RecommendedWatcher, RecursiveMode};
    use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};
    use serde_json::json;
    use uuid::Uuid;

    use super::FILE_CHANGED;
    use crate::extensions::bridge::dispatch::Emit;
    use crate::extensions::bridge::events::emit_to_frames;
    use crate::extensions::host::ExtensionHost;

    /// Changes within this time come as one batch.
    const DEBOUNCE: Duration = Duration::from_millis(500);

    type Watcher = Debouncer<RecommendedWatcher, RecommendedCache>;

    /// The running watches, by (extension, `ruleId`).
    #[derive(Default)]
    pub struct Watches(Mutex<HashMap<(Uuid, String), Watcher>>);

    impl Watches {
        fn map(&self) -> MutexGuard<'_, HashMap<(Uuid, String), Watcher>> {
            self.0.lock().unwrap_or_else(PoisonError::into_inner)
        }

        pub fn start(
            &self,
            host: Weak<ExtensionHost>,
            emitter: Arc<dyn Emit>,
            extension_id: Uuid,
            rule_id: String,
            root: PathBuf,
        ) -> Result<(), String> {
            let rule = rule_id.clone();
            let base = root.clone();
            let mut watcher = new_debouncer(DEBOUNCE, None, move |result: DebounceEventResult| {
                let Ok(events) = result else {
                    return;
                };
                let Some(host) = host.upgrade() else {
                    return;
                };
                for event in events {
                    let change = match event.kind {
                        EventKind::Create(_) => "created",
                        EventKind::Modify(_) => "modified",
                        EventKind::Remove(_) => "removed",
                        _ => "any",
                    };
                    for path in &event.paths {
                        emit_to_frames(
                            emitter.as_ref(),
                            &host,
                            extension_id,
                            FILE_CHANGED,
                            &json!({
                                "ruleId": rule,
                                "changeType": change,
                                "path": relative(&base, path),
                            }),
                        );
                    }
                }
            })
            .map_err(|e| e.to_string())?;
            watcher
                .watch(&root, RecursiveMode::Recursive)
                .map_err(|e| e.to_string())?;
            self.map().insert((extension_id, rule_id), watcher);
            Ok(())
        }

        pub fn stop(&self, extension_id: Uuid, rule_id: &str) -> bool {
            self.map()
                .remove(&(extension_id, rule_id.to_owned()))
                .is_some()
        }

        pub fn running(&self, extension_id: Uuid, rule_id: &str) -> bool {
            self.map().contains_key(&(extension_id, rule_id.to_owned()))
        }

        pub fn end_all(&self, extension_id: Uuid) {
            self.map().retain(|(owner, _), _| *owner != extension_id);
        }
    }

    /// `path` below `root` as the SDK reports it, else as it is.
    fn relative(root: &Path, path: &Path) -> String {
        path.strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned()
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub use desktop::Watches;

/// No watching on this platform.
#[cfg(any(target_os = "android", target_os = "ios"))]
#[derive(Default)]
pub struct Watches;

#[cfg(any(target_os = "android", target_os = "ios"))]
impl Watches {
    pub fn end_all(&self, _extension_id: uuid::Uuid) {}
}

/// `{ruleId, path}`: watches the folder and everything below it; a watch with the same `ruleId`
/// is replaced.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn watch(ctx: &CallContext, params: &Value) -> Result<Value, BridgeError> {
    let rule = rule_id(params)?;
    let raw = params
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| BridgeError::new(ExtensionErrorCode::Validation, "path is missing"))?;
    let root = authorize(ctx, raw, Access::Read, Reach::Tree)?;
    ctx.host
        .fs
        .watches
        .start(
            std::sync::Arc::downgrade(&ctx.host),
            std::sync::Arc::clone(&ctx.emitter),
            ctx.session.extension_id,
            rule,
            root,
        )
        .map_err(|e| {
            BridgeError::new(ExtensionErrorCode::Filesystem, format!("watch failed: {e}"))
        })?;
    Ok(Value::Null)
}

/// `{ruleId}`: ends the caller's own watch.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn unwatch(ctx: &CallContext, params: &Value) -> Result<Value, BridgeError> {
    let rule = rule_id(params)?;
    ctx.host.fs.watches.stop(ctx.session.extension_id, &rule);
    Ok(Value::Null)
}

/// `{ruleId}` → whether the caller watches with it.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn is_watching(ctx: &CallContext, params: &Value) -> Result<Value, BridgeError> {
    let rule = rule_id(params)?;
    Ok(json!(ctx
        .host
        .fs
        .watches
        .running(ctx.session.extension_id, &rule)))
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn watch(_ctx: &CallContext, _params: &Value) -> Result<Value, BridgeError> {
    Err(BridgeError::not_available())
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn unwatch(_ctx: &CallContext, _params: &Value) -> Result<Value, BridgeError> {
    Err(BridgeError::not_available())
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn is_watching(_ctx: &CallContext, _params: &Value) -> Result<Value, BridgeError> {
    Err(BridgeError::not_available())
}
