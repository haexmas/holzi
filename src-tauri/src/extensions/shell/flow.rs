//! Backpressure for `shell:output` (spec 017, US11): the SDK of each frame acknowledges the
//! output events it handed on (`extension_shell_ack {sessionId, count}`), and a shell's output is
//! read on only while every acknowledging frame has fewer than [`WINDOW`] of them open. Holding
//! the reading back holds the shell too: it blocks on its terminal, as on a slow one.
//!
//! A frame that never acknowledged (an SDK from before the ack, a frame still loading) is not
//! waited for, and neither is a closed one. Ending the session stops all waiting.

use std::collections::HashMap;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

/// Output events a frame may have open before the reading waits; at most 8 KiB each.
pub const WINDOW: u64 = 32;
/// How often a waiting reader looks again whether a frame it waits for has closed.
const RECHECK: Duration = Duration::from_millis(250);

/// The output events of one session per frame: sent, and acknowledged by frames that do.
#[derive(Default)]
pub(super) struct Flow {
    state: Mutex<State>,
    changed: Condvar,
}

#[derive(Default)]
struct State {
    sent: HashMap<String, u64>,
    acknowledged: HashMap<String, u64>,
    ended: bool,
}

impl Flow {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// One output event went to `frame`.
    pub(super) fn sent(&self, frame: &str) {
        *self.lock().sent.entry(frame.to_owned()).or_default() += 1;
    }

    /// `frame` handed on `count` more events; never more than it was sent.
    pub(super) fn acknowledge(&self, frame: &str, count: u64) {
        let mut state = self.lock();
        let sent = state.sent.get(frame).copied().unwrap_or(0);
        let done = state.acknowledged.entry(frame.to_owned()).or_default();
        *done = done.saturating_add(count).min(sent);
        self.changed.notify_all();
    }

    /// The session ended: nobody waits any more.
    pub(super) fn end(&self) {
        self.lock().ended = true;
        self.changed.notify_all();
    }

    /// Waits until every acknowledging frame among `open` has room for another event. Frames no
    /// longer in `open` are forgotten.
    pub(super) fn wait_for_room(&self, open: impl Fn() -> Vec<String>) {
        loop {
            let frames = open();
            let mut state = self.lock();
            state.sent.retain(|frame, _| frames.contains(frame));
            state.acknowledged.retain(|frame, _| frames.contains(frame));
            let State {
                sent,
                acknowledged,
                ended,
            } = &*state;
            let full = acknowledged.iter().any(|(frame, done)| {
                sent.get(frame).copied().unwrap_or(0).saturating_sub(*done) >= WINDOW
            });
            if *ended || !full {
                return;
            }
            let _ = self.changed.wait_timeout(state, RECHECK);
        }
    }
}

#[cfg(test)]
#[path = "flow_tests.rs"]
mod flow_tests;
