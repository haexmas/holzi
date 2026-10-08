use super::*;

#[test]
fn every_file_of_the_vault_goes_and_others_stay() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("anna.db");
    for name in [
        "anna.db",
        "anna.db.pending",
        "anna.db.lock",
        "anna.db-wal",
        "anna.db-shm",
        "anna.db.vault-id",
        "anna.db.1f2e.tmp",
        "anna-2.db",
        "anna-2.db.vault-id",
        "anna.dbx",
    ] {
        std::fs::write(dir.path().join(name), b"x").unwrap();
    }

    assert!(remove_vault_files(&db));

    let mut left: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    left.sort();
    assert_eq!(left, ["anna-2.db", "anna-2.db.vault-id", "anna.dbx"]);
}

#[test]
fn missing_files_are_no_failure() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("b.db.pending"), b"").unwrap();
    assert!(remove_vault_files(&dir.path().join("b.db")));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn the_marker_stays_while_a_file_of_the_vault_cannot_go() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("c.db.pending"), b"").unwrap();
    // A folder where the database file should be: removing it as a file fails.
    std::fs::create_dir(dir.path().join("c.db")).unwrap();
    assert!(!remove_vault_files(&dir.path().join("c.db")));
    assert!(dir.path().join("c.db.pending").exists());
}
