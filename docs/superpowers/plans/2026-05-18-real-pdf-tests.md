# Real-PDF Test Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move tests in `src/merge_tests.rs` and `src/runner_tests.rs` off in-memory synthesized `Document` factories onto fixtures loaded via `Document::load(...)`, and add tests for currently uncovered branches (`version.max`, `NoPages`, `PageSelection`).

**Architecture:** 6 PDF fixtures under `tests/fixtures/` — 2 existing LuaLaTeX (`1page.pdf`, `3pages.pdf`), 2 new LuaLaTeX (`with-outline.pdf`, `v1.7.pdf`), 2 new synthesized via lopdf (`inherited-resources.pdf`, `0page.pdf`). Page identity verified via decompressed `/Contents` stream byte comparison.

**Tech Stack:** Rust 2024 edition, lopdf 0.40, tempfile 3, LuaLaTeX (TeX Live 2026 Homebrew).

**Branch:** `refactor/real-pdf-tests` (already created from `main` after the tempfile PR landed). Spec at `docs/superpowers/specs/2026-05-18-real-pdf-tests-design.md` is already committed.

**Environment note:** LuaLaTeX runs need `TEXMFVAR` / `TEXMFCACHE` set to a writable dir (e.g. `$TMPDIR/texmfvar`) when the sandbox blocks the default `~/Library` cache.

---

## Task 1: Build LuaLaTeX fixtures

**Files:**
- Create: `.local/with-outline.tex` (gitignored, kept for regeneration)
- Create: `.local/v1.7.tex` (gitignored, kept for regeneration)
- Create: `tests/fixtures/with-outline.pdf`
- Create: `tests/fixtures/v1.7.pdf`

- [ ] **Step 1: Write `.local/with-outline.tex`**

```latex
\documentclass{ltjsarticle}
\usepackage{hyperref}
\begin{document}
\HUGE
\section{First}
PAGE1
\clearpage
\section{Second}
PAGE2
\end{document}
```

- [ ] **Step 2: Write `.local/v1.7.tex`**

```latex
\documentclass{ltjsarticle}
\pdfvariable minorversion=7
\begin{document}
\HUGE
PAGE1(v1.7.pdf)
\end{document}
```

- [ ] **Step 3: Build both deterministically**

From `.local/`:
```
SOURCE_DATE_EPOCH=0 TEXMFVAR="$TMPDIR/texmfvar" TEXMFCACHE="$TMPDIR/texmfvar" \
  latexmk with-outline.tex v1.7.tex
```

Expected: `.local/with-outline.pdf` and `.local/v1.7.pdf` produced.

- [ ] **Step 4: Verify structural properties**

```
cargo run --quiet -- .local/with-outline.pdf --npages
```
Expected: `pages: 2`

```
cargo run --quiet -- .local/v1.7.pdf --npages
```
Expected: `pages: 1`

For the version, inspect raw header:
```
head -c 8 .local/v1.7.pdf
```
Expected: `%PDF-1.7`

- [ ] **Step 5: Move to fixtures dir**

```
cp .local/with-outline.pdf .local/v1.7.pdf tests/fixtures/
```

---

## Task 2: Synthetic fixture generator example

**Files:**
- Create: `examples/gen_synthetic_fixtures.rs`
- Create: `tests/fixtures/inherited-resources.pdf` (output)
- Create: `tests/fixtures/0page.pdf` (output)

- [ ] **Step 1: Write the example program**

