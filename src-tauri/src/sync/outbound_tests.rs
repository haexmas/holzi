use serde_json::json;
use uuid::Uuid;

use super::*;
use crate::sync::change::join_parts;

/// An HLC of `origin` at `time`, in haex-crdt's format.
fn hlc(time: u64, origin: Uuid) -> String {
    let node = u128::from_le_bytes(*origin.as_bytes());
    format!("{time}/{node:x}")
}

fn cell(time: u64, origin: Uuid, column: &str, value: &str) -> ColumnChange {
    ColumnChange {
        table_name: "chat_threads".to_string(),
        row_pks: format!(r#"{{"id":"{time}"}}"#),
        column_name: column.to_string(),
        hlc_timestamp: hlc(time, origin),
        value: json!(value),
        device_id: String::new(),
        sig: None,
    }
}

fn drain(mut outbox: Outbox) -> Vec<Page> {
    std::iter::from_fn(|| outbox.next_page()).collect()
}

#[test]
fn cells_are_sorted_by_hlc_across_origins() {
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    let changes = vec![
        cell(30, a, "title", "a3"),
        cell(10, b, "title", "b1"),
        cell(20, a, "title", "a2"),
    ];

    let pages = drain(Outbox::new(changes, Vector::new(), PAGE_BUDGET));

    assert_eq!(pages.len(), 1);
    let times: Vec<&str> = pages[0].changes.iter().map(|c| c.value.as_str()).collect();
    assert_eq!(times, [r#""b1""#, r#""a2""#, r#""a3""#]);
}

#[test]
fn pages_never_share_a_group() {
    let a = Uuid::new_v4();
    let changes: Vec<ColumnChange> = (1..=6)
        .flat_map(|t| [cell(t, a, "title", "x"), cell(t, a, "created_at", "y")])
        .collect();
    let group_size: usize = changes[..2]
        .iter()
        .map(|c| Change::from_column(c).wire_size())
        .sum();

    let pages = drain(Outbox::new(changes, Vector::new(), group_size * 2 + 1));

    assert_eq!(pages.len(), 3);
    for (i, page) in pages.iter().enumerate() {
        assert_eq!(page.changes.len(), 4);
        assert!(!page.group_continues);
        assert_eq!(page.more, i < 2);
    }
}

#[test]
fn a_group_larger_than_a_page_continues_over_pages() {
    let a = Uuid::new_v4();
    let mut changes: Vec<ColumnChange> =
        (0..5).map(|i| cell(10, a, &format!("c{i}"), "v")).collect();
    changes.push(cell(20, a, "c9", "v"));
    let one = Change::from_column(&changes[0]).wire_size();

    let pages = drain(Outbox::new(changes, Vector::new(), one * 2));

    let continues: Vec<bool> = pages.iter().map(|p| p.group_continues).collect();
    assert_eq!(continues, [true, true, false]);
    assert_eq!(pages[2].changes.last().expect("changes").hlc, hlc(20, a));
}

#[test]
fn a_value_larger_than_a_page_travels_in_parts() {
    let a = Uuid::new_v4();
    let big = "z".repeat(1000);
    let changes = vec![cell(10, a, "title", &big)];
    let budget = 200;

    let pages = drain(Outbox::new(changes.clone(), Vector::new(), budget));

    assert!(pages.len() > 1);
    assert!(pages[..pages.len() - 1].iter().all(|p| p.group_continues));
    let parts: Vec<Change> = pages.into_iter().flat_map(|p| p.changes).collect();
    assert!(parts.iter().all(|p| p.wire_size() <= budget));
    assert_eq!(
        join_parts(parts).expect("join"),
        vec![Change::from_column(&changes[0])]
    );
}

#[test]
fn only_the_last_page_names_the_served_progress() {
    let a = Uuid::new_v4();
    let served = Vector::from([(a, hlc(20, a))]);
    let changes: Vec<ColumnChange> = (1..=3).map(|t| cell(t, a, "title", "x")).collect();
    let one = Change::from_column(&changes[0]).wire_size();

    let pages = drain(Outbox::new(changes, served.clone(), one));

    assert_eq!(pages.len(), 3);
    assert!(pages[..2].iter().all(|p| p.served.is_empty()));
    assert_eq!(pages[2].served, served);

    let empty = drain(Outbox::new(Vec::new(), served.clone(), one));
    assert_eq!(empty.len(), 1);
    assert!(!empty[0].more && empty[0].changes.is_empty());
    assert_eq!(empty[0].served, served);
}

#[test]
fn a_cell_is_wanted_beyond_its_origins_cursor_and_up_to_the_served_progress() {
    let (a, b, legacy) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let theirs = Vector::from([(a, hlc(10, a))]);
    let served = Vector::from([(a, hlc(20, a)), (b, hlc(20, b))]);

    assert!(
        !wanted(&cell(10, a, "t", "x"), &theirs, &served),
        "at the cursor"
    );
    assert!(wanted(&cell(11, a, "t", "x"), &theirs, &served));
    assert!(
        !wanted(&cell(21, a, "t", "x"), &theirs, &served),
        "beyond what was served"
    );
    assert!(
        wanted(&cell(1, b, "t", "x"), &theirs, &served),
        "an origin they lack"
    );
    assert!(
        wanted(&cell(1, legacy, "t", "x"), &theirs, &served),
        "data without progress"
    );
}

#[test]
fn the_scan_starts_at_the_smallest_cursor_unless_an_origin_is_missing() {
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    let served = Vector::from([(a, hlc(30, a)), (b, hlc(30, b))]);
    let complete = Vector::from([(a, hlc(20, a)), (b, hlc(10, b))]);
    assert_eq!(scan_cursor(&complete, &served), Some(hlc(10, b)));
    let partial = Vector::from([(a, hlc(20, a))]);
    assert_eq!(scan_cursor(&partial, &served), None);
}
