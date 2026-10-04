//! References between entries (spec 036, FR-044 to FR-050, `contracts/references.md`, ADR-0009):
//! the grammar `{$<uuid>:username}`, `{$<uuid>:password}` and `{$<uuid>:extra:<key>}`, finding and
//! building placeholders, and the resolution and the cycle check over an abstract lookup. Pure: the
//! database side is `references_db.rs`. This is the only implementation of the grammar; the window
//! asks it through commands and never parses a placeholder itself (research R11).

/// How many references a chain may follow (KeePass `SprEngine.MaxRecursionDepth`, research R4).
pub const MAX_REFERENCE_DEPTH: usize = 12;

/// How many source values one resolution may read. A placeholder used several times in each step
/// of a chain is no cycle, but without a budget it would make the reads grow exponentially.
pub const MAX_REFERENCE_LOOKUPS: usize = 256;

/// The longest value (in bytes) one resolution may build.
pub const MAX_RESOLVED_BYTES: usize = 64 * 1024;

/// The value a placeholder points at.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RefKind {
    Username,
    Password,
    /// The value of the first custom field with this key (case counts).
    Extra(String),
}

/// A field of an entry, as a chain and the cycle check name it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Field {
    Username,
    Password,
    Url,
    Note,
    Extra(String),
}

impl From<&RefKind> for Field {
    fn from(kind: &RefKind) -> Self {
        match kind {
            RefKind::Username => Field::Username,
            RefKind::Password => Field::Password,
            RefKind::Extra(key) => Field::Extra(key.clone()),
        }
    }
}

/// One placeholder in a text: its byte range and what it points at (the id in lower case).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub start: usize,
    pub end: usize,
    pub item_id: String,
    pub kind: RefKind,
}

/// Why a value with placeholders could not be resolved. Never a value, never a text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceError {
    /// The source or its custom field does not exist, or the caller may not see it (FR-047: the
    /// two look the same from outside).
    Missing,
    /// The chain comes back to a field it is reading.
    Cycle,
    /// The chain is longer than [`MAX_REFERENCE_DEPTH`], or the resolution exceeds
    /// [`MAX_REFERENCE_LOOKUPS`] or [`MAX_RESOLVED_BYTES`].
    TooDeep,
}

impl ReferenceError {
    /// The kind as the window names it (`ReferenceError { kind }`).
    pub fn as_str(self) -> &'static str {
        match self {
            ReferenceError::Missing => "missing",
            ReferenceError::Cycle => "cycle",
            ReferenceError::TooDeep => "tooDeep",
        }
    }
}

/// What a lookup knows about the value a placeholder points at.
pub enum Lookup {
    /// The raw stored value (it may hold placeholders itself).
    Value(String),
    /// No such entry or field, or not visible to the caller.
    Missing,
}

fn is_uuid(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => *b == b'-',
            _ => b.is_ascii_hexdigit(),
        })
}

/// Reads one placeholder that starts at `start` (on `{$`); `None` when the text there is not one.
fn parse_at(text: &str, start: usize) -> Option<Found> {
    let rest = &text[start + 2..];
    let id = rest.get(..36).filter(|id| is_uuid(id))?;
    let after = rest[36..].strip_prefix(':')?;
    let head = start + 2 + 37;
    if after.starts_with("username}") {
        return Some(Found {
            start,
            end: head + "username}".len(),
            item_id: id.to_ascii_lowercase(),
            kind: RefKind::Username,
        });
    }
    if after.starts_with("password}") {
        return Some(Found {
            start,
            end: head + "password}".len(),
            item_id: id.to_ascii_lowercase(),
            kind: RefKind::Password,
        });
    }
    let key_text = after.strip_prefix("extra:")?;
    let mut key = String::new();
    let mut chars = key_text.char_indices();
    while let Some((at, c)) = chars.next() {
        match c {
            '\\' => key.push(chars.next()?.1),
            '}' => {
                if key.is_empty() {
                    return None;
                }
                return Some(Found {
                    start,
                    end: head + "extra:".len() + at + 1,
                    item_id: id.to_ascii_lowercase(),
                    kind: RefKind::Extra(key),
                });
            }
            other => key.push(other),
        }
    }
    None
}

/// Every placeholder in `text`, left to right. Anything that does not match the grammar exactly is
/// text and is skipped.
pub fn find(text: &str) -> Vec<Found> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(offset) = text[from..].find("{$") {
        let start = from + offset;
        match parse_at(text, start) {
            Some(hit) => {
                from = hit.end;
                found.push(hit);
            }
            None => from = start + 1,
        }
    }
    found
}

/// Whether `text` holds at least one placeholder.
pub fn contains_reference(text: &str) -> bool {
    !find(text).is_empty()
}