`examples/gen_synthetic_fixtures.rs`:
```rust
//! Generate the synthetic PDF fixtures that LuaLaTeX cannot produce.
//!
//! Run from the crate root:
//!     cargo run --example gen_synthetic_fixtures
//!
//! Outputs:
//!   tests/fixtures/inherited-resources.pdf
//!   tests/fixtures/0page.pdf

use lopdf::{Dictionary, Document, Object};

fn write_inherited_resources(path: &str) {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.add_object(Dictionary::new());

    // /Resources lives on the /Pages node — leaves inherit it.
    let mut font = Dictionary::new();
    font.set("Type", "Font");
    font.set("Subtype", "Type1");
    font.set("BaseFont", "Helvetica");
    let font_id = doc.add_object(font);
    let mut fonts = Dictionary::new();
    fonts.set("F1", font_id);
    let mut resources = Dictionary::new();
    resources.set("Font", fonts);

    let kids: Vec<Object> = (0..2)
        .map(|_| {
            let mut page = Dictionary::new();
            page.set("Type", "Page");
            page.set("Parent", pages_id);
            page.set(
                "MediaBox",
                Object::Array(vec![
                    Object::Integer(0),
                    Object::Integer(0),
                    Object::Integer(100),
                    Object::Integer(100),
                ]),
            );
            // NO /Resources on the leaf — it inherits from /Pages.
            Object::Reference(doc.add_object(page))
        })
        .collect();

    let mut pages = Dictionary::new();
    pages.set("Type", "Pages");
    pages.set("Count", 2i64);
    pages.set("Kids", kids);
    pages.set("Resources", resources);
    doc.objects.insert(pages_id, Object::Dictionary(pages));

    let mut catalog = Dictionary::new();
    catalog.set("Type", "Catalog");
    catalog.set("Pages", pages_id);
    let catalog_id = doc.add_object(catalog);
    doc.trailer.set("Root", catalog_id);

    doc.save(path).unwrap();
}

fn write_zero_pages(path: &str) {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.add_object(Dictionary::new());
    let mut pages = Dictionary::new();
    pages.set("Type", "Pages");
    pages.set("Count", 0i64);
    pages.set("Kids", Object::Array(vec![]));
    doc.objects.insert(pages_id, Object::Dictionary(pages));

    let mut catalog = Dictionary::new();
    catalog.set("Type", "Catalog");
    catalog.set("Pages", pages_id);
    let catalog_id = doc.add_object(catalog);
    doc.trailer.set("Root", catalog_id);

    doc.save(path).unwrap();
}

fn main() {
    write_inherited_resources("tests/fixtures/inherited-resources.pdf");
    write_zero_pages("tests/fixtures/0page.pdf");
    println!("Wrote tests/fixtures/inherited-resources.pdf");
    println!("Wrote tests/fixtures/0page.pdf");
}
```

- [ ] **Step 2: Run the example**

```
cargo run --example gen_synthetic_fixtures
```

Expected:
```
Wrote tests/fixtures/inherited-resources.pdf
Wrote tests/fixtures/0page.pdf
```

- [ ] **Step 3: Sanity-check inherited-resources via pdfcat**

```
cargo run --quiet -- tests/fixtures/inherited-resources.pdf --npages
```
Expected: `pages: 2`

- [ ] **Step 4: Sanity-check 0page**

```
cargo run --quiet -- tests/fixtures/0page.pdf --npages 2>&1
```
Expected: non-zero exit, stderr contains `this PDF has no pages`.

---

## Task 3: Update fixtures README and commit fixture set

**Files:**
- Modify: `tests/fixtures/README.md`

- [ ] **Step 1: Update the README**

Replace `tests/fixtures/README.md` with:
```markdown
# Test fixtures

Treat these PDFs as opaque inputs. Tests must not depend on
producer-specific quirks (e.g. exact `/Producer` strings); rely only on
structural facts (page count, page size, presence/absence of features).

To add a new fixture, drop the PDF here and append one line below noting
where it came from.

## Provenance

- `1page.pdf` — LuaLaTeX (`ltjsarticle`), one page with a large "PAGE1"
  body. PDF 1.5, ~5 KB. Built with `SOURCE_DATE_EPOCH=0` for byte-stable
  output.
- `3pages.pdf` — LuaLaTeX (`ltjsarticle`), three pages labelled
  "PAGE1"/"PAGE2"/"PAGE3". PDF 1.5, ~7 KB. Built with
  `SOURCE_DATE_EPOCH=0`.
- `with-outline.pdf` — LuaLaTeX (`ltjsarticle`) + `hyperref` with two
  `\section`s. Has `/Outlines`. PDF 1.5. Used to verify that
  `DROPPED_CATALOG_KEYS` are stripped during merge.
- `v1.7.pdf` — LuaLaTeX (`ltjsarticle`) with `\pdfvariable
  minorversion=7`. Single page. PDF 1.7. Used to verify that `merge`
  picks the highest PDF version among inputs.
- `inherited-resources.pdf` — **Synthesized** via
  `cargo run --example gen_synthetic_fixtures`. Two-page document whose
  single `/Pages` node carries `/Resources` (a Type1 Helvetica font);
  leaves omit `/Resources` and inherit it. Used to verify
  `INHERITABLE` flatten + `STALE_PAGES_KEYS` strip.
- `0page.pdf` — **Synthesized** via
  `cargo run --example gen_synthetic_fixtures`. Catalog + `/Pages` with
  `/Count 0` and empty `/Kids`. Used to verify the `NoPages` error
  branch in `runner::load_one_source`.
```

