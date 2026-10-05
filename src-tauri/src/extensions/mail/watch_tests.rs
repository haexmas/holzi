//! Watching a mailbox (spec 017, US11, T107): IDLE on the test server, polling where the server
//! has no IDLE, permission, stop and replacement.

use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::test_server::deliver;
use super::test_setup::{setup, Setup};
use super::watch::NEW_MESSAGES;

fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let started = Instant::now();
    while !done() {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "timed out: {what}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

impl Setup {
    fn watch(&self, mailbox: &str) -> Result<Value, crate::extensions::error::BridgeError> {
        self.call(
            "extension_mail_start_watch",
            json!({ "accountId": "work", "mailboxName": mailbox, "intervalSeconds": 1, "imap": self.imap() }),
        )
    }

    fn commands_with(&self, text: &str) -> usize {
        self.server
            .lock()
            .unwrap()
            .commands
            .iter()
            .filter(|c| c.contains(text))
            .count()
    }
}

fn polling() -> Setup {
    let s = setup();
    s.grant("poll", "*");
    s
}

#[test]
fn a_new_message_is_reported_through_idle_and_old_ones_are_not() {
    let s = polling();
    s.watch("INBOX").unwrap();
    wait_until("the watch to wait in IDLE", || s.commands_with("IDLE") > 0);
    assert!(
        s.recorded.heard(NEW_MESSAGES).is_empty(),
        "the two old messages count as seen"
    );

    deliver(&s.server, "INBOX", "new");
    wait_until("the report", || !s.recorded.heard(NEW_MESSAGES).is_empty());
    assert_eq!(
        s.recorded.heard(NEW_MESSAGES)[0],
        json!({ "accountId": "work", "mailboxName": "INBOX", "newCount": 1 })
    );
    wait_until("IDLE again", || s.commands_with("IDLE") > 1);
    deliver(&s.server, "INBOX", "newer");
    wait_until("the second report", || {
        s.recorded.heard(NEW_MESSAGES).len() == 2
    });
    assert_eq!(s.recorded.heard(NEW_MESSAGES)[1]["newCount"], 1);
}

#[test]
fn a_server_without_idle_is_asked_in_intervals() {
    let s = polling();
    s.server.lock().unwrap().no_idle = true;
    s.watch("INBOX").unwrap();
    wait_until("the watch to start", || s.commands_with("SELECT") > 0);
    deliver(&s.server, "INBOX", "new");
    wait_until("the report", || !s.recorded.heard(NEW_MESSAGES).is_empty());
    wait_until("an interval to pass", || s.commands_with("NOOP") > 0);
    deliver(&s.server, "INBOX", "newer");
    wait_until("the report after an interval", || {
        s.recorded.heard(NEW_MESSAGES).len() == 2
    });
    assert_eq!(s.commands_with("IDLE"), 0);
}

#[test]
fn watching_needs_the_poll_permission_for_the_server() {
    let s = setup();
    s.grant("fetch", "*");
    let asked = s.watch("INBOX").unwrap_err();
    assert_eq!(asked.code.as_u16(), 1004);
    assert_eq!(asked.details.unwrap()["action"], "poll");
    assert_eq!(s.commands_with("LOGIN"), 0, "nothing connected");
}

#[test]
fn a_stopped_watch_reports_nothing_and_a_second_stop_finds_none() {
    let s = polling();
    s.watch("INBOX").unwrap();
    wait_until("IDLE", || s.commands_with("IDLE") > 0);
    let stop = json!({ "accountId": "work", "mailboxName": "INBOX" });
    s.call("extension_mail_stop_watch", stop.clone()).unwrap();
    assert_eq!(s.code("extension_mail_stop_watch", stop), 1001);
    deliver(&s.server, "INBOX", "late");
    std::thread::sleep(Duration::from_millis(300));
    assert!(s.recorded.heard(NEW_MESSAGES).is_empty());

    s.watch("INBOX").unwrap();
    s.watch("INBOX").unwrap();
    assert_eq!(
        s.ctx.host.mail_watches.count(s.ctx.session.extension_id),
        1,
        "a second start replaces the first"
    );
    s.ctx.host.mail_watches.end_all(s.ctx.session.extension_id);
    assert_eq!(s.ctx.host.mail_watches.count(s.ctx.session.extension_id), 0);
}
