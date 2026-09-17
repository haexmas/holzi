use super::imp::{capped_event_payload, TranscriptionResultWire};
use crate::stt::interrupt::InterruptCommand;

#[test]
fn capped_event_payload_keeps_success_and_drops_failure() {
    let payload = capped_event_payload(Ok(TranscriptionResultWire {
        text: "hello".into(),
        interrupt: Some(InterruptCommand::Stop),
    }));
    let payload = payload.expect("successful cap outcome should be emitted");
    assert_eq!(payload.text, "hello");
    assert_eq!(payload.interrupt, Some(InterruptCommand::Stop));

    assert!(capped_event_payload(Err(crate::error::HolziError::NotRecording)).is_none());
}
