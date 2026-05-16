mod cli;
mod merge;
mod pages;

use std::io::{self, Write};
use std::process::ExitCode;

use lopdf::Document;
use thiserror::Error;

use cli::{Command, Input};
use pages::{PageSpecError, fmt_ranges, resolve_ranges};

/// A `Write` adapter that forwards to an inner writer and counts the bytes
/// that were actually written (i.e. accepted by the inner writer).
struct CountingWriter<W: Write> {
    inner: W,
    count: u64,
}

impl<W: Write> CountingWriter<W> {
    fn new(inner: W) -> Self {
        CountingWriter { inner, count: 0 }
    }

    fn count(&self) -> u64 {
        self.count
    }
}

impl<W: Write> Write for CountingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.count += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        // Bare `pdfcat`: show usage instead of an error line.
        Err(Error::NoArguments) => {
            eprint!("{}", cli::HELP);
            ExitCode::FAILURE
        }
        Err(err) => {
            eprintln!("pdfcat: error: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Anything that can stop pdfcat from producing its output.
#[derive(Debug, Error)]
enum Error {
    #[error("no arguments given")]
    NoArguments,
    #[error(transparent)]
    Cli(#[from] cli::CliError),
    #[error(transparent)]
    Merge(#[from] merge::MergeError),
    #[error("{path}: cannot read PDF: {source}")]
    ReadInput { path: String, source: lopdf::Error },
    #[error("{path}: this PDF has no pages")]
    NoPages { path: String },
    #[error("{path}: {source}")]
    PageSelection { path: String, source: PageSpecError },
    #[error("cannot write {path}: {source}")]
    WriteOutput {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to write count to stdout: {0}")]
    ReportIo(std::io::Error),
}

fn write_count(
    report: &mut impl Write,
    label: &str,
    n: impl std::fmt::Display,
    quiet: bool,
) -> Result<(), Error> {
    if quiet {
        writeln!(report, "{n}").map_err(Error::ReportIo)
    } else {
        writeln!(report, "{label}: {n}").map_err(Error::ReportIo)
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_run(
    merged: &mut Document,
    output: Option<&str>,
    count_pages: bool,
    count_bytes: bool,
    quiet: bool,
    verbose: bool,
    report: &mut impl Write,
    log: &mut impl Write,
) -> Result<(), Error> {
    if verbose {
        writeln!(log, "merged: {} pages", merged.get_pages().len()).map_err(Error::ReportIo)?;
    }

    if count_pages {
        write_count(report, "pages", merged.get_pages().len(), quiet)?;
    }

    match (output, count_bytes) {
        (Some(path), need_bytes) => {
            // Always go through a CountingWriter so the verbose `wrote` line
            // can include the byte count even when --count-bytes is absent.
            let file = std::fs::File::create(path).map_err(|source| Error::WriteOutput {
                path: path.to_string(),
                source,
            })?;
            let mut w = CountingWriter::new(std::io::BufWriter::new(file));
            merged
                .save_to(&mut w)
                .map_err(|source| Error::WriteOutput {
                    path: path.to_string(),
                    source,
                })?;
            w.flush().map_err(|source| Error::WriteOutput {
                path: path.to_string(),
                source,
            })?;
            if need_bytes {
                write_count(report, "bytes", w.count(), quiet)?;
            }
            if verbose {
                writeln!(log, "wrote {path} ({} bytes)", w.count()).map_err(Error::ReportIo)?;
            }
        }
        (None, true) => {
            let mut w = CountingWriter::new(io::sink());
            merged
                .save_to(&mut w)
                .map_err(|source| Error::WriteOutput {
                    path: "<none>".to_string(),
                    source,
                })?;
            write_count(report, "bytes", w.count(), quiet)?;
        }
        (None, false) => {
            // Only page count requested; nothing more to do.
        }
    }

    Ok(())
}

fn run() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        return Err(Error::NoArguments);
    }

    match cli::parse(&args)? {
        Command::Help => {
            print!("{}", cli::HELP);
            Ok(())
        }
        Command::Version => {
            println!("pdfcat {}", cli::VERSION);
            Ok(())
        }
        Command::Run {
            inputs,
            output,
            count_pages,
            count_bytes,
            quiet,
            verbose,
        } => {
            let stdout = io::stdout();
            let mut report = stdout.lock();
            let stderr = io::stderr();
            if verbose {
                let mut log = stderr.lock();
                let sources = load_sources(&inputs, true, &mut log)?;
                let mut merged = merge::merge(sources)?;
                execute_run(
                    &mut merged,
                    output.as_deref(),
                    count_pages,
                    count_bytes,
                    quiet,
                    true,
                    &mut report,
                    &mut log,
                )
            } else {
                let mut sink = io::sink();
                let sources = load_sources(&inputs, false, &mut sink)?;
                let mut merged = merge::merge(sources)?;
                execute_run(
                    &mut merged,
                    output.as_deref(),
                    count_pages,
                    count_bytes,
                    quiet,
                    false,
                    &mut report,
                    &mut sink,
                )
            }
        }
    }
}

/// Load each input document and resolve its page selection against the actual
/// page count. When `verbose` is true, write a header line to `log` *before*
/// loading each file (so failures are attributable) and a detail line after.
fn load_sources(
    inputs: &[Input],
    verbose: bool,
    log: &mut impl Write,
) -> Result<Vec<(Document, Vec<u32>)>, Error> {
    let mut sources = Vec::with_capacity(inputs.len());
    let total_inputs = inputs.len();
    let indent = " ".repeat(fmt_header_index(1, total_inputs).len() + 1);
    for (idx, input) in inputs.iter().enumerate() {
        if verbose {
            let head = fmt_header_index(idx + 1, total_inputs);
            match &input.ranges {
                Some(ranges) => {
                    writeln!(log, "{head} {} -p {}", input.path, fmt_ranges(ranges))
                }
                None => writeln!(log, "{head} {}", input.path),
            }
            .map_err(Error::ReportIo)?;
            log.flush().map_err(Error::ReportIo)?;
        }
        let doc = Document::load(&input.path).map_err(|source| Error::ReadInput {
            path: input.path.clone(),
            source,
        })?;
        let total = doc.get_pages().len() as u32;
        if total == 0 {
            return Err(Error::NoPages {
                path: input.path.clone(),
            });
        }
        let selected: Vec<u32> = match &input.ranges {
            None => (1..=total).collect(),
            Some(ranges) => {
                resolve_ranges(ranges, total).map_err(|source| Error::PageSelection {
                    path: input.path.clone(),
                    source,
                })?
            }
        };
        if verbose {
            let count_str = if input.ranges.is_none() {
                "all".to_string()
            } else {
                format!("{} selected", selected.len())
            };
            writeln!(log, "{indent}{total} pages total, {count_str}").map_err(Error::ReportIo)?;
        }
        sources.push((doc, selected));
    }
    Ok(sources)
}

/// Format `[i/total]` with `i` right-justified to the width of `total`.
fn fmt_header_index(i: usize, total: usize) -> String {
    let width = total.to_string().len();
    format!("[{i:>width$}/{total}]")
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
