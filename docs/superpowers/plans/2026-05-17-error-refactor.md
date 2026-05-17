# Error Classification and Message Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Clean up error classifications and shorten user-facing error messages across pdfcat: drop the redundant `error:` prefix, split mis-classified variants (IO vs. parse, sink-vs-file, stdout-vs-report), attach input path context to merge errors, remove unreachable variants, and prefer user input strings over normalised labels where they diverge.

**Architecture:** Touch four error enums (`main::Error`, `cli::CliError`, `pages::PageSpecError`, `merge::MergeError`), the `eprintln!` prefix in `main`, and tests. The `merge::merge` signature gains a path per source so merge-time errors can name the offending file. No module rearrangement, no behavioural change beyond message text and variant identity. Exit code stays `1` on any failure.

**Tech Stack:** Rust 2024 edition, `thiserror` 2.0, `lopdf` 0.40 (`Document::save_to` returns `std::io::Result<()>`; `lopdf::Error::IO(std::io::Error)` is the variant we split on).

**Branch:** `refactor/error-messages` (per CLAUDE.md, `refactor/` prefix per saved memory). Plan document is a development artifact and **must be removed before opening the PR** — its contents get summarised into the PR description / commit message, not committed to main.

**Out of scope:** behaviour changes (atomic write, page selection, merge logic). Format helpers (`fmt_header_index`, `fmt_ranges`). README rewrites beyond the error-relevant sentences.

---

## File Structure

| File | Responsibility | Change |
| --- | --- | --- |
| `src/main.rs` | top-level `Error` enum, prefix print | Drop `error:` prefix, remove `NoArguments`, split `ReadInput` → `OpenInput`/`ParseInput`, add `SerializeForCount`, rename/clarify `ReportIo`. |
| `src/cli.rs` | `CliError`, argument parser | `UnexpectedValue` takes user input string, `NoInputs` gets a hint. |
| `src/pages.rs` | `PageSpecError` | Tweak `Empty` wording (via custom `BadPageSpec` Display in cli.rs). |
| `src/merge.rs` | `MergeError`, merge pipeline | `merge::merge` takes `(String, Document, Vec<u32>)` triples; `MergeError` variants carry `path`; remove `NoPages`. |
| `src/runner.rs` | runner; wires errors | Pass path into `merge::merge`; reroute `count_bytes_to_sink` to `SerializeForCount`. |
| `src/*_tests.rs` | unit tests | Update enum-variant assertions and string fixtures where touched. |

---

## Tasks

Each task contains: a failing test (or expected diff), the implementation, a verification command, and a commit. Run `cargo fmt && cargo clippy --all-targets --all-features -- -D warnings && cargo test` at the end of every task — these are the project's standing pre-commit checks (per CLAUDE.md).

After each task: review the diff, then commit. Intra-PR commit history is intentionally fine-grained; the PR is squash-merged so main only sees one commit.

---

### Task 1: Set up branch

**Files:** none yet.

- [ ] **Step 1: Create and switch to branch**

```bash
git switch -c refactor/error-messages
```

- [ ] **Step 2: Verify clean state**

```bash
git status
```

Expected: `On branch refactor/error-messages\nnothing to commit, working tree clean` (apart from the plan file under `docs/superpowers/plans/`).

- [ ] **Step 3: Commit the plan**

```bash
git add docs/superpowers/plans/2026-05-17-error-refactor.md
git commit -m "docs: add error-refactor implementation plan"
```

(This commit will be dropped at squash time; it exists so subagents can read the plan from git.)

---

### Task 2: Drop `error:` prefix

**Files:**
- Modify: `src/main.rs:22-26`
- Test: no new test (no test asserts on the prefix today; verified by grep)

Verify no test currently pins the old prefix:

```bash
grep -rn "pdfcat: error:" src/ && echo FOUND || echo OK
```

Expected: `OK`.

- [ ] **Step 1: Edit `src/main.rs` error branch**

