//! Tests for `startup::cleanup_orphans_in_dir` — pure filesystem worker
//! that does not need a Tauri `AppHandle`.

use std::fs;

use crate::instances::startup::cleanup_orphans_in_dir;

#[test]
fn cleanup_removes_marker_and_sibling_db() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();

    // Orphan pair.
    fs::write(dir.join("orphan.db"), b"partial-genesis").unwrap();
    fs::write(dir.join("orphan.db.pending"), b"").unwrap();

    // Healthy instance without marker — must be left alone.
    fs::write(dir.join("healthy.db"), b"complete").unwrap();

    let removed = cleanup_orphans_in_dir(dir).unwrap();
    assert_eq!(removed, 1);

    assert!(!dir.join("orphan.db").exists(), "orphan .db removed");
    assert!(
        !dir.join("orphan.db.pending").exists(),
        "orphan marker removed"
    );
    assert!(
        dir.join("healthy.db").exists(),
        "healthy instance untouched"
    );
}

#[test]
fn cleanup_survives_marker_without_sibling_db() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();

    // Marker exists but the .db was never created (crash between
    // create_new marker and Database::open). Cleanup must still
    // succeed and remove the marker.
    fs::write(dir.join("half.db.pending"), b"").unwrap();

    let removed = cleanup_orphans_in_dir(dir).unwrap();
    assert_eq!(removed, 1);
    assert!(!dir.join("half.db.pending").exists());
}

#[test]
fn cleanup_missing_dir_is_not_error() {
    let tmp = tempfile::tempdir().unwrap();
    let missing = tmp.path().join("does-not-exist");
    assert_eq!(cleanup_orphans_in_dir(&missing).unwrap(), 0);
}

#[test]
fn cleanup_keeps_marker_when_sibling_cannot_be_removed() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();

    // A directory at the sibling path makes remove_file fail without relying
    // on platform-specific permissions or running the test as a non-root user.
    fs::create_dir(dir.join("blocked.db")).unwrap();
    fs::write(dir.join("blocked.db.pending"), b"").unwrap();

    assert_eq!(cleanup_orphans_in_dir(dir).unwrap(), 0);
    assert!(dir.join("blocked.db").is_dir());
    assert!(dir.join("blocked.db.pending").exists());
}
