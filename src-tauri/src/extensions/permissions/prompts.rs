//! Asking the user at run time (US3, T065, T066, contracts/permissions.md §Anfrage zur Laufzeit).
//!
//! A call that needs a permission answers 1004 (the vault-sdk waits for
//! `extension:permission-resolved` and repeats). holzi's window gets one
//! `extension-permission-request` per open question: identical questions of several frames are
//! merged, and a question disappears when all frames that wait for it are closed. The decision is
//! remembered in `extension_permissions`, or held in memory until the process ends (ADR-0003).

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::{Permission, PermissionKind, PermissionStatus, VAULT_WIDE};
use crate::extensions::protocol::token;

/// One open question, as holzi's window shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/types/bindings/")]
pub struct PermissionRequestEvent {
    pub request_id: String,
    pub extension_id: String,
    pub display_name: String,
    pub kind: String,
    pub action: String,
    pub target: String,
    /// The manifest declares this permission.
    pub declared: bool,
    /// Remembered for this device only unless "for all devices" is chosen.
    pub device_scoped: bool,
    /// A `database` target of an extension that is not installed: only "Verweigern" is offered.
    pub target_missing: bool,
}

/// The answer of holzi's window (`extension_permission_resolve`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/types/bindings/")]
pub enum PermissionDecision {
    Allow,
    Deny,
}

/// The identity of a question: who asks for what.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Question {
    pub extension_id: Uuid,
    pub kind: PermissionKind,
    pub action: String,
    pub target: String,
}

/// A decision held in memory, in the text form the rows use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldDecision {
    pub kind: PermissionKind,
    pub action: String,
    pub target: String,
    pub status: PermissionStatus,
}

impl HeldDecision {
    /// As a permission for decisions; one holzi cannot read is absent (FR-022).
    pub fn permission(&self) -> Option<Permission> {
        Permission::from_row(
            self.kind.as_str(),
            &self.action,
            &self.target,
            self.status.as_str(),
            VAULT_WIDE,
        )
    }
}

struct Pending {
    question: Question,
    frames: HashSet<String>,
}

/// Open questions and the decisions held in memory.
#[derive(Default)]
pub struct PermissionState {
    pending: Mutex<HashMap<String, Pending>>,
    /// Decisions without "remember", per extension; they end with the process.
    temporary: Mutex<HashMap<Uuid, Vec<HeldDecision>>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl PermissionState {
    /// Registers that `frame` waits for `question`. Returns the request id and whether the
    /// question is new (only then holzi's window is told).
    pub fn ask(&self, question: Question, frame: &str) -> (String, bool) {
        let mut pending = lock(&self.pending);
        if let Some((id, open)) = pending.iter_mut().find(|(_, p)| p.question == question) {
            open.frames.insert(frame.to_owned());
            return (id.clone(), false);
        }
        let id = token::mint();
        pending.insert(
            id.clone(),
            Pending {
                question,
                frames: HashSet::from([frame.to_owned()]),
            },
        );
        (id, true)
    }

    /// Takes an open question out to answer it.
    pub fn take(&self, request_id: &str) -> Option<Question> {
        lock(&self.pending).remove(request_id).map(|p| p.question)
    }

    /// A frame closed: questions nobody waits for any more disappear. Returns their ids.
    pub fn frame_closed(&self, frame: &str) -> Vec<String> {
        let mut pending = lock(&self.pending);
        for open in pending.values_mut() {
            open.frames.remove(frame);
        }
        let gone: Vec<String> = pending
            .iter()
            .filter(|(_, p)| p.frames.is_empty())
            .map(|(id, _)| id.clone())
            .collect();
        for id in &gone {
            pending.remove(id);
        }
        gone
    }

    /// Holds a decision for the rest of the process.
    pub fn hold(&self, question: &Question, status: PermissionStatus) {
        let mut temporary = lock(&self.temporary);
        let list = temporary.entry(question.extension_id).or_default();
        list.retain(|d| {
            !(d.kind == question.kind && d.action == question.action && d.target == question.target)
        });
        list.push(HeldDecision {
            kind: question.kind,
            action: question.action.clone(),
            target: question.target.clone(),
            status,
        });
    }

    /// The decisions held for an extension, of one kind, as permissions.
    pub fn temporary(&self, extension_id: Uuid, kind: PermissionKind) -> Vec<Permission> {
        lock(&self.temporary)
            .get(&extension_id)
            .map(|list| {
                list.iter()
                    .filter(|d| d.kind == kind)
                    .filter_map(HeldDecision::permission)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Every decision held for an extension (settings view).
    pub fn held(&self, extension_id: Uuid) -> Vec<HeldDecision> {
        lock(&self.temporary)
            .get(&extension_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Removes one held decision (settings: "Entfernen").
    pub fn forget(&self, extension_id: Uuid, kind: PermissionKind, action: &str, target: &str) {
        if let Some(list) = lock(&self.temporary).get_mut(&extension_id) {
            list.retain(|d| !(d.kind == kind && d.action == action && d.target == target));
        }
    }
}

#[cfg(test)]
#[path = "prompts_tests.rs"]
mod tests;
