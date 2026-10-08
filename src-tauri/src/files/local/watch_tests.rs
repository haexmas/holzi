use std::sync::mpsc;
use std::time::Duration;

use super::*;

/// Long enough for the debounce and a slow CI machine.
const WAIT: Duration = Duration::from_secs(10);

fn real_tempdir() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let real = std::fs::canonicalize(dir.path()).unwrap();
    (dir, real)
}

#[test]
fn a_new_file_in_the_folder_is_reported() {
    let (_dir, real) = real_tempdir();
    let (sender, receiver) = mpsc::channel();
    let _watch = watch_folder(&real, false, move |changes| {
        let _ = sender.send(changes);
    })
    .unwrap();
    std::fs::write(real.join("neu.txt"), "x").unwrap();
    let changes = receiver.recv_timeout(WAIT).unwrap();
    assert!(changes
        .iter()
        .any(|change| change.path == real.join("neu.txt")));
}

#[test]
fn a_non_recursive_watch_ignores_deeper_folders() {
    let (_dir, real) = real_tempdir();
    std::fs::create_dir(real.join("sub")).unwrap();
    let (sender, receiver) = mpsc::channel();
    let _watch = watch_folder(&real, false, move |changes| {
        let _ = sender.send(changes);
    })
    .unwrap();
    std::fs::write(real.join("sub").join("deep.txt"), "x").unwrap();
    std::fs::write(real.join("top.txt"), "x").unwrap();
    let mut seen = Vec::new();
    while let Ok(changes) = receiver.recv_timeout(Duration::from_secs(3)) {
        seen.extend(changes);
    }
    assert!(seen.iter().any(|c| c.path == real.join("top.txt")));
    assert!(!seen
        .iter()
        .any(|c| c.path == real.join("sub").join("deep.txt")));
}

#[test]
fn an_ended_watch_reports_nothing() {
    let (_dir, real) = real_tempdir();
    let (sender, receiver) = mpsc::channel();
    let watch = watch_folder(&real, true, move |changes| {
        let _ = sender.send(changes);
    })
    .unwrap();
    drop(watch);
    std::fs::write(real.join("late.txt"), "x").unwrap();
    assert!(receiver.recv_timeout(Duration::from_secs(2)).is_err());
}

#[test]
fn change_kinds_keep_the_names_extensions_see() {
    assert_eq!(ChangeKind::Created.as_str(), "created");
    assert_eq!(ChangeKind::Modified.as_str(), "modified");
    assert_eq!(ChangeKind::Removed.as_str(), "removed");
    assert_eq!(ChangeKind::Other.as_str(), "any");
}
