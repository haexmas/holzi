//! `shell:output` only as fast as the frames acknowledge it (spec 017, US11).

use std::time::Duration;

use serde_json::{json, Value};

use super::{of, output, setup, sh, OUTPUT};
use crate::extensions::bridge::dispatch::call;
use crate::extensions::shell::flow::WINDOW;

#[test]
fn an_acknowledging_frame_holds_a_flooding_shell_back() {
    let s = setup();
    s.allow(&s.notes, &sh());
    let session = s.start(&s.notes);
    let ack = |count: u64| {
        call(
            &s.notes,
            "extension_shell_ack",
            &json!({ "sessionId": session, "count": count }),
        )
    };
    let count = |events: &[(String, Value)]| of(events, OUTPUT, &session).len() as u64;
    let events = || count(&s.recorded.all());
    s.write(&s.notes, &session, "echo ready\n").unwrap();
    s.recorded.waited_for_output(&session, "ready");
    // From its first acknowledgement on, the frame is waited for.
    ack(events()).unwrap();
    let acknowledged = events();

    // About 2 MB: hundreds of events, far more than one window.
    s.write(
        &s.notes,
        &session,
        "head -c 2000000 /dev/zero | tr '\\0' x; echo; echo flood-$((6 * 7))\n",
    )
    .unwrap();
    s.recorded
        .wait_until("a full window", |e| count(e) >= acknowledged + WINDOW);
    std::thread::sleep(Duration::from_millis(300));
    let held = events();
    assert!(
        held <= acknowledged + WINDOW,
        "{held} events, window {WINDOW}"
    );
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(events(), held, "the reading waits");

    // A frame that keeps up gets everything (the echoed line has no "flood-42").
    let mut acked = acknowledged;
    while !output(&s.recorded.all(), &session).contains("flood-42") {
        let now = events();
        if now > acked {
            ack(now - acked).unwrap();
            acked = now;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let flooded = output(&s.recorded.all(), &session)
        .chars()
        .filter(|c| *c == 'x')
        .count();
    assert!(flooded >= 2_000_000, "{flooded}");

    assert_eq!(
        ack(0).unwrap_err().code.as_u16(),
        3001,
        "a count is at least 1"
    );
    call(
        &s.notes,
        "extension_shell_close",
        &json!({ "sessionId": session }),
    )
    .unwrap();
    s.recorded.waited_for_exit(&session);
}
