use super::*;

#[test]
fn the_development_origins_join_frame_src_and_nothing_else_changes() {
    let csp = "default-src 'self'; frame-src holzi-ext://localhost; script-src 'self'";
    assert_eq!(
        with_dev_frames(csp),
        "default-src 'self'; frame-src holzi-ext://localhost http://localhost:* \
         http://127.0.0.1:*; script-src 'self'"
    );
}

#[test]
fn a_policy_without_frame_src_gets_one() {
    assert_eq!(
        with_dev_frames("default-src 'self';"),
        "default-src 'self'; frame-src http://localhost:* http://127.0.0.1:*"
    );
}
