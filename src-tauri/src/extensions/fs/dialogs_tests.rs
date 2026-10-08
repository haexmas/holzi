//! The open and save dialogs with a chosen document that has no path (spec 043 FR-022: Android
//! hands a `content://` address): the frame reads and writes exactly that document and never sees
//! a path.

use std::sync::Arc;

use serde_json::json;

use crate::extensions::bridge::dispatch::Emit;

use super::super::test_support::setup;
use super::super::{lock, ops};
use super::*;

const ADDRESS: &str = "content://provider/Bericht.txt";

#[test]
fn an_opened_document_is_read_through_its_address_and_only_by_its_frame() {
    let s = setup();
    std::fs::write(s.documents.join("Bericht.txt"), "vom Anbieter").unwrap();
    *lock(&s.dialogs.document) = Some(ADDRESS.to_string());
    let frame = s.frame("good-notes-like.xt");

    let chosen = select_file(&frame, &json!({})).unwrap();
    assert_eq!(
        chosen,
        json!([ADDRESS]),
        "the frame gets the address, no path"
    );

    let content = ops::read_file(&frame, &json!({ "path": ADDRESS })).unwrap();
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(content.as_str().unwrap())
        .unwrap();
    assert_eq!(decoded, b"vom Anbieter");
    assert_eq!(
        ops::exists(&frame, &json!({ "path": ADDRESS })).unwrap(),
        json!(true)
    );

    // Opened for reading only.
    assert!(ops::write_file(&frame, &json!({ "path": ADDRESS, "data": "eA==" })).is_err());
    // Another frame of the same extension did not choose it.
    let other = CallContext {
        session: s
            .host
            .frames
            .open(frame.session.extension_id, uuid::Uuid::new_v4(), "tab-2"),
        db: s.vault.clone(),
        host: Arc::clone(&s.host),
        device: s.device,
        emitter: Arc::clone(&s.recorded) as Arc<dyn Emit>,
    };
    assert!(ops::read_file(&other, &json!({ "path": ADDRESS })).is_err());
}

#[test]
fn a_saved_document_is_written_through_its_address_and_stays_writable_for_the_frame() {
    let s = setup();
    *lock(&s.dialogs.document) = Some(ADDRESS.to_string());
    let frame = s.frame("good-notes-like.xt");

    let saved = save_file(&frame, &json!({ "data": [104, 105] })).unwrap();
    assert_eq!(saved, json!({ "path": ADDRESS, "success": true }));
    assert_eq!(
        std::fs::read(s.documents.join("Bericht.txt")).unwrap(),
        b"hi"
    );

    ops::write_file(&frame, &json!({ "path": ADDRESS, "data": "bmV1" })).unwrap();
    assert_eq!(
        std::fs::read(s.documents.join("Bericht.txt")).unwrap(),
        b"neu"
    );
}

#[test]
fn the_choice_of_a_document_ends_with_its_frame() {
    let s = setup();
    std::fs::write(s.documents.join("Bericht.txt"), "x").unwrap();
    *lock(&s.dialogs.document) = Some(ADDRESS.to_string());
    let frame = s.frame("good-notes-like.xt");
    select_file(&frame, &json!({})).unwrap();

    s.host
        .fs
        .frame_closed(&frame.session.frame, frame.session.extension_id, true);

    assert!(ops::read_file(&frame, &json!({ "path": ADDRESS })).is_err());
}

#[test]
fn a_chosen_document_that_is_gone_no_longer_exists() {
    let s = setup();
    std::fs::write(s.documents.join("Bericht.txt"), "x").unwrap();
    *lock(&s.dialogs.document) = Some(ADDRESS.to_string());
    let frame = s.frame("good-notes-like.xt");
    select_file(&frame, &json!({})).unwrap();

    std::fs::remove_file(s.documents.join("Bericht.txt")).unwrap();

    assert_eq!(
        ops::exists(&frame, &json!({ "path": ADDRESS })).unwrap(),
        json!(false)
    );
}
