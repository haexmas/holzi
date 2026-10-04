use super::*;

#[test]
fn a_relative_path_is_refused() {
    assert!(resolve("notes.txt").is_err());
}

#[test]
fn an_existing_path_resolves_to_its_real_target() {
    let dir = tempfile::tempdir().unwrap();
    let real = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::create_dir(real.join("a")).unwrap();
    let spelled = format!("{}/a/../a/./", real.display());
    assert_eq!(resolve(&spelled).unwrap(), real.join("a"));
}

#[test]
fn a_missing_target_resolves_through_its_nearest_existing_ancestor() {
    let dir = tempfile::tempdir().unwrap();
    let real = std::fs::canonicalize(dir.path()).unwrap();
    let missing = format!("{}/new/deeper/file.txt", real.display());
    assert_eq!(
        resolve(&missing).unwrap(),
        real.join("new").join("deeper").join("file.txt")
    );
    assert!(resolve(&format!("{}/new/../escape.txt", real.display())).is_err());
}

#[cfg(unix)]
#[test]
fn a_link_resolves_to_where_it_points() {
    let dir = tempfile::tempdir().unwrap();
    let real = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::create_dir(real.join("elsewhere")).unwrap();
    std::os::unix::fs::symlink(real.join("elsewhere"), real.join("link")).unwrap();
    assert_eq!(
        resolve(&format!("{}/link/new.txt", real.display())).unwrap(),
        real.join("elsewhere").join("new.txt")
    );
}
