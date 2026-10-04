//! Spec 036, FR-050 (research R13): KeePass references on import.

use super::references::{convert, Conversion, ImportField};
use super::ImportItem;
use crate::passwords::model::KeyValueInput;

const KONTO_REF: &str = "0123456789abcdef0123456789abcdef";

fn item(title: &str) -> ImportItem {
    ImportItem {
        title: Some(title.to_string()),
        ..ImportItem::default()
    }
}

fn ids(n: usize) -> Vec<Option<String>> {
    (0..n)
        .map(|i| Some(format!("{i:08x}-0000-4000-8000-000000000000")))
        .collect()
}

fn rewrite(conversion: &Conversion, index: usize, field: ImportField) -> Option<&str> {
    conversion
        .rewrites
        .iter()
        .find(|(i, f, _)| *i == index && *f == field)
        .map(|(_, _, text)| text.as_str())
}

#[test]
fn a_reference_by_id_becomes_a_placeholder() {
    let mut konto = item("Konto");
    konto.source_ref = Some("01234567-89AB-CDEF-0123-456789ABCDEF".to_string());
    let mut zweit = item("Zweit");
    zweit.password = Some(format!("{{REF:P@I:{}}}", KONTO_REF.to_uppercase()));
    zweit.username = Some(format!("pre-{{ref:u@i:{KONTO_REF}}}"));
    let ids = ids(2);
    let conversion = convert(&[konto, zweit], &ids);
    let konto_id = ids[0].as_deref().expect("id");
    assert_eq!(
        rewrite(&conversion, 1, ImportField::Password),
        Some(format!("{{${konto_id}:password}}").as_str())
    );
    assert_eq!(
        rewrite(&conversion, 1, ImportField::Username),
        Some(format!("pre-{{${konto_id}:username}}").as_str())
    );
    assert_eq!((conversion.converted, conversion.left_as_text), (2, 0));
}

#[test]
fn a_search_by_text_converts_only_with_exactly_one_match() {
    let mut a = item("Mail Arbeit");
    a.username = Some("anna".into());
    let b = item("Mail Privat");
    let mut c = item("Bank");
    c.note = Some("{REF:U@T:arbeit}".into());
    c.password = Some("{REF:P@T:Mail}".into());
    let ids = ids(3);
    let conversion = convert(&[a, b, c], &ids);
    let a_id = ids[0].as_deref().expect("id");
    assert_eq!(
        rewrite(&conversion, 2, ImportField::Note),
        Some(format!("{{${a_id}:username}}").as_str())
    );
    // Two titles contain "Mail": ambiguous, stays text.
    assert_eq!(rewrite(&conversion, 2, ImportField::Password), None);
    assert_eq!((conversion.converted, conversion.left_as_text), (1, 1));
}

#[test]
fn other_wanted_fields_and_unknown_searches_stay_text() {
    let mut a = item("Konto");
    a.source_ref = Some(KONTO_REF.into());
    let mut b = item("Ziel");
    b.key_values = vec![KeyValueInput {
        key: "Felder".into(),
        value: Some(format!(
            "{{REF:T@I:{KONTO_REF}}}{{REF:A@I:{KONTO_REF}}}{{REF:N@I:{KONTO_REF}}}{{REF:I@I:{KONTO_REF}}}{{REF:O@I:{KONTO_REF}}}{{REF:P@X:1}}"
        )),
    }];
    let conversion = convert(&[a, b], &ids(2));
    assert!(conversion.rewrites.is_empty());
    assert_eq!((conversion.converted, conversion.left_as_text), (0, 6));
}

#[test]
fn a_source_skipped_as_duplicate_leaves_the_reference_as_text() {
    let mut a = item("Konto");
    a.source_ref = Some(KONTO_REF.into());
    let mut b = item("Ziel");
    b.password = Some(format!("{{REF:P@I:{KONTO_REF}}}"));
    let mut ids = ids(2);
    ids[0] = None;
    let conversion = convert(&[a, b], &ids);
    assert!(conversion.rewrites.is_empty());
    assert_eq!((conversion.converted, conversion.left_as_text), (0, 1));
}

#[test]
fn a_reference_in_a_custom_field_converts_too() {
    let mut a = item("Konto");
    a.source_ref = Some(KONTO_REF.into());
    let mut b = item("Ziel");
    b.key_values = vec![KeyValueInput {
        key: "PIN".into(),
        value: Some(format!("{{REF:P@I:{KONTO_REF}}}")),
    }];
    let ids = ids(2);
    let conversion = convert(&[a, b], &ids);
    assert_eq!(
        rewrite(&conversion, 1, ImportField::KeyValue(0)),
        Some(format!("{{${}:password}}", ids[0].as_deref().expect("id")).as_str())
    );
}
