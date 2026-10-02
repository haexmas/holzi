//! The in-memory state of the extension host for the one vault of this process (ADR-0003): open
//! frame sessions and the started bundles the protocol handler serves. It ends with the process.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use uuid::Uuid;

use super::bridge::frames::FrameRegistry;
use super::registry::start::Started;

#[derive(Default)]
pub struct ExtensionHost {
    pub frames: FrameRegistry,
    /// Entry and Content-Security-Policy per bundle started in this process.
    started: Mutex<HashMap<Uuid, Arc<Started>>>,
}

impl ExtensionHost {
    fn started_map(&self) -> MutexGuard<'_, HashMap<Uuid, Arc<Started>>> {
        self.started.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn remember_started(&self, started: Started) -> Arc<Started> {
        let started = Arc::new(started);
        self.started_map()
            .insert(started.bundle_id, Arc::clone(&started));
        started
    }

    pub fn started(&self, bundle_id: Uuid) -> Option<Arc<Started>> {
        self.started_map().get(&bundle_id).cloned()
    }
}
