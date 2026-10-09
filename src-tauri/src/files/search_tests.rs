use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use super::*;
use crate::files::kind::FileCategory;
use crate::files::local::OwnPlaces;

fn folder() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let real = std::fs::canonicalize(dir.path()).unwrap();
    (dir, real)
}

fn write(root: &Path, name: &str, bytes: usize) {
    let path = root.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, vec![b'x'; bytes]).unwrap();
}

/// Runs a search to its end; returns the names of the hits and the end.
fn search_all(root: &Path, query: &str, options: &SearchOptions) -> (Vec<String>, SearchEnd) {
    let mut names = Vec::new();
    let end = search(
        root,
        query,
        options,
        &CancellationToken::new(),
        &mut |batch| names.extend(batch.into_iter().map(|hit| hit.entry.name)),
    );
    names.sort();
    (names, end)
}

fn user() -> SearchOptions {
    SearchOptions {
        filters: SearchFilters::default(),
        show_hidden: false,
        own: OwnPlaces::default(),
        hide_own: false,
        limits: SearchLimits::USER,
    }
}

#[test]
fn a_small_typo_still_finds_the_file() {
    let (_dir, root) = folder();
    write(&root, "tief/drin/notiz.txt", 1);
    write(&root, "anderes.md", 1);
    let (names, end) = search_all(&root, "noitz", &user());
    assert_eq!(names, ["notiz.txt"]);
    assert_eq!(end, SearchEnd::Done { truncated: false });
}

#[test]
fn short_needles_allow_one_typo_longer_ones_two() {
    assert_eq!(max_typos("notz"), 1);
    assert_eq!(max_typos("noitz"), 1);
    assert_eq!(max_typos("urlaub"), 2);
    assert_eq!(max_typos("Ürlaub"), 2, "counted in characters, not bytes");
}

#[test]
fn folders_are_hits_too() {
    let (_dir, root) = folder();
    write(&root, "Urlaub/bild.jpg", 1);
    let (names, _) = search_all(&root, "urlaub", &user());
    assert_eq!(names, ["Urlaub"]);
}

#[test]
fn a_cancelled_search_stops() {
    let (_dir, root) = folder();
    for i in 0..50 {
        write(&root, &format!("d{i}/notiz{i}.txt"), 1);
    }
    let cancel = CancellationToken::new();
    cancel.cancel();
    let mut hits = 0;
    let end = search(&root, "notiz", &user(), &cancel, &mut |batch| {
        hits += batch.len()
    });
    assert_eq!(end, SearchEnd::Cancelled);
    assert_eq!(hits, 0);
}

#[cfg(unix)]
#[test]
fn a_link_loop_is_not_followed() {
    let (_dir, root) = folder();
    write(&root, "a/notiz.txt", 1);
    std::os::unix::fs::symlink(&root, root.join("a/kreis")).unwrap();
    let (names, end) = search_all(&root, "notiz", &user());
    assert_eq!(names, ["notiz.txt"]);
    assert_eq!(end, SearchEnd::Done { truncated: false });
}

#[cfg(unix)]
#[test]
fn a_folder_without_access_is_skipped_silently() {
    use std::os::unix::fs::PermissionsExt;
    let (_dir, root) = folder();
    write(&root, "zu/notiz-geheim.txt", 1);
    write(&root, "offen/notiz.txt", 1);
    std::fs::set_permissions(root.join("zu"), std::fs::Permissions::from_mode(0o000)).unwrap();
    let (names, end) = search_all(&root, "notiz", &user());
    std::fs::set_permissions(root.join("zu"), std::fs::Permissions::from_mode(0o755)).unwrap();
    // As root the folder stays readable; the search still ends without an error.
    assert!(names.contains(&"notiz.txt".to_owned()));
    assert_eq!(end, SearchEnd::Done { truncated: false });
}

#[test]
fn hidden_entries_count_only_when_shown() {
    let (_dir, root) = folder();
    write(&root, ".git/notiz.txt", 1);
    write(&root, ".notiz.txt", 1);
    write(&root, "notiz.txt", 1);
    let (names, _) = search_all(&root, "notiz", &user());
    assert_eq!(names, ["notiz.txt"]);
    let shown = SearchOptions {
        show_hidden: true,
        ..user()
    };
    let (names, _) = search_all(&root, "notiz", &shown);
    assert_eq!(names, [".notiz.txt", "notiz.txt", "notiz.txt"]);
}

