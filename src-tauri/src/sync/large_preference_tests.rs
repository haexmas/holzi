//! A workspace background syncs as one preference (spec 042, FR-018, research R6, T037): a value of
//! the size a 2560-px WebP reaches arrives whole on the other device.

use crate::storage::preferences::{self, PrefScope};
use crate::storage::preferences_commands::{validate_value, BACKGROUND_KEY, BACKGROUND_PREFIX};
use crate::storage::query;
use crate::sync::test_support::Device;

#[test]
fn a_background_of_about_500_kb_arrives_whole_on_the_other_device() {
    let (a, b) = (Device::new(), Device::new());
    let value = format!("{BACKGROUND_PREFIX}{}", "QUJD".repeat(500 * 1024 / 4));
    validate_value(BACKGROUND_KEY, &value).expect("a valid background");
    a.db()
        .write(|tx| {
            preferences::insert_or_update(tx, PrefScope::Vault, BACKGROUND_KEY, &value).map(|_| ())
        })
        .expect("write the background");

    b.pull_from(&a);

    let arrived = query::read(b.db(), |r| {
        preferences::get(r, PrefScope::Vault, BACKGROUND_KEY)
    })
    .expect("read the background");
    assert_eq!(arrived.as_deref().map(str::len), Some(value.len()));
    assert!(
        arrived.as_deref() == Some(value.as_str()),
        "the value changed on the way"
    );
}
