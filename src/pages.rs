//! Parsing and resolution of page-selection specs (`1`, `-N`, `N-`, `N-M`,
//! comma-separated combinations).

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
mod tests {
    use super::*;

    fn r(start: u32, end: Option<u32>) -> Range {
        Range { start, end }
    }

    fn expand(spec: &str, total: u32) -> Result<Vec<u32>, PageSpecError> {
        resolve_ranges(&parse_ranges(spec)?, total)
    }

    #[test]
    fn token_forms() {
        assert_eq!(parse_ranges("7").unwrap(), [r(7, Some(7))]);
        assert_eq!(parse_ranges("-3").unwrap(), [r(1, Some(3))]);
        assert_eq!(parse_ranges("4-").unwrap(), [r(4, None)]);
        assert_eq!(parse_ranges("2-5").unwrap(), [r(2, Some(5))]);
        assert_eq!(
            parse_ranges("1,-2,3-4,5-").unwrap(),
            [r(1, Some(1)), r(1, Some(2)), r(3, Some(4)), r(5, None),]
        );
    }

    #[test]
    fn whitespace_and_trailing_commas_tolerated() {
        assert_eq!(
            parse_ranges(" 1 , 2 - 3 ,").unwrap(),
            [r(1, Some(1)), r(2, Some(3))]
        );
        assert_eq!(
            parse_ranges("1,,,2").unwrap(),
            [r(1, Some(1)), r(2, Some(2))]
        );
        // whitespace on either side of the dash
        assert_eq!(parse_ranges(" - 3").unwrap(), [r(1, Some(3))]);
        assert_eq!(parse_ranges("4 - ").unwrap(), [r(4, None)]);
    }

    #[test]
    fn expansion_preserves_order_and_duplicates() {
        assert_eq!(expand("1", 3).unwrap(), vec![1]);
        assert_eq!(expand("-2", 5).unwrap(), vec![1, 2]);
        assert_eq!(expand("4-", 5).unwrap(), vec![4, 5]);
        assert_eq!(expand("2-4", 5).unwrap(), vec![2, 3, 4]);
        assert_eq!(expand("-2,4-", 5).unwrap(), vec![1, 2, 4, 5]);
        assert_eq!(expand("1-3,5,", 5).unwrap(), vec![1, 2, 3, 5]);
        assert_eq!(expand("3,1", 3).unwrap(), vec![3, 1]);
        assert_eq!(expand("1-3,2", 3).unwrap(), vec![1, 2, 3, 2]); // overlap kept
    }

    #[test]
    fn parse_errors() {
        use PageSpecError::*;
        assert_eq!(parse_ranges("1,x"), Err(InvalidNumber("x".into())));
        assert_eq!(parse_ranges("0"), Err(ZeroPage));
        assert_eq!(parse_ranges("-0"), Err(ZeroPage));
        assert_eq!(parse_ranges("1-2-3"), Err(InvalidNumber("2-3".into())));
        assert_eq!(parse_ranges("-"), Err(InvalidRange("-".into())));
        assert_eq!(parse_ranges(""), Err(Empty));
        assert_eq!(parse_ranges("   "), Err(Empty));
        assert_eq!(parse_ranges("abc"), Err(InvalidNumber("abc".into())));
    }

    #[test]
    fn resolve_errors() {
        use PageSpecError::*;
        assert_eq!(expand("4-", 3), Err(OutOfRange { page: 4, total: 3 }));
        assert_eq!(expand("3-10", 5), Err(OutOfRange { page: 10, total: 5 }));
        assert_eq!(expand("6", 5), Err(OutOfRange { page: 6, total: 5 }));
        assert_eq!(expand("3-1", 5), Err(StartAfterEnd { start: 3, end: 1 }));
        assert_eq!(expand("5-", 5).unwrap(), vec![5]); // last page exactly
    }

    #[test]
    fn error_messages() {
        assert_eq!(
            PageSpecError::InvalidNumber("x".into()).to_string(),
            "invalid page number `x`"
        );
        assert_eq!(
            PageSpecError::OutOfRange { page: 4, total: 3 }.to_string(),
            "page 4 is out of range (document has 3 pages)"
        );
    }
}
