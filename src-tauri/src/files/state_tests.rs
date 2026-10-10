use std::sync::mpsc;
use std::time::Duration;

use super::*;
use crate::files::local::watch::watch_folder;

/// Long enough for the debounce and a slow CI machine.
const WAIT: Duration = Duration::from_secs(10);

fn state() -> (tempfile::TempDir, FilesState) {
    let dir = tempfile::tempdir().unwrap();
    let state = FilesState::new(
        OwnPlaces::default(),
        Vec::new(),
        dir.path().join("thumbnails"),
    );
    (dir, state)
}

#[test]
fn a_reload_ends_every_watch() {
    let (dir, files) = state();
    let real = std::fs::canonicalize(dir.path()).unwrap();
    let (sender, receiver) = mpsc::channel();
    let first = files.keep_watch(
        watch_folder(&real, false, move |changes| {
            let _ = sender.send(changes);
        })
        .unwrap(),
    );
    let second = files.keep_watch(watch_folder(&real, false, |_| {}).unwrap());

    files.unwatch_all();

    std::fs::write(real.join("neu.txt"), "x").unwrap();
    assert!(receiver.recv_timeout(WAIT).is_err());
    assert!(!files.unwatch(first));
    assert!(!files.unwatch(second));
}