- [ ] **Step 2: Stage and commit the fixture set**

```
git add examples/gen_synthetic_fixtures.rs tests/fixtures/
git status --short
```

Expected: `examples/gen_synthetic_fixtures.rs` and 6 entries under `tests/fixtures/` staged.

```
git commit -m "Add real-PDF test fixtures (LaTeX + synthesized)"
```

---

## Task 4: Add `page_content_bytes` helper in merge_tests.rs

**Files:**
- Modify: `src/merge_tests.rs:67-81` (replace `page_widths` with the new helper; keep `page_ids`)

- [ ] **Step 1: Replace `page_widths` with `page_content_bytes`**

In `src/merge_tests.rs`, replace lines 67-77 (the `page_widths` function and its docstring) with:

```rust
/// The decompressed `/Contents` stream bytes of every page, in page
/// order. Used as a page-identity fingerprint that survives `merge`
/// (which carries content streams byte-for-byte).
fn page_content_bytes(doc: &Document) -> Vec<Vec<u8>> {
    doc.get_pages()
        .values()
        .map(|&id| page_contents_one(doc, id))
        .collect()
}

fn page_contents_one(doc: &Document, page_id: ObjectId) -> Vec<u8> {
    let page = doc.get_object(page_id).unwrap().as_dict().unwrap();
    let contents_id = page.get(b"Contents").unwrap().as_reference().unwrap();
    let mut stream = doc
        .get_object(contents_id)
        .unwrap()
        .as_stream()
        .unwrap()
        .clone();
    let _ = stream.decompress();
    stream.content
}
```

- [ ] **Step 2: Verify the file still compiles**

```
cargo build --tests 2>&1 | tail -5
```

Expected: warnings about unused helpers (`doc_with_widths`, `doc_with_inherited_box`) and / or unused functions are acceptable at this stage; no errors.

If callers of `page_widths` cause errors, they remain in the file. They'll be migrated in Task 5. To suppress those errors temporarily, allow this task to leave a non-compiling intermediate state — but it's cleaner to do Step 1 and Task 5 in a single commit. **Defer the commit until end of Task 5.**

---

## Task 5: Migrate merge_tests.rs to real fixtures

**Files:**
- Modify: `src/merge_tests.rs` (whole file rewrite)

This task migrates all 10 existing merge tests, adds 1 new test, and deletes the now-dead helpers. Done as a single commit at the end.

- [ ] **Step 1: Migrate `concatenates_pages_in_order`**

Replace the test body (lines 83-101) with:

```rust
#[test]
fn concatenates_pages_in_order() {
    let one = Document::load("tests/fixtures/1page.pdf").unwrap();
    let three = Document::load("tests/fixtures/3pages.pdf").unwrap();
    let expected = [
        page_contents_one(&one, one.get_pages()[&1]),
        page_contents_one(&three, three.get_pages()[&1]),
        page_contents_one(&three, three.get_pages()[&2]),
        page_contents_one(&three, three.get_pages()[&3]),
    ];

    let merged = merge(vec![
        (
            "1page.pdf".to_string(),
            Document::load("tests/fixtures/1page.pdf").unwrap(),
            vec![1],
        ),
        (
            "3pages.pdf".to_string(),
            Document::load("tests/fixtures/3pages.pdf").unwrap(),
            vec![1, 2, 3],
        ),
    ])
    .unwrap();
    assert_eq!(page_content_bytes(&merged), expected);
    let root = merged.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let catalog = merged.get_object(root).unwrap().as_dict().unwrap();
    let pages_id = catalog.get(b"Pages").unwrap().as_reference().unwrap();
    let pages = merged.get_object(pages_id).unwrap().as_dict().unwrap();
    assert_eq!(pages.get(b"Count").unwrap().as_i64().unwrap(), 4);
}
```

