//! Consistent UUID decoding for storage rows.
//!
//! SQLite stores vault UUIDs as text. A malformed value is a corrupt row and
//! must remain an explicit conversion error; treating it as a missing row can
//! silently select a fallback provider or device.

use haex_crdt::rusqlite::{Result, Row};
use uuid::Uuid;

/// Decodes one UUID and preserves the source column in rusqlite's conversion
/// error for callers that expose typed storage rows.
pub fn parse(value: &str, column: usize) -> Result<Uuid> {
    Uuid::parse_str(value).map_err(|error| {
        haex_crdt::rusqlite::Error::FromSqlConversionFailure(
            column,
            haex_crdt::rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}

/// Reads and decodes a required UUID column from a row.
pub fn from_row(row: &Row<'_>, column: usize) -> Result<Uuid> {
    parse(&row.get::<_, String>(column)?, column)
}

/// Decodes an optional UUID column while preserving conversion failures.
pub fn optional(value: Option<String>, column: usize) -> Result<Option<Uuid>> {
    value
        .as_deref()
        .map(|value| parse(value, column))
        .transpose()
}

#[cfg(test)]
#[path = "uuid_tests.rs"]
mod tests;
