use super::*;

#[test]
fn a_session_is_found_by_frame_and_by_its_token_for_its_extension_only() {
    let frames = FrameRegistry::default();
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    let session = frames.open(a, Uuid::new_v4(), "tab-1");
    frames.open(b, Uuid::new_v4(), "tab-2");

    assert_eq!(frames.get(&session.frame).unwrap().tab_id, "tab-1");
    assert!(frames.by_token(a, &session.token).is_some());
    assert!(
        frames.by_token(b, &session.token).is_none(),
        "another extension's token"
    );
    assert!(frames.by_token(a, "nope").is_none());
    assert_eq!(frames.of_extension(a).len(), 1);

    frames.close(&session.frame);
    assert!(frames.get(&session.frame).is_none());
    assert!(frames.by_token(a, &session.token).is_none());
}
