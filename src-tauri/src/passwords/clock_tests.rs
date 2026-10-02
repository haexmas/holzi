//! Tests for the time stamps of the password manager (spec 034).

use super::clock::{format_millis, now, now_after, parse_millis};

#[test]
fn formats_known_instants() {
    assert_eq!(format_millis(0), "1970-01-01T00:00:00.000Z");
    assert_eq!(format_millis(1), "1970-01-01T00:00:00.001Z");
    // 2000-02-29 (a leap day) at noon.
    assert_eq!(format_millis(951_825_600_000), "2000-02-29T12:00:00.000Z");
    // The RFC 6238 test time 1111111109 s.
    assert_eq!(format_millis(1_111_111_109_123), "2005-03-18T01:58:29.123Z");
    // Before the epoch rounds down.
    assert_eq!(format_millis(-1), "1969-12-31T23:59:59.999Z");
}

#[test]
fn now_has_the_shape_and_orders_as_text() {
    let a = now();
    assert_eq!(a.len(), 24, "{a}");
    assert!(a.ends_with('Z') && a.as_bytes()[10] == b'T');
    // The text order is the time order, which the conflict check and the history rely on.
    assert!(format_millis(1_000) < format_millis(1_001));
    assert!(format_millis(999_999_999_999) < format_millis(1_000_000_000_000));
}

#[test]
fn parsing_is_the_inverse_of_formatting() {
    for millis in [0, 1, 951_825_600_000, 1_111_111_109_123, 4_102_444_800_999] {
        assert_eq!(parse_millis(&format_millis(millis)), Some(millis));
    }
    assert_eq!(parse_millis("2026-10-02 09:41:45"), None);
    assert_eq!(parse_millis("garbage"), None);
    assert_eq!(parse_millis("2026-13-02T09:41:45.123Z"), None);
}

#[test]
fn now_after_never_returns_the_previous_token() {
    let far_future = format_millis(32_503_680_000_000);
    let next = now_after(Some(&far_future));
    assert_eq!(parse_millis(&next), Some(32_503_680_000_001));
    assert!(now_after(None) > format_millis(1_700_000_000_000));
    assert!(now_after(Some("not a time")) > format_millis(1_700_000_000_000));
}
