use super::*;
use crate::storage::query;
use crate::sync::test_support::Device;

fn seen(device: &Device, who: &[u8; 32]) -> Option<i64> {
    query::read(device.db(), |r| {
        r.query_row(
            "SELECT last_seen FROM device_presence_no_sync WHERE device_pubkey = ?1",
            params![who.as_slice()],
            |row| row.get(0),
        )
    })
    .expect("read")
}

#[test]
fn a_time_only_moves_forward() {
    let d = Device::new();
    let other = [1u8; 32];

    assert!(touch(&d.replica, &other, 5_000).expect("first"));
    assert!(!touch(&d.replica, &other, 4_000).expect("older"));
    assert!(!touch(&d.replica, &other, 5_000).expect("same"));
    assert!(touch(&d.replica, &other, 6_000).expect("newer"));

    assert_eq!(seen(&d, &other), Some(6_000));
}

#[test]
fn a_report_raises_what_a_third_device_knew_and_never_this_devices_own_time() {
    let d = Device::new();
    let (own, a, b) = ([9u8; 32], [1u8; 32], [2u8; 32]);
    touch(&d.replica, &a, 5_000).expect("a");

    let moved = merge(
        &d.replica,
        &[(a, 8_000), (b, 3_000), (own, 99_000)],
        &own,
        10_000,
    )
    .expect("merge");

    assert!(moved);
    assert_eq!(seen(&d, &a), Some(8_000), "the newer report wins");
    assert_eq!(
        seen(&d, &b),
        Some(3_000),
        "a device only a report named is kept"
    );
    assert_eq!(
        seen(&d, &own),
        None,
        "this device is never taken from a report"
    );
}

#[test]
fn a_report_from_the_future_counts_as_now() {
    let d = Device::new();
    let (own, a) = ([9u8; 32], [1u8; 32]);

    merge(&d.replica, &[(a, 50_000_000)], &own, 10_000).expect("merge");

    assert_eq!(seen(&d, &a), Some(10_000));
}

#[test]
fn a_snapshot_names_every_known_time_and_this_device_as_now() {
    let d = Device::new();
    let (own, a) = ([9u8; 32], [1u8; 32]);
    touch(&d.replica, &a, 5_000).expect("a");
    touch(&d.replica, &own, 1_000).expect("stale own row");

    let mut snap = query::read(d.db(), |r| snapshot(r, &own, 20_000)).expect("snapshot");
    snap.sort();

    assert_eq!(snap, vec![(a, 5_000), (own, 20_000)]);
}
