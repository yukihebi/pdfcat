use std::fmt;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use lopdf::Document;
use tempfile::Builder;

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
/// All methods are no-ops when `enabled` is `false`.
struct VerboseLog<W: Write> {
    enabled: bool,
    inner: W,
}

impl<W: Write> VerboseLog<W> {
    fn new(enabled: bool, inner: W) -> Self {
        Self { enabled, inner }
    }

    /// Append a formatted line + `\n` to the log. No-op when disabled.
    fn log(&mut self, args: fmt::Arguments<'_>) -> Result<(), Error> {
        if !self.enabled {
            return Ok(());
        }
        writeln!(self.inner, "{args}").map_err(Error::ReportIo)
    }

    /// Flush the underlying writer. No-op when disabled.
    fn flush(&mut self) -> Result<(), Error> {
        if !self.enabled {
            return Ok(());
        }
        self.inner.flush().map_err(Error::ReportIo)
    }

    /// Emit the `merged: N pages` line (after merging is complete).
    fn log_merged(&mut self, page_count: usize) -> Result<(), Error> {
        self.log(format_args!("merged: {page_count} pages"))
    }

    /// Emit `wrote <path> (<B> bytes)` after a successful file write.
    fn log_wrote(&mut self, path: &str, bytes: u64) -> Result<(), Error> {
        self.log(format_args!("wrote {path} ({bytes} bytes)"))
    }

    /// Emit `[i/N] <path>[ -p <spec>]` *and flush*, so that a subsequent
    /// `Document::load` failure is attributable to this input.
    fn log_input_header(&mut self, idx: usize, total: usize, input: &Input) -> Result<(), Error> {
        let head = fmt_header_index(idx, total);
        match &input.ranges {
            Some(ranges) => self.log(format_args!(
                "{head} {} -p {}",
                input.path,
                fmt_ranges(ranges)
            ))?,
            None => self.log(format_args!("{head} {}", input.path))?,
        }
        self.flush()
    }

    /// Emit `<indent><total> pages total, <count_str>` where `count_str`
    /// is `all` when `selected` is `None`, or `<N> selected` otherwise.
    fn log_input_detail(
        &mut self,
        indent: &str,
        total_pages: u32,
        selected: Option<usize>,
    ) -> Result<(), Error> {
        match selected {
            None => self.log(format_args!("{indent}{total_pages} pages total, all")),
            Some(n) => self.log(format_args!(
                "{indent}{total_pages} pages total, {n} selected"
            )),
        }
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
    vlog.log_merged(merged.get_pages().len())?;
    if opts.count_pages {
        write_count(report, "pages", merged.get_pages().len(), opts.quiet)?;
    }
    match opts.output {
        Some(path) => write_to_file(merged, path, opts, report, vlog),
        None if opts.count_bytes => count_bytes_to_sink(merged, opts, report),
        None => Ok(()),
    }
}

/// Write the merged document to `path` atomically: serialise to a sibling
/// `.tmp` file first, then rename. A failure anywhere along the way leaves
/// any pre-existing file at `path` untouched and cleans the tmp behind us.
/// Emits the labelled byte count to `report` when `opts.count_bytes`, and
/// the verbose `wrote ... (B bytes)` line when verbose is enabled.
fn write_to_file<R: Write, W: Write>(
    merged: &mut Document,
    path: &str,
    opts: &OutputOpts<'_>,
    report: &mut R,
    vlog: &mut VerboseLog<W>,
) -> Result<(), Error> {
    let count = atomic_write(path, |w| merged.save_to(w))?;
    if opts.count_bytes {
        write_count(report, "bytes", count, opts.quiet)?;
    }
    vlog.log_wrote(path, count)?;
    Ok(())
}

/// Write to a sibling tmp file via `save`, then atomically replace
/// `final_path`. Returns the number of bytes the inner writer accepted.
/// `NamedTempFile`'s `Drop` removes the tmp on any early return, so a
/// failure leaves the previous contents of `final_path` (if any) intact.
fn atomic_write<F>(final_path: &str, save: F) -> Result<u64, Error>
where
    F: FnOnce(&mut CountingWriter<BufWriter<&mut File>>) -> io::Result<()>,
{
    fn to_err(path: &str) -> impl FnOnce(io::Error) -> Error + '_ {
        move |source| Error::WriteOutput {
            path: path.to_string(),
            source,
        }
    }
    let parent = Path::new(final_path)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut tmp = Builder::new()
        .prefix(".pdfcat-")
        .suffix(".tmp")
        .tempfile_in(parent)
        .map_err(to_err(final_path))?;
    let count = {
        let mut w = CountingWriter::new(BufWriter::new(tmp.as_file_mut()));
        save(&mut w).map_err(to_err(final_path))?;
        w.flush().map_err(to_err(final_path))?;
        w.count()
    };
    tmp.persist(final_path).map_err(|e| Error::WriteOutput {
        path: final_path.to_string(),
        source: e.error,
    })?;
    Ok(count)
}

/// Serialise `merged` to a sink just to count bytes; emits the labelled
/// byte count to `report`.
fn count_bytes_to_sink<R: Write>(
    merged: &mut Document,
    opts: &OutputOpts<'_>,
    report: &mut R,
) -> Result<(), Error> {
    let mut w = CountingWriter::new(io::sink());
    merged.save_to(&mut w).map_err(Error::SerializeForCount)?;
    write_count(report, "bytes", w.count(), opts.quiet)
}

/// Load each input document and resolve its page selection against the actual
/// page count. When verbose is enabled, write a header line to the log
/// *before* loading each file (so failures are attributable) and a detail line
/// after.
fn load_sources<W: Write>(
    inputs: &[Input],
    vlog: &mut VerboseLog<W>,
) -> Result<Vec<(String, Document, Vec<u32>)>, Error> {
    let total = inputs.len();
    let indent = " ".repeat(fmt_header_index(1, total).len() + 1);
    let mut sources = Vec::with_capacity(total);
    for (i, input) in inputs.iter().enumerate() {
        sources.push(load_one_source(input, i + 1, total, &indent, vlog)?);
    }
    Ok(sources)
}

/// Load one input PDF: emit the verbose header (flushed) via `vlog`, then
/// load and resolve page selection, then emit the verbose detail.
fn load_one_source<W: Write>(
    input: &Input,
    idx: usize,
    total: usize,
    indent: &str,
    vlog: &mut VerboseLog<W>,
) -> Result<(String, Document, Vec<u32>), Error> {
    vlog.log_input_header(idx, total, input)?;
    let doc = Document::load(&input.path).map_err(|source| match source {
        lopdf::Error::IO(io) => Error::OpenInput {
            path: input.path.clone(),
            source: io,
        },
        other => Error::ParseInput {
            path: input.path.clone(),
            source: other,
        },
    })?;
    let total_pages = doc.get_pages().len() as u32;
    if total_pages == 0 {
        return Err(Error::NoPages {
            path: input.path.clone(),
        });
    }
    let selected: Vec<u32> = match &input.ranges {
        None => (1..=total_pages).collect(),
        Some(ranges) => {
            resolve_ranges(ranges, total_pages).map_err(|source| Error::PageSelection {
                path: input.path.clone(),
                source,
            })?
        }
    };
    let count = input.ranges.as_ref().map(|_| selected.len());
    vlog.log_input_detail(indent, total_pages, count)?;
    Ok((input.path.clone(), doc, selected))
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
