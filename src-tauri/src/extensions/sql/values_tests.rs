use serde_json::json;

use super::*;

#[test]
fn parameters_become_sqlite_values() {
    assert_eq!(
        params(Some(
            &json!([null, true, false, 7, 1.5, "x", {"$bytes": "AAEC"}, [1, 2], {"a": 1}])
        ))
        .unwrap(),
        vec![
            SqlValue::Null,
            SqlValue::Integer(1),
            SqlValue::Integer(0),
            SqlValue::Integer(7),
            SqlValue::Real(1.5),
            SqlValue::Text("x".into()),
            SqlValue::Blob(vec![0, 1, 2]),
            SqlValue::Text("[1,2]".into()),
            SqlValue::Text("{\"a\":1}".into()),
        ]
    );
    assert!(params(None).unwrap().is_empty());
    assert!(params(Some(&json!({"a": 1}))).is_err());
    assert!(params(Some(&json!([{"$bytes": "not base64!"}]))).is_err());
}

#[test]
fn results_become_json_with_blobs_as_base64_and_no_nan() {
    assert_eq!(to_json(ValueRef::Integer(5)).0, json!(5));
    assert_eq!(to_json(ValueRef::Real(f64::NAN)).0, Value::Null);
    assert_eq!(to_json(ValueRef::Real(f64::INFINITY)).0, Value::Null);
    assert_eq!(to_json(ValueRef::Text(b"hi")).0, json!("hi"));
    assert_eq!(to_json(ValueRef::Blob(&[0, 1, 2])).0, json!("AAEC"));
    assert_eq!(to_json(ValueRef::Null).0, Value::Null);
}

#[test]
fn statement_entries_are_sql_and_optional_params() {
    let entry = json!(["SELECT ?", [1]]);
    let (sql, p) = statement_entry(&entry).unwrap();
    assert_eq!(sql, "SELECT ?");
    assert_eq!(p, Some(&json!([1])));
    assert!(statement_entry(&json!("SELECT 1")).is_err());
    assert!(statement_entry(&json!([1])).is_err());
}