- [ ] **Step 2: Migrate `selects_and_reorders_pages`**

Replace lines 103-112 with:

```rust
#[test]
fn selects_and_reorders_pages() {
    let src = Document::load("tests/fixtures/3pages.pdf").unwrap();
    let expected = [
        page_contents_one(&src, src.get_pages()[&3]),
        page_contents_one(&src, src.get_pages()[&1]),
        page_contents_one(&src, src.get_pages()[&2]),
    ];
    let merged = merge(vec![(
        "src.pdf".to_string(),
        Document::load("tests/fixtures/3pages.pdf").unwrap(),
        vec![3, 1, 2],
    )])
    .unwrap();
    assert_eq!(page_content_bytes(&merged), expected);
}
```

- [ ] **Step 3: Migrate `duplicate_page_gets_a_fresh_id`**

Replace lines 114-126:

```rust
#[test]
fn duplicate_page_gets_a_fresh_id() {
    let src = Document::load("tests/fixtures/3pages.pdf").unwrap();
    let p1 = page_contents_one(&src, src.get_pages()[&1]);
    let p2 = page_contents_one(&src, src.get_pages()[&2]);
    let merged = merge(vec![(
        "src.pdf".to_string(),
        Document::load("tests/fixtures/3pages.pdf").unwrap(),
        vec![1, 1, 2],
    )])
    .unwrap();
    assert_eq!(page_content_bytes(&merged), [p1.clone(), p1, p2]);
    let ids = page_ids(&merged);
    let unique: HashSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), 3, "every page must be a distinct object");
}
```

- [ ] **Step 4: Migrate `duplicate_then_more_inputs_keeps_ids_disjoint`**

Replace lines 128-140:

```rust
#[test]
fn duplicate_then_more_inputs_keeps_ids_disjoint() {
    // A duplicated page in the first input must not steal an id that the
    // second input's objects will be renumbered onto.
    let three = Document::load("tests/fixtures/3pages.pdf").unwrap();
    let one = Document::load("tests/fixtures/1page.pdf").unwrap();
    let p1 = page_contents_one(&three, three.get_pages()[&1]);
    let p_one = page_contents_one(&one, one.get_pages()[&1]);
    let merged = merge(vec![
        (
            "a.pdf".to_string(),
            Document::load("tests/fixtures/3pages.pdf").unwrap(),
            vec![1, 1],
        ),
        (
            "b.pdf".to_string(),
            Document::load("tests/fixtures/1page.pdf").unwrap(),
            vec![1, 1],
        ),
    ])
    .unwrap();
    assert_eq!(
        page_content_bytes(&merged),
        [p1.clone(), p1, p_one.clone(), p_one]
    );
    let unique: HashSet<_> = page_ids(&merged).into_iter().collect();
    assert_eq!(unique.len(), 4);
}
```

- [ ] **Step 5: Migrate `keeps_supporting_objects_but_drops_outlines`**

Replace lines 142-175:

```rust
#[test]
fn keeps_supporting_objects_but_drops_outlines() {
    // with-outline.pdf has /Outlines (from hyperref's \section) and real
    // page content streams. After merging, content streams must survive
    // and /Outlines must not.
    let merged = merge(vec![(
        "with-outline.pdf".to_string(),
        Document::load("tests/fixtures/with-outline.pdf").unwrap(),
        vec![1, 2],
    )])
    .unwrap();

    // Every merged page still has a usable /Contents reference.
    for &id in merged.get_pages().values() {
        let page = merged.get_object(id).unwrap().as_dict().unwrap();
        let contents_ref = page.get(b"Contents").unwrap().as_reference().unwrap();
        assert!(
            merged.get_object(contents_ref).is_ok(),
            "content stream kept"
        );
    }

    let root = merged.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let catalog = merged.get_object(root).unwrap().as_dict().unwrap();
    assert!(catalog.get(b"Outlines").is_err(), "/Outlines dropped");
}
```

- [ ] **Step 6: Migrate the inheritance test (rename + new fixture)**

Replace lines 177-196:

```rust
#[test]
fn inheritable_attrs_flattened_and_stale_keys_stripped() {
    // inherited-resources.pdf carries /Resources on its /Pages node;
    // leaves do not have /Resources. After merge, (a) each leaf must
    // carry the flattened /Resources, and (b) the rebuilt /Pages node
    // must no longer carry /Resources (STALE_PAGES_KEYS strip).
    let merged = merge(vec![(
        "inherited-resources.pdf".to_string(),
        Document::load("tests/fixtures/inherited-resources.pdf").unwrap(),
        vec![1, 2],
    )])
    .unwrap();

    for &id in merged.get_pages().values() {
        let page = merged.get_object(id).unwrap().as_dict().unwrap();
        assert!(
            page.get(b"Resources").is_ok(),
            "leaf must have /Resources after flattening"
        );
    }

    let root = merged.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let catalog = merged.get_object(root).unwrap().as_dict().unwrap();
    let pages_id = catalog.get(b"Pages").unwrap().as_reference().unwrap();
    let pages = merged.get_object(pages_id).unwrap().as_dict().unwrap();
    assert!(
        pages.get(b"Resources").is_err(),
        "/Pages node must not retain stale /Resources"
    );
}
```

- [ ] **Step 7: Migrate `info_dictionary_is_carried_over`**

Replace lines 204-219:

```rust
#[test]
fn info_dictionary_is_carried_over() {
    // 1page.pdf has /Info (LuaTeX writes /Producer etc.).
    let merged = merge(vec![(
        "1page.pdf".to_string(),
        Document::load("tests/fixtures/1page.pdf").unwrap(),
        vec![1],
    )])
    .unwrap();
    let info_ref = merged.trailer.get(b"Info").unwrap().as_reference().unwrap();
    let info = merged.get_object(info_ref).unwrap().as_dict().unwrap();
    // Don't assert the exact producer string (toolchain-dependent);
    // just that /Producer is present.
    assert!(info.get(b"Producer").is_ok());
}
```

- [ ] **Step 8: Migrate `empty_selection_yields_zero_page_document`**

Replace lines 233-246:

```rust
#[test]
fn empty_selection_yields_zero_page_document() {
    // With NoPages removed, asking for zero pages from a real document
    // now produces a valid 0-page merged document. This path is
    // unreachable from the CLI (parse_ranges rejects empty specs, and
    // load_one_source rejects 0-page inputs), but the in-module
    // contract is pinned here.
    let merged = merge(vec![(
        "3pages.pdf".to_string(),
        Document::load("tests/fixtures/3pages.pdf").unwrap(),
        vec![],
    )])
    .unwrap();
    assert_eq!(merged.get_pages().len(), 0);
}
```

- [ ] **Step 9: Add `picks_highest_pdf_version`**

Append at end of file (above the `#[cfg(test)] #[path...] mod` block if present, else end of file):

```rust
#[test]
fn picks_highest_pdf_version() {
    // 3pages.pdf is PDF 1.5; v1.7.pdf is PDF 1.7. merge should pick 1.7.
    let merged = merge(vec![
        (
            "3pages.pdf".to_string(),
            Document::load("tests/fixtures/3pages.pdf").unwrap(),
            vec![1],
        ),
        (
            "v1.7.pdf".to_string(),
            Document::load("tests/fixtures/v1.7.pdf").unwrap(),
            vec![1],
        ),
    ])
    .unwrap();
    assert_eq!(merged.version, "1.7");
}
```

- [ ] **Step 10: Delete dead helpers**

Remove from `src/merge_tests.rs`:
- The `doc_with_widths` function (lines ~5-29)
- The `doc_with_inherited_box` function (lines ~33-56)
- The `media_box` helper if it's no longer referenced (was used by `doc_with_widths`)

Keep:
- `page_content_bytes` and `page_contents_one` (added in Task 4)
- `page_ids` (still used)

- [ ] **Step 11: Verify all merge tests pass**

```
cargo test --quiet merge:: 2>&1 | tail -10
```

Expected: 11 tests pass (10 migrated + 1 new).

- [ ] **Step 12: Commit**

```
git add src/merge_tests.rs
git commit -m "Migrate merge_tests.rs to real PDF fixtures"
```

---

## Task 6a: Migrate `execute_run_*` tests in runner_tests.rs

**Files:**
- Modify: `src/runner_tests.rs` (12 test bodies)

**Pattern:** Replace `let mut doc = tiny_doc(n);` with
`let mut doc = Document::load("tests/fixtures/<fixture>.pdf").unwrap();`
and adjust any hard-coded page count in assertions.

