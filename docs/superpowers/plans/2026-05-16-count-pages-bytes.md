# Page/byte count flags — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `--count-pages` / `--count-bytes` (plus aliases) to pdfcat so users can print the merged result's page count and/or byte count to stdout, with or without `-o`.

**Architecture:** Two boolean flags added to `Command::Run`; `output` becomes optional. A small `CountingWriter` adapter wraps the output sink (a real `File` or `io::sink()`) so the byte count is computed in a single pass without allocation. Stdout receives one labeled line per requested count, in fixed `pages → bytes` order.

**Tech Stack:** Rust 2024 edition, hand-rolled CLI parser (no new dependencies), `lopdf 0.40` for PDF I/O, `thiserror` for errors.

**Spec:** `docs/superpowers/specs/2026-05-16-count-pages-bytes-design.md`

**Branch:** `feature/count-pages-bytes`

**File map:**
- Modify `src/cli.rs` — error enum, `Command::Run` shape, parser, parser tests
- Modify `src/main.rs` — `CountingWriter`, extracted `execute_run` helper, runtime wiring, unit tests
- Modify `src/help.txt` — new flag entries
- Modify `README.md` — usage table, examples

---

## Task 1: CLI surface — parse new flags, output becomes optional

**Files:**
- Modify: `src/cli.rs`

### What changes

- `CliError::MissingOutput` → renamed to `NoAction` with message `must specify --output and/or --count-pages/--count-bytes`.
- `Command::Run` grows two `bool` fields and `output` becomes `Option<String>`:
  ```rust
  Command::Run {
      inputs: Vec<Input>,
      output: Option<String>,
      count_pages: bool,
      count_bytes: bool,
  }
  ```
- `Parser` gains two state booleans and matches all 16 alias strings.
- `Parser::finish` enforces "at least one action".
- Repetition of a count flag is idempotent (no error, just sets the bool).

- [ ] **Step 1: Update the error enum and `Command::Run` shape**

Edit `src/cli.rs`, replace `MissingOutput` with `NoAction`:

```rust
#[error("must specify --output and/or --count-pages/--count-bytes")]
NoAction,
```

Change `Command::Run` to:

```rust
pub enum Command {
    Run {
        inputs: Vec<Input>,
        output: Option<String>,
        count_pages: bool,
        count_bytes: bool,
    },
    Help,
    Version,
}
```

- [ ] **Step 2: Add parser state and accept the new flag aliases**

Add two booleans to `Parser`:

```rust
struct Parser<'a> {
    args: &'a [String],
    pos: usize,
    inputs: Vec<Input>,
    output: Option<String>,
    count_pages: bool,
    count_bytes: bool,
    options_done: bool,
}
```

Initialize them to `false` in `Parser::new`.

In `Parser::step`, extend the `match opt` arms with the two new flag families (place them before the catch-all `_ if opt.starts_with('-')` arm):

```rust
"--count-pages" | "--count-page"
| "--page-count" | "--page-counts"
| "--num-pages" | "--num-page"
| "--npages" | "--npage" => self.count_pages = true,
"--count-bytes" | "--count-byte"
| "--byte-count" | "--byte-counts"
| "--num-bytes" | "--num-byte"
| "--nbytes" | "--nbyte" => self.count_bytes = true,
```

These flags do not take a value; the `inline` from `split_inline` is intentionally ignored (matches existing behavior for `--help`/`--version`).

- [ ] **Step 3: Update `finish` to enforce "at least one action"**

```rust
fn finish(self) -> Result<Command, CliError> {
    if self.inputs.is_empty() {
        return Err(CliError::NoInputs);
    }
    if self.output.is_none() && !self.count_pages && !self.count_bytes {
        return Err(CliError::NoAction);
    }
    Ok(Command::Run {
        inputs: self.inputs,
        output: self.output,
        count_pages: self.count_pages,
        count_bytes: self.count_bytes,
    })
}
```

Note: `NoInputs` is checked first because "no inputs" is the more fundamental error; the previous order (output → inputs) was arbitrary, but checking inputs first matches the error users will see when they type `pdfcat` with no args at all (handled separately in `main.rs`) and `pdfcat -o w.pdf` (this branch).

