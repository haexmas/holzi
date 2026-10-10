use super::{fits, free_bytes};

const MB: u64 = 1_000_000;

#[test]
fn a_download_fits_with_a_tenth_of_its_size_to_spare() {
    assert!(fits(Some(1000 * MB), Some(1100 * MB)));
    assert!(fits(Some(1000 * MB), Some(5000 * MB)));
}

#[test]
fn a_download_without_the_reserve_does_not_fit() {
    assert!(!fits(Some(1000 * MB), Some(1099 * MB)));
    assert!(!fits(Some(1000 * MB), Some(500 * MB)));
    assert!(!fits(Some(1), Some(0)));
}

#[test]
fn without_a_size_or_the_free_space_there_is_nothing_to_warn_about() {
    assert!(fits(None, Some(0)));
    assert!(fits(Some(u64::MAX), None));
    assert!(fits(None, None));
}

#[test]
fn a_huge_size_does_not_overflow() {
    assert!(!fits(Some(u64::MAX), Some(u64::MAX - 1)));
}

#[test]
fn the_free_space_of_an_existing_folder_is_known() {
    let dir = tempfile::tempdir().unwrap();
    assert!(free_bytes(dir.path()).is_some());
}

#[test]
fn a_missing_folder_has_no_free_space() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(free_bytes(&dir.path().join("missing")), None);
}
