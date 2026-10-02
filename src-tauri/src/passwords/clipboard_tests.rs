//! Tests for the clipboard clearing (spec 034, FR-006, research R9) with a fake clipboard and
//! paused time: no sleeps, the clock moves only when the test says so.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::clipboard::{ClipboardClearer, ClipboardPort};
use crate::error::Result;

#[derive(Default)]
struct FakeClipboard {
    content: Mutex<Option<String>>,
}

impl FakeClipboard {
    fn content(&self) -> Option<String> {
        self.content.lock().expect("lock").clone()
    }

    /// Someone else copies something.
    fn set(&self, text: &str) {
        *self.content.lock().expect("lock") = Some(text.to_string());
    }
}

impl ClipboardPort for FakeClipboard {
    fn write(&self, text: &str) -> Result<()> {
        self.set(text);
        Ok(())
    }

    fn read(&self) -> Result<Option<String>> {
        Ok(self.content())
    }

    fn clear(&self) -> Result<()> {
        *self.content.lock().expect("lock") = None;
        Ok(())
    }
}

fn port() -> (Arc<FakeClipboard>, Arc<dyn ClipboardPort>) {
    let fake = Arc::new(FakeClipboard::default());
    let dynamic: Arc<dyn ClipboardPort> = fake.clone();
    (fake, dynamic)
}

#[tokio::test(start_paused = true)]
async fn the_clipboard_is_cleared_after_the_delay() {
    let (fake, port) = port();
    let clearer = ClipboardClearer::new();
    clearer
        .copy(port, "secret", Some(Duration::from_secs(30)))
        .expect("copy");
    assert_eq!(fake.content().as_deref(), Some("secret"));
    tokio::time::sleep(Duration::from_secs(29)).await;
    assert_eq!(fake.content().as_deref(), Some("secret"), "not yet");
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert_eq!(fake.content(), None, "cleared after 30 seconds");
}

#[tokio::test(start_paused = true)]
async fn it_does_not_clear_what_the_user_copied_in_the_meantime() {
    let (fake, port) = port();
    let clearer = ClipboardClearer::new();
    clearer
        .copy(port, "secret", Some(Duration::from_secs(30)))
        .expect("copy");
    fake.set("something else");
    tokio::time::sleep(Duration::from_secs(40)).await;
    assert_eq!(fake.content().as_deref(), Some("something else"));
}

#[tokio::test(start_paused = true)]
async fn a_second_copy_cancels_the_first_timer() {
    let (fake, port) = port();
    let clearer = ClipboardClearer::new();
    clearer
        .copy(Arc::clone(&port), "first", Some(Duration::from_secs(30)))
        .expect("first");
    tokio::time::sleep(Duration::from_secs(20)).await;
    clearer
        .copy(port, "second", Some(Duration::from_secs(30)))
        .expect("second");
    // The first timer would fire at 30 s; the second value must survive until its own 30 s.
    tokio::time::sleep(Duration::from_secs(15)).await;
    assert_eq!(fake.content().as_deref(), Some("second"));
    tokio::time::sleep(Duration::from_secs(20)).await;
    assert_eq!(fake.content(), None);
}

#[tokio::test(start_paused = true)]
async fn without_a_delay_it_never_clears() {
    let (fake, port) = port();
    let clearer = ClipboardClearer::new();
    clearer.copy(port, "secret", None).expect("copy");
    tokio::time::sleep(Duration::from_secs(3600)).await;
    assert_eq!(fake.content().as_deref(), Some("secret"));
}

#[tokio::test(start_paused = true)]
async fn clear_now_clears_a_pending_value_but_not_a_foreign_one() {
    let (fake, port) = port();
    let clearer = ClipboardClearer::new();
    clearer
        .copy(Arc::clone(&port), "secret", Some(Duration::from_secs(30)))
        .expect("copy");
    clearer.clear_now();
    assert_eq!(fake.content(), None, "the vault is closing");
    // A foreign value stays.
    clearer
        .copy(port, "secret", Some(Duration::from_secs(30)))
        .expect("copy");
    fake.set("foreign");
    clearer.clear_now();
    assert_eq!(fake.content().as_deref(), Some("foreign"));
    // Nothing pending: nothing happens.
    clearer.clear_now();
}