- [ ] **Step 4: Update the `run_command` test helper signature**

In the `tests` module of `src/cli.rs`, change:

```rust
fn run_command(args: &[&str]) -> (Vec<Input>, Option<String>) {
    match parse_args(args).unwrap() {
        Command::Run { inputs, output, .. } => (inputs, output),
        other => panic!("expected Run, got {other:?}"),
    }
}
```

Update existing tests that compare `output` to a string literal:

```rust
// Before: assert_eq!(output, "w.pdf");
// After:  assert_eq!(output.as_deref(), Some("w.pdf"));
```

Apply this transformation in tests `concatenation`, `inline_and_positional_output`, and `double_dash_ends_options`.

- [ ] **Step 5: Update the `errors` test to use `NoAction` instead of `MissingOutput`**

```rust
assert_eq!(parse_args(&["a.pdf"]), Err(NoAction));
```

Keep the rest of `errors` intact.

- [ ] **Step 6: Add a new test for count flags (parsing only)**

Add this test in the `tests` module of `src/cli.rs`:

```rust
#[test]
fn count_flags_and_aliases() {
    let parse_one = |arg: &str| match parse_args(&["a.pdf", arg]).unwrap() {
        Command::Run { count_pages, count_bytes, output, .. } => {
            (count_pages, count_bytes, output)
        }
        other => panic!("expected Run, got {other:?}"),
    };

    // Every page-count alias sets count_pages and leaves output empty.
    for alias in [
        "--count-pages", "--count-page",
        "--page-count", "--page-counts",
        "--num-pages", "--num-page",
        "--npages", "--npage",
    ] {
        let (cp, cb, out) = parse_one(alias);
        assert!(cp, "{alias} should set count_pages");
        assert!(!cb, "{alias} should not set count_bytes");
        assert!(out.is_none(), "{alias} should not require -o");
    }

    // Every byte-count alias sets count_bytes and leaves output empty.
    for alias in [
        "--count-bytes", "--count-byte",
        "--byte-count", "--byte-counts",
        "--num-bytes", "--num-byte",
        "--nbytes", "--nbyte",
    ] {
        let (cp, cb, out) = parse_one(alias);
        assert!(!cp, "{alias} should not set count_pages");
        assert!(cb, "{alias} should set count_bytes");
        assert!(out.is_none(), "{alias} should not require -o");
    }

    // Combinations: both flags + -o together.
    match parse_args(&[
        "a.pdf", "--count-pages", "--count-bytes", "-o", "w.pdf",
    ])
    .unwrap()
    {
        Command::Run { count_pages, count_bytes, output, .. } => {
            assert!(count_pages);
            assert!(count_bytes);
            assert_eq!(output.as_deref(), Some("w.pdf"));
        }
        other => panic!("expected Run, got {other:?}"),
    }

    // Repetition is idempotent.
    let (cp, _, _) = parse_one("--count-pages");
    assert!(cp);
    match parse_args(&["a.pdf", "--count-pages", "--num-pages"]).unwrap() {
        Command::Run { count_pages, .. } => assert!(count_pages),
        other => panic!("expected Run, got {other:?}"),
    }

    // No -o and no count flag → NoAction (existing test covers `["a.pdf"]`).
}
```

- [ ] **Step 7: Run the tests, confirm everything passes**

```bash
cargo test --lib cli
```

Expected: all `cli::tests::*` tests pass, including the new `count_flags_and_aliases`.

If `main.rs` no longer compiles (because it still destructures the old `Command::Run` shape), defer that: this task only updates `cli`. Add a temporary `..` pattern in `main.rs` if needed to keep the build green. The next task replaces that wiring properly.

Concretely, in `src/main.rs`:

```rust
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
```

This is a deliberate stopgap — Task 3 replaces it.

- [ ] **Step 8: Format, clippy, full test run, commit**

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

All must pass. Then commit:

```bash
git add src/cli.rs src/main.rs
git commit -m "cli: parse --count-pages / --count-bytes flags

Adds eight alias spellings each, makes -o optional, and replaces
MissingOutput with a broader NoAction error. main.rs wiring is
stubbed; the next commit consumes the new fields."
```

