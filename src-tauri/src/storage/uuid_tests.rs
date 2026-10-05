//! Tests for `storage::uuid`: valid values decode, malformed ones stay conversion errors.

use super::*;

#[test]
fn decodes_valid_and_optional_values() {
    let value = Uuid::from_u128(7);
    assert_eq!(parse(&value.to_string(), 0).unwrap(), value);
    assert_eq!(optional(None, 2).unwrap(), None);
    assert_eq!(optional(Some(value.to_string()), 2).unwrap(), Some(value));
}

#[test]
fn rejects_malformed_values_instead_of_dropping_them() {
    assert!(parse("not-a-uuid", 3).is_err());
    assert!(optional(Some("not-a-uuid".to_string()), 4).is_err());
}
