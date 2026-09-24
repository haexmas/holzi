//! Tests for `startup::cleanup_orphans_in_dir` — pure filesystem worker
//! that does not need a Tauri `AppHandle`.

use std::fs;

use crate::instances::presence::ProcessPresence;
use crate::instances::startup::cleanup_orphans_in_dir;
use crate::models::import::cleanup_staging_in_dir;

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

/// T067/FR-026/SC-010: the combined startup cleanup (instance orphans plus model staging
/// leftovers, the same two pure functions `cleanup_orphans_on_startup` calls) runs only through
/// `ProcessPresence::announce`, so it runs while alone and is skipped while another presence is
/// alive — no AppHandle needed, since both cleanup functions already take a plain `&Path`.
#[test]
fn combined_startup_cleanup_runs_through_presence_when_alone_and_skips_while_another_is_alive() {
    let app_local_data = tempfile::tempdir().unwrap();
    let instances_dir = app_local_data.path().join("instances");
    let models_root = app_local_data.path().join("models");
    fs::create_dir_all(&instances_dir).unwrap();
    fs::create_dir_all(&models_root).unwrap();
    fs::write(instances_dir.join("orphan.db"), b"partial").unwrap();
    fs::write(instances_dir.join("orphan.db.pending"), b"").unwrap();
    let staging_dir = models_root.join("model-slug");
    fs::create_dir_all(&staging_dir).unwrap();
    fs::write(staging_dir.join("model.gguf.tmp"), b"partial").unwrap();

    let cleanup = |instances_dir: std::path::PathBuf, models_root: std::path::PathBuf| {
        move || {
            let _ = cleanup_orphans_in_dir(&instances_dir);
            let _ = cleanup_staging_in_dir(&models_root);
        }
    };

    // Alone: both leftovers are removed.
    let presence = ProcessPresence::announce(
        app_local_data.path(),
        cleanup(instances_dir.clone(), models_root.clone()),
    )
    .expect("announce alone");
    assert!(!instances_dir.join("orphan.db").exists());
    assert!(!instances_dir.join("orphan.db.pending").exists());
    assert!(!staging_dir.join("model.gguf.tmp").exists());

    // A second leftover pair appears, as a real overlapping second process's own half-finished
    // work would — the second announce below must leave it alone entirely.
    fs::write(instances_dir.join("orphan2.db"), b"partial").unwrap();
    fs::write(instances_dir.join("orphan2.db.pending"), b"").unwrap();

    let _second_presence = ProcessPresence::announce(
        app_local_data.path(),
        cleanup(instances_dir.clone(), models_root.clone()),
    )
    .expect("announce while another presence is alive");
    assert!(
        instances_dir.join("orphan2.db").exists(),
        "left alone while another presence is alive"
    );
    assert!(instances_dir.join("orphan2.db.pending").exists());

    drop(presence);
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
