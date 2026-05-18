use super::*;
use lopdf::{Dictionary, Document, Object};
use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

// Bring VerboseLog into scope for all tests in this file.
use super::VerboseLog;

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
    let mut doc = Document::load("tests/fixtures/3pages.pdf").unwrap();
    let mut report = Vec::new();
    let mut log = Vec::new();
    let mut vlog = VerboseLog::new(false, &mut log);
    let opts = OutputOpts {
        output: None,
        count_pages: true,
        count_bytes: false,
        quiet: false,
    };
    execute_run(&mut doc, &opts, &mut report, &mut vlog).unwrap();
    assert_eq!(report, b"pages: 3\n");
    assert!(log.is_empty());
}

#[test]
fn execute_run_count_bytes_only_matches_serialized_size() {
    // Two independent docs because save_to may mutate internal state and
    // re-saving the same doc is not guaranteed to produce the same bytes.
    let mut doc_ref = Document::load("tests/fixtures/3pages.pdf").unwrap();
    let mut expected = Vec::new();
    doc_ref.save_to(&mut expected).unwrap();

    let mut doc = Document::load("tests/fixtures/3pages.pdf").unwrap();
    let mut report = Vec::new();
    let mut log = Vec::new();
    let mut vlog = VerboseLog::new(false, &mut log);
    let opts = OutputOpts {
        output: None,
        count_pages: false,
        count_bytes: true,
        quiet: false,
    };
    execute_run(&mut doc, &opts, &mut report, &mut vlog).unwrap();
    let line = std::str::from_utf8(&report).unwrap();
    let prefix = "bytes: ";
    assert!(line.starts_with(prefix), "got {line:?}");
    let n: usize = line[prefix.len()..line.len() - 1].parse().unwrap();
    assert_eq!(n, expected.len());
    assert!(log.is_empty());
}

#[test]
fn execute_run_both_flags_emit_pages_then_bytes() {
    let mut doc = Document::load("tests/fixtures/1page.pdf").unwrap();
    let mut report = Vec::new();
    let mut log = Vec::new();
    let mut vlog = VerboseLog::new(false, &mut log);
    let opts = OutputOpts {
        output: None,
        count_pages: true,
        count_bytes: true,
        quiet: false,
    };
    execute_run(&mut doc, &opts, &mut report, &mut vlog).unwrap();
    let s = std::str::from_utf8(&report).unwrap();
    assert!(s.starts_with("pages: 1\n"), "got {s:?}");
    assert!(s.contains("\nbytes: "), "got {s:?}");
    assert!(log.is_empty());
}

#[test]
fn execute_run_writes_file_when_output_given() {
    let mut doc = Document::load("tests/fixtures/3pages.pdf").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("out.pdf");
    let path = target.to_str().unwrap();

    let mut report = Vec::new();
    let mut log = Vec::new();
    let mut vlog = VerboseLog::new(false, &mut log);
    let opts = OutputOpts {
        output: Some(path),
        count_pages: false,
        count_bytes: true,
        quiet: false,
    };
    execute_run(&mut doc, &opts, &mut report, &mut vlog).unwrap();

    let on_disk = std::fs::metadata(&target).unwrap().len();
    let s = std::str::from_utf8(&report).unwrap();
    let reported: u64 = s.trim_start_matches("bytes: ").trim().parse().unwrap();
    assert_eq!(on_disk, reported);
    assert!(log.is_empty());
}

#[test]
fn execute_run_quiet_count_pages_omits_label() {
    let mut doc = Document::load("tests/fixtures/3pages.pdf").unwrap();
    let mut report = Vec::new();
    let mut log = Vec::new();
    let mut vlog = VerboseLog::new(false, &mut log);
    let opts = OutputOpts {
        output: None,
        count_pages: true,
        count_bytes: false,
        quiet: true,
    };
    execute_run(&mut doc, &opts, &mut report, &mut vlog).unwrap();
    assert_eq!(report, b"3\n");
    assert!(log.is_empty());
}

#[test]
fn execute_run_quiet_count_bytes_omits_label() {
    let mut doc_ref = Document::load("tests/fixtures/3pages.pdf").unwrap();
    let mut expected = Vec::new();
    doc_ref.save_to(&mut expected).unwrap();

    let mut doc = Document::load("tests/fixtures/3pages.pdf").unwrap();
    let mut report = Vec::new();
    let mut log = Vec::new();
    let mut vlog = VerboseLog::new(false, &mut log);
    let opts = OutputOpts {
        output: None,
        count_pages: false,
        count_bytes: true,
        quiet: true,
    };
    execute_run(&mut doc, &opts, &mut report, &mut vlog).unwrap();
    let s = std::str::from_utf8(&report).unwrap();
    let n: usize = s.trim_end().parse().unwrap();
    assert_eq!(n, expected.len());
    assert!(log.is_empty());
}

