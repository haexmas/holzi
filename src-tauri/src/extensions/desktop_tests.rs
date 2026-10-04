use super::*;

#[test]
fn only_a_tap_or_a_button_is_a_click() {
    assert_eq!(response("tap"), NotificationResponse::Body);
    assert_eq!(
        response("button:tap"),
        NotificationResponse::Button("tap".to_owned())
    );
    // Android and iOS report swiping a notification away as `dismiss`.
    assert_eq!(response("dismiss"), NotificationResponse::Closed);
    assert_eq!(response("something"), NotificationResponse::Closed);
}
