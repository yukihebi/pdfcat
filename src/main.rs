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
// Will be used in Task 3 to measure merged PDF size.
struct CountingWriter<W: Write> {
    inner: W,
    count: u64,
}

#[allow(dead_code)]
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
        Command::Run { inputs, output, .. } => {
            let output = output.expect("temporary: --count-* not yet wired up");
            let sources = load_sources(&inputs)?;
            let mut merged = merge::merge(sources)?;
            merged.save(&output).map_err(|source| Error::WriteOutput {
                path: output.clone(),
                source,
            })?;
            Ok(())
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
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn counting_writer_counts_bytes_through_to_inner() {
        let mut sink: Vec<u8> = Vec::new();
        let mut w = CountingWriter::new(&mut sink);
        w.write_all(b"hello").unwrap();
        w.write_all(b", world").unwrap();
        w.flush().unwrap();
        assert_eq!(w.count(), 12);
        assert_eq!(sink, b"hello, world");
    }

    #[test]
    fn counting_writer_partial_write_counts_only_what_was_written() {
        struct OneByteAtATime(Vec<u8>);
        impl Write for OneByteAtATime {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                if buf.is_empty() {
                    return Ok(0);
                }
                self.0.push(buf[0]);
                Ok(1)
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut inner = OneByteAtATime(Vec::new());
        let mut w = CountingWriter::new(&mut inner);
        w.write_all(b"abcd").unwrap();
        assert_eq!(w.count(), 4);
        assert_eq!(inner.0, b"abcd");
    }
}