#[test]
fn own_places_are_marked_for_the_user_and_left_out_for_agents() {
    let (_dir, root) = folder();
    write(&root, "eigen/notiz.txt", 1);
    write(&root, "frei/notiz.txt", 1);
    let own = OwnPlaces::new(vec![root.join("eigen")]);
    let user = SearchOptions {
        own: own.clone(),
        ..user()
    };
    let mut owned = Vec::new();
    search(
        &root,
        "notiz",
        &user,
        &CancellationToken::new(),
        &mut |batch| owned.extend(batch.into_iter().map(|hit| hit.entry.holzi_owned)),
    );
    owned.sort();
    assert_eq!(owned, [false, true]);
    let agent = SearchOptions {
        hide_own: true,
        ..user
    };
    let (names, _) = search_all(&root, "notiz", &agent);
    assert_eq!(names, ["notiz.txt"]);
}

#[test]
fn the_agent_limit_truncates_at_500_hits() {
    let (_dir, root) = folder();
    for i in 0..520 {
        write(&root, &format!("notiz-{i:03}.txt"), 1);
    }
    let options = SearchOptions {
        limits: SearchLimits::AGENT,
        ..user()
    };
    let (names, end) = search_all(&root, "notiz", &options);
    assert_eq!(names.len(), 500);
    assert_eq!(end, SearchEnd::Done { truncated: true });
}

#[test]
fn a_time_limit_truncates() {
    let (_dir, root) = folder();
    write(&root, "notiz.txt", 1);
    let options = SearchOptions {
        limits: SearchLimits {
            max_hits: 10,
            max_time: Some(Duration::ZERO),
        },
        ..user()
    };
    let (_, end) = search_all(&root, "notiz", &options);
    assert_eq!(end, SearchEnd::Done { truncated: true });
}

#[test]
fn filters_by_type_size_and_date() {
    let (_dir, root) = folder();
    write(&root, "urlaub.jpg", 2_000);
    write(&root, "urlaub-klein.jpg", 10);
    write(&root, "urlaub.txt", 2_000);
    write(&root, "urlaub/bericht.pdf", 2_000);
    let images = SearchOptions {
        filters: SearchFilters {
            types: Some(vec![FileCategory::Image]),
            ..SearchFilters::default()
        },
        ..user()
    };
    let (names, _) = search_all(&root, "urlaub", &images);
    assert_eq!(names, ["urlaub-klein.jpg", "urlaub.jpg"], "no folders");
    let large = SearchOptions {
        filters: SearchFilters {
            size_min: Some(1_000),
            ..SearchFilters::default()
        },
        ..user()
    };
    let (names, _) = search_all(&root, "urlaub", &large);
    assert_eq!(names, ["urlaub.jpg", "urlaub.txt"]);
    let future = SearchOptions {
        filters: SearchFilters {
            modified_from: Some(i64::MAX / 2),
            ..SearchFilters::default()
        },
        ..user()
    };
    let (names, _) = search_all(&root, "urlaub", &future);
    assert!(names.is_empty());
}

#[test]
fn an_empty_query_finds_nothing() {
    let (_dir, root) = folder();
    write(&root, "notiz.txt", 1);
    let (names, _) = search_all(&root, "  ", &user());
    assert!(names.is_empty());
}

/// SC-004: the first hit goes out while the walk runs, not with the next hit or at the end; the
/// files of a folder come before its sub folders.
#[test]
fn the_first_hit_goes_out_before_the_walk_ends() {
    let (_dir, root) = folder();
    for i in 0..200 {
        write(&root, &format!("a/d{i}/leer.bin"), 1);
    }
    write(&root, "notiz.txt", 1);
    let cancel = CancellationToken::new();
    let mut batches = Vec::new();
    let end = search(&root, "notiz", &user(), &cancel, &mut |batch| {
        batches.push(batch.len());
        cancel.cancel();
    });
    assert_eq!(batches, [1]);
    assert_eq!(end, SearchEnd::Cancelled, "the walk went on after the hit");
}