/// The placeholder for a value of an entry, with `\` and `}` in the key escaped; `None` for an id
/// that is not a UUID or an empty key.
pub fn build_token(item_id: &str, kind: &RefKind) -> Option<String> {
    if !is_uuid(item_id) {
        return None;
    }
    let id = item_id.to_ascii_lowercase();
    Some(match kind {
        RefKind::Username => format!("{{${id}:username}}"),
        RefKind::Password => format!("{{${id}:password}}"),
        RefKind::Extra(key) => {
            if key.is_empty() {
                return None;
            }
            let escaped: String = key
                .chars()
                .flat_map(|c| match c {
                    '\\' | '}' => vec!['\\', c],
                    other => vec![other],
                })
                .collect();
            format!("{{${id}:extra:{escaped}}}")
        }
    })
}

/// `text` with the placeholders in `found` replaced by `values` (same order and length).
pub fn replace(text: &str, found: &[Found], values: &[String]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for (hit, value) in found.iter().zip(values) {
        out.push_str(&text[last..hit.start]);
        out.push_str(value);
        last = hit.end;
    }
    out.push_str(&text[last..]);
    out
}

/// Resolves every placeholder of `text`, which is the value of `origin`. The chain is a stack of
/// the fields from `origin` to here (the same value twice side by side is no cycle); a chain longer
/// than [`MAX_REFERENCE_DEPTH`] is `TooDeep`, and so is a resolution that needs more than
/// [`MAX_REFERENCE_LOOKUPS`] reads or builds a value longer than [`MAX_RESOLVED_BYTES`]. One failing
/// placeholder fails the whole value: the result is never a partial or an empty text because of an
/// error (FR-046).
pub fn resolve<E>(
    text: &str,
    origin: (String, Field),
    lookup: &mut impl FnMut(&str, &RefKind) -> Result<Lookup, E>,
) -> Result<Result<String, ReferenceError>, E> {
    let mut chain = vec![origin];
    let mut budget = MAX_REFERENCE_LOOKUPS;
    resolve_in(text, &mut chain, &mut budget, lookup)
}

fn resolve_in<E>(
    text: &str,
    chain: &mut Vec<(String, Field)>,
    budget: &mut usize,
    lookup: &mut impl FnMut(&str, &RefKind) -> Result<Lookup, E>,
) -> Result<Result<String, ReferenceError>, E> {
    let found = find(text);
    if found.is_empty() {
        return Ok(Ok(text.to_string()));
    }
    let mut values = Vec::with_capacity(found.len());
    for hit in &found {
        let node = (hit.item_id.clone(), Field::from(&hit.kind));
        if chain.contains(&node) {
            return Ok(Err(ReferenceError::Cycle));
        }
        // The chain holds the origin, so its length is the number of the step being taken.
        if chain.len() > MAX_REFERENCE_DEPTH {
            return Ok(Err(ReferenceError::TooDeep));
        }
        if *budget == 0 {
            return Ok(Err(ReferenceError::TooDeep));
        }
        *budget -= 1;
        let raw = match lookup(&hit.item_id, &hit.kind)? {
            Lookup::Value(raw) => raw,
            Lookup::Missing => return Ok(Err(ReferenceError::Missing)),
        };
        chain.push(node);
        let value = resolve_in(&raw, chain, budget, lookup)?;
        chain.pop();
        match value {
            Ok(value) => values.push(value),
            Err(error) => return Ok(Err(error)),
        }
    }
    let out = replace(text, &found, &values);
    if out.len() > MAX_RESOLVED_BYTES {
        return Ok(Err(ReferenceError::TooDeep));
    }
    Ok(Ok(out))
}

/// The check on save (research R4): whether one of the new texts of `item_id` reaches the same
/// field of `item_id` again through its placeholders, searched without a depth limit (a cycle that
/// closes only on step 13 is found too). `new_texts` are the texts the entry will hold; `lookup`
/// gives the stored raw value of any other field (missing values end a path). Returns the source of
/// the first step of a path back, which the error names.
pub fn find_cycle<E>(
    item_id: &str,
    new_texts: &[(Field, String)],
    lookup: &mut impl FnMut(&str, &RefKind) -> Result<Option<String>, E>,
) -> Result<Option<String>, E> {
    let own = |field: &Field| {
        new_texts
            .iter()
            .find(|(candidate, _)| candidate == field)
            .map(|(_, text)| text.clone())
    };
    for (field, text) in new_texts {
        for first in find(text) {
            let mut stack = vec![(first.item_id.clone(), first.kind.clone())];
            let mut seen = std::collections::HashSet::new();
            while let Some((node_item, kind)) = stack.pop() {
                let node_field = Field::from(&kind);
                if node_item == item_id && &node_field == field {
                    return Ok(Some(first.item_id));
                }
                if !seen.insert((node_item.clone(), node_field.clone())) {
                    continue;
                }
                let raw = if node_item == item_id {
                    own(&node_field)
                } else {
                    lookup(&node_item, &kind)?
                };
                if let Some(raw) = raw {
                    stack.extend(find(&raw).into_iter().map(|hit| (hit.item_id, hit.kind)));
                }
            }
        }
    }
    Ok(None)
}

/// A byte offset of `text` as the UTF-16 offset the window counts in.
pub fn utf16_offset(text: &str, byte_offset: usize) -> usize {
    text[..byte_offset].encode_utf16().count()
}
