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
    execute_run(&mut doc, None, true, false, false, &mut report).unwrap();
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
    execute_run(&mut doc, None, false, true, false, &mut report).unwrap();
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
    execute_run(&mut doc, None, true, true, false, &mut report).unwrap();
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
    execute_run(&mut doc, Some(&path), false, true, false, &mut report).unwrap();

    let on_disk = std::fs::metadata(&tmp).unwrap().len();
    let s = std::str::from_utf8(&report).unwrap();
    let reported: u64 = s.trim_start_matches("bytes: ").trim().parse().unwrap();
    assert_eq!(on_disk, reported);

    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn execute_run_quiet_count_pages_omits_label() {
    let mut doc = tiny_doc(3);
    let mut report = Vec::new();
    execute_run(&mut doc, None, true, false, true, &mut report).unwrap();
    assert_eq!(report, b"3\n");
}

#[test]
fn execute_run_quiet_count_bytes_omits_label() {
    let mut doc_ref = tiny_doc(2);
    let mut expected = Vec::new();
    doc_ref.save_to(&mut expected).unwrap();

    let mut doc = tiny_doc(2);
    let mut report = Vec::new();
    execute_run(&mut doc, None, false, true, true, &mut report).unwrap();
    let s = std::str::from_utf8(&report).unwrap();
    let n: usize = s.trim_end().parse().unwrap();
    assert_eq!(n, expected.len());
}

#[test]
fn execute_run_quiet_both_emits_two_bare_numbers() {
    let mut doc = tiny_doc(4);
    let mut report = Vec::new();
    execute_run(&mut doc, None, true, true, true, &mut report).unwrap();
    let s = std::str::from_utf8(&report).unwrap();
    let mut lines = s.lines();
    assert_eq!(lines.next(), Some("4"));
    let bytes: u64 = lines.next().unwrap().parse().unwrap();
    assert!(bytes > 0);
    assert_eq!(lines.next(), None);
}

#[test]
fn execute_run_quiet_no_counts_emits_nothing() {
    let mut doc = tiny_doc(2);
    let tmp =
        std::env::temp_dir().join(format!("pdfcat-quiet-no-counts-{}.pdf", std::process::id()));
    let _ = std::fs::remove_file(&tmp);
    let path = tmp.to_str().unwrap().to_string();

    let mut report = Vec::new();
    execute_run(&mut doc, Some(&path), false, false, true, &mut report).unwrap();
    assert!(report.is_empty());

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