#[test]
fn execute_run_quiet_both_emits_two_bare_numbers() {
    let mut doc = Document::load("tests/fixtures/3pages.pdf").unwrap();
    let mut report = Vec::new();
    let mut log = Vec::new();
    let mut vlog = VerboseLog::new(false, &mut log);
    let opts = OutputOpts {
        output: None,
        count_pages: true,
        count_bytes: true,
        quiet: true,
    };
    execute_run(&mut doc, &opts, &mut report, &mut vlog).unwrap();
    let s = std::str::from_utf8(&report).unwrap();
    let mut lines = s.lines();
    assert_eq!(lines.next(), Some("3"));
    let bytes: u64 = lines.next().unwrap().parse().unwrap();
    assert!(bytes > 0);
    assert_eq!(lines.next(), None);
    assert!(log.is_empty());
}

#[test]
fn execute_run_quiet_no_counts_emits_nothing() {
    let mut doc = Document::load("tests/fixtures/3pages.pdf").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("out.pdf");
    let path = target.to_str().unwrap();

    let mut report = Vec::new();
    let mut log = Vec::new();
    let mut vlog = VerboseLog::new(false, &mut log);
    let opts = OutputOpts {
        output: Some(path),
        count_pages: false,
        count_bytes: false,
        quiet: true,
    };
    execute_run(&mut doc, &opts, &mut report, &mut vlog).unwrap();
    assert!(report.is_empty());
    assert!(log.is_empty());
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

#[test]
fn fmt_header_index_single_digit_total() {
    assert_eq!(fmt_header_index(1, 3), "[1/3]");
    assert_eq!(fmt_header_index(3, 3), "[3/3]");
}

#[test]
fn fmt_header_index_pads_for_two_digit_total() {
    assert_eq!(fmt_header_index(1, 10), "[ 1/10]");
    assert_eq!(fmt_header_index(9, 10), "[ 9/10]");
    assert_eq!(fmt_header_index(10, 10), "[10/10]");
}

#[test]
fn fmt_header_index_pads_for_three_digit_total() {
    assert_eq!(fmt_header_index(1, 100), "[  1/100]");
    assert_eq!(fmt_header_index(100, 100), "[100/100]");
}

fn write_tiny_pdf(n: usize) -> tempfile::NamedTempFile {
    let mut doc = tiny_doc(n);
    let tmp = tempfile::Builder::new()
        .prefix("pdfcat-")
        .suffix(".pdf")
        .tempfile()
        .unwrap();
    doc.save(tmp.path()).unwrap();
    tmp
}

#[test]
fn load_sources_verbose_logs_header_and_detail_with_pages() {
    let pdf = write_tiny_pdf(5);
    let path = pdf.path().to_str().unwrap();
    let inputs = vec![crate::cli::Input {
        path: path.to_string(),
        ranges: Some(crate::pages::parse_ranges("1,3").unwrap()),
    }];
    let mut log = Vec::new();
    let mut vlog = VerboseLog::new(true, &mut log);
    load_sources(&inputs, &mut vlog).unwrap();
    let s = std::str::from_utf8(&log).unwrap();
    let expected = format!("[1/1] {path} -p 1,3\n      5 pages total, 2 selected\n");
    assert_eq!(s, expected);
}

#[test]
fn load_sources_verbose_uses_all_when_ranges_absent() {
    let pdf = write_tiny_pdf(3);
    let path = pdf.path().to_str().unwrap();
    let inputs = vec![crate::cli::Input {
        path: path.to_string(),
        ranges: None,
    }];
    let mut log = Vec::new();
    let mut vlog = VerboseLog::new(true, &mut log);
    load_sources(&inputs, &mut vlog).unwrap();
    let s = std::str::from_utf8(&log).unwrap();
    let expected = format!("[1/1] {path}\n      3 pages total, all\n");
    assert_eq!(s, expected);
}

#[test]
fn load_sources_verbose_pads_header_index() {
    let pdf = write_tiny_pdf(2);
    let p = pdf.path().to_str().unwrap().to_string();
    let inputs: Vec<crate::cli::Input> = (0..10)
        .map(|_| crate::cli::Input {
            path: p.clone(),
            ranges: None,
        })
        .collect();
    let mut log = Vec::new();
    let mut vlog = VerboseLog::new(true, &mut log);
    load_sources(&inputs, &mut vlog).unwrap();
    let s = std::str::from_utf8(&log).unwrap();
    assert!(s.contains(&format!("[ 1/10] {p}\n")), "got: {s}");
    assert!(s.contains(&format!("[10/10] {p}\n")), "got: {s}");
}

#[test]
fn load_sources_silent_when_verbose_false() {
    let pdf = write_tiny_pdf(2);
    let inputs = vec![crate::cli::Input {
        path: pdf.path().to_str().unwrap().to_string(),
        ranges: None,
    }];
    let mut log = Vec::new();
    let mut vlog = VerboseLog::new(false, &mut log);
    load_sources(&inputs, &mut vlog).unwrap();
    assert!(log.is_empty());
}

#[test]
fn execute_run_verbose_logs_merged_and_wrote_with_bytes() {
    let mut doc = Document::load("tests/fixtures/3pages.pdf").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("out.pdf");
    let path = target.to_str().unwrap();

    let mut report = Vec::new();
    let mut log = Vec::new();
    let mut vlog = VerboseLog::new(true, &mut log);
    let opts = OutputOpts {
        output: Some(path),
        count_pages: false,
        count_bytes: false,
        quiet: false,
    };
    execute_run(&mut doc, &opts, &mut report, &mut vlog).unwrap();

    let on_disk = std::fs::metadata(&target).unwrap().len();
    let s = std::str::from_utf8(&log).unwrap();
    let lines: Vec<&str> = s.lines().collect();
    assert_eq!(lines.len(), 2, "got: {s}");
    assert_eq!(lines[0], "merged: 3 pages");
    let wrote_prefix = format!("wrote {path} (");
    assert!(lines[1].starts_with(&wrote_prefix), "got: {}", lines[1]);
    assert!(lines[1].ends_with(" bytes)"), "got: {}", lines[1]);
    let inside = lines[1]
        .trim_start_matches(&wrote_prefix)
        .trim_end_matches(" bytes)");
    let reported: u64 = inside.parse().unwrap();
    assert_eq!(reported, on_disk);
    assert!(report.is_empty(), "report should be empty: {report:?}");
}

#[test]
fn execute_run_verbose_no_output_skips_wrote_line() {
    let mut doc = Document::load("tests/fixtures/3pages.pdf").unwrap();
    let mut report = Vec::new();
    let mut log = Vec::new();
    let mut vlog = VerboseLog::new(true, &mut log);
    let opts = OutputOpts {
        output: None,
        count_pages: false,
        count_bytes: true,
        quiet: false,
    };
    execute_run(&mut doc, &opts, &mut report, &mut vlog).unwrap();
    let s = std::str::from_utf8(&log).unwrap();
    let lines: Vec<&str> = s.lines().collect();
    assert_eq!(lines, vec!["merged: 3 pages"]);
    let r = std::str::from_utf8(&report).unwrap();
    assert!(r.starts_with("bytes: "), "got: {r}");
}

#[test]
fn execute_run_verbose_with_count_bytes_still_writes_one_wrote_line() {
    let mut doc = Document::load("tests/fixtures/3pages.pdf").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("out.pdf");
    let path = target.to_str().unwrap();

    let mut report = Vec::new();
    let mut log = Vec::new();
    let mut vlog = VerboseLog::new(true, &mut log);
    let opts = OutputOpts {
        output: Some(path),
        count_pages: false,
        count_bytes: true,
        quiet: false,
    };
    execute_run(&mut doc, &opts, &mut report, &mut vlog).unwrap();

    let log_s = std::str::from_utf8(&log).unwrap();
    let report_s = std::str::from_utf8(&report).unwrap();
    let on_disk = std::fs::metadata(&target).unwrap().len();

    assert!(log_s.starts_with("merged: 3 pages\n"), "log: {log_s}");
    assert!(
        log_s.contains(&format!("wrote {path} ({on_disk} bytes)\n")),
        "log: {log_s}"
    );

    assert_eq!(report_s, format!("bytes: {on_disk}\n"));
}

#[test]
fn execute_run_quiet_and_verbose_coexist() {
    let mut doc = Document::load("tests/fixtures/3pages.pdf").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("out.pdf");
    let path = target.to_str().unwrap();

    let mut report = Vec::new();
    let mut log = Vec::new();
    let mut vlog = VerboseLog::new(true, &mut log); // verbose
    let opts = OutputOpts {
        output: Some(path),
        count_pages: true,
        count_bytes: true,
        quiet: true,
    };
    execute_run(&mut doc, &opts, &mut report, &mut vlog).unwrap();

    let on_disk = std::fs::metadata(&target).unwrap().len();
    let report_s = std::str::from_utf8(&report).unwrap();
    let log_s = std::str::from_utf8(&log).unwrap();

    let mut report_lines = report_s.lines();
    assert_eq!(report_lines.next(), Some("3"));
    let bytes_line = report_lines.next().unwrap();
    assert_eq!(bytes_line.parse::<u64>().unwrap(), on_disk);
    assert_eq!(report_lines.next(), None);

    assert!(log_s.starts_with("merged: 3 pages\n"), "log: {log_s}");
    assert!(
        log_s.contains(&format!("wrote {path} ({on_disk} bytes)\n")),
        "log: {log_s}"
    );
}

#[test]
fn verbose_log_disabled_writes_nothing() {
    let mut sink = Vec::<u8>::new();
    let mut vlog = VerboseLog::new(false, &mut sink);
    vlog.log_merged(7).unwrap();
    vlog.log_wrote("out.pdf", 12345).unwrap();
    vlog.flush().unwrap();
    assert!(sink.is_empty());
}

#[test]
fn verbose_log_enabled_log_merged_writes_line() {
    let mut sink = Vec::<u8>::new();
    let mut vlog = VerboseLog::new(true, &mut sink);
    vlog.log_merged(42).unwrap();
    assert_eq!(sink, b"merged: 42 pages\n");
}

#[test]
fn verbose_log_enabled_log_wrote_writes_line() {
    let mut sink = Vec::<u8>::new();
    let mut vlog = VerboseLog::new(true, &mut sink);
    vlog.log_wrote("out.pdf", 12345).unwrap();
    assert_eq!(sink, b"wrote out.pdf (12345 bytes)\n");
}

#[test]
fn verbose_log_enabled_log_input_detail_with_ranges() {
    let mut sink = Vec::<u8>::new();
    let mut vlog = VerboseLog::new(true, &mut sink);
    vlog.log_input_detail("      ", 10, Some(3)).unwrap();
    assert_eq!(sink, b"      10 pages total, 3 selected\n");
}

#[test]
fn verbose_log_enabled_log_input_detail_all() {
    let mut sink = Vec::<u8>::new();
    let mut vlog = VerboseLog::new(true, &mut sink);
    vlog.log_input_detail("      ", 5, None).unwrap();
    assert_eq!(sink, b"      5 pages total, all\n");
}

fn siblings_of(target: &Path) -> Vec<OsString> {
    let parent = target.parent().unwrap();
    let target_name = target.file_name().unwrap();
    fs::read_dir(parent)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name())
        .filter(|n| n != target_name)
        .collect()
}

