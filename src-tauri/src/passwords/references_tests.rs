//! Spec 036 (`contracts/references.md`): the grammar, the resolution with depth and cycles, and
//! the check on save, against the vectors in `tests/fixtures/reference_vectors.json`.

use std::collections::HashMap;
use std::convert::Infallible;

use serde_json::Value;

use super::references::{
    build_token, find, find_cycle, resolve, utf16_offset, Field, Lookup, RefKind, ReferenceError,
    MAX_REFERENCE_DEPTH,
};

const VECTORS: &str = include_str!("../../tests/fixtures/reference_vectors.json");

fn vectors() -> Value {
    serde_json::from_str(VECTORS).expect("vectors")
}

/// The UUID a name in the vectors stands for; `UPPER` is written in upper case.
fn uuid(name: &str) -> String {
    if name == "UPPER" {
        return "ABCDEF01-0000-4000-8000-000000000000".to_string();
    }
    let n = name
        .strip_prefix('I')
        .and_then(|rest| rest.parse::<u32>().ok())
        .map_or_else(|| u32::from(name.as_bytes()[0]), |n| 1000 + n);
    format!("{n:08x}-0000-4000-8000-000000000000")
}

/// `<A>` … in a vector text replaced by the UUIDs.
fn expand(text: &str) -> String {
    let mut out = text.to_string();
    while let Some(start) = out.find('<') {
        let end = out[start..].find('>').expect("closing >") + start;
        let name = out[start + 1..end].to_string();
        out.replace_range(start..=end, &uuid(&name));
    }
    out
}

fn kind_of(kind: &str, key: Option<&str>) -> RefKind {
    match kind {
        "username" => RefKind::Username,
        "password" => RefKind::Password,
        _ => RefKind::Extra(key.expect("key").to_string()),
    }
}

fn field_of(name: &str) -> Field {
    match name {
        "username" => Field::Username,
        "password" => Field::Password,
        "url" => Field::Url,
        "note" => Field::Note,
        other => Field::Extra(other.strip_prefix("extra:").expect("extra:").to_string()),
    }
}

fn field_name(kind: &RefKind) -> String {
    match kind {
        RefKind::Username => "username".into(),
        RefKind::Password => "password".into(),
        RefKind::Extra(key) => format!("extra:{key}"),
    }
}