---

## Task 2: `CountingWriter` adapter

**Files:**
- Modify: `src/main.rs`

### What changes

A small struct that wraps any `Write` and tracks total bytes written. Used in Task 3 to count the bytes lopdf serializes, whether the inner writer is a `File` or `io::sink()`.

- [ ] **Step 1: Add a failing test for `CountingWriter`**

In `src/main.rs`, append a `#[cfg(test)] mod tests` block (if one exists, add to it):

```rust
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
        // A writer that accepts only the first byte each call.
        struct OneByteAtATime(Vec<u8>);
        impl Write for OneByteAtATime {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                if buf.is_empty() { return Ok(0); }
                self.0.push(buf[0]);
                Ok(1)
            }
            fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
        }
        let mut inner = OneByteAtATime(Vec::new());
        let mut w = CountingWriter::new(&mut inner);
        // `write_all` keeps calling until done — final count must still match.
        w.write_all(b"abcd").unwrap();
        assert_eq!(w.count(), 4);
        assert_eq!(inner.0, b"abcd");
    }
}
```

- [ ] **Step 2: Run the test, expect a compile error**

```bash
cargo test --lib counting_writer
```

Expected: compile error, `CountingWriter` not found.

- [ ] **Step 3: Implement `CountingWriter`**

Near the top of `src/main.rs`, add:

```rust
use std::io::{self, Write};

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
```

- [ ] **Step 4: Run the tests, expect them to pass**

```bash
cargo test --lib counting_writer
```

Both tests pass.

- [ ] **Step 5: Format, clippy, commit**

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

```bash
git add src/main.rs
git commit -m "main: add CountingWriter to track bytes written"
```

---

## Task 3: Wire counts into `run`

**Files:**
- Modify: `src/main.rs`

### What changes

Replace the stub from Task 1 with the real logic:

1. After merging, if `count_pages`, print `pages: {n}` to stdout.
2. If `count_bytes`:
   - With `-o`: open the file and wrap it in `CountingWriter`. `save_to` it. Then print `bytes: {n}`.
   - Without `-o`: wrap `io::sink()` in `CountingWriter`. `save_to` it. Then print `bytes: {n}`.
3. If `!count_bytes` and `-o`: save directly to the file (current behavior).
4. If `!count_bytes` and `!-o`: skip serialization entirely. (Only `count_pages` was requested.)

To make this testable, extract the "report + save" step into a helper that takes a writer for stdout-style output:

```rust
fn execute_run(
    merged: &mut Document,
    output: Option<&str>,
    count_pages: bool,
    count_bytes: bool,
    report: &mut impl Write,
) -> Result<(), Error>
```

`run()` calls this with `&mut io::stdout().lock()`.

- [ ] **Step 1: Add failing tests for `execute_run`**

In the `tests` module of `src/main.rs`, add tiny in-memory document helpers (similar to `merge.rs::tests` but minimal) and tests:

```rust
use lopdf::{Dictionary, Document, Object};

fn tiny_doc(n: usize) -> Document {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.add_object(Dictionary::new());
    let kids: Vec<Object> = (0..n)
        .map(|_| {
            let mut page = Dictionary::new();
            page.set("Type", "Page");
            page.set("Parent", pages_id);
            page.set("MediaBox", Object::Array(vec![
                Object::Integer(0), Object::Integer(0),
                Object::Integer(10), Object::Integer(10),
            ]));
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
    // Build two independent docs because lopdf::Document::save_to may mutate
    // internal state (e.g. renumbering, cross-ref construction) and re-saving
    // the same doc twice is not guaranteed to produce the same bytes.
    let mut doc_ref = tiny_doc(2);
    let mut expected = Vec::new();
    doc_ref.save_to(&mut expected).unwrap();

    let mut doc = tiny_doc(2);
    let mut report = Vec::new();
    execute_run(&mut doc, None, false, true, &mut report).unwrap();
    let line = std::str::from_utf8(&report).unwrap();
    let prefix = "bytes: ";
    assert!(line.starts_with(prefix), "got {line:?}");
    let n: usize = line[prefix.len()..line.len()-1].parse().unwrap();
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
    // Unique per-test path so parallel `cargo test` runs don't collide.
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
```

