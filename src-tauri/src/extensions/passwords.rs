//! The password functions of extensions (spec 017, US10, T105, FR-053, research R21): a thin
//! adapter onto the password manager's service (spec 034, `contracts/access.md`). The extension is
//! the caller (`Caller::Extension`), its `passwords` permissions are its grants, and every rule
//! Z1–Z13 is the service's, not this module's.
//!
//! A refusal asks the user only for a permission the extension holds in the state "ask" (declared
//! in its manifest and not ticked at install, or remembered so); without one it is 1002 (Z3).

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::HolziError;
use crate::extensions::bridge::blocking::block_on;
use crate::extensions::bridge::dispatch::CallContext;
use crate::extensions::error::{BridgeError, ExtensionErrorCode};
use crate::extensions::permissions::store::candidates;
use crate::extensions::permissions::{
    Action, Permission, PermissionKind, PermissionStatus, Target,
};
use crate::passwords::access::{Caller, Grant, GrantAction, Scope};
use crate::passwords::model::{
    ItemHeader, ItemInput, ItemPatch, KeyValueInput, KeyValuePatch, Patch, SecretItem,
};
use crate::passwords::service::{Headers, PasswordsService};

pub const MODULE: &str = module_path!();

fn invalid(message: &str) -> BridgeError {
    BridgeError::new(ExtensionErrorCode::Validation, message)
}

fn unavailable() -> BridgeError {
    BridgeError::new(ExtensionErrorCode::Database, "passwords unavailable")
}

/// The stored text of a `passwords` target.
fn target_text(target: &Target) -> Option<String> {
    match target {
        Target::Any => Some("*".to_owned()),
        Target::Tag(tag) => Some(tag.clone()),
        _ => None,
    }
}

/// The calling extension's `passwords` permissions on this device: remembered and held in memory.
fn permissions(ctx: &CallContext) -> Result<Vec<Permission>, BridgeError> {
    let (extension_id, device) = (ctx.session.extension_id, ctx.device);
    let mut permissions = ctx
        .db
        .read_blocking(move |q| {
            candidates(q, extension_id, PermissionKind::Passwords, device).map_err(Into::into)
        })
        .map_err(|_| unavailable())?;
    permissions.extend(
        ctx.host
            .permissions
            .temporary(extension_id, PermissionKind::Passwords),
    );
    Ok(permissions)
}

/// The grants of the service: every granted permission. A denied `*` takes them all away; a
/// denied tag removes nothing the service could subtract, it only keeps holzi from asking again.
pub fn grants_of(permissions: &[Permission]) -> Vec<Grant> {
    let denied_all = permissions
        .iter()
        .any(|p| p.status == PermissionStatus::Denied && p.target == Target::Any);
    if denied_all {
        return Vec::new();
    }
    permissions
        .iter()
        .filter(|p| p.status == PermissionStatus::Granted)
        .filter_map(|p| {
            let action = match p.action {
                Action::Read => GrantAction::Read,
                Action::ReadWrite => GrantAction::ReadWrite,
                _ => return None,
            };
            let scope = match &p.target {
                Target::Any => Scope::All,
                Target::Tag(tag) => Scope::tags([tag]),
                _ => return None,
            };
            Some(Grant::new(action, scope))
        })
        .collect()
}

/// The answer to a refusal of the service: a question for a permission in the state "ask" that
/// would cover the call (one of `tags` first, for a write that names them), else 1002.
fn refused(permissions: &[Permission], write: bool, tags: &[String]) -> BridgeError {
    let needed = if write {
        Action::ReadWrite
    } else {
        Action::Read
    };
    let asking: Vec<&Permission> = permissions
        .iter()
        .filter(|p| p.status == PermissionStatus::Ask && p.action.covers(&needed))
        .collect();
    let names = |p: &Permission| match &p.target {
        Target::Any => true,
        Target::Tag(tag) => tags.iter().any(|t| crate::passwords::ids::fold(t) == *tag),
        _ => false,
    };
    let chosen = asking
        .iter()
        .copied()
        .find(|p| names(p))
        .or_else(|| asking.first().copied().filter(|_| tags.is_empty()));
    match chosen.and_then(|p| target_text(&p.target)) {
        Some(target) => BridgeError::new(
            ExtensionErrorCode::PermissionPromptRequired,
            "permission required",
        )
        .with_details(json!({
            "resourceType": "passwords",
            "action": if write { "readWrite" } else { "read" },
            "target": target,
        })),
        None => BridgeError::new(ExtensionErrorCode::PermissionDenied, "permission denied"),
    }
}