Change:

```rust
        Err(err) => {
            eprintln!("pdfcat: error: {err}");
            ExitCode::FAILURE
        }
```

to:

```rust
        Err(err) => {
            eprintln!("pdfcat: {err}");
            ExitCode::FAILURE
        }
```

- [ ] **Step 2: Run full build/test**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings && cargo test
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "Drop redundant 'error:' prefix from diagnostics"
```

---

### Task 3: Remove `Error::NoArguments`

**Files:**
- Modify: `src/main.rs` (the enum, `main`, and `run`)

- [ ] **Step 1: Rewrite `main` and `run` in `src/main.rs`**

Replace lines 14-86 (the entire `main`, `Error` enum, and `run` definitions) so that:

1. `main` collects args once, prints help and returns FAILURE when none, otherwise dispatches to `run(&args)`.
2. `Error` no longer has `NoArguments`.
3. `run` takes `args: &[String]` (no `std::env::args` inside).

New shape:

```rust
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprint!("{}", cli::HELP);
        return ExitCode::FAILURE;
    }
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("pdfcat: {err}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug, Error)]
enum Error {
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
    WriteOutput { path: String, source: std::io::Error },
    #[error("failed to write count to stdout: {0}")]
    ReportIo(std::io::Error),
}

fn run(args: &[String]) -> Result<(), Error> {
    match cli::parse(args)? {
        Command::Help => {
            print!("{}", cli::HELP);
            Ok(())
        }
        Command::Version => {
            println!("pdfcat {}", cli::VERSION);
            Ok(())
        }
        Command::Run { inputs, output, count_pages, count_bytes, quiet, verbose } => {
            let opts = OutputOpts {
                output: output.as_deref(),
                count_pages,
                count_bytes,
                quiet,
            };
            runner::run_pipeline(&inputs, &opts, verbose)
        }
    }
}
```

(`ReadInput`, `WriteOutput`, `ReportIo` are unchanged in this task; they are touched in Tasks 4/5/6.)

- [ ] **Step 2: Verify**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings && cargo test
```

Expected: PASS.

- [ ] **Step 3: Smoke-test the help-on-no-args path**

```bash
cargo run --quiet 2>&1 | head -2
```

Expected: starts with `pdfcat - concatenate PDFs and extract pages`.

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "Remove Error::NoArguments; print help directly from main"
```

---

### Task 4: Split `ReadInput` into `OpenInput` and `ParseInput`

**Files:**
- Modify: `src/main.rs` (enum)
- Modify: `src/runner.rs:252-255` (the `Document::load` error mapping)
- Test: `src/runner_tests.rs` (add two tests)

- [ ] **Step 1: Write failing tests in `src/runner_tests.rs`**

Append to the end of the file:

```rust
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
    let tmp = std::env::temp_dir().join(format!("pdfcat-not-a-pdf-{}.pdf", std::process::id()));
    std::fs::write(&tmp, b"this is not a PDF").unwrap();
    let input = crate::cli::Input {
        path: tmp.to_str().unwrap().to_string(),
        ranges: None,
    };
    let mut log = Vec::new();
    let mut vlog = VerboseLog::new(false, &mut log);
    let err = load_one_source(&input, 1, 1, "", &mut vlog).unwrap_err();
    let _ = std::fs::remove_file(&tmp);
    match err {
        Error::ParseInput { path, .. } => assert_eq!(path, input.path),
        other => panic!("expected ParseInput, got {other:?}"),
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test load_one_source_missing_file_returns_open_input load_one_source_corrupt_file_returns_parse_input
```

Expected: COMPILE ERROR (`OpenInput`/`ParseInput` don't exist yet).

- [ ] **Step 3: Edit the `Error` enum in `src/main.rs`**

Replace:

```rust
    #[error("{path}: cannot read PDF: {source}")]
    ReadInput { path: String, source: lopdf::Error },
```

with:

```rust
    #[error("{path}: cannot open: {source}")]
    OpenInput { path: String, source: std::io::Error },
    #[error("{path}: invalid PDF: {source}")]
    ParseInput { path: String, source: lopdf::Error },
```

- [ ] **Step 4: Edit `src/runner.rs` to classify**

Replace lines 252-255 (the `Document::load` call):

```rust
    let doc = Document::load(&input.path).map_err(|source| Error::ReadInput {
        path: input.path.clone(),
        source,
    })?;
```

with:

```rust
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
```

- [ ] **Step 5: Verify**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings && cargo test
```

Expected: PASS (including the two new tests).

- [ ] **Step 6: Commit**

```bash
git add src/main.rs src/runner.rs src/runner_tests.rs
git commit -m "Split ReadInput into OpenInput (io::Error) and ParseInput (lopdf)"
```

---

### Task 5: Replace `WriteOutput { path: "<none>" }` with `SerializeForCount`

**Files:**
- Modify: `src/main.rs` (enum)
- Modify: `src/runner.rs:210-223` (the `count_bytes_to_sink` body)

- [ ] **Step 1: Edit the `Error` enum in `src/main.rs`**

Add a new variant (keep `WriteOutput` as-is):

```rust
    #[error("failed to serialize merged PDF: {0}")]
    SerializeForCount(std::io::Error),
```

- [ ] **Step 2: Edit `count_bytes_to_sink` in `src/runner.rs`**

Replace:

```rust
fn count_bytes_to_sink<R: Write>(
    merged: &mut Document,
    opts: &OutputOpts<'_>,
    report: &mut R,
) -> Result<(), Error> {
    let mut w = CountingWriter::new(io::sink());
    merged
        .save_to(&mut w)
        .map_err(|source| Error::WriteOutput {
            path: "<none>".to_string(),
            source,
        })?;
    write_count(report, "bytes", w.count(), opts.quiet)
}
```

with:

```rust
fn count_bytes_to_sink<R: Write>(
    merged: &mut Document,
    opts: &OutputOpts<'_>,
    report: &mut R,
) -> Result<(), Error> {
    let mut w = CountingWriter::new(io::sink());
    merged.save_to(&mut w).map_err(Error::SerializeForCount)?;
    write_count(report, "bytes", w.count(), opts.quiet)
}
```

- [ ] **Step 3: Verify**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings && cargo test
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/main.rs src/runner.rs
git commit -m "Add SerializeForCount variant; remove '<none>' WriteOutput sentinel"
```

---

### Task 6: Rename `ReportIo` and broaden its message

**Files:**
- Modify: `src/main.rs` (enum)
- Modify: `src/runner.rs:40, 48, 129, 131` (call sites — name change only)

- [ ] **Step 1: Edit the `Error` enum in `src/main.rs`**

Replace:

```rust
    #[error("failed to write count to stdout: {0}")]
    ReportIo(std::io::Error),
```

with:

```rust
    #[error("failed to write report output: {0}")]
    ReportIo(std::io::Error),
```

(Name unchanged; message generalised to cover stderr/verbose paths.)

- [ ] **Step 2: Verify**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings && cargo test
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "Generalise ReportIo message (stderr verbose log shares the variant)"
```

---

### Task 7: Attach path context to `MergeError`

This is the largest task. Order: change `merge::merge` signature, update its callers and internals, then tests.

**Files:**
- Modify: `src/merge.rs` (enum + `merge`, `collect`, `clone_dict`, `flatten_inherited`)
- Modify: `src/runner.rs:282-301` (pass paths into `merge::merge`)
- Modify: `src/merge_tests.rs` (call sites; add a coverage test)

- [ ] **Step 1: Write failing test in `src/merge_tests.rs`**

Append:

```rust
#[test]
fn no_catalog_error_carries_first_input_path() {
    // A document missing /Root in the trailer trips skeleton extraction.
    let mut doc = Document::with_version("1.5");
    // No Root set on trailer.
    let err = merge(vec![("missing-root.pdf".to_string(), doc, vec![])]).unwrap_err();
    match err {
        MergeError::NoCatalog { path } => assert_eq!(path, "missing-root.pdf"),
        other => panic!("expected NoCatalog, got {other:?}"),
    }
}
```

(The existing 7 `merge(vec![...])` call sites at lines 85, 101, 111, 122, 153, 169, 197 in this file also need updating in Step 4 — re-grep before that step in case line numbers drift.)

- [ ] **Step 2: Run to verify failure**

```bash
cargo test no_catalog_error_carries_first_input_path
```

Expected: COMPILE ERROR (signature mismatch).

- [ ] **Step 3: Rewrite `MergeError` and `merge` in `src/merge.rs`**

Replace the enum (lines 24-34):

```rust
#[derive(Debug, Error)]
pub enum MergeError {
    #[error("{path}: input has no usable document catalog / page tree")]
    NoCatalog { path: String },
    #[error("{path}: page {page} unexpectedly missing")]
    PageMissing { path: String, page: u32 },
    #[error("{path}: broken PDF object: {source}")]
    BrokenObject {
        path: String,
        #[source]
        source: lopdf::Error,
    },
}
```

(`MergeError::NoPages` removed — see Task 8 if you prefer two commits, but the deletion is bundled here because `collect`'s post-loop check is also removed in this step.)

Replace `merge` (lines 37-64):

```rust
pub fn merge(sources: Vec<(String, Document, Vec<u32>)>) -> Result<Document, MergeError> {
    let collected = collect(sources)?;
    let Skeleton { catalog_id, pages_id, catalog, pages_node } = collected.skeleton;

    let (major, minor) = collected.version;
    let mut document = Document::with_version(format!("{major}.{minor}"));
    copy_supporting_objects(&mut document, &collected.objects);
    install_pages(&mut document, &collected.pages, pages_id);
    document.objects.insert(
        pages_id,
        Object::Dictionary(finalize_pages_node(pages_node, &collected.pages)),
    );
    document.objects.insert(
        catalog_id,
        Object::Dictionary(finalize_catalog(catalog, pages_id)),
    );

    document.trailer.set("Root", catalog_id);
    carry_over_info(&mut document, collected.info_id);
    document.max_id = collected.next_id;
    compact(&mut document);
    Ok(document)
}
```

Add a `Skeleton` struct and reshape `Collected`:

```rust
struct Skeleton {
    catalog_id: ObjectId,
    pages_id: ObjectId,
    catalog: Dictionary,
    pages_node: Dictionary,
}

struct Collected {
    pages: Vec<(ObjectId, Dictionary)>,
    objects: BTreeMap<ObjectId, Object>,
    skeleton: Skeleton,
    info_id: Option<ObjectId>,
    next_id: u32,
    version: (u32, u32),
}
```

Rewrite `collect` so skeleton extraction happens on the **first source** (using its path for any failure), and per-page failures carry the current source's path:

```rust
fn collect(sources: Vec<(String, Document, Vec<u32>)>) -> Result<Collected, MergeError> {
    let mut iter = sources.into_iter();
    let (first_path, mut first_doc, first_selected) = iter
        .next()
        .expect("merge called with no sources; runner enforces at least one input");

    let mut next_id: u32 = 1;
    let mut version = parse_version(&first_doc.version);
    first_doc.renumber_objects_with(next_id);
    next_id = first_doc.max_id + 1;

    let skeleton = extract_skeleton(&first_doc, &first_path)?;
    let info_id = first_info_id(&first_doc);

    let mut pages: Vec<(ObjectId, Dictionary)> = Vec::new();
    let mut objects: BTreeMap<ObjectId, Object> = BTreeMap::new();

    collect_one(&first_path, &first_doc, &first_selected, &mut pages, &mut next_id)?;
    objects.extend(first_doc.objects);

    for (path, mut doc, selected) in iter {
        version = version.max(parse_version(&doc.version));
        doc.renumber_objects_with(next_id);
        next_id = doc.max_id + 1;
        collect_one(&path, &doc, &selected, &mut pages, &mut next_id)?;
        objects.extend(doc.objects);
    }

    Ok(Collected { pages, objects, skeleton, info_id, next_id, version })
}

fn collect_one(
    path: &str,
    doc: &Document,
    selected: &[u32],
    pages: &mut Vec<(ObjectId, Dictionary)>,
    next_id: &mut u32,
) -> Result<(), MergeError> {
    let doc_pages = doc.get_pages();
    let mut seen = HashSet::new();
    for &page_no in selected {
        let src_id = *doc_pages.get(&page_no).ok_or_else(|| MergeError::PageMissing {
            path: path.to_string(),
            page: page_no,
        })?;
        let dict = flatten_inherited(doc, src_id).map_err(|source| MergeError::BrokenObject {
            path: path.to_string(),
            source,
        })?;
        let id = if seen.insert(src_id) {
            src_id
        } else {
            let fresh = (*next_id, 0);
            *next_id += 1;
            fresh
        };
        pages.push((id, dict));
    }
    Ok(())
}

fn extract_skeleton(doc: &Document, path: &str) -> Result<Skeleton, MergeError> {
    let broken = |source| MergeError::BrokenObject { path: path.to_string(), source };
    let catalog_id = doc.trailer.get(b"Root").and_then(Object::as_reference).map_err(|_| MergeError::NoCatalog { path: path.to_string() })?;
    let catalog_dict = doc
        .get_object(catalog_id)
        .and_then(Object::as_dict)
        .map_err(|_| MergeError::NoCatalog { path: path.to_string() })?;
    let pages_id = catalog_dict
        .get(b"Pages")
        .and_then(Object::as_reference)
        .map_err(|_| MergeError::NoCatalog { path: path.to_string() })?;
    let pages_node = doc
        .get_object(pages_id)
        .and_then(Object::as_dict)
        .map_err(|_| MergeError::NoCatalog { path: path.to_string() })?
        .clone();
    let _ = broken; // unused once NoCatalog covers all skeleton failures
    Ok(Skeleton {
        catalog_id,
        pages_id,
        catalog: catalog_dict.clone(),
        pages_node,
    })
}

fn first_info_id(doc: &Document) -> Option<ObjectId> {
    if let Ok(Object::Reference(id)) = doc.trailer.get(b"Info") {
        Some(*id)
    } else {
        None
    }
}
```

Change `flatten_inherited`'s return type to `Result<Dictionary, lopdf::Error>` (the `MergeError` wrap now happens at the call site so the path can be attached):

```rust
fn flatten_inherited(doc: &Document, page_id: ObjectId) -> Result<Dictionary, lopdf::Error> {
    let mut dict = doc.get_object(page_id)?.as_dict()?.clone();
    // ... body unchanged ...
    Ok(dict)
}
```

Delete the old free-standing `clone_dict` helper — it's no longer needed (the skeleton dicts are cloned inside `extract_skeleton`).

- [ ] **Step 4: Update `runner.rs` to pass paths**

Replace the `merge::merge(sources)` call (lines 292-293, 297-298):

```rust
let sources_with_paths: Vec<(String, Document, Vec<u32>)> = inputs
    .iter()
    .map(|input| input.path.clone())
    .zip(sources)
    .map(|(path, (doc, pages))| (path, doc, pages))
    .collect();
let mut merged = merge::merge(sources_with_paths)?;
```

…or refactor `load_sources` to return `Vec<(String, Document, Vec<u32>)>` directly. Pick the second (cleaner). Change `load_one_source`'s return to `Result<(String, Document, Vec<u32>), Error>` and have it produce `(input.path.clone(), doc, selected)`. Update `load_sources`'s signature and the two `run_pipeline` branches accordingly.

- [ ] **Step 5: Update existing merge_tests.rs call sites**

Every `merge(vec![(doc, pages), ...])` becomes `merge(vec![("test.pdf".to_string(), doc, pages), ...])`. Use a meaningful name per call where it helps (e.g. `"left.pdf"`, `"right.pdf"`).

- [ ] **Step 6: Verify**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings && cargo test
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/merge.rs src/runner.rs src/merge_tests.rs
git commit -m "MergeError carries path context; drop unreachable NoPages variant"
```

---

### Task 8: `CliError::UnexpectedValue` echoes user input

**Files:**
- Modify: `src/cli.rs:18, 131, 136, 141, 145, 185-191`
- Modify: `src/cli_tests.rs:264-288`

- [ ] **Step 1: Update test expectations in `src/cli_tests.rs`**

Replace the block at lines 263-288 (the `value_less_flags_reject_inline_value` test) so that the expected label matches the user-typed alias:

```rust
#[test]
fn value_less_flags_reject_inline_value() {
    use CliError::UnexpectedValue;
    assert_eq!(
        parse_args(&["a.pdf", "--count-pages=foo"]),
        Err(UnexpectedValue("--count-pages".into()))
    );
    assert_eq!(
        parse_args(&["a.pdf", "--num-pages=foo"]),
        Err(UnexpectedValue("--num-pages".into()))
    );
    assert_eq!(
        parse_args(&["a.pdf", "--count-bytes=foo"]),
        Err(UnexpectedValue("--count-bytes".into()))
    );
    assert_eq!(
        parse_args(&["a.pdf", "--nbytes=foo"]),
        Err(UnexpectedValue("--nbytes".into()))
    );
    assert_eq!(
        parse_args(&["a.pdf", "--count-pages", "--quiet=foo"]),
        Err(UnexpectedValue("--quiet".into()))
    );
    assert_eq!(
        parse_args(&["a.pdf", "--count-pages", "-q=foo"]),
        Err(UnexpectedValue("-q".into()))
    );
}
```

- [ ] **Step 2: Run to verify failure**

```bash
cargo test value_less_flags_reject_inline_value
```

Expected: FAIL (type mismatch: `&'static str` vs `String`).

- [ ] **Step 3: Change `CliError::UnexpectedValue` to `String`**

In `src/cli.rs:18`:

```rust
    #[error("{0} does not take a value")]
    UnexpectedValue(String),
```

Update `reject_value` (lines 185-191):

```rust
    fn reject_value(label: &str, inline: Option<&str>) -> Result<(), CliError> {
        if inline.is_some() {
            Err(CliError::UnexpectedValue(label.to_string()))
        } else {
            Ok(())
        }
    }
```

Update the four call sites (lines 131, 136, 141, 145) to pass `opt` (the user's typed name) instead of the normalised canonical name:

```rust
            "--count-pages" | "--count-page" | "--page-count" | "--page-counts" | "--num-pages"
            | "--num-page" | "--npages" | "--npage" => {
                Self::reject_value(opt, inline)?;
                self.count_pages = true;
            }
            "--count-bytes" | "--count-byte" | "--byte-count" | "--byte-counts" | "--num-bytes"
            | "--num-byte" | "--nbytes" | "--nbyte" => {
                Self::reject_value(opt, inline)?;
                self.count_bytes = true;
            }
            "-q" | "--quiet" => {
                Self::reject_value(opt, inline)?;
                self.quiet = true;
            }
            "-v" | "--verbose" => {
                Self::reject_value(opt, inline)?;
                self.verbose = true;
            }
```

- [ ] **Step 4: Verify**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings && cargo test
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/cli.rs src/cli_tests.rs
git commit -m "UnexpectedValue echoes the alias the user typed"
```

---

### Task 9: `BadPageSpec` custom Display for `Empty` case

**Files:**
- Modify: `src/cli.rs:28-33` (replace `thiserror`'s derived `Display` for `BadPageSpec` with a manual `impl`)
- Test: `src/cli_tests.rs` (add one case)

Goal: `pdfcat: --pages `,` has no page numbers` instead of `pdfcat: invalid page spec `,`: no pages given`.

- [ ] **Step 1: Add a failing test in `src/cli_tests.rs`**

Append near the `errors` test:

```rust
#[test]
fn bad_page_spec_empty_renders_naturally() {
    let err = parse_args(&["a.pdf", "-p", ",", "-o", "w.pdf"]).unwrap_err();
    let s = err.to_string();
    assert_eq!(s, "--pages `,` has no page numbers");
}
```

- [ ] **Step 2: Run to verify failure**

```bash
cargo test bad_page_spec_empty_renders_naturally
```

Expected: FAIL (current output is `invalid page spec `,`: no pages given`).

- [ ] **Step 3: Replace `BadPageSpec`'s derived Display in `src/cli.rs`**

Remove the `#[error(...)]` line from the `BadPageSpec` variant (and drop the `#[source]` since we'll surface `source` ourselves):

```rust
    BadPageSpec {
        spec: String,
        source: PageSpecError,
    },
```

Then implement `Display` for the whole enum manually — but that's wider than needed. Cleaner: keep `thiserror` for everything **except** `BadPageSpec`, by using `#[error(transparent)]` won't work either since we need spec context.

Simpler approach: keep `thiserror`'s derive but switch `BadPageSpec` to a function-style display via `#[error(fmt = ...)]`. `thiserror` 2.0 supports this:

```rust
    #[error(fmt = fmt_bad_page_spec)]
    BadPageSpec {
        spec: String,
        source: PageSpecError,
    },
```

…and add at module scope:

```rust
fn fmt_bad_page_spec(
    spec: &String,
    source: &PageSpecError,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    match source {
        PageSpecError::Empty => write!(f, "--pages `{spec}` has no page numbers"),
        _ => write!(f, "invalid page spec `{spec}`: {source}"),
    }
}
```

(Verify `#[error(fmt = ...)]` works in `thiserror = "2.0.18"`. If not, fall back to: derive `Debug` only on `CliError`, write a manual `impl std::fmt::Display for CliError` that delegates to each variant's message but special-cases `BadPageSpec`.)

- [ ] **Step 4: Verify**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings && cargo test
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/cli.rs src/cli_tests.rs
git commit -m "Render empty page spec naturally ('has no page numbers')"
```

---

### Task 10: `NoInputs` hint

**Files:**
- Modify: `src/cli.rs:26-27`

- [ ] **Step 1: Update the message**

Replace:

```rust
    #[error("no input files")]
    NoInputs,
```

with:

```rust
    #[error("no input files (need at least one PDF)")]
    NoInputs,
```

- [ ] **Step 2: Verify**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings && cargo test
```

Expected: PASS (no test pins this string).

- [ ] **Step 3: Commit**

```bash
git add src/cli.rs
git commit -m "Hint at PDF requirement in NoInputs message"
```

---

### Task 11: Final review pass

**Files:** README.md (only if needed)

- [ ] **Step 1: Hand-verify the new error surface**

Run each of these and confirm the message format. The test suite covers the semantics; this is a final UX-level read.

```bash
cargo build --release
target/release/pdfcat                             # → help to stderr
target/release/pdfcat -o out.pdf                  # → "pdfcat: no input files (need at least one PDF)"
target/release/pdfcat /tmp/does-not-exist.pdf -o out.pdf
                                                  # → "pdfcat: /tmp/does-not-exist.pdf: cannot open: No such file or directory ..."
echo not-a-pdf > /tmp/junk.pdf
target/release/pdfcat /tmp/junk.pdf -o out.pdf    # → "pdfcat: /tmp/junk.pdf: invalid PDF: ..."
target/release/pdfcat a.pdf --num-pages=foo       # → "pdfcat: --num-pages does not take a value"
target/release/pdfcat a.pdf -p ',' -o out.pdf     # → "pdfcat: --pages `,` has no page numbers"
rm -f /tmp/junk.pdf
```

- [ ] **Step 2: Run the full test suite once more**

```bash
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: all clean.

- [ ] **Step 3: Delete the plan file**

Per CLAUDE.md (development docs must not survive into the PR):

```bash
git rm docs/superpowers/plans/2026-05-17-error-refactor.md
rmdir docs/superpowers/plans 2>/dev/null
rmdir docs/superpowers 2>/dev/null
rmdir docs 2>/dev/null
git commit -m "Remove implementation plan (dev artifact)"
```

- [ ] **Step 4: Self-review with subagent**

Dispatch the Explore subagent (per CLAUDE.md "サブエージェントでセルフレビュー") to scan the diff:

> Review the diff on `refactor/error-messages` against `main`. Specifically check: (a) no `error:` prefix remains in user-facing strings; (b) no `Error::NoArguments`/`MergeError::NoPages`/`<none>` path-sentinel survives anywhere in the tree; (c) `MergeError` variants all carry `path`; (d) the test suite has at least one assertion for each of `OpenInput`, `ParseInput`, `MergeError::NoCatalog`-with-path, and `BadPageSpec` Empty wording. Report any gap.

Fix anything the subagent finds. Re-run `cargo test`. Commit any follow-ups.

- [ ] **Step 5: Push and open PR**

```bash
git push -u origin refactor/error-messages
gh pr create --title "Refactor error classifications and shorten messages" --body "$(cat <<'EOF'
## Summary
- Drop redundant `error:` prefix; diagnostics now read `pdfcat: ...`
- Split `ReadInput` into `OpenInput` (io::Error) and `ParseInput` (lopdf parse)
- New `SerializeForCount` variant for `--count-bytes`-only sink writes (removes the `path: "<none>"` sentinel)
- `ReportIo` message generalised (also covers verbose stderr writes)
- `MergeError` variants now carry the offending input path
- Removed unreachable `MergeError::NoPages` and the `Error::NoArguments` pseudo-error
- `CliError::UnexpectedValue` echoes the alias the user typed (was: canonical name)
- `BadPageSpec` with empty spec now renders as `--pages \`X\` has no page numbers`
- `NoInputs` hints that a PDF is required

## Test plan
- [ ] `cargo test` green
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` clean
- [ ] Manual checks from plan Task 11 Step 1 produce expected messages
EOF
)"
```

---

## Self-Review

Spec coverage (cross-check vs. the 9 items in the consolidated proposal + prefix change):

| Item | Task |
| --- | --- |
| Prefix shortening | Task 2 |
| `Error::NoArguments` removal | Task 3 |
| `ReadInput` split | Task 4 |
| `WriteOutput "<none>"` → `SerializeForCount` | Task 5 |
| `ReportIo` message generalisation | Task 6 |
| `MergeError` path context | Task 7 |
| `MergeError::NoPages` removal | Task 7 (bundled) |
| `UnexpectedValue` user-input echo | Task 8 |
| `BadPageSpec` Empty wording | Task 9 |
| `NoInputs` hint | Task 10 |

No placeholders. Each step shows exact code or exact command. Identifiers used consistently (`OpenInput`/`ParseInput`/`SerializeForCount`/`Skeleton`/`MergeError::{NoCatalog,PageMissing,BrokenObject}` are all introduced in the task that first names them).

Caveat: Task 9 Step 3 references `thiserror`'s `#[error(fmt = ...)]` attribute. If `thiserror = "2.0.18"` doesn't support it, the fallback path is spelled out (manual `impl Display`). Verify when executing.
