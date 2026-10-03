use super::*;

#[test]
fn tokens_are_random_hex_and_compare_exactly() {
    let a = mint();
    let b = mint();
    assert_eq!(a.len(), 64);
    assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
    assert_ne!(a, b);
    assert!(matches(&a, &a.clone()));
    assert!(!matches(&a, &b));
    assert!(!matches(&a, &a[..63]));
    assert!(!matches(&a, ""));
}
