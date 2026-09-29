use serde_json::json;

use super::*;

fn column(value: serde_json::Value) -> ColumnChange {
    ColumnChange {
        table_name: "providers".to_string(),
        row_pks: r#"{"id":"p1"}"#.to_string(),
        column_name: "credentials".to_string(),
        hlc_timestamp: "7000000000000000000/1".to_string(),
        value,
        device_id: "scanner".to_string(),
        sig: None,
    }
}

#[test]
fn a_change_keeps_its_value_and_drops_the_scanning_device() {
    for value in [
        json!(null),
        json!(42),
        json!("text"),
        json!({"$blob_hex": "00ff10"}),
    ] {
        let original = column(value.clone());
        let back = Change::from_column(&original).to_column().expect("decode");
        assert_eq!(back.value, value);
        assert_eq!(back.hlc_timestamp, original.hlc_timestamp);
        assert_ne!(
            back.device_id, "scanner",
            "the scanning device never travels"
        );
    }
}

#[test]
fn a_split_value_joins_back_on_character_boundaries() {
    let text = "ä€𝄞 plain ".repeat(50);
    let change = Change::from_column(&column(json!(text)));
    let budget = change.wire_size() - change.value.len() + 17;

    let parts = change.clone().split(budget);

    assert!(parts.len() > 1);
    assert!(parts.iter().all(|p| p.wire_size() <= budget));
    assert!(parts[..parts.len() - 1].iter().all(|p| p.continues));
    assert!(!parts.last().expect("parts").continues);
    assert_eq!(join_parts(parts).expect("join"), vec![change]);
}

#[test]
fn a_broken_part_chain_is_rejected() {
    let change = Change::from_column(&column(json!("x".repeat(200))));
    let budget = change.wire_size() - change.value.len() + 50;
    let mut parts = change.split(budget);
    parts.pop();
    assert_eq!(join_parts(parts.clone()), Err(ChangeError::Value));

    let mut other = parts[0].clone();
    other.column = "name".to_string();
    parts.insert(1, other);
    assert_eq!(join_parts(parts), Err(ChangeError::Value));
}

#[test]
fn a_group_is_measured_like_a_local_write() {
    let group = vec![
        column(json!("abcd")),
        column(json!({"$blob_hex": "00ff10"})),
        column(json!(7)),
        column(json!(null)),
    ];
    assert_eq!(group_bytes(&group), Ok(4 + 3 + 8));
}

#[test]
fn a_page_survives_postcard() {
    let page = Page {
        changes: vec![Change::from_column(&column(json!({"$blob_hex": "ab"})))],
        group_continues: false,
        more: false,
        served: Vector::from([(uuid::Uuid::new_v4(), "7/1".to_string())]),
    };
    let bytes = postcard::to_stdvec(&page).expect("encode");
    assert_eq!(postcard::from_bytes::<Page>(&bytes).expect("decode"), page);
}