/// Items as `uuid → field → raw text` from a vector object.
fn items(value: &Value) -> HashMap<String, HashMap<String, String>> {
    value
        .as_object()
        .map(|object| {
            object
                .iter()
                .map(|(name, fields)| {
                    let fields = fields
                        .as_object()
                        .expect("fields")
                        .iter()
                        .map(|(field, text)| (field.clone(), expand(text.as_str().expect("text"))))
                        .collect();
                    (uuid(name), fields)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn error_of(name: &str) -> ReferenceError {
    match name {
        "missing" => ReferenceError::Missing,
        "cycle" => ReferenceError::Cycle,
        "tooDeep" => ReferenceError::TooDeep,
        other => panic!("unknown error {other}"),
    }
}

fn resolve_in(
    items: &HashMap<String, HashMap<String, String>>,
    item: &str,
    field: &str,
) -> Result<String, ReferenceError> {
    let text = items[item][field].clone();
    let mut lookup = |id: &str, kind: &RefKind| -> Result<Lookup, Infallible> {
        let Some(fields) = items.get(id) else {
            return Ok(Lookup::Missing);
        };
        if fields.contains_key("hidden") {
            return Ok(Lookup::Missing);
        }
        Ok(fields
            .get(&field_name(kind))
            .map_or(Lookup::Missing, |raw| Lookup::Value(raw.clone())))
    };
    match resolve(&text, (item.to_string(), field_of(field)), &mut lookup) {
        Ok(result) => result,
        Err(never) => match never {},
    }
}

#[test]
fn find_follows_the_grammar() {
    for case in vectors()["find"].as_array().expect("find") {
        let text = expand(case["text"].as_str().expect("text"));
        let found: Vec<(String, RefKind)> = find(&text)
            .into_iter()
            .map(|hit| (hit.item_id, hit.kind))
            .collect();
        let expected: Vec<(String, RefKind)> = case["found"]
            .as_array()
            .expect("found")
            .iter()
            .map(|hit| {
                (
                    uuid(hit[0].as_str().expect("item")).to_ascii_lowercase(),
                    kind_of(hit[1].as_str().expect("kind"), hit[2].as_str()),
                )
            })
            .collect();
        assert_eq!(found, expected, "{text}");
    }
}

#[test]
fn a_found_range_covers_the_placeholder() {
    let text = expand("admin-{$<A>:extra:a\\}b}!");
    let hit = &find(&text)[0];
    assert_eq!(&text[hit.start..hit.end], &text[6..text.len() - 1]);
    assert_eq!(utf16_offset("äb{$", 3), 2);
}

#[test]
fn tokens_round_trip_with_escaping() {
    for case in vectors()["tokens"].as_array().expect("tokens") {
        let item = uuid(case["item"].as_str().expect("item"));
        let item = if case["item"] == "not-a-uuid" {
            "not-a-uuid".to_string()
        } else {
            item
        };
        let kind = kind_of(case["kind"].as_str().expect("kind"), case["key"].as_str());
        let token = build_token(&item, &kind);
        let expected = case["token"].as_str().map(expand);
        assert_eq!(token, expected, "{case}");
        if let Some(token) = token {
            let found = find(&token);
            assert_eq!(found.len(), 1);
            assert_eq!(found[0].kind, kind);
            assert_eq!((found[0].start, found[0].end), (0, token.len()));
        }
    }
}

#[test]
fn resolution_follows_the_vectors() {
    for case in vectors()["resolve"].as_array().expect("resolve") {
        let items = items(&case["items"]);
        let read = &case["read"];
        let result = resolve_in(
            &items,
            &uuid(read[0].as_str().expect("item")),
            read[1].as_str().expect("field"),
        );
        let expected = match case["value"].as_str() {
            Some(value) => Ok(value.to_string()),
            None => Err(error_of(case["error"].as_str().expect("error"))),
        };
        assert_eq!(result, expected, "{}", case["name"]);
    }
}

#[test]
fn a_chain_of_twelve_resolves_and_thirteen_is_too_deep() {
    assert_eq!(MAX_REFERENCE_DEPTH, 12);
    for case in vectors()["chains"].as_array().expect("chains") {
        let length = case["length"].as_u64().expect("length") as u32;
        let mut items = HashMap::new();
        for i in 0..length {
            let next = format!("{{$<I{}>:password}}", i + 1);
            items.insert(
                uuid(&format!("I{i}")),
                HashMap::from([("password".to_string(), expand(&next))]),
            );
        }
        items.insert(
            uuid(&format!("I{length}")),
            HashMap::from([("password".to_string(), "end".to_string())]),
        );
        let result = resolve_in(&items, &uuid("I0"), "password");
        let expected = match case["value"].as_str() {
            Some(value) => Ok(value.to_string()),
            None => Err(error_of(case["error"].as_str().expect("error"))),
        };
        assert_eq!(result, expected, "length {length}");
    }
}

#[test]
fn the_check_on_save_follows_the_vectors() {
    for case in vectors()["cycles"].as_array().expect("cycles") {
        let item = uuid(case["item"].as_str().expect("item"));
        let new: Vec<(Field, String)> = case["new"]
            .as_object()
            .expect("new")
            .iter()
            .map(|(field, text)| (field_of(field), expand(text.as_str().expect("text"))))
            .collect();
        let stored = items(&case["stored"]);
        let mut lookup = |id: &str, kind: &RefKind| -> Result<Option<String>, Infallible> {
            Ok(stored
                .get(id)
                .and_then(|fields| fields.get(&field_name(kind)).cloned()))
        };
        let source = match find_cycle(&item, &new, &mut lookup) {
            Ok(source) => source,
            Err(never) => match never {},
        };
        let expected = case["source"].as_str().map(uuid);
        assert_eq!(source, expected, "{}", case["name"]);
    }
}

#[test]
fn a_cycle_that_closes_on_step_thirteen_is_found_on_save() {
    // A → I1 → … → I13 → A: the resolution would stop at the depth, the check on save does not.
    let a = uuid("A");
    let mut stored: HashMap<String, String> = HashMap::new();
    for i in 1..13 {
        stored.insert(
            uuid(&format!("I{i}")),
            expand(&format!("{{$<I{}>:password}}", i + 1)),
        );
    }
    stored.insert(uuid("I13"), expand("{$<A>:password}"));
    let mut lookup = |id: &str, _: &RefKind| -> Result<Option<String>, Infallible> {
        Ok(stored.get(id).cloned())
    };
    let new = vec![(Field::Password, expand("{$<I1>:password}"))];
    let found = find_cycle(&a, &new, &mut lookup).unwrap_or_else(|never| match never {});
    assert_eq!(found, Some(uuid("I1")));
}
