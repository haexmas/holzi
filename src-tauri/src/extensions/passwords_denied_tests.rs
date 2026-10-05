//! Denied `passwords` permissions of an extension (spec 017, FR-017): a denied `*` takes every
//! grant away; a denied tag hides its entries from a `*` grant and voids a grant for the same tag,
//! but a granted tag on the same entry beats it.

use serde_json::{json, Value};

use super::tests::{setup, Setup};
use crate::passwords::access::Caller;
use crate::passwords::model::ItemInput;
use crate::passwords::model_references::RefMarkKind;
use crate::passwords::service::PasswordsService;

use super::block_on;

impl Setup {
    fn create(&self, input: ItemInput) -> String {
        block_on(PasswordsService::new(self.vault.clone()).create_item(
            &Caller::User,
            &[],
            input,
            None,
        ))
        .unwrap()
    }

    fn listed(&self) -> Vec<String> {
        self.call("extension_password_list", json!({}))
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["id"].as_str().unwrap().to_owned())
            .collect()
    }
}

fn tagged(title: &str, tags: &[&str]) -> ItemInput {
    ItemInput {
        title: Some(title.into()),
        tags: tags.iter().map(|t| t.to_string()).collect(),
        ..ItemInput::default()
    }
}

#[test]
fn a_denied_star_takes_every_grant_away() {
    let s = setup();
    s.permit("readWrite", "haex-calendar", "granted");
    s.permit("read", "*", "denied");
    assert_eq!(s.code("extension_password_list", json!({})), 1002);
    assert_eq!(
        s.code("extension_password_read", json!({ "itemId": s.calendar })),
        1002
    );
}

#[test]
fn under_a_denied_star_holzi_does_not_ask() {
    let s = setup();
    s.permit("readWrite", "haex-calendar", "ask");
    s.permit("read", "*", "denied");
    assert_eq!(
        s.code("extension_password_read", json!({ "itemId": s.calendar })),
        1002
    );
}

#[test]
fn a_denied_tag_hides_its_entries_from_a_star_grant_also_behind_a_reference() {
    let s = setup();
    s.permit("readWrite", "*", "granted");
    s.permit("read", "private", "denied");
    let untagged = s.create(tagged("untagged", &[]));
    let password = s.token(&s.private, RefMarkKind::Password, None);
    let pointing = s.create(ItemInput {
        password: Some(password),
        ..tagged("pointing", &["haex-calendar"])
    });

    let listed = s.listed();
    assert!(listed.contains(&s.calendar) && listed.contains(&untagged));
    assert!(!listed.contains(&s.private), "{listed:?}");
    assert_eq!(
        s.code("extension_password_read", json!({ "itemId": s.private })),
        1001,
        "hidden like an entry outside the scope"
    );
    let read = s
        .call("extension_password_read", json!({ "itemId": pointing }))
        .unwrap();
    assert_eq!(read["password"], Value::Null, "the source is hidden");
    assert_eq!(
        s.code(
            "extension_password_create",
            json!({ "input": { "title": "x", "tags": ["Private"] } })
        ),
        1002,
        "no new entry it could not see"
    );
    assert_eq!(
        s.code("extension_password_delete", json!({ "itemId": s.private })),
        1001
    );
}

#[test]
fn a_granted_tag_beats_a_denied_one_on_the_same_entry() {
    let s = setup();
    s.permit("read", "private", "granted");
    s.permit("read", "haex-calendar", "denied");
    let both = s.create(tagged("both", &["private", "haex-calendar"]));

    let listed = s.listed();
    assert!(listed.contains(&s.private) && listed.contains(&both));
    assert!(!listed.contains(&s.calendar), "{listed:?}");
    assert_eq!(
        s.code("extension_password_read", json!({ "itemId": both })),
        0
    );
}

#[test]
fn a_grant_and_a_denial_of_the_same_tag_is_a_denial() {
    let s = setup();
    s.permit("readWrite", "haex-calendar", "granted");
    s.permit("read", "haex-calendar", "denied");
    assert_eq!(
        s.code("extension_password_read", json!({ "itemId": s.calendar })),
        1002
    );
}
