use super::*;

fn parse_args(args: &[&str]) -> Result<Command, CliError> {
    parse(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
}

fn run_command(args: &[&str]) -> (Vec<Input>, Option<String>) {
    match parse_args(args).unwrap() {
        Command::Run { inputs, output, .. } => (inputs, output),
        other => panic!("expected Run, got {other:?}"),
    }
}

#[test]
fn concatenation() {
    let (inputs, output) = run_command(&["a.pdf", "b.pdf", "c.pdf", "-o", "w.pdf"]);
    assert_eq!(output.as_deref(), Some("w.pdf"));
    assert_eq!(
        inputs.iter().map(|i| i.path.as_str()).collect::<Vec<_>>(),
        ["a.pdf", "b.pdf", "c.pdf"]
    );
    assert!(inputs.iter().all(|i| i.ranges.is_none()));
}

#[test]
fn pages_bind_to_preceding_input() {
    let (inputs, _) = run_command(&["a.pdf", "-p", "1-2", "b.pdf", "-o", "w.pdf"]);
    assert_eq!(inputs[0].ranges, Some(parse_ranges("1-2").unwrap()));
    assert_eq!(inputs[1].ranges, None);
}

#[test]
fn repeated_pages_accumulate() {
    let (inputs, _) = run_command(&["a.pdf", "-p", "1", "--pages", "3-4", "-o", "w.pdf"]);
    assert_eq!(inputs[0].ranges, Some(parse_ranges("1,3-4").unwrap()));
}

#[test]
fn inline_and_positional_output() {
    assert_eq!(
        run_command(&["a.pdf", "-o=w.pdf"]).1.as_deref(),
        Some("w.pdf")
    );
    assert_eq!(
        run_command(&["-o", "w.pdf", "a.pdf"]).1.as_deref(),
        Some("w.pdf")
    );
    assert_eq!(
        run_command(&["a.pdf", "--output=w.pdf"]).1.as_deref(),
        Some("w.pdf")
    );
}

#[test]
fn inline_pages_and_aliases() {
    let (inputs, _) = run_command(&["a.pdf", "-p=1-2", "b.pdf", "--pages=3", "-o", "w.pdf"]);
    assert_eq!(inputs[0].ranges, Some(parse_ranges("1-2").unwrap()));
    assert_eq!(inputs[1].ranges, Some(parse_ranges("3").unwrap()));
    // `-pp` alias and a bare `-` filename.
    let (inputs, _) = run_command(&["-", "-pp", "1", "-o", "w.pdf"]);
    assert_eq!(inputs[0].path, "-");
    assert_eq!(inputs[0].ranges, Some(parse_ranges("1").unwrap()));
}

#[test]
fn double_dash_ends_options() {
    let (inputs, output) = run_command(&["-o", "w.pdf", "--", "-weird.pdf", "--also.pdf"]);
    assert_eq!(output.as_deref(), Some("w.pdf"));
    assert_eq!(
        inputs.iter().map(|i| i.path.as_str()).collect::<Vec<_>>(),
        ["-weird.pdf", "--also.pdf"]
    );
    // after `--`, even `-p` is just a file name
    let (inputs, _) = run_command(&["a.pdf", "-o", "w.pdf", "--", "-p", "x.pdf"]);
    assert_eq!(
        inputs.iter().map(|i| i.path.as_str()).collect::<Vec<_>>(),
        ["a.pdf", "-p", "x.pdf"]
    );
}

#[test]
fn empty_option_values_rejected() {
    assert_eq!(
        parse_args(&["a.pdf", "-o", ""]),
        Err(CliError::MissingValue("--output"))
    );
    assert_eq!(
        parse_args(&["a.pdf", "-o="]),
        Err(CliError::MissingValue("--output"))
    );
    assert_eq!(
        parse_args(&["a.pdf", "-p=", "-o", "w.pdf"]),
        Err(CliError::MissingValue("--pages"))
    );
}

#[test]
fn help_and_version_short_circuit() {
    assert_eq!(parse_args(&["-h"]), Ok(Command::Help));
    assert_eq!(parse_args(&["a.pdf", "--help", "-o"]), Ok(Command::Help));
    assert_eq!(parse_args(&["--version"]), Ok(Command::Version));
    assert_eq!(parse_args(&["-v"]), Ok(Command::Version));
    assert_eq!(parse_args(&["-V"]), Ok(Command::Version));
}

#[test]
fn count_flags_and_aliases() {
    let parse_one = |arg: &str| match parse_args(&["a.pdf", arg]).unwrap() {
        Command::Run {
            count_pages,
            count_bytes,
            output,
            ..
        } => (count_pages, count_bytes, output),
        other => panic!("expected Run, got {other:?}"),
    };

    for alias in [
        "--count-pages",
        "--count-page",
        "--page-count",
        "--page-counts",
        "--num-pages",
        "--num-page",
        "--npages",
        "--npage",
    ] {
        let (cp, cb, out) = parse_one(alias);
        assert!(cp, "{alias} should set count_pages");
        assert!(!cb, "{alias} should not set count_bytes");
        assert!(out.is_none(), "{alias} should not require -o");
    }

    for alias in [
        "--count-bytes",
        "--count-byte",
        "--byte-count",
        "--byte-counts",
        "--num-bytes",
        "--num-byte",
        "--nbytes",
        "--nbyte",
    ] {
        let (cp, cb, out) = parse_one(alias);
        assert!(!cp, "{alias} should not set count_pages");
        assert!(cb, "{alias} should set count_bytes");
        assert!(out.is_none(), "{alias} should not require -o");
    }

    match parse_args(&["a.pdf", "--count-pages", "--count-bytes", "-o", "w.pdf"]).unwrap() {
        Command::Run {
            count_pages,
            count_bytes,
            output,
            ..
        } => {
            assert!(count_pages);
            assert!(count_bytes);
            assert_eq!(output.as_deref(), Some("w.pdf"));
        }
        other => panic!("expected Run, got {other:?}"),
    }

    // Repeating equivalent aliases is idempotent (no DuplicateOutput-style error).
    match parse_args(&["a.pdf", "--count-pages", "--num-pages"]).unwrap() {
        Command::Run { count_pages, .. } => assert!(count_pages),
        other => panic!("expected Run, got {other:?}"),
    }
}

#[test]
fn errors() {
    use CliError::*;
    assert_eq!(parse_args(&["a.pdf"]), Err(NoAction));
    assert_eq!(parse_args(&["-o", "w.pdf"]), Err(NoInputs));
    assert_eq!(
        parse_args(&["a.pdf", "-o", "w.pdf", "-o", "x.pdf"]),
        Err(DuplicateOutput)
    );
    assert_eq!(
        parse_args(&["-p", "1", "a.pdf", "-o", "w.pdf"]),
        Err(PagesWithoutInput)
    );
    assert_eq!(
        parse_args(&["a.pdf", "--frobnicate", "-o", "w.pdf"]),
        Err(UnknownOption("--frobnicate".into()))
    );
    assert_eq!(parse_args(&["a.pdf", "-o"]), Err(MissingValue("--output")));
    assert_eq!(parse_args(&["a.pdf", "-p"]), Err(MissingValue("--pages")));
    // a bad page spec keeps the offending spec and the underlying reason
    match parse_args(&["a.pdf", "-p", "0", "-o", "w.pdf"]) {
        Err(BadPageSpec { spec, source }) => {
            assert_eq!(spec, "0");
            assert_eq!(source, PageSpecError::ZeroPage);
        }
        other => panic!("expected BadPageSpec, got {other:?}"),
    }
}
