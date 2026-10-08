use super::*;

#[test]
fn the_name_comes_from_the_file_name() {
    assert_eq!(name_for("Anna's Tresor.db"), "Anna-s-Tresor");
    assert_eq!(name_for("arbeit.db"), "arbeit");
    assert_eq!(name_for("_.hidden.db"), "hidden");
    assert_eq!(name_for("…"), "vault");
    assert_eq!(name_for(&format!("{}.db", "a".repeat(80))).len(), 64);
}

#[test]
fn a_taken_name_gets_a_number_within_the_length() {
    assert_eq!(candidate("anna", 1), "anna");
    assert_eq!(candidate("anna", 2), "anna-2");
    let long = "a".repeat(64);
    assert_eq!(candidate(&long, 12).len(), 64);
    assert!(candidate(&long, 12).ends_with("-12"));
}

#[test]
fn reserving_skips_names_with_a_file_or_a_marker() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("anna.db"), b"").unwrap();
    std::fs::write(dir.path().join("anna-2.db.pending"), b"").unwrap();
    let (name, path) = reserve_name(dir.path(), "anna").unwrap();
    assert_eq!(name, "anna-3");
    assert_eq!(path, dir.path().join("anna-3.db"));
    assert!(dir.path().join("anna-3.db.pending").exists());
}
