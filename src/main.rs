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

fn execute_run(
    merged: &mut Document,
    output: Option<&str>,
    count_pages: bool,
    count_bytes: bool,
    report: &mut impl Write,
) -> Result<(), Error> {
    if count_pages {
        writeln!(report, "pages: {}", merged.get_pages().len()).map_err(Error::ReportIo)?;
    }

    match (output, count_bytes) {
        (Some(path), true) => {
            let file = std::fs::File::create(path).map_err(|source| Error::WriteOutput {
                path: path.to_string(),
                source,
            })?;
            let mut w = CountingWriter::new(file);
            merged
                .save_to(&mut w)
                .map_err(|source| Error::WriteOutput {
                    path: path.to_string(),
                    source,
                })?;
            writeln!(report, "bytes: {}", w.count()).map_err(Error::ReportIo)?;
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
            writeln!(report, "bytes: {}", w.count()).map_err(Error::ReportIo)?;
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
mod tests {
    use super::*;
    use lopdf::{Dictionary, Document, Object};
    use std::io::Write;

    fn tiny_doc(n: usize) -> Document {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.add_object(Dictionary::new());
        let kids: Vec<Object> = (0..n)
            .map(|_| {
                let mut page = Dictionary::new();
                page.set("Type", "Page");
                page.set("Parent", pages_id);
                page.set(
                    "MediaBox",
                    Object::Array(vec![
                        Object::Integer(0),
                        Object::Integer(0),
                        Object::Integer(10),
                        Object::Integer(10),
                    ]),
                );
                Object::Reference(doc.add_object(page))
            })
            .collect();
        let mut pages = Dictionary::new();
        pages.set("Type", "Pages");
        pages.set("Count", n as i64);
        pages.set("Kids", kids);
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let mut catalog = Dictionary::new();
        catalog.set("Type", "Catalog");
        catalog.set("Pages", pages_id);
        let catalog_id = doc.add_object(catalog);
        doc.trailer.set("Root", catalog_id);
        doc
    }

    #[test]
    fn execute_run_count_pages_only_writes_no_file() {
        let mut doc = tiny_doc(3);
        let mut report = Vec::new();
        execute_run(&mut doc, None, true, false, &mut report).unwrap();
        assert_eq!(report, b"pages: 3\n");
    }

    #[test]
    fn execute_run_count_bytes_only_matches_serialized_size() {
        // Two independent docs because save_to may mutate internal state and
        // re-saving the same doc is not guaranteed to produce the same bytes.
        let mut doc_ref = tiny_doc(2);
        let mut expected = Vec::new();
        doc_ref.save_to(&mut expected).unwrap();

        let mut doc = tiny_doc(2);
        let mut report = Vec::new();
        execute_run(&mut doc, None, false, true, &mut report).unwrap();
        let line = std::str::from_utf8(&report).unwrap();
        let prefix = "bytes: ";
        assert!(line.starts_with(prefix), "got {line:?}");
        let n: usize = line[prefix.len()..line.len() - 1].parse().unwrap();
        assert_eq!(n, expected.len());
    }

    #[test]
    fn execute_run_both_flags_emit_pages_then_bytes() {
        let mut doc = tiny_doc(1);
        let mut report = Vec::new();
        execute_run(&mut doc, None, true, true, &mut report).unwrap();
        let s = std::str::from_utf8(&report).unwrap();
        assert!(s.starts_with("pages: 1\n"), "got {s:?}");
        assert!(s.contains("\nbytes: "), "got {s:?}");
    }

    #[test]
    fn execute_run_writes_file_when_output_given() {
        let mut doc = tiny_doc(2);
        let tmp = std::env::temp_dir().join(format!(
            "pdfcat-execute_run_writes_file-{}.pdf",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&tmp);
        let path = tmp.to_str().unwrap().to_string();

        let mut report = Vec::new();
        execute_run(&mut doc, Some(&path), false, true, &mut report).unwrap();

        let on_disk = std::fs::metadata(&tmp).unwrap().len();
        let s = std::str::from_utf8(&report).unwrap();
        let reported: u64 = s.trim_start_matches("bytes: ").trim().parse().unwrap();
        assert_eq!(on_disk, reported);

        let _ = std::fs::remove_file(&tmp);
    }

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
