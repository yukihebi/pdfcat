use std::fs::File;
use std::io::{self, BufWriter, Write};

use lopdf::Document;

use crate::Error;
use crate::cli::Input;
use crate::merge;
use crate::pages::{fmt_ranges, resolve_ranges};

/// Options that control the stdout-side output: where to write (if anywhere),
/// what to report (page count, byte count), and whether to drop the labels.
pub(crate) struct OutputOpts<'a> {
    pub(crate) output: Option<&'a str>,
    pub(crate) count_pages: bool,
    pub(crate) count_bytes: bool,
    pub(crate) quiet: bool,
}

/// Pair of an enabled flag and a writer for the verbose stderr log.
/// When `enabled()` is `false`, the writer is not touched.
pub(crate) struct VerboseLog<W: Write> {
    enabled: bool,
    inner: W,
}

impl<W: Write> VerboseLog<W> {
    pub(crate) fn new(enabled: bool, inner: W) -> Self {
        VerboseLog { enabled, inner }
    }

    pub(crate) fn enabled(&self) -> bool {
        self.enabled
    }

    pub(crate) fn writer(&mut self) -> &mut W {
        &mut self.inner
    }
}

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

fn execute_run<R: Write, W: Write>(
    merged: &mut Document,
    opts: &OutputOpts<'_>,
    report: &mut R,
    vlog: &mut VerboseLog<W>,
) -> Result<(), Error> {
    if vlog.enabled() {
        writeln!(vlog.writer(), "merged: {} pages", merged.get_pages().len())
            .map_err(Error::ReportIo)?;
    }

    if opts.count_pages {
        write_count(report, "pages", merged.get_pages().len(), opts.quiet)?;
    }

    match (opts.output, opts.count_bytes) {
        (Some(path), need_bytes) => {
            // Always go through a CountingWriter so the verbose `wrote` line
            // can include the byte count even when --count-bytes is absent.
            let file = File::create(path).map_err(|source| Error::WriteOutput {
                path: path.to_string(),
                source,
            })?;
            let mut w = CountingWriter::new(BufWriter::new(file));
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
                write_count(report, "bytes", w.count(), opts.quiet)?;
            }
            if vlog.enabled() {
                writeln!(vlog.writer(), "wrote {path} ({} bytes)", w.count())
                    .map_err(Error::ReportIo)?;
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
            write_count(report, "bytes", w.count(), opts.quiet)?;
        }
        (None, false) => {
            // Only page count requested; nothing more to do.
        }
    }

    Ok(())
}

/// Load each input document and resolve its page selection against the actual
/// page count. When `vlog.enabled()` is true, write a header line to the log
/// *before* loading each file (so failures are attributable) and a detail line
/// after.
fn load_sources<W: Write>(
    inputs: &[Input],
    vlog: &mut VerboseLog<W>,
) -> Result<Vec<(Document, Vec<u32>)>, Error> {
    let mut sources = Vec::with_capacity(inputs.len());
    let total_inputs = inputs.len();
    let indent = " ".repeat(fmt_header_index(1, total_inputs).len() + 1);
    for (idx, input) in inputs.iter().enumerate() {
        if vlog.enabled() {
            let head = fmt_header_index(idx + 1, total_inputs);
            match &input.ranges {
                Some(ranges) => {
                    writeln!(
                        vlog.writer(),
                        "{head} {} -p {}",
                        input.path,
                        fmt_ranges(ranges)
                    )
                }
                None => writeln!(vlog.writer(), "{head} {}", input.path),
            }
            .map_err(Error::ReportIo)?;
            vlog.writer().flush().map_err(Error::ReportIo)?;
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
        if vlog.enabled() {
            let count_str = if input.ranges.is_none() {
                "all".to_string()
            } else {
                format!("{} selected", selected.len())
            };
            writeln!(vlog.writer(), "{indent}{total} pages total, {count_str}")
                .map_err(Error::ReportIo)?;
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

pub(crate) fn run_pipeline(
    inputs: &[Input],
    opts: &OutputOpts<'_>,
    verbose: bool,
) -> Result<(), Error> {
    let stdout = io::stdout();
    let mut report = stdout.lock();
    let stderr = io::stderr();
    if verbose {
        let mut vlog = VerboseLog::new(true, stderr.lock());
        let sources = load_sources(inputs, &mut vlog)?;
        let mut merged = merge::merge(sources)?;
        execute_run(&mut merged, opts, &mut report, &mut vlog)
    } else {
        let mut vlog = VerboseLog::new(false, io::sink());
        let sources = load_sources(inputs, &mut vlog)?;
        let mut merged = merge::merge(sources)?;
        execute_run(&mut merged, opts, &mut report, &mut vlog)
    }
}

#[cfg(test)]
#[path = "runner_tests.rs"]
mod tests;
