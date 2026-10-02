//! Tests for the types of the password manager (spec 034, T009): a secret never reaches a `Debug`
//! print, and a partial update tells absent, `null` and a value apart.

use super::model::{ItemInput, ItemPatch, KeyValueInput, KeyValuePatch, Patch, RevealedSecret};
use zeroize::Zeroizing;

const MARKER: &str = "SECRET-MARKER-MODEL";

#[test]
fn debug_of_an_input_with_secrets_prints_no_value() {
    let input = ItemInput {
        title: Some("Mail".to_string()),
        password: Some(MARKER.to_string()),
        otp_secret: Some(MARKER.to_string()),
        note: Some(MARKER.to_string()),
        key_values: vec![KeyValueInput {
            key: "PIN".to_string(),
            value: Some(MARKER.to_string()),
        }],
        ..ItemInput::default()
    };
    let printed = format!("{input:?} {:#?}", input);
    assert!(!printed.contains(MARKER), "{printed}");
    assert!(printed.contains("Mail"), "non-secret fields stay visible");
    assert!(printed.contains("<redacted>"));
}

#[test]
fn debug_of_a_patch_and_a_revealed_secret_prints_no_value() {
    let patch: ItemPatch = serde_json::from_value(serde_json::json!({
        "password": MARKER,
        "otpSecret": MARKER,
        "keyValues": [{ "key": "PIN", "value": MARKER }],
    }))
    .expect("patch");
    assert!(!format!("{patch:?}").contains(MARKER));
    assert!(!format!("{:?}", patch.key_values).contains(MARKER));
    let revealed = RevealedSecret {
        value: Zeroizing::new(MARKER.to_string()),
    };
    assert!(!format!("{revealed:?}").contains(MARKER));
    let one = KeyValuePatch {
        id: None,
        key: "k".to_string(),
        value: Some(MARKER.to_string()),
    };
    assert!(!format!("{one:?}").contains(MARKER));
}

#[test]
fn a_revealed_secret_is_the_one_type_that_serialises_its_value() {
    let revealed = RevealedSecret {
        value: Zeroizing::new("abc".to_string()),
    };
    assert_eq!(
        serde_json::to_value(&revealed).expect("serialise"),
        serde_json::json!({ "value": "abc" })
    );
}

#[test]
fn a_patch_tells_absent_null_and_a_value_apart() {
    let patch: ItemPatch = serde_json::from_value(serde_json::json!({
        "password": null,
        "title": "New",
        "otpDigits": 8,
    }))
    .expect("patch");
    assert_eq!(patch.password, Patch::Clear, "null clears");
    assert_eq!(patch.title, Patch::Set("New".to_string()), "a value sets");
    assert_eq!(patch.otp_digits, Patch::Set(8));
    assert_eq!(patch.username, Patch::Keep, "absent keeps");
    assert_eq!(patch.otp_secret, Patch::Keep);
    assert!(patch.tags.is_none() && patch.key_values.is_none());
}

#[test]
fn a_patch_with_tags_and_fields_replaces_those_sets() {
    let patch: ItemPatch = serde_json::from_value(serde_json::json!({
        "tags": ["a", "b"],
        "keyValues": [{ "id": "k1", "key": "PIN" }, { "key": "New", "value": "v" }],
    }))
    .expect("patch");
    assert_eq!(
        patch.tags.as_deref(),
        Some(&["a".to_string(), "b".to_string()][..])
    );
    let fields = patch.key_values.expect("fields");
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].id.as_deref(), Some("k1"));
    assert_eq!(fields[0].value, None, "no value keeps the stored one");
}

#[test]
fn an_input_may_be_empty_in_every_field() {
    let input: ItemInput = serde_json::from_value(serde_json::json!({})).expect("input");
    assert!(input.title.is_none() && input.password.is_none());
    assert!(input.tags.is_empty() && input.key_values.is_empty());
}