Mapping (test → fixture → assertion change):
- `count_pages_only_writes_no_file`: 3pages.pdf, assertion `"pages: 3\n"` (unchanged)
- `count_bytes_only_matches_serialized_size`: 3pages.pdf (both `doc_ref` and `doc`)
- `both_flags_emit_pages_then_bytes`: 1page.pdf, assertion `"pages: 1\n"`
- `writes_file_when_output_given`: 3pages.pdf
- `quiet_count_pages_omits_label`: 3pages.pdf, assertion `b"3\n"`
- `quiet_count_bytes_omits_label`: 3pages.pdf
- `quiet_both_emits_two_bare_numbers`: 3pages.pdf, assertion `Some("3")` (was 4)
- `quiet_no_counts_emits_nothing`: 3pages.pdf
- `verbose_logs_merged_and_wrote_with_bytes`: 3pages.pdf, assertion `"merged: 3 pages"` (was 4)
- `verbose_no_output_skips_wrote_line`: 3pages.pdf, assertion `vec!["merged: 3 pages"]` (was 2)
- `verbose_with_count_bytes_still_writes_one_wrote_line`: 3pages.pdf, assertion `"merged: 3 pages\n"`
- `quiet_and_verbose_coexist`: 3pages.pdf, assertion `Some("3")` (was 2), `"merged: 3 pages\n"`

- [ ] **Step 1: Apply the pattern to all 12 tests**

For each test above, change the first line from
`let mut doc = tiny_doc(N);`
to
`let mut doc = Document::load("tests/fixtures/<F>.pdf").unwrap();`

and update assertion literals where listed. `count_bytes_only_matches_serialized_size` has TWO `tiny_doc(2)` calls (one for `doc_ref`); both become `Document::load(...)`.

- [ ] **Step 2: Verify execute_run tests still pass**

```
cargo test --quiet runner::execute_run 2>&1 | tail -5
```

Expected: 12 tests pass.

- [ ] **Step 3: Commit**

```
git add src/runner_tests.rs
git commit -m "Migrate execute_run tests to real PDF fixtures"
```

---

## Task 6b: Migrate `load_sources_*` tests

**Files:**
- Modify: `src/runner_tests.rs`

Each load_sources test calls `write_tiny_pdf(n)` to get a temp PDF path. Migrate by using fixture paths directly.

Mapping:
- `verbose_logs_header_and_detail_with_pages`: uses `write_tiny_pdf(5)` with `parse_ranges("1,3")` and asserts `"5 pages total, 2 selected"`. **Switch to** 3pages.pdf with `parse_ranges("1,3")`, assert `"3 pages total, 2 selected"`.
- `verbose_uses_all_when_ranges_absent`: `write_tiny_pdf(3)`, asserts `"3 pages total, all"`. Use 3pages.pdf, same assertion.
- `verbose_pads_header_index`: `write_tiny_pdf(2)`, 10 copies, asserts padding format. Use 1page.pdf.
- `silent_when_verbose_false`: `write_tiny_pdf(2)`, asserts log empty. Use 1page.pdf.

- [ ] **Step 1: Migrate `verbose_logs_header_and_detail_with_pages`**

Replace the test body with:

```rust
#[test]
fn load_sources_verbose_logs_header_and_detail_with_pages() {
    let path = "tests/fixtures/3pages.pdf";
    let inputs = vec![crate::cli::Input {
        path: path.to_string(),
        ranges: Some(crate::pages::parse_ranges("1,3").unwrap()),
    }];
    let mut log = Vec::new();
    let mut vlog = VerboseLog::new(true, &mut log);
    load_sources(&inputs, &mut vlog).unwrap();
    let s = std::str::from_utf8(&log).unwrap();
    let expected = format!("[1/1] {path} -p 1,3\n      3 pages total, 2 selected\n");
    assert_eq!(s, expected);
}
```

- [ ] **Step 2: Migrate `verbose_uses_all_when_ranges_absent`**

```rust
#[test]
fn load_sources_verbose_uses_all_when_ranges_absent() {
    let path = "tests/fixtures/3pages.pdf";
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
```

- [ ] **Step 3: Migrate `verbose_pads_header_index`**

