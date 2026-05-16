//! Parsing and resolution of page-selection specs (`1`, `-N`, `N-`, `N-M`,
//! comma-separated combinations).

use std::fmt;
use std::ops::RangeInclusive;

use thiserror::Error;

/// Something wrong with a page-selection spec or its resolution against a
/// concrete page count. Callers add the surrounding context (the raw spec
/// string, the file name) when reporting these.
#[derive(Debug, PartialEq, Eq, Error)]
pub enum PageSpecError {
    #[error("invalid page number `{0}`")]
    InvalidNumber(String),
    #[error("page numbers are 1-based, got 0")]
    ZeroPage,
    #[error("invalid range `{0}`")]
    InvalidRange(String),
    #[error("no pages given")]
    Empty,
    #[error("range start {start} is after end {end}")]
    StartAfterEnd { start: u32, end: u32 },
    #[error("page {page} is out of range (document has {total} pages)")]
    OutOfRange { page: u32, total: u32 },
}

/// A single page-range token. `end == None` means "through the last page".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Range {
    pub start: u32,
    pub end: Option<u32>,
}

impl Range {
    /// Resolve against a document's page count into an inclusive 1-based range.
    fn resolve(self, total: u32) -> Result<RangeInclusive<u32>, PageSpecError> {
        if self.start > total {
            return Err(PageSpecError::OutOfRange {
                page: self.start,
                total,
            });
        }
        let end = match self.end {
            None => total,
            Some(e) if e > total => return Err(PageSpecError::OutOfRange { page: e, total }),
            Some(e) => e,
        };
        if self.start > end {
            return Err(PageSpecError::StartAfterEnd {
                start: self.start,
                end,
            });
        }
        Ok(self.start..=end)
    }
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.end {
            Some(e) if e == self.start => write!(f, "{}", self.start),
            Some(e) => write!(f, "{}-{}", self.start, e),
            None => write!(f, "{}-", self.start),
        }
    }
}

/// Re-format a parsed range list as a comma-separated spec, preserving
/// order and duplicates. Used by the verbose header.
#[allow(dead_code)]
pub fn fmt_ranges(ranges: &[Range]) -> String {
    let mut out = String::new();
    for (i, r) in ranges.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        use std::fmt::Write;
        let _ = write!(&mut out, "{r}");
    }
    out
}

/// Parse a page spec like `-2,4-,7` into range tokens (order preserved).
pub fn parse_ranges(spec: &str) -> Result<Vec<Range>, PageSpecError> {
    let mut out = Vec::new();
    for token in spec.split(',') {
        let token = token.trim();
        if token.is_empty() {
            continue; // tolerate trailing/empty entries
        }
        out.push(parse_token(token)?);
    }
    if out.is_empty() {
        return Err(PageSpecError::Empty);
    }
    Ok(out)
}

/// Parse a single token: `N`, `-N`, `N-`, or `N-M`.
fn parse_token(token: &str) -> Result<Range, PageSpecError> {
    let Some((a, b)) = token.split_once('-') else {
        let n = parse_page_num(token)?;
        return Ok(Range {
            start: n,
            end: Some(n),
        });
    };
    let (a, b) = (a.trim(), b.trim());
    match (a.is_empty(), b.is_empty()) {
        (true, true) => Err(PageSpecError::InvalidRange(token.to_string())),
        (true, false) => Ok(Range {
            start: 1,
            end: Some(parse_page_num(b)?),
        }),
        (false, true) => Ok(Range {
            start: parse_page_num(a)?,
            end: None,
        }),
        (false, false) => Ok(Range {
            start: parse_page_num(a)?,
            end: Some(parse_page_num(b)?),
        }),
    }
}

/// Parse a 1-based page number; rejects `0` and non-numeric input.
fn parse_page_num(s: &str) -> Result<u32, PageSpecError> {
    let n: u32 = s
        .parse()
        .map_err(|_| PageSpecError::InvalidNumber(s.to_string()))?;
    if n == 0 {
        return Err(PageSpecError::ZeroPage);
    }
    Ok(n)
}

/// Expand range tokens against a known page count, preserving order/duplicates.
pub fn resolve_ranges(ranges: &[Range], total: u32) -> Result<Vec<u32>, PageSpecError> {
    let mut out = Vec::new();
    for range in ranges {
        out.extend(range.resolve(total)?);
    }
    Ok(out)
}

#[cfg(test)]
#[path = "pages_tests.rs"]
mod tests;
