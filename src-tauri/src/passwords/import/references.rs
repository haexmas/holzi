//! KeePass references on import (spec 036, FR-050, research R13, `contracts/references.md`
//! §KeePass-Import): `{REF:<Feld>@<Suche>:<Text>}` becomes a holzi placeholder when it asks for the
//! user name (`U`) or the password (`P`) of exactly one entry of the file that is imported too.
//! Searching by id (`I`, 32 hex digits) or by text in the title, user name, password, address, notes
//! or a custom field (`T`, `U`, `P`, `A`, `N`, `O`, case ignored) is understood; anything else, an
//! ambiguous or missing match and a source skipped as a duplicate stay text and are counted. Pure.

use super::ImportItem;
use crate::passwords::references::{build_token, RefKind};

/// A text field of an imported entry that may hold references.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportField {
    Username,
    Password,
    Url,
    Note,
    /// The custom field at this position.
    KeyValue(usize),
}

/// The new texts and the counts of a conversion.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Conversion {
    /// Entry (position in the model), field and its new text.
    pub rewrites: Vec<(usize, ImportField, String)>,
    pub converted: u32,
    pub left_as_text: u32,
}

/// One `{REF:…}` in a text: its byte range, the wanted field letter, the search letter and text.
struct KeepassRef<'a> {
    start: usize,
    end: usize,
    wanted: char,
    search: char,
    text: &'a str,
}

fn find_refs(text: &str) -> Vec<KeepassRef<'_>> {
    let mut found = Vec::new();
    let upper = text.to_ascii_uppercase();
    let mut from = 0;
    while let Some(offset) = upper[from..].find("{REF:") {
        let start = from + offset;
        let body_start = start + 5;
        let Some(close) = text[body_start..].find('}') else {
            break;
        };
        let body = &text[body_start..body_start + close];
        let mut chars = body.chars();
        let parsed = match (chars.next(), chars.next(), chars.next(), chars.next()) {
            (Some(wanted), Some('@'), Some(search), Some(':')) => Some(KeepassRef {
                start,
                end: body_start + close + 1,
                wanted: wanted.to_ascii_uppercase(),
                search: search.to_ascii_uppercase(),
                text: &body[4..],
            }),
            _ => None,
        };
        match parsed {
            Some(reference) => {
                from = reference.end;
                found.push(reference);
            }
            None => from = start + 1,
        }
    }
    found
}

fn texts(item: &ImportItem) -> Vec<(ImportField, &str)> {
    let mut out = Vec::new();
    for (field, text) in [
        (ImportField::Username, &item.username),
        (ImportField::Password, &item.password),
        (ImportField::Url, &item.url),
        (ImportField::Note, &item.note),
    ] {
        if let Some(text) = text {
            out.push((field, text.as_str()));
        }
    }
    for (index, field) in item.key_values.iter().enumerate() {
        if let Some(value) = &field.value {
            out.push((ImportField::KeyValue(index), value.as_str()));
        }
    }
    out
}

fn contains(haystack: &Option<String>, needle: &str) -> bool {
    haystack
        .as_deref()
        .is_some_and(|text| text.to_lowercase().contains(needle))
}

/// The one entry a reference names, by position; `None` for no match or more than one.
fn source_of(items: &[ImportItem], reference: &KeepassRef<'_>) -> Option<usize> {
    let needle = reference.text.to_lowercase();
    let matches: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| match reference.search {
            'I' => {
                let wanted = needle.replace('-', "");
                wanted.len() == 32
                    && item
                        .source_ref
                        .as_deref()
                        .is_some_and(|id| id.replace('-', "").to_lowercase() == wanted)
            }
            'T' => contains(&item.title, &needle),
            'U' => contains(&item.username, &needle),
            'P' => contains(&item.password, &needle),
            'A' => contains(&item.url, &needle),
            'N' => contains(&item.note, &needle),
            'O' => item
                .key_values
                .iter()
                .any(|kv| contains(&kv.value, &needle)),
            _ => false,
        })
        .map(|(index, _)| index)
        .collect();
    match matches.as_slice() {
        [one] => Some(*one),
        _ => None,
    }
}

/// The new texts of the entries with KeePass references. `ids` holds the id each entry gets, `None`
/// for one that is not written (a skipped duplicate): a reference to it stays text.
pub fn convert(items: &[ImportItem], ids: &[Option<String>]) -> Conversion {
    let mut conversion = Conversion::default();
    for (index, item) in items.iter().enumerate() {
        if ids.get(index).is_none_or(Option::is_none) {
            continue;
        }
        for (field, text) in texts(item) {
            let refs = find_refs(text);
            if refs.is_empty() {
                continue;
            }
            let mut out = String::with_capacity(text.len());
            let mut last = 0;
            for reference in &refs {
                out.push_str(&text[last..reference.start]);
                let kind = match reference.wanted {
                    'U' => Some(RefKind::Username),
                    'P' => Some(RefKind::Password),
                    _ => None,
                };
                let token = kind.and_then(|kind| {
                    let source = source_of(items, reference)?;
                    let id = ids.get(source)?.as_deref()?;
                    build_token(id, &kind)
                });
                match token {
                    Some(token) => {
                        out.push_str(&token);
                        conversion.converted += 1;
                    }
                    None => {
                        out.push_str(&text[reference.start..reference.end]);
                        conversion.left_as_text += 1;
                    }
                }
                last = reference.end;
            }
            out.push_str(&text[last..]);
            if out != text {
                conversion.rewrites.push((index, field, out));
            }
        }
    }
    conversion
}
