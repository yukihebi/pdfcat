//! Command-line parsing.

use thiserror::Error;

use crate::pages::{PageSpecError, Range, parse_ranges};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub const HELP: &str = include_str!("help.txt");

/// A malformed command line.
#[derive(Debug, PartialEq, Eq, Error)]
pub enum CliError {
    #[error("unknown option: {0}")]
    UnknownOption(String),
    #[error("{0} expects a value")]
    MissingValue(&'static str),
    #[error("--output specified more than once")]
    DuplicateOutput,
    #[error("--pages must follow an input file")]
    PagesWithoutInput,
    #[error("must specify --output and/or --count-pages/--count-bytes")]
    NoAction,
    #[error("no input files")]
    NoInputs,
    #[error("invalid page spec `{spec}`: {source}")]
    BadPageSpec {
        spec: String,
        #[source]
        source: PageSpecError,
    },
}

/// One input file together with its (optional) page selection.
#[derive(Debug, PartialEq, Eq)]
pub struct Input {
    pub path: String,
    /// `None` => all pages; otherwise the ordered list of range tokens.
    pub ranges: Option<Vec<Range>>,
}

/// What the parsed command line asks pdfcat to do.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Run {
        inputs: Vec<Input>,
        output: Option<String>,
        count_pages: bool,
        count_bytes: bool,
    },
    Help,
    Version,
}

/// Parse the program arguments (without `argv[0]`).
pub fn parse(args: &[String]) -> Result<Command, CliError> {
    Parser::new(args).parse()
}

/// Split a `--opt=value` / `-o=value` argument into its name and inline value.
fn split_inline(arg: &str) -> (&str, Option<&str>) {
    match arg.split_once('=') {
        Some((opt, val)) if opt.starts_with('-') => (opt, Some(val)),
        _ => (arg, None),
    }
}

struct Parser<'a> {
    args: &'a [String],
    pos: usize,
    inputs: Vec<Input>,
    output: Option<String>,
    count_pages: bool,
    count_bytes: bool,
    /// Set once `--` is seen; everything after it is a literal input path.
    options_done: bool,
}

impl<'a> Parser<'a> {
    fn new(args: &'a [String]) -> Self {
        Parser {
            args,
            pos: 0,
            inputs: Vec::new(),
            output: None,
            count_pages: false,
            count_bytes: false,
            options_done: false,
        }
    }

    fn parse(mut self) -> Result<Command, CliError> {
        while self.pos < self.args.len() {
            if let Some(command) = self.step()? {
                return Ok(command);
            }
            self.pos += 1;
        }
        self.finish()
    }

    /// Consume the argument at `self.pos`. Returns `Some` for an option that
    /// short-circuits the rest of the command line (`--help`, `--version`).
    fn step(&mut self) -> Result<Option<Command>, CliError> {
        let arg = &self.args[self.pos];
        if self.options_done {
            self.push_input(arg);
            return Ok(None);
        }
        if arg == "--" {
            self.options_done = true;
            return Ok(None);
        }
        let (opt, inline) = split_inline(arg);
        match opt {
            "-h" | "--help" => return Ok(Some(Command::Help)),
            "-V" | "-v" | "--version" => return Ok(Some(Command::Version)),
            "-o" | "--output" => self.set_output(inline)?,
            "-p" | "-pp" | "--page" | "--pages" => self.add_pages(inline)?,
            // No-value flags: any `=value` is silently ignored, as for --help/--version.
            "--count-pages" | "--count-page" | "--page-count" | "--page-counts" | "--num-pages"
            | "--num-page" | "--npages" | "--npage" => self.count_pages = true,
            "--count-bytes" | "--count-byte" | "--byte-count" | "--byte-counts" | "--num-bytes"
            | "--num-byte" | "--nbytes" | "--nbyte" => self.count_bytes = true,
            _ if opt.starts_with('-') && opt != "-" => {
                return Err(CliError::UnknownOption(opt.to_string()));
            }
            _ => self.push_input(arg),
        }
        Ok(None)
    }

    fn push_input(&mut self, path: &str) {
        self.inputs.push(Input {
            path: path.to_string(),
            ranges: None,
        });
    }

    /// The value for an option: either inline (`--opt=value`) or the next arg.
    /// An empty value is rejected (a blank file name or page spec is never valid).
    fn take_value(
        &mut self,
        label: &'static str,
        inline: Option<&str>,
    ) -> Result<String, CliError> {
        let value = match inline {
            Some(value) => value.to_string(),
            None => {
                self.pos += 1;
                self.args
                    .get(self.pos)
                    .cloned()
                    .ok_or(CliError::MissingValue(label))?
            }
        };
        if value.is_empty() {
            return Err(CliError::MissingValue(label));
        }
        Ok(value)
    }

    fn set_output(&mut self, inline: Option<&str>) -> Result<(), CliError> {
        let value = self.take_value("--output", inline)?;
        if self.output.is_some() {
            return Err(CliError::DuplicateOutput);
        }
        self.output = Some(value);
        Ok(())
    }

    fn add_pages(&mut self, inline: Option<&str>) -> Result<(), CliError> {
        let spec = self.take_value("--pages", inline)?;
        let ranges = parse_ranges(&spec).map_err(|source| CliError::BadPageSpec {
            spec: spec.clone(),
            source,
        })?;
        let last = self.inputs.last_mut().ok_or(CliError::PagesWithoutInput)?;
        last.ranges.get_or_insert_with(Vec::new).extend(ranges);
        Ok(())
    }

    fn finish(self) -> Result<Command, CliError> {
        if self.inputs.is_empty() {
            return Err(CliError::NoInputs);
        }
        if self.output.is_none() && !self.count_pages && !self.count_bytes {
            return Err(CliError::NoAction);
        }
        Ok(Command::Run {
            inputs: self.inputs,
            output: self.output,
            count_pages: self.count_pages,
            count_bytes: self.count_bytes,
        })
    }
}

#[cfg(test)]
mod tests {
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
}