/// Maps a failure of the service; no message carries a value of the entry (Z10).
fn map_error(
    error: HolziError,
    permissions: &[Permission],
    write: bool,
    tags: &[String],
) -> BridgeError {
    match error {
        HolziError::PasswordsForbidden => refused(permissions, write, tags),
        HolziError::PasswordsNotFound => {
            BridgeError::new(ExtensionErrorCode::NotFound, "not found")
        }
        HolziError::InvalidInput { reason }
        | HolziError::PasswordsConflict { reason }
        | HolziError::PasswordsReference { reason } => {
            BridgeError::new(ExtensionErrorCode::Validation, reason)
        }
        HolziError::PasswordsReferenceCycle { .. } => invalid("reference cycle"),
        _ => unavailable(),
    }
}

/// The SDK's `PasswordInput`.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct SdkInput {
    title: Option<String>,
    username: Option<String>,
    password: Option<String>,
    note: Option<String>,
    icon: Option<String>,
    color: Option<String>,
    url: Option<String>,
    otp_secret: Option<String>,
    otp_digits: Option<u32>,
    otp_period: Option<u32>,
    otp_algorithm: Option<String>,
    autofill_aliases: Option<HashMap<String, Vec<String>>>,
    expires_at: Option<String>,
    tags: Vec<String>,
    key_values: Option<Vec<SdkKeyValue>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct SdkKeyValue {
    key: Option<String>,
    value: Option<String>,
}

fn input_of(params: &Value) -> Result<SdkInput, BridgeError> {
    let input = params
        .get("input")
        .filter(|i| i.is_object())
        .ok_or_else(|| invalid("input must be an object"))?;
    serde_json::from_value(input.clone()).map_err(|_| invalid("input not understood"))
}

fn item_id(params: &Value) -> Result<String, BridgeError> {
    params
        .get("itemId")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| invalid("itemId must be a string"))
}

fn aliases_text(aliases: Option<HashMap<String, Vec<String>>>) -> Option<String> {
    aliases.and_then(|a| serde_json::to_string(&a).ok())
}

impl SdkInput {
    fn into_create(self) -> ItemInput {
        ItemInput {
            title: self.title,
            username: self.username,
            password: self.password,
            note: self.note,
            url: self.url,
            icon: self.icon,
            color: self.color,
            expires_at: self.expires_at,
            otp_secret: self.otp_secret,
            otp_digits: self.otp_digits,
            otp_period: self.otp_period,
            otp_algorithm: self.otp_algorithm,
            autofill_aliases: aliases_text(self.autofill_aliases),
            tags: self.tags,
            key_values: self
                .key_values
                .unwrap_or_default()
                .into_iter()
                .map(|kv| KeyValueInput {
                    key: kv.key.unwrap_or_default(),
                    value: kv.value,
                })
                .collect(),
        }
    }

    /// The SDK sends the whole entry: every field it leaves out is cleared, the tags and the
    /// custom fields are replaced.
    fn into_patch(self) -> ItemPatch {
        fn set<T>(value: Option<T>) -> Patch<T> {
            value.map_or(Patch::Clear, Patch::Set)
        }
        ItemPatch {
            title: set(self.title),
            username: set(self.username),
            password: set(self.password),
            note: set(self.note),
            url: set(self.url),
            icon: set(self.icon),
            color: set(self.color),
            expires_at: set(self.expires_at),
            otp_secret: set(self.otp_secret),
            otp_digits: set(self.otp_digits),
            otp_period: set(self.otp_period),
            otp_algorithm: set(self.otp_algorithm),
            autofill_aliases: set(aliases_text(self.autofill_aliases)),
            tags: Some(self.tags),
            key_values: Some(
                self.key_values
                    .unwrap_or_default()
                    .into_iter()
                    .map(|kv| KeyValuePatch {
                        id: None,
                        key: kv.key.unwrap_or_default(),
                        value: kv.value,
                    })
                    .collect(),
            ),
        }
    }
}

/// The SDK's `PasswordItemSummary`.
fn summary(header: ItemHeader) -> Value {
    json!({
        "id": header.id,
        "title": header.title,
        "username": header.username,
        "url": header.url,
        "icon": header.icon,
        "color": header.color,
        "tags": header.tags.into_iter().map(|t| t.name).collect::<Vec<_>>(),
        "createdAt": header.created_at,
        "updatedAt": header.updated_at,
    })
}

