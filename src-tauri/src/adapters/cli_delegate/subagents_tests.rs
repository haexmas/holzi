use super::subagents::{Tracker, TrackerEvent};

#[test]
fn a_single_dispatch_promoted_reports_a_batch_of_one() {
    let mut tracker = Tracker::new();
    tracker.observe_top_level_tool_use(&["a".to_string()]);
    assert_eq!(
        tracker.observe_parent_reference("a"),
        TrackerEvent::Update {
            active_count: 1,
            batch_size: Some(1),
        }
    );
}

#[test]
fn two_ids_from_the_same_dispatch_report_the_batch_size_only_on_first_promotion() {
    let mut tracker = Tracker::new();
    tracker.observe_top_level_tool_use(&["a".to_string(), "b".to_string()]);

    assert_eq!(
        tracker.observe_parent_reference("a"),
        TrackerEvent::Update {
            active_count: 1,
            batch_size: Some(2),
        }
    );
    assert_eq!(
        tracker.observe_parent_reference("b"),
        TrackerEvent::Update {
            active_count: 2,
            batch_size: None,
        }
    );
}

#[test]
fn further_messages_from_an_already_active_sub_agent_are_a_no_op() {
    let mut tracker = Tracker::new();
    tracker.observe_top_level_tool_use(&["a".to_string()]);
    tracker.observe_parent_reference("a");
    assert_eq!(tracker.observe_parent_reference("a"), TrackerEvent::None);
}

#[test]
fn a_tool_result_for_an_active_id_decrements_the_active_count() {
    let mut tracker = Tracker::new();
    tracker.observe_top_level_tool_use(&["a".to_string(), "b".to_string()]);
    tracker.observe_parent_reference("a");
    tracker.observe_parent_reference("b");

    assert_eq!(
        tracker.observe_tool_result("a"),
        TrackerEvent::Update {
            active_count: 1,
            batch_size: None,
        }
    );
    assert_eq!(
        tracker.observe_tool_result("b"),
        TrackerEvent::Update {
            active_count: 0,
            batch_size: None,
        }
    );
}

#[test]
fn an_unrelated_parent_or_result_id_is_a_no_op() {
    let mut tracker = Tracker::new();
    assert_eq!(
        tracker.observe_parent_reference("ghost"),
        TrackerEvent::None
    );
    assert_eq!(tracker.observe_tool_result("ghost"), TrackerEvent::None);
}

#[test]
fn a_pending_dispatch_never_referenced_is_never_surfaced() {
    // A plain (non-Agent) tool call dispatched at the top level: nothing
    // ever references it as a parent, so it should never appear as active.
    let mut tracker = Tracker::new();
    tracker.observe_top_level_tool_use(&["a".to_string()]);
    assert_eq!(tracker.observe_tool_result("a"), TrackerEvent::None);
}

#[test]
fn two_separate_dispatches_are_two_independent_batches() {
    let mut tracker = Tracker::new();
    tracker.observe_top_level_tool_use(&["a".to_string()]);
    assert_eq!(
        tracker.observe_parent_reference("a"),
        TrackerEvent::Update {
            active_count: 1,
            batch_size: Some(1),
        }
    );

    tracker.observe_top_level_tool_use(&["b".to_string()]);
    assert_eq!(
        tracker.observe_parent_reference("b"),
        TrackerEvent::Update {
            active_count: 2,
            batch_size: Some(1),
        }
    );
}
