//! Tests for Genesis publication — in particular that a vault is never
//! reported as created while its pending marker still exists.

use super::*;

#[test]
fn genesis_is_not_published_if_pending_marker_cannot_be_removed() {
    let tmp = tempfile::tempdir().unwrap();
    let db = open_new_database(
        "test-passphrase",
        &tmp.path().join("vault.db"),
        &installation_id_path(tmp.path()),
    )
    .unwrap();
    let marker = tmp.path().join("vault.db.pending");
    std::fs::create_dir(&marker).unwrap(); // remove_file deterministically fails
    let state = AppState::default();
    assert!(publish_active(&state, "vault", &db, &marker).is_err());
    assert!(state.active_name().unwrap().is_none());
}

#[test]
fn published_genesis_survives_startup_cleanup() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("vault.db");
    let db =
        open_new_database("test-passphrase", &path, &installation_id_path(tmp.path())).unwrap();
    let marker = get_pending_marker_path(&path);
    std::fs::write(&marker, "").unwrap();
    let state = AppState::default();
    publish_active(&state, "vault", &db, &marker).unwrap();
    assert!(!marker.exists());
    assert_eq!(
        super::super::startup::cleanup_orphans_in_dir(tmp.path()).unwrap(),
        0
    );
    assert!(path.is_file());
}
