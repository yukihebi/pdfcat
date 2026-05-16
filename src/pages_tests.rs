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