Note: `lopdf::Document::save_to` requires `&mut self` (it mutates internal cross-reference state), which is why `execute_run` takes `&mut Document`. If a future lopdf version relaxes that, the signature can follow.

- [ ] **Step 2: Run the tests, expect them to fail**

```bash
cargo test --lib execute_run
```

Expected: compile error (function not defined) or all four fail.

- [ ] **Step 3: Implement `execute_run` and rewire `run`**

In `src/main.rs`:

```rust
fn execute_run(
    merged: &mut Document,
    output: Option<&str>,
    count_pages: bool,
    count_bytes: bool,
    report: &mut impl Write,
) -> Result<(), Error> {
    if count_pages {
        writeln!(report, "pages: {}", merged.get_pages().len())
            .map_err(|e| Error::ReportIo(e))?;
    }

    match (output, count_bytes) {
        (Some(path), true) => {
            let file = std::fs::File::create(path).map_err(|source| Error::WriteOutput {
                path: path.to_string(),
                source,
            })?;
            let mut w = CountingWriter::new(file);
            merged.save_to(&mut w).map_err(|source| Error::WriteOutput {
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
            merged.save_to(&mut w).map_err(|source| Error::WriteOutput {
                path: "<none>".to_string(),
                source,
            })?;
            writeln!(report, "bytes: {}", w.count()).map_err(Error::ReportIo)?;
        }
        (None, false) => {
            // Nothing to do beyond the page-count line already printed.
        }
    }

    Ok(())
}
```

Add the `ReportIo` variant to the `Error` enum:

```rust
#[error("failed to write count to stdout: {0}")]
ReportIo(#[from] std::io::Error),
```

Update `run()` to call `execute_run`:

```rust
Command::Run { inputs, output, count_pages, count_bytes } => {
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
```

Remove the stopgap added in Task 1.

- [ ] **Step 4: Run the tests, expect them to pass**

```bash
cargo test
```

All tests pass, including the four `execute_run_*` ones.

- [ ] **Step 5: Manual smoke test**

