//! Tests for the derived identifiers of the password manager (spec 034, T008).

use super::ids::{
    fold, fold_for_search, item_tag_id, passkey_id, tag_id, NS_ITEM_TAG, NS_PASSKEY, NS_TAG,
};

// Escapes keep the two forms visibly different in the source. The pinned ids below were
// computed independently with Python's `uuid.uuid5`.
const E_COMPOSED: &str = "caf\u{e9}";
const E_DECOMPOSED: &str = "cafe\u{301}";
const UE_COMPOSED: &str = "m\u{fc}ller";
const UE_DECOMPOSED: &str = "mu\u{308}ller";

#[test]
fn the_namespaces_never_change() {
    // A changed namespace gives the same tag a different id on an old and a new device.
    assert_eq!(NS_TAG.to_string(), "1318bf99-b22a-4792-9940-1e637fe41214");
    assert_eq!(
        NS_ITEM_TAG.to_string(),
        "2f0740cd-8b85-40a1-aea7-a5b02c6bf7b3"
    );
    assert_eq!(
        NS_PASSKEY.to_string(),
        "e9e2f6e7-716b-43c4-85ba-a83fb47392c8"
    );
}

#[test]
fn derived_ids_are_pinned_for_fixed_inputs() {
    assert_eq!(
        tag_id("Work").to_string(),
        "000df697-b84a-522b-a061-2fa8a99bcc00"
    );
    assert_eq!(
        item_tag_id("item-1", "tag-1").to_string(),
        "731ae719-4b1e-59a6-b5d6-158d9812d0c0"
    );
    assert_eq!(
        passkey_id("Y3JlZA==").to_string(),
        "da51b0ea-5961-56d0-808e-1af8077b606e"
    );
}

#[test]
fn fold_ignores_case_and_surrounding_space() {
    assert_eq!(fold("Work"), fold(" work "));
    assert_eq!(fold("WORK"), "work");
    assert_eq!(tag_id("Work"), tag_id(" work "));
}

#[test]
fn composed_and_decomposed_forms_fold_equal() {
    assert_ne!(E_COMPOSED, E_DECOMPOSED);
    assert_eq!(fold(E_COMPOSED), fold(E_DECOMPOSED));
    assert_eq!(tag_id(E_COMPOSED), tag_id(E_DECOMPOSED));
    assert_ne!(UE_COMPOSED, UE_DECOMPOSED);
    assert_eq!(fold(UE_COMPOSED), fold(UE_DECOMPOSED));
    assert_eq!(tag_id(UE_COMPOSED), tag_id(UE_DECOMPOSED));
}

#[test]
fn different_names_give_different_ids() {
    assert_ne!(tag_id("Work"), tag_id("Home"));
    assert_ne!(item_tag_id("a", "b"), item_tag_id("b", "a"));
    assert_ne!(passkey_id("one"), passkey_id("two"));
}

#[test]
fn a_name_that_is_empty_after_trim_folds_to_the_empty_string() {
    // The caller rejects it; `fold` only reports it.
    assert_eq!(fold("   "), "");
    assert_eq!(fold(""), "");
}

#[test]
fn search_folding_drops_accents_and_case() {
    assert_eq!(fold_for_search("Gerät"), "gerat");
    assert_eq!(fold_for_search("CAF\u{c9}"), "cafe");
    assert_eq!(fold_for_search("cafe\u{301}"), "cafe");
    assert_eq!(fold_for_search("Mail"), fold_for_search("mAIL"));
}