```rust
#[test]
fn load_sources_verbose_pads_header_index() {
    let path = "tests/fixtures/1page.pdf";
    let inputs: Vec<crate::cli::Input> = (0..10)
        .map(|_| crate::cli::Input {
            path: path.to_string(),
            ranges: None,
        })
        .collect();
    let mut log = Vec::new();
    let mut vlog = VerboseLog::new(true, &mut log);
    load_sources(&inputs, &mut vlog).unwrap();
    let s = std::str::from_utf8(&log).unwrap();
    assert!(s.contains(&format!("[ 1/10] {path}\n")), "got: {s}");
    assert!(s.contains(&format!("[10/10] {path}\n")), "got: {s}");
}
```

- [ ] **Step 4: Migrate `silent_when_verbose_false`**

```rust
#[test]
fn load_sources_silent_when_verbose_false() {
    let inputs = vec![crate::cli::Input {
        path: "tests/fixtures/1page.pdf".to_string(),
        ranges: None,
    }];
    let mut log = Vec::new();
    let mut vlog = VerboseLog::new(false, &mut log);
    load_sources(&inputs, &mut vlog).unwrap();
    assert!(log.is_empty());
}
```

- [ ] **Step 5: Verify**

```
cargo test --quiet runner::load_sources 2>&1 | tail -5
```

Expected: 4 tests pass.

- [ ] **Step 6: Commit**

```
git add src/runner_tests.rs
git commit -m "Migrate load_sources tests to real PDF fixtures"
```

---

## Task 6c: Add `NoPages` and `PageSelection` tests

**Files:**
- Modify: `src/runner_tests.rs`

- [ ] **Step 1: Add `load_one_source_no_pages_returns_no_pages`**

Append after the existing `load_one_source_corrupt_file_*` test:

```rust
#[test]
fn load_one_source_no_pages_returns_no_pages() {
    use crate::Error;
    let input = crate::cli::Input {
        path: "tests/fixtures/0page.pdf".to_string(),
        ranges: None,
    };
    let mut log = Vec::new();
    let mut vlog = VerboseLog::new(false, &mut log);
    let err = load_one_source(&input, 1, 1, "", &mut vlog).unwrap_err();
    match err {
        Error::NoPages { path } => assert_eq!(path, input.path),
        other => panic!("expected NoPages, got {other:?}"),
    }
}
```

- [ ] **Step 2: Add `load_one_source_page_selection_out_of_range`**

Append after the previous test:

```rust
#[test]
fn load_one_source_page_selection_out_of_range() {
    use crate::Error;
    use crate::pages::PageSpecError;
    let input = crate::cli::Input {
        path: "tests/fixtures/3pages.pdf".to_string(),
        ranges: Some(crate::pages::parse_ranges("10").unwrap()),
    };
    let mut log = Vec::new();
    let mut vlog = VerboseLog::new(false, &mut log);
    let err = load_one_source(&input, 1, 1, "", &mut vlog).unwrap_err();
    match err {
        Error::PageSelection { path, source } => {
            assert_eq!(path, input.path);
            assert_eq!(source, PageSpecError::OutOfRange { page: 10, total: 3 });
        }
        other => panic!("expected PageSelection, got {other:?}"),
    }
}
```

- [ ] **Step 3: Verify both pass**

```
cargo test --quiet load_one_source 2>&1 | tail -5
```

Expected: 4 tests pass (2 existing + 2 new).

- [ ] **Step 4: Commit**

```
git add src/runner_tests.rs
git commit -m "Test NoPages and PageSelection error branches in load_one_source"
```

---

## Task 6d: Delete dead helpers in runner_tests.rs

**Files:**
- Modify: `src/runner_tests.rs`

After Task 6a–6c, the helpers `tiny_doc` and `write_tiny_pdf` are unused.

- [ ] **Step 1: Delete `tiny_doc`**

Remove the function `tiny_doc(n: usize) -> Document` (~32 lines starting at the top of the file).

- [ ] **Step 2: Delete `write_tiny_pdf`**

Remove the function `write_tiny_pdf(n: usize) -> tempfile::NamedTempFile` (added in the previous PR, ~8 lines).

- [ ] **Step 3: Verify the file compiles and all tests pass**

```
cargo build --tests 2>&1 | tail -5
cargo test 2>&1 | grep "test result"
```

