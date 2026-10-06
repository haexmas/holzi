use std::sync::Arc;
use std::time::{Duration, Instant};

use super::*;

fn frames(names: &[&str]) -> Vec<String> {
    names.iter().map(|n| (*n).to_owned()).collect()
}

fn send(flow: &Flow, frame: &str, count: u64) {
    for _ in 0..count {
        flow.sent(frame);
    }
}

/// Whether `wait_for_room` returns within a short time.
fn has_room(flow: &Arc<Flow>, open: &[&str]) -> bool {
    let (flow, open) = (Arc::clone(flow), frames(open));
    let waiter = std::thread::spawn(move || flow.wait_for_room(|| open.clone()));
    let until = Instant::now() + Duration::from_millis(100);
    while !waiter.is_finished() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    waiter.is_finished()
}

#[test]
fn a_frame_that_never_acknowledged_is_not_waited_for() {
    let flow = Arc::new(Flow::default());
    send(&flow, "old-sdk", WINDOW * 4);
    assert!(has_room(&flow, &["old-sdk"]));
}

#[test]
fn an_acknowledging_frame_holds_the_reading_at_the_window() {
    let flow = Arc::new(Flow::default());
    send(&flow, "f", 1);
    flow.acknowledge("f", 1);
    send(&flow, "f", WINDOW - 1);
    assert!(has_room(&flow, &["f"]));
    send(&flow, "f", 1);
    assert!(!has_room(&flow, &["f"]), "a full window waits");
    flow.acknowledge("f", 1);
    assert!(has_room(&flow, &["f"]));
}

#[test]
fn a_waiting_reader_goes_on_when_acknowledged_closed_or_ended() {
    let full = || {
        let flow = Arc::new(Flow::default());
        flow.acknowledge("f", 0);
        send(&flow, "f", WINDOW);
        flow
    };
    let released = |flow: &Arc<Flow>, open: &'static [&'static str], release: &dyn Fn(&Flow)| {
        let waiting = Arc::clone(flow);
        let waiter = std::thread::spawn(move || waiting.wait_for_room(|| frames(open)));
        std::thread::sleep(Duration::from_millis(50));
        assert!(!waiter.is_finished());
        release(flow);
        waiter.join().unwrap();
    };
    released(&full(), &["f"], &|flow| flow.acknowledge("f", 5));
    released(&full(), &["f"], &|flow| flow.end());
    // The frame closed: the next look (after `RECHECK`) no longer finds it.
    let flow = full();
    let closed = Arc::new(Mutex::new(false));
    let open = {
        let closed = Arc::clone(&closed);
        move || {
            if *closed.lock().unwrap() {
                Vec::new()
            } else {
                frames(&["f"])
            }
        }
    };
    let waiting = Arc::clone(&flow);
    let waiter = std::thread::spawn(move || waiting.wait_for_room(open));
    std::thread::sleep(Duration::from_millis(50));
    assert!(!waiter.is_finished());
    *closed.lock().unwrap() = true;
    waiter.join().unwrap();
}

#[test]
fn the_slowest_acknowledging_frame_sets_the_pace() {
    let flow = Arc::new(Flow::default());
    for frame in ["fast", "slow"] {
        flow.acknowledge(frame, 0);
        send(&flow, frame, WINDOW);
    }
    flow.acknowledge("fast", WINDOW);
    assert!(!has_room(&flow, &["fast", "slow"]));
    flow.acknowledge("slow", 1);
    assert!(has_room(&flow, &["fast", "slow"]));
}

#[test]
fn a_frame_cannot_acknowledge_more_than_it_was_sent() {
    let flow = Arc::new(Flow::default());
    // The first acknowledgement only starts the counting.
    flow.acknowledge("f", 1_000);
    send(&flow, "f", WINDOW);
    assert!(!has_room(&flow, &["f"]));
    flow.acknowledge("f", 1_000);
    assert_eq!(flow.open_of("f"), Some(0));
    send(&flow, "f", WINDOW);
    assert!(
        !has_room(&flow, &["f"]),
        "a count beyond the sent is cut off"
    );
}

#[test]
fn a_frame_that_stops_acknowledging_is_waited_for_only_until_the_stall() {
    let stall = Duration::from_millis(400);
    let flow = Arc::new(Flow::with_stall(stall));
    for frame in ["live", "stuck"] {
        flow.acknowledge(frame, 0);
        send(&flow, frame, WINDOW);
    }
    flow.acknowledge("live", WINDOW);
    assert!(
        !has_room(&flow, &["live", "stuck"]),
        "full, not yet stalled"
    );
    let waiting = Arc::clone(&flow);
    let started = Instant::now();
    std::thread::spawn(move || waiting.wait_for_room(|| frames(&["live", "stuck"])))
        .join()
        .unwrap();
    assert!(started.elapsed() + Duration::from_millis(100) >= stall);
    assert_eq!(flow.open_of("stuck"), None, "no longer waited for");
    assert_eq!(
        flow.open_of("live"),
        Some(0),
        "the other frame still counts"
    );

    // Acknowledging again, the frame counts again, from zero.
    flow.acknowledge("stuck", 7);
    assert_eq!(flow.open_of("stuck"), Some(0));
    send(&flow, "stuck", WINDOW);
    assert!(!has_room(&flow, &["live", "stuck"]));
    flow.acknowledge("stuck", 1);
    assert!(has_room(&flow, &["live", "stuck"]));
}

#[test]
fn a_frame_whose_page_loaded_anew_is_not_waited_for_until_it_acknowledges() {
    let flow = Arc::new(Flow::default());
    flow.acknowledge("f", 0);
    send(&flow, "f", WINDOW);
    assert!(!has_room(&flow, &["f"]));
    flow.forget("f");
    assert!(has_room(&flow, &["f"]));
    send(&flow, "f", WINDOW * 2);
    assert!(
        has_room(&flow, &["f"]),
        "the new page has not acknowledged yet"
    );
    flow.acknowledge("f", 1);
    send(&flow, "f", WINDOW);
    assert!(!has_room(&flow, &["f"]));
}
