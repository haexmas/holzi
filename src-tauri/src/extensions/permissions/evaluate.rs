//! The decision for one request (contracts/permissions.md §Auswertung).

use uuid::Uuid;

use super::model::{Permission, PermissionRequest, PermissionStatus};

/// The outcome of [`evaluate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Allow,
    /// The vault-sdk's `DENIED` (1002).
    Deny,
    /// Ask the user (`PROMPT_REQUIRED`, 1004).
    Prompt,
}

/// Decides `request` on `device` from the remembered and temporary permissions of the extension.
///
/// Only permissions that hold on `device` (vault-wide or this device) and apply to the request
/// count. Among them a denial wins over a grant, and a grant over "ask"; without any, the user is
/// asked. The fixed rules that no permission changes (core tables, blocked paths, missing
/// functions, a disabled extension) are checked before this, by the caller.
pub fn evaluate(candidates: &[Permission], request: &PermissionRequest, device: Uuid) -> Decision {
    let mut granted = false;
    for permission in candidates
        .iter()
        .filter(|p| p.scope.holds_on(device) && p.applies_to(request))
    {
        match permission.status {
            PermissionStatus::Denied => return Decision::Deny,
            PermissionStatus::Granted => granted = true,
            PermissionStatus::Ask => {}
        }
    }
    if granted {
        Decision::Allow
    } else {
        Decision::Prompt
    }
}
