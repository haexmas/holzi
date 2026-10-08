//! The `Range` header of a media request (spec 044 FR-012, contracts/media-server.md): one byte
//! range, `bytes=N-M`, `bytes=N-` or `bytes=-N`. Several ranges are not served. An end past the file
//! is cut to its last byte (RFC 7233 §2.1); a start past it is unsatisfiable.

/// What a request asks for, given the size of the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Span {
    /// No `Range` header: the whole file with `200`.
    Whole,
    /// `[start, end]`, both inclusive, with `206`.
    Part { start: u64, end: u64 },
    /// A range the file cannot satisfy: `416` with `bytes */total`.
    Unsatisfiable,
}

/// The span for an optional `Range` header value and a file of `total` bytes.
pub fn span(header: Option<&str>, total: u64) -> Span {
    let Some(header) = header else {
        return Span::Whole;
    };
    parse(header, total).unwrap_or(Span::Unsatisfiable)
}

fn parse(header: &str, total: u64) -> Option<Span> {
    let rest = header.trim().strip_prefix("bytes=")?;
    if rest.contains(',') || total == 0 {
        return None;
    }
    let (first, last) = rest.split_once('-')?;
    let (first, last) = (first.trim(), last.trim());
    if first.is_empty() {
        let suffix: u64 = last.parse().ok()?;
        if suffix == 0 {
            return None;
        }
        return Some(Span::Part {
            start: total.saturating_sub(suffix),
            end: total - 1,
        });
    }
    let start: u64 = first.parse().ok()?;
    let end = if last.is_empty() {
        total - 1
    } else {
        last.parse::<u64>().ok()?.min(total - 1)
    };
    (start <= end && start < total).then_some(Span::Part { start, end })
}

#[cfg(test)]
#[path = "range_tests.rs"]
mod tests;