Build, then run against a real PDF (use any small `.pdf` you have handy, e.g. one of pdfcat's own outputs or a downloaded sample):

```bash
cargo build --release
# Replace path with any small PDF on the machine.
PDF=/path/to/some.pdf
./target/release/pdfcat "$PDF" --count-pages
./target/release/pdfcat "$PDF" --count-bytes
./target/release/pdfcat "$PDF" --count-pages --count-bytes
./target/release/pdfcat "$PDF" -o /tmp/pdfcat-smoke.pdf --count-pages --count-bytes
ls -l /tmp/pdfcat-smoke.pdf  # size matches the reported bytes line
./target/release/pdfcat "$PDF"        # → NoAction error on stderr
```

Expected outputs match the spec:
- single flag → labeled single line
- both flags → `pages:` then `bytes:` two lines
- with `-o` and `--count-bytes`: `ls -l` size equals the reported number

If anything diverges, stop and fix before committing.

- [ ] **Step 6: Format, clippy, commit**

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

```bash
git add src/main.rs
git commit -m "main: report page and byte counts to stdout

Hooks the new flags into the merge path: pages from get_pages().len(),
bytes via a CountingWriter wrapping either the output File or io::sink()
so the count is exact in one pass."
```

---

## Task 4: Documentation

**Files:**
- Modify: `src/help.txt`
- Modify: `README.md`

### What changes

Help and README should mention the new flags using their primary names; aliases are noted briefly so users discover them but the docs don't list all eight.

- [ ] **Step 1: Update `src/help.txt`**

Replace the file with this content (additions: new flags + the "or a count flag" hint in USAGE):

```
pdfcat - concatenate PDFs and extract pages

USAGE:
    pdfcat <INPUT> [-p <PAGES>] [<INPUT> [-p <PAGES>] ...] [-o <OUTPUT>] [--count-pages] [--count-bytes]

    At least one of -o, --count-pages, --count-bytes must be given.

OPTIONS:
    -h, --help              Print this help
    -V, --version           Print version
    -o, --output <FILE>     Output file
    -p, --pages <SPEC>      Page selection for the preceding input file
                            (aliases: --page, -pp)
    --count-pages           Print the merged page count to stdout
                            (aliases: --count-page, --page-count, --num-pages,
                            --npages, ...)
    --count-bytes           Print the merged byte count to stdout
                            (aliases: --count-byte, --byte-count, --num-bytes,
                            --nbytes, ...)
    --                      Treat every following argument as an input file

PAGE SPEC (1-based, comma-separated, trailing comma optional):
    N        page N
    -N       pages from the first to N (inclusive)
    N-       pages from N to the last (inclusive)
    N-M      pages N to M (inclusive)
    e.g.  -p 1        -p -2,4-        -p 1-3,5

EXAMPLES:
    pdfcat x.pdf y.pdf z.pdf -o out.pdf
    pdfcat x.pdf -p 1 -o out.pdf
    pdfcat x.pdf -p -2,4- y.pdf -o out.pdf
    pdfcat x.pdf -p 1-3 y.pdf -p 5- -o out.pdf
    pdfcat -o out.pdf -- -scan.pdf
    pdfcat x.pdf --count-pages
    pdfcat x.pdf y.pdf --count-pages --count-bytes
    pdfcat x.pdf -p 1-3 -o out.pdf --count-bytes
```

- [ ] **Step 2: Update `README.md`**

Two changes:

1. In the "Usage" code block, change the synopsis to:

   ```
   pdfcat <INPUT> [-p <PAGES>] [<INPUT> [-p <PAGES>] ...] [-o <OUTPUT>] [--count-pages] [--count-bytes]
   ```

   And update the surrounding paragraph: `-o`/`--output` is no longer required; at least one of `-o`, `--count-pages`, `--count-bytes` must be given.

2. Extend the options table:

   | Option | Description |
   | --- | --- |
   | `--count-pages` | Print the merged page count to stdout. Aliases: `--count-page`, `--page-count`, `--page-counts`, `--num-pages`, `--num-page`, `--npages`, `--npage` |
   | `--count-bytes` | Print the merged byte count to stdout. Aliases: `--count-byte`, `--byte-count`, `--byte-counts`, `--num-bytes`, `--num-byte`, `--nbytes`, `--nbyte` |

3. Add to the "Examples" section:

   ```sh
   # Inspect the result without writing a file
   pdfcat x.pdf y.pdf --count-pages --count-bytes

   # Write and report at the same time
   pdfcat x.pdf -p 1-3 -o out.pdf --count-bytes
   ```

4. Add to the "Behaviour" section (new bullet):

   ```
   - `--count-pages` and `--count-bytes` print one labeled line each to
     stdout (in `pages → bytes` order), and may be combined with `-o`
     or used on their own.
   ```

- [ ] **Step 3: Sanity check the rendered help**

```bash
cargo run -- --help
```

Expected: the new lines appear correctly, no stray characters.

- [ ] **Step 4: Format, clippy, full test run**

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

- [ ] **Step 5: Commit**

```bash
git add src/help.txt README.md
git commit -m "docs: document --count-pages and --count-bytes"
```

---

## Post-tasks

Before opening a PR (per `CLAUDE.md`'s "PRはsquash mergeされるので…ポインタ含め削除しておくこと"):

- [ ] **Step 1: Remove in-branch design/plan docs**

```bash
git rm docs/superpowers/specs/2026-05-16-count-pages-bytes-design.md \
       docs/superpowers/plans/2026-05-16-count-pages-bytes.md
# Also remove docs/superpowers/ if empty.
rmdir docs/superpowers/specs docs/superpowers/plans docs/superpowers docs 2>/dev/null || true
git commit -m "chore: drop in-branch design/plan docs (kept only in history)"
```

(Mirrors the prior commit `df719d8`.)

- [ ] **Step 2: Final verification before PR**

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

All green. Now open the PR per the project's PR workflow.
