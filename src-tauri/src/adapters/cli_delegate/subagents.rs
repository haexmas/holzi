//! Tracks Claude Code sub-agent activity across one delegate invocation
//! (spec 011-composer-toolbar-parity), based on the documented
//! `parent_tool_use_id` field on `assistant`/`user` stream-json messages
//! (research.md §2): `null` means "main conversation", a non-null value is
//! the id of the (`Agent`) tool call that spawned it. Multiple sub-agents
//! dispatched together in one assistant turn share the same batch.
//!
//! Deliberately its own module — a pure state machine over synthetic ids,
//! directly unit-testable with no NDJSON parsing or process involved
//! (`subagents_tests.rs`), mirroring `claude.rs::parse_line`'s own
//! test-friendliness.

use std::collections::{HashMap, HashSet};

/// What happened to the tracker's active count as a result of observing
/// one stream-json line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TrackerEvent {
    /// Nothing the composer's agent-activity indicator needs to see.
    None,
    /// `active_count` after this line's effect. `batch_size` is `Some(n)`
    /// only on the update where a new batch of `n` sub-agents was just
    /// confirmed (its first member promoted from pending to active).
    Update {
        active_count: usize,
        batch_size: Option<usize>,
    },
}

#[derive(Debug, Default)]
pub(super) struct Tracker {
    /// tool_use id -> the batch id it was dispatched under.
    pending: HashMap<String, usize>,
    /// ids confirmed active (something referenced them as a parent).
    active: HashSet<String>,
    next_batch_id: usize,
    /// Batch ids whose first member has already been promoted — so a
    /// batch's size is reported exactly once, on that first promotion.
    announced_batches: HashSet<usize>,
}

impl Tracker {
    pub(super) fn new() -> Self {
        Self::default()
    }

    /// Records one or more `tool_use` ids seen together in a single
    /// top-level (`parent_tool_use_id: null`) `assistant` message as one
    /// new batch. Not yet confirmed as sub-agents — a plain tool call that
    /// never gets referenced as a parent simply stays pending forever and
    /// is never surfaced (research.md §2: not every tool call spawns a
    /// sub-agent, only `Agent` ones do, and this tracker doesn't need to
    /// know that tool's name to be correct).
    pub(super) fn observe_top_level_tool_use(&mut self, ids: &[String]) {
        if ids.is_empty() {
            return;
        }
        let batch_id = self.next_batch_id;
        self.next_batch_id += 1;
        for id in ids {
            self.pending.insert(id.clone(), batch_id);
        }
    }

    /// Call for every message whose `parent_tool_use_id` is non-null.
    /// Promotes that id from pending to active on its first reference;
    /// later messages from the same sub-agent (same parent id) are a
    /// harmless no-op.
    pub(super) fn observe_parent_reference(&mut self, parent_tool_use_id: &str) -> TrackerEvent {
        if self.active.contains(parent_tool_use_id) {
            return TrackerEvent::None;
        }
        let Some(batch_id) = self.pending.remove(parent_tool_use_id) else {
            // Not a tool_use id this tracker ever saw dispatched at the top
            // level — nothing to promote.
            return TrackerEvent::None;
        };
        self.active.insert(parent_tool_use_id.to_string());
        // Only the batch's *first* promotion reports a size — at that
        // point `pending` still holds every other not-yet-promoted member
        // of the batch, so "however many remain pending, plus the one just
        // promoted" is the batch's original total. A later promotion of
        // the same batch would undercount by that same logic, so it
        // reports no size at all instead (data-model.md).
        let batch_size = if self.announced_batches.insert(batch_id) {
            let remaining_in_batch = self.pending.values().filter(|&&b| b == batch_id).count();
            Some(remaining_in_batch + 1)
        } else {
            None
        };
        TrackerEvent::Update {
            active_count: self.active.len(),
            batch_size,
        }
    }

    /// Call for a main-thread (`parent_tool_use_id: null`) `tool_result`
    /// block. Removes the id from `active` if it was there.
    pub(super) fn observe_tool_result(&mut self, tool_use_id: &str) -> TrackerEvent {
        if !self.active.remove(tool_use_id) {
            return TrackerEvent::None;
        }
        TrackerEvent::Update {
            active_count: self.active.len(),
            batch_size: None,
        }
    }
}
