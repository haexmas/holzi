//! Backpressure for `shell:output` (spec 017, US11): the SDK of each frame acknowledges the
//! output events it handed on (`extension_shell_ack {sessionId, count}`), and a shell's output is
//! read on only while every acknowledging frame has fewer than [`WINDOW`] of them open. Holding
//! the reading back holds the shell too: it blocks on its terminal, as on a slow one.
//!
//! A frame that never acknowledged (an SDK from before the ack, a frame still loading) is not
//! waited for, and neither is a closed one. Events or acknowledgements can be lost (the frame's
//! page loaded anew, an `ack` call failed), so a frame never holds the reading back for good: one
//! that has a full window and acknowledges nothing for [`STALL`] is no longer waited for. With its
//! next acknowledgement it counts again, from zero; so does a frame whose page loaded anew
//! ([`Flow::forget`]). Ending the session stops all waiting.
//!
//! An event is counted before it goes out, so its acknowledgement never comes first; a count
//! beyond what was sent (events of the time before the counting started afresh) is cut off.

use std::collections::HashMap;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// Output events a frame may have open before the reading waits; at most 8 KiB each.
pub const WINDOW: u64 = 32;
/// How long a frame with a full window may acknowledge nothing before it is no longer waited for.
pub const STALL: Duration = Duration::from_secs(2);
/// How often a waiting reader looks again whether a frame it waits for has closed or stalled.
const RECHECK: Duration = Duration::from_millis(250);

/// The output events of one session per frame: sent, and acknowledged by frames that do.
pub(super) struct Flow {
    state: Mutex<State>,
    changed: Condvar,
    stall: Duration,
}

impl Default for Flow {
    fn default() -> Self {
        Self::with_stall(STALL)
    }
}

#[derive(Default)]
struct State {
    frames: HashMap<String, Counter>,
    ended: bool,
}

/// One frame's events of one session.
#[derive(Default)]
struct Counter {
    sent: u64,
    acknowledged: u64,
    /// The frame acknowledges: the reading waits for it.
    acking: bool,
    /// Since when the frame's window has been full without an acknowledgement.
    full_since: Option<Instant>,
}

impl Counter {
    fn open(&self) -> u64 {
        self.sent.saturating_sub(self.acknowledged)
    }
}

impl Flow {
    pub(super) fn with_stall(stall: Duration) -> Self {
        Self {
            state: Mutex::default(),
            changed: Condvar::new(),
            stall,
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// One output event goes to `frame`; called before it goes out.
    pub(super) fn sent(&self, frame: &str) {
        self.lock().frames.entry(frame.to_owned()).or_default().sent += 1;
    }

    /// `frame` handed on `count` more events; never more than it was sent. A frame that was not
    /// waited for (never acknowledged, stalled, loaded anew) starts counting afresh: the events
    /// this count is about went out before.
    pub(super) fn acknowledge(&self, frame: &str, count: u64) {
        let mut state = self.lock();
        let counter = state.frames.entry(frame.to_owned()).or_default();
        if counter.acking {
            counter.acknowledged = counter.acknowledged.saturating_add(count).min(counter.sent);
        } else {
            *counter = Counter {
                acking: true,
                ..Counter::default()
            };
        }
        counter.full_since = None;
        self.changed.notify_all();
    }

    /// `frame` loaded a new page: what its old page had open is lost. It is not waited for until
    /// it acknowledges again.
    pub(super) fn forget(&self, frame: &str) {
        self.lock().frames.remove(frame);
        self.changed.notify_all();
    }

    /// The session ended: nobody waits any more.
    pub(super) fn end(&self) {
        self.lock().ended = true;
        self.changed.notify_all();
    }

    /// Waits until every acknowledging frame among `open` has room for another event. Frames no
    /// longer in `open` are forgotten; a frame whose window stays full for the stall time is no
    /// longer waited for.
    pub(super) fn wait_for_room(&self, open: impl Fn() -> Vec<String>) {
        loop {
            let frames = open();
            let mut state = self.lock();
            if state.ended {
                return;
            }
            state.frames.retain(|frame, _| frames.contains(frame));
            let now = Instant::now();
            let mut full = false;
            for counter in state.frames.values_mut() {
                if !counter.acking || counter.open() < WINDOW {
                    counter.full_since = None;
                    continue;
                }
                let since = *counter.full_since.get_or_insert(now);
                if now.duration_since(since) >= self.stall {
                    counter.acking = false;
                    counter.full_since = None;
                } else {
                    full = true;
                }
            }
            if !full {
                return;
            }
            let _ = self.changed.wait_timeout(state, RECHECK.min(self.stall));
        }
    }

    /// The events `frame` has open, if it is waited for.
    #[cfg(test)]
    pub(super) fn open_of(&self, frame: &str) -> Option<u64> {
        self.lock()
            .frames
            .get(frame)
            .filter(|c| c.acking)
            .map(Counter::open)
    }
}

#[cfg(test)]
#[path = "flow_tests.rs"]
mod flow_tests;