Expected: clean build, all tests pass.

- [ ] **Step 4: Commit**

```
git add src/runner_tests.rs
git commit -m "Drop now-unused tiny_doc and write_tiny_pdf helpers"
```

---

## Task 7: Final validation, self-review, and PR

**Files:**
- No source changes. Validation only.

- [ ] **Step 1: Format check**

```
cargo fmt --check
```

Expected: clean (no output).

If it fails: `cargo fmt` and re-stage / amend the most recent commit.

- [ ] **Step 2: Clippy with denied warnings**

```
cargo clippy --all-targets -- -D warnings 2>&1 | tail -10
```

Expected: `Finished` with no warnings.

- [ ] **Step 3: Full test run**

```
cargo test 2>&1 | grep "test result"
```

Expected: `test result: ok. 84 passed; 0 failed`.

- [ ] **Step 4: Subagent self-review**

Dispatch a general-purpose agent (under 200 words):
> Self-review the real-PDF test migration on branch
> `refactor/real-pdf-tests`. Read `docs/superpowers/specs/2026-05-18-real-pdf-tests-design.md`
> for intent. Then `git diff main...HEAD` and check: (1) every spec item
> is implemented; (2) no in-memory `Document` factory remains in
> migrated tests; (3) helper deletions match the spec; (4) fixtures
> committed; (5) `cargo test`, `cargo fmt --check`, and
> `cargo clippy --all-targets -- -D warnings` pass. Report problems
> with file:line or confirm clean.

- [ ] **Step 5: Push and open PR**

```
git push -u origin refactor/real-pdf-tests
```

(`gh pr create` may need `dangerouslyDisableSandbox: true` for TLS keychain access on macOS.)

```
gh pr create --base main --head refactor/real-pdf-tests \
  --title "Migrate tests to real PDF fixtures" \
  --body "$(cat <<'EOF'
## Summary

- 6 PDF fixtures under `tests/fixtures/` (2 existing LuaLaTeX, 2 new LuaLaTeX, 2 new synthesized via lopdf).
- `merge_tests.rs` (10 → 11): all existing tests migrated; new `picks_highest_pdf_version` covers the `version.max(...)` branch.
- `runner_tests.rs` (30 → 32): all `tiny_doc(n)` / `write_tiny_pdf(n)` call sites migrated; new `load_one_source_no_pages_returns_no_pages` and `load_one_source_page_selection_out_of_range` cover previously untested error paths.
- Dead in-memory factories (`tiny_doc`, `doc_with_widths`, `doc_with_inherited_box`, `page_widths`, `write_tiny_pdf`) removed.
- Synthetic fixtures regeneratable via `cargo run --example gen_synthetic_fixtures`.

Design doc: `docs/superpowers/specs/2026-05-18-real-pdf-tests-design.md`.

## Test plan
- [x] `cargo test` — 84/84 pass
- [x] `cargo fmt --check`
- [x] `cargo clippy --all-targets -- -D warnings`
EOF
)"
```

Expected: PR URL printed.

- [ ] **Step 6: Remove dev docs per CLAUDE.md squash-merge policy**

CLAUDE.md says: "PRはsquash mergeされるので中間コミットに開発ドキュメントを含めることは構わないが，PRを出す時はREADMEやコメント等にまとめ，ポインタ含め削除しておくこと".

The spec doc (`docs/superpowers/specs/2026-05-18-real-pdf-tests-design.md`) and this plan are intermediate dev docs. After the PR is opened, decide:
- If the user wants them kept (some teams keep specs for archeology), leave them and the commit history captures their role.
- If not, remove them in a final cleanup commit on this branch and force-push:
  ```
  git rm docs/superpowers/specs/2026-05-18-real-pdf-tests-design.md \
         docs/superpowers/plans/2026-05-18-real-pdf-tests.md
  git commit -m "Remove dev docs before squash merge"
  git push
  ```

Default: ask the user before deleting. The squash merge collapses these anyway, but a future `git log -p` on the branch ref still finds them; final cleanup avoids that.

---

## Done

After Task 7, the PR is open and the branch is ready for review. The 6-fixture corpus is in place, all 84 tests pass, three previously-untested branches now have direct coverage, and the in-memory factory helpers are gone.
