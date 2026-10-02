//! Frame sessions (research R13, R14): holzi's own window opens a session for every extension
//! frame it mounts. The session, never the extension, says which extension a bridge call comes
//! from. Sessions live in memory and end with the frame or the process (ADR-0003: one process, one
//! vault).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use uuid::Uuid;

use crate::extensions::protocol::token;

#[derive(Debug)]
pub struct FrameSession {
    /// Random id the frontend uses for bridge calls; the extension never sees it.
    pub frame: String,
    pub extension_id: Uuid,
    /// The bundle this frame runs.
    pub bundle_id: Uuid,
    pub tab_id: String,
    /// Start token in the frame URL; HTML of the extension is served only with it.
    pub token: String,
    pub opened_at: Instant,
}

#[derive(Default)]
pub struct FrameRegistry {
    sessions: Mutex<HashMap<String, Arc<FrameSession>>>,
}

impl FrameRegistry {
    fn sessions(&self) -> std::sync::MutexGuard<'_, HashMap<String, Arc<FrameSession>>> {
        self.sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn open(&self, extension_id: Uuid, bundle_id: Uuid, tab_id: &str) -> Arc<FrameSession> {
        let session = Arc::new(FrameSession {
            frame: token::mint(),
            extension_id,
            bundle_id,
            tab_id: tab_id.to_owned(),
            token: token::mint(),
            opened_at: Instant::now(),
        });
        self.sessions()
            .insert(session.frame.clone(), Arc::clone(&session));
        session
    }

    pub fn close(&self, frame: &str) -> Option<Arc<FrameSession>> {
        self.sessions().remove(frame)
    }

    pub fn get(&self, frame: &str) -> Option<Arc<FrameSession>> {
        self.sessions().get(frame).cloned()
    }

    /// The session of `extension_id` whose start token is `given`.
    pub fn by_token(&self, extension_id: Uuid, given: &str) -> Option<Arc<FrameSession>> {
        self.sessions()
            .values()
            .find(|s| s.extension_id == extension_id && token::matches(&s.token, given))
            .cloned()
    }

    /// Every open frame of an extension, newest first.
    pub fn of_extension(&self, extension_id: Uuid) -> Vec<Arc<FrameSession>> {
        let mut frames: Vec<_> = self
            .sessions()
            .values()
            .filter(|s| s.extension_id == extension_id)
            .cloned()
            .collect();
        frames.sort_by(|a, b| b.opened_at.cmp(&a.opened_at));
        frames
    }
}

#[cfg(test)]
#[path = "frames_tests.rs"]
mod tests;
