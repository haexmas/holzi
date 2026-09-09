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
    assert!(dir.join("healthy.db").exists(), "healthy instance untouched");
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
