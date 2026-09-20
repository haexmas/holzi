use std::io::Write;

use super::{classify_attachment, read_attachment_content, usability_for, AttachmentUsability};
use crate::adapters::AttachmentKind;
use crate::model_capabilities::ModelCapabilities;

fn write_temp_file(name: &str, bytes: &[u8]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join(name);
    let mut file = std::fs::File::create(&path).expect("create temp file");
    file.write_all(bytes).expect("write temp file");
    dir
}

#[test]
fn a_recognized_small_image_is_usable() {
    let bytes = b"not a real png but small";
    let dir = write_temp_file("photo.png", bytes);
    let info = classify_attachment(&dir.path().join("photo.png")).expect("classify");
    assert!(info.usable);
    assert!(info.reason.is_none());
    assert_eq!(info.size_bytes, bytes.len() as u64);
}

#[test]
fn an_unrecognized_extension_is_unusable_with_a_reason() {
    let dir = write_temp_file("archive.zip", b"PK\x03\x04");
    let info = classify_attachment(&dir.path().join("archive.zip")).expect("classify");
    assert!(!info.usable);
    assert!(info.reason.is_some());
}

#[test]
fn an_oversized_text_file_is_rejected_with_a_specific_reason() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("huge.txt");
    // Larger than the 5 MB text cap.
    let file = std::fs::File::create(&path).expect("create");
    file.set_len(6 * 1024 * 1024).expect("set_len");
    let info = classify_attachment(&path).expect("classify");
    assert!(!info.usable);
    assert!(info.reason.as_deref().unwrap().contains("exceeds"));
    assert!(read_attachment_content(&path).is_err());
}

#[test]
fn classify_errors_only_for_a_missing_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("nope.png");
    assert!(classify_attachment(&missing).is_err());
}

fn accepting(kinds: Vec<AttachmentKind>) -> ModelCapabilities {
    ModelCapabilities {
        accepted_attachment_kinds: Some(kinds),
        ..ModelCapabilities::default()
    }
}

#[test]
fn a_kind_in_the_determined_list_is_usable() {
    let caps = accepting(vec![AttachmentKind::Text, AttachmentKind::Image]);

    assert_eq!(
        usability_for(&AttachmentKind::Image, Some(&caps)),
        AttachmentUsability::Usable
    );
}

#[test]
fn a_kind_missing_from_the_determined_list_is_not_accepted() {
    let caps = accepting(vec![AttachmentKind::Text]);

    assert_eq!(
        usability_for(&AttachmentKind::Document, Some(&caps)),
        AttachmentUsability::NotAccepted
    );
    // An authoritative empty list (a local model) accepts nothing.
    assert_eq!(
        usability_for(&AttachmentKind::Text, Some(&accepting(Vec::new()))),
        AttachmentUsability::NotAccepted
    );
}

#[test]
fn undetermined_support_is_neither_usable_nor_reported_as_unsupported() {
    assert_eq!(
        usability_for(&AttachmentKind::Image, None),
        AttachmentUsability::Undetermined
    );
    assert_eq!(
        usability_for(&AttachmentKind::Image, Some(&ModelCapabilities::default())),
        AttachmentUsability::Undetermined
    );
}