/// The SDK's `PasswordItemFull`.
fn full(item: SecretItem) -> Value {
    let aliases = item
        .autofill_aliases
        .as_deref()
        .and_then(|text| serde_json::from_str::<HashMap<String, Vec<String>>>(text).ok());
    json!({
        "id": item.id,
        "title": item.title,
        "username": item.username,
        "password": item.password,
        "note": item.note,
        "icon": item.icon,
        "color": item.color,
        "url": item.url,
        "otpSecret": item.otp_secret,
        "otpDigits": item.otp_digits,
        "otpPeriod": item.otp_period,
        "otpAlgorithm": item.otp_algorithm,
        "autofillAliases": aliases,
        "tags": item.tags,
        "keyValues": item.key_values.into_iter().map(|kv| json!({
            "id": kv.id,
            "key": kv.key,
            "value": kv.value,
        })).collect::<Vec<_>>(),
        "expiresAt": item.expires_at,
        "createdAt": item.created_at,
        "updatedAt": item.updated_at,
    })
}

/// What every call needs: the service, the caller and its permissions.
struct Call {
    service: PasswordsService,
    caller: Caller,
    permissions: Vec<Permission>,
    grants: Vec<Grant>,
}

fn call(ctx: &CallContext) -> Result<Call, BridgeError> {
    let permissions = permissions(ctx)?;
    Ok(Call {
        service: PasswordsService::new(ctx.db.clone()),
        caller: Caller::Extension {
            id: ctx.session.extension_id.to_string(),
        },
        grants: grants_of(&permissions),
        permissions,
    })
}

/// `extension_password_list`: the entries in scope, without secrets (Z4).
pub fn list(ctx: &CallContext, _params: &Value) -> Result<Value, BridgeError> {
    let c = call(ctx)?;
    let headers = block_on(c.service.list_headers(&c.caller, &c.grants))
        .map_err(|e| map_error(e, &c.permissions, false, &[]))?;
    match headers {
        Headers::Items(items) => Ok(Value::Array(items.into_iter().map(summary).collect())),
        Headers::Agent(_) => Err(unavailable()),
    }
}

/// `extension_password_read {itemId}`: one entry with its secrets (Z5, Z13).
pub fn read(ctx: &CallContext, params: &Value) -> Result<Value, BridgeError> {
    let id = item_id(params)?;
    let c = call(ctx)?;
    block_on(c.service.read_secret_item(&c.caller, &c.grants, id))
        .map(full)
        .map_err(|e| map_error(e, &c.permissions, false, &[]))
}

/// `extension_password_create {input}` → the new id (Z6).
pub fn create(ctx: &CallContext, params: &Value) -> Result<Value, BridgeError> {
    let input = input_of(params)?.into_create();
    let tags = input.tags.clone();
    let c = call(ctx)?;
    block_on(c.service.create_item(&c.caller, &c.grants, input, None))
        .map(Value::String)
        .map_err(|e| map_error(e, &c.permissions, true, &tags))
}

/// `extension_password_update {itemId, input}`: the SDK has no version token, so the current one
/// is read first, through the same checks (Z7, Z12).
pub fn update(ctx: &CallContext, params: &Value) -> Result<Value, BridgeError> {
    let id = item_id(params)?;
    let input = input_of(params)?;
    let tags = input.tags.clone();
    let c = call(ctx)?;
    let fail = |e| map_error(e, &c.permissions, true, &tags);
    let current =
        block_on(c.service.read_secret_item(&c.caller, &c.grants, id.clone())).map_err(fail)?;
    let token = current.updated_at.unwrap_or_default();
    block_on(
        c.service
            .update_item(&c.caller, &c.grants, id, token, input.into_patch()),
    )
    .map(|_| Value::Null)
    .map_err(fail)
}

/// `extension_password_delete {itemId}`: into the trash (Z8).
pub fn delete(ctx: &CallContext, params: &Value) -> Result<Value, BridgeError> {
    let id = item_id(params)?;
    let c = call(ctx)?;
    block_on(c.service.delete_item(&c.caller, &c.grants, id))
        .map(|()| Value::Null)
        .map_err(|e| map_error(e, &c.permissions, true, &[]))
}

#[cfg(test)]
#[path = "passwords_tests.rs"]
mod tests;
