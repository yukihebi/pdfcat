mod cli;
mod merge;
mod pages;

use std::io::{self, Write};
use std::process::ExitCode;

use lopdf::Document;
use thiserror::Error;

use cli::{Command, Input};
use pages::{PageSpecError, resolve_ranges};

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

fn execute_run(
    merged: &mut Document,
    output: Option<&str>,
    count_pages: bool,
    count_bytes: bool,
    quiet: bool,
    report: &mut impl Write,
) -> Result<(), Error> {
    if count_pages {
        write_count(report, "pages", merged.get_pages().len(), quiet)?;
    }

    match (output, count_bytes) {
        (Some(path), true) => {
            // Mirror lopdf::Document::save's BufWriter so disk writes stay batched;
            // CountingWriter sits above it and tallies the same bytes that reach disk.
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
            write_count(report, "bytes", w.count(), quiet)?;
        }
        (Some(path), false) => {
            merged.save(path).map_err(|source| Error::WriteOutput {
                path: path.to_string(),
                source,
            })?;
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
        } => {
            let sources = load_sources(&inputs)?;
            let mut merged = merge::merge(sources)?;
            let stdout = io::stdout();
            let mut report = stdout.lock();
            execute_run(
                &mut merged,
                output.as_deref(),
                count_pages,
                count_bytes,
                quiet,
                &mut report,
            )
        }
    }
}

/// Load each input document and resolve its page selection against the actual
/// page count.
fn load_sources(inputs: &[Input]) -> Result<Vec<(Document, Vec<u32>)>, Error> {
    let mut sources = Vec::with_capacity(inputs.len());
    for input in inputs {
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
        sources.push((doc, selected));
    }
    Ok(sources)
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
