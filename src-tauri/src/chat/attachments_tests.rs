use std::io::Write;

use super::{classify_attachment, read_attachment_content, usability_for};
use crate::adapters::AttachmentKind;
use crate::storage::providers::ProviderKind;

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
}

#[test]
fn classify_errors_only_for_a_missing_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("nope.png");
    assert!(classify_attachment(&missing).is_err());
}

#[test]
fn usability_matrix_matches_the_four_backends() {
    assert!(usability_for(
        &AttachmentKind::Image,
        ProviderKind::ApiKey,
        Some("anthropic")
    ));
    assert!(usability_for(
        &AttachmentKind::Document,
        ProviderKind::CliDelegate,
        Some("claude")
    ));
    assert!(!usability_for(
        &AttachmentKind::Image,
        ProviderKind::Local,
        None
    ));
    assert!(!usability_for(
        &AttachmentKind::Image,
        ProviderKind::CliDelegate,
        Some("codex")
    ));
}

#[test]
fn read_attachment_content_reports_the_right_media_type() {
    let dir = write_temp_file("note.txt", b"hello");
    let attachment = read_attachment_content(&dir.path().join("note.txt")).expect("read");
    assert_eq!(attachment.media_type, "text/plain");
    assert_eq!(attachment.bytes, b"hello");
}

#[test]
fn read_attachment_content_fails_for_a_missing_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(read_attachment_content(&dir.path().join("gone.png")).is_err());
}
