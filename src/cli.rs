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
#[path = "cli_tests.rs"]
mod tests;
