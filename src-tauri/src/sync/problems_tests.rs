use super::*;
use crate::sync::test_support::Device;

fn device() -> [u8; 32] {
    [7; 32]
}

#[test]
fn a_problem_is_set_once_and_cleared_once() {
    let d = Device::new();

    assert!(set(&d.replica, &device(), Problem::Duplicate).expect("set"));
    assert!(
        !set(&d.replica, &device(), Problem::Duplicate).expect("set again"),
        "the same problem again changes nothing"
    );
    assert_eq!(
        query::read(d.db(), |r| of(r, &device())).expect("read"),
        Some(Problem::Duplicate)
    );

    assert!(clear(&d.replica, &device()).expect("clear"));
    assert!(!clear(&d.replica, &device()).expect("clear again"));
    assert_eq!(
        query::read(d.db(), |r| of(r, &device())).expect("read"),
        None
    );
}

#[test]
fn a_new_problem_replaces_the_old_one() {
    let d = Device::new();
    set(&d.replica, &device(), Problem::IncompatibleVersion).expect("set");

    assert!(set(&d.replica, &device(), Problem::Duplicate).expect("replace"));

    assert_eq!(
        query::read(d.db(), |r| of(r, &device())).expect("read"),
        Some(Problem::Duplicate)
    );
}

#[test]
fn a_fresh_meeting_does_not_clear_a_problem() {
    let d = Device::new();
    set(&d.replica, &device(), Problem::IncompatibleVersion).expect("set");

    d.db()
        .write(|tx| crate::sync::presence::record_seen(tx, &device(), 5, None))
        .expect("record");

    assert_eq!(
        query::read(d.db(), |r| of(r, &device())).expect("read"),
        Some(Problem::IncompatibleVersion)
    );
}

#[test]
fn a_duplicate_is_held_back_until_its_hold_lapses() {
    let watch = DuplicateWatch::default();
    assert!(!watch.holds(&device()));

    watch.note(device());
    assert!(watch.holds(&device()));

    // A hold that lapsed is forgotten on the next look.
    watch.seen.lock().expect("lock").insert(
        device(),
        Instant::now() - DUPLICATE_HOLD - Duration::from_secs(1),
    );
    assert!(!watch.holds(&device()));
    assert!(watch.seen.lock().expect("lock").is_empty());
}
