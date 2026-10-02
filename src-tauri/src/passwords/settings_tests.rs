//! Tests for the clipboard setting of the password manager (spec 034, FR-006).

use std::time::Duration;

use super::settings::{clear_after, is_valid_clear_seconds, CLIPBOARD_CLEAR_KEY};
use super::test_support::open_test_vault;
use crate::storage::preferences::{self, PrefScope};
use crate::storage::query;

#[test]
fn only_the_choices_are_valid() {
    for value in ["0", "15", "30", "60", "120"] {
        assert!(is_valid_clear_seconds(value), "{value}");
    }
    for value in [
        "",
        "10",
        "-15",
        "30.0",
        " 30",
        "030",
        "off",
        "121",
        "99999999999",
    ] {
        assert!(!is_valid_clear_seconds(value), "{value:?}");
    }
}

#[test]
fn the_default_is_thirty_seconds_and_off_means_never() {
    let (_dir, db) = open_test_vault();
    let delay = |db: &haex_crdt::Database| {
        query::read(db, |q| clear_after(q).map_err(Into::into)).expect("read")
    };
    assert_eq!(delay(&db), Some(Duration::from_secs(30)), "nothing stored");
    let set = |value: &str| {
        db.write(|tx| {
            preferences::insert_or_update(tx, PrefScope::Vault, CLIPBOARD_CLEAR_KEY, value)
                .map(|_| ())
        })
        .expect("set");
    };
    set("15");
    assert_eq!(delay(&db), Some(Duration::from_secs(15)));
    set("0");
    assert_eq!(delay(&db), None);
    // A value that is not a choice (written by something else) reads as the default.
    set("7");
    assert_eq!(delay(&db), Some(Duration::from_secs(30)));
}
