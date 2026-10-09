use std::path::Path;

use super::*;

#[test]
fn a_relative_path_is_refused() {
    assert!(resolve(Path::new("notes.txt")).is_err());
}

#[test]
fn an_existing_path_resolves_to_its_real_target() {
    let dir = tempfile::tempdir().unwrap();
    let real = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::create_dir(real.join("a")).unwrap();
    let spelled = format!("{}/a/../a/./", real.display());
    assert_eq!(resolve(Path::new(&spelled)).unwrap(), real.join("a"));
}

#[test]
fn a_missing_target_resolves_through_its_nearest_existing_ancestor() {
    let dir = tempfile::tempdir().unwrap();
    let real = std::fs::canonicalize(dir.path()).unwrap();
    let missing = format!("{}/new/deeper/file.txt", real.display());
    assert_eq!(
        resolve(Path::new(&missing)).unwrap(),
        real.join("new").join("deeper").join("file.txt")
    );
    assert!(resolve(Path::new(&format!("{}/new/../escape.txt", real.display()))).is_err());
}

#[cfg(unix)]
#[test]
fn a_broken_link_is_refused_anywhere_in_the_path() {
    let dir = tempfile::tempdir().unwrap();
    let real = std::fs::canonicalize(dir.path()).unwrap();
    let elsewhere = real.join("elsewhere");
    std::os::unix::fs::symlink(elsewhere.join("vault.db"), real.join("file-link")).unwrap();
    std::os::unix::fs::symlink(elsewhere.join("folder"), real.join("folder-link")).unwrap();
    std::os::unix::fs::symlink(real.join("loop"), real.join("loop")).unwrap();
    for path in ["file-link", "folder-link/new.txt", "loop", "loop/new.txt"] {
        assert!(
            resolve(Path::new(&format!("{}/{path}", real.display()))).is_err(),
            "{path}"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_link_resolves_to_where_it_points() {
    let dir = tempfile::tempdir().unwrap();
    let real = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::create_dir(real.join("elsewhere")).unwrap();
    std::os::unix::fs::symlink(real.join("elsewhere"), real.join("link")).unwrap();
    assert_eq!(
        resolve(Path::new(&format!("{}/link/new.txt", real.display()))).unwrap(),
        real.join("elsewhere").join("new.txt")
    );
}

#[cfg(unix)]
#[test]
fn an_entry_keeps_its_own_name_even_when_it_is_a_link() {
    let dir = tempfile::tempdir().unwrap();
    let real = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::create_dir(real.join("target")).unwrap();
    std::os::unix::fs::symlink(real.join("target"), real.join("link")).unwrap();
    let spelled = format!("{}/target/../link", real.display());
    assert_eq!(
        resolve_entry(Path::new(&spelled)).unwrap(),
        real.join("link")
    );
    assert_eq!(resolve(Path::new(&spelled)).unwrap(), real.join("target"));
}

#[test]
fn an_entry_must_have_a_name() {
    assert!(resolve_entry(Path::new("/")).is_err());
    assert!(resolve_entry(Path::new("/tmp/..")).is_err());
    assert!(resolve_entry(Path::new("relative")).is_err());
}
