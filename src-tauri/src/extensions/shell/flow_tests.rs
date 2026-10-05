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
    flow.acknowledge("f", 1_000);
    send(&flow, "f", WINDOW);
    assert!(!has_room(&flow, &["f"]));
}
