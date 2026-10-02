//! A strict RFC 4180 reader for the CSV exports (spec 034, US7), a thin wrapper over the `csv`
//! crate: line breaks inside quoted fields, CRLF, doubled quotes and a leading byte order mark are
//! handled. Header names are lower-cased and trimmed; a row with fewer cells than the header reads
//! as empty cells, one with more is not an error either (the extra cells are ignored).

use super::failed;
use crate::error::Result;

/// A table with its header row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl Table {
    /// The cell of `column` in `row`, trimmed of nothing: `None` for an unknown column, `""` for an
    /// empty or missing cell.
    pub fn cell<'a>(&self, row: &'a [String], column: &str) -> Option<&'a str> {
        let index = self.headers.iter().position(|h| h == column)?;
        Some(row.get(index).map(String::as_str).unwrap_or(""))
    }

    pub fn has_column(&self, column: &str) -> bool {
        self.headers.iter().any(|h| h == column)
    }
}

/// Parses a CSV file. Text that is not UTF-8 or a file without a header row is `corrupt`.
pub fn parse(bytes: &[u8]) -> Result<Table> {
    let text = std::str::from_utf8(bytes).map_err(|_| failed("corrupt"))?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut reader = ::csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());
    let mut records = reader.records();
    let header = match records.next() {
        Some(Ok(record)) => record,
        _ => return Err(failed("corrupt")),
    };
    let headers: Vec<String> = header.iter().map(|h| h.trim().to_lowercase()).collect();
    let mut rows = Vec::new();
    for record in records {
        let record = record.map_err(|_| failed("corrupt"))?;
        if record.iter().all(|cell| cell.is_empty()) {
            continue;
        }
        rows.push(record.iter().map(str::to_string).collect());
    }
    Ok(Table { headers, rows })
}

#[cfg(test)]
#[path = "csv_tests.rs"]
mod tests;