#[test]
fn atomic_write_renames_tmp_to_target_on_success() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("out.pdf");

    let count = atomic_write(target.to_str().unwrap(), |w| w.write_all(b"final bytes")).unwrap();
    assert_eq!(count, b"final bytes".len() as u64);
    assert_eq!(fs::read(&target).unwrap(), b"final bytes");

    let leftovers = siblings_of(&target);
    assert!(leftovers.is_empty(), "tmp leftover: {leftovers:?}");
}

#[test]
fn atomic_write_preserves_existing_target_on_failure() {
    // The hallmark of atomic write: if the save fails partway through,
    // the pre-existing file at the destination must still be intact, and
    // no half-written .tmp must be left behind.
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("out.pdf");
    fs::write(&target, b"ORIGINAL").unwrap();

    let result = atomic_write(target.to_str().unwrap(), |w| {
        w.write_all(b"partial junk")?;
        Err(io::Error::other("simulated mid-write failure"))
    });
    assert!(result.is_err(), "expected the save callback to surface");

    assert_eq!(fs::read(&target).unwrap(), b"ORIGINAL");

    let leftovers = siblings_of(&target);
    assert!(leftovers.is_empty(), "tmp leftover: {leftovers:?}");
}

#[test]
fn load_one_source_missing_file_returns_open_input() {
    use crate::Error;
    let input = crate::cli::Input {
        path: "/nonexistent/pdfcat-test-missing.pdf".to_string(),
        ranges: None,
    };
    let mut log = Vec::new();
    let mut vlog = VerboseLog::new(false, &mut log);
    let err = load_one_source(&input, 1, 1, "", &mut vlog).unwrap_err();
    match err {
        Error::OpenInput { path, .. } => assert_eq!(path, input.path),
        other => panic!("expected OpenInput, got {other:?}"),
    }
}

#[test]
fn load_one_source_corrupt_file_returns_parse_input() {
    use crate::Error;
    let mut tmp = tempfile::Builder::new()
        .prefix("pdfcat-")
        .suffix(".pdf")
        .tempfile()
        .unwrap();
    tmp.write_all(b"this is not a PDF").unwrap();
    let input = crate::cli::Input {
        path: tmp.path().to_str().unwrap().to_string(),
        ranges: None,
    };
    let mut log = Vec::new();
    let mut vlog = VerboseLog::new(false, &mut log);
    let err = load_one_source(&input, 1, 1, "", &mut vlog).unwrap_err();
    match err {
        Error::ParseInput { path, .. } => assert_eq!(path, input.path),
        other => panic!("expected ParseInput, got {other:?}"),
    }
}
