# `--verbose` option Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `-v` / `--verbose` flag that streams human-readable progress to stderr (per-input header + detail lines, `merged:` line, `wrote ... (B bytes)` line).

**Architecture:** Verbose log routes through a `&mut impl Write` argument added to `execute_run` and `load_sources`. `run()` substitutes `io::stderr().lock()` when `verbose=true` and `io::sink()` when false, keeping every helper unit-testable with a `Vec<u8>` buffer. `Range` gains a `Display` impl plus a `fmt_ranges` joiner used to re-print the normalised `-p` spec in the header line. The `-o` write path is collapsed to always go through `CountingWriter` so the wrote-line can always include the byte count.

**Tech Stack:** Rust 2024 edition, `lopdf`, `thiserror`, `std::io::Write` / `std::io::sink()` / `std::io::stderr()`.

**Spec:** `docs/superpowers/specs/2026-05-16-verbose-option-design.md`.

---

## File Structure

- Modify: `src/pages.rs` — `impl Display for Range`, `pub fn fmt_ranges(&[Range]) -> String`.
- Modify: `src/pages_tests.rs` — Display and `fmt_ranges` tests.
- Modify: `src/cli.rs` — `Command::Run { ..., verbose: bool }`, `Parser` field, `-v`/`--verbose` arm; remove `-v` from `--version` aliases.
- Modify: `src/cli_tests.rs` — update existing `-v` test, add verbose tests.
- Modify: `src/main.rs` — `fmt_header_index` helper, `execute_run` and `load_sources` gain `verbose: bool` + `log: &mut impl Write`, collapse `-o` arms to always use `CountingWriter`, `run()` wires `io::stderr().lock()` or `io::sink()`.
- Modify: `src/main_tests.rs` — adjust every existing `execute_run` call for the new args; add new tests for verbose paths and `fmt_header_index`.
- Modify: `src/help.txt` — add `-v, --verbose` row.
- Modify: `README.md` — add `-v, --verbose` row to options table; add Behaviour bullet describing the stderr log and independence from `-q`.
- Delete (last commit before PR): `docs/superpowers/specs/2026-05-16-verbose-option-design.md` and `docs/superpowers/plans/2026-05-16-verbose-option.md` — per `.claude/CLAUDE.md`, dev docs are removed before PR.

---

## Task 1: `Range` Display and `fmt_ranges`

**Files:**
- Modify: `src/pages.rs`
- Modify: `src/pages_tests.rs`

- [ ] **Step 1: Add failing tests for `Range` Display and `fmt_ranges`**

Append to `src/pages_tests.rs`:

```rust
#[test]
fn range_display_single() {
    let r = Range { start: 3, end: Some(3) };
    assert_eq!(r.to_string(), "3");
}

#[test]
fn range_display_open_ended() {
    let r = Range { start: 4, end: None };
    assert_eq!(r.to_string(), "4-");
}

#[test]
fn range_display_closed_pair() {
    let r = Range { start: 2, end: Some(5) };
    assert_eq!(r.to_string(), "2-5");
}

#[test]
fn range_display_parsed_leading_dash_becomes_one_dash_n() {
    // The parser normalises "-N" into Range { start: 1, end: Some(N) },
    // so re-formatting emits "1-N" rather than "-N".
    let parsed = parse_ranges("-3").unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].to_string(), "1-3");
}

#[test]
fn fmt_ranges_joins_with_commas_preserving_order_and_dupes() {
    let parsed = parse_ranges("1,1,2,4-").unwrap();
    assert_eq!(fmt_ranges(&parsed), "1,1,2,4-");
}

#[test]
fn fmt_ranges_strips_trailing_comma_input() {
    let parsed = parse_ranges("1-3,5,").unwrap();
    assert_eq!(fmt_ranges(&parsed), "1-3,5");
}

#[test]
fn fmt_ranges_single_token() {
    let parsed = parse_ranges("7").unwrap();
    assert_eq!(fmt_ranges(&parsed), "7");
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --quiet range_display fmt_ranges 2>&1 | tail -20
```

Expected: compilation error — `to_string()` requires `Display`, `fmt_ranges` is undefined.

- [ ] **Step 3: Implement `Display` and `fmt_ranges` in `src/pages.rs`**

Add `use std::fmt;` near the top (alongside the existing `use std::ops::RangeInclusive;`).

Append, immediately after the `impl Range { … }` block (after line 56):

```rust
impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.end {
            Some(e) if e == self.start => write!(f, "{}", self.start),
            Some(e) => write!(f, "{}-{}", self.start, e),
            None => write!(f, "{}-", self.start),
        }
    }
}

/// Re-format a parsed range list as a comma-separated spec, preserving
/// order and duplicates. Used by the verbose header.
pub fn fmt_ranges(ranges: &[Range]) -> String {
    let mut out = String::new();
    for (i, r) in ranges.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        use std::fmt::Write;
        let _ = write!(&mut out, "{r}");
    }
    out
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test --quiet range_display fmt_ranges 2>&1 | tail -20
```

Expected: all listed tests pass.

- [ ] **Step 5: Run the full test suite**

```bash
cargo test --quiet 2>&1 | tail -5
```

Expected: all tests pass (no regressions).

- [ ] **Step 6: Format and lint**

```bash
cargo fmt
cargo clippy --quiet -- -D warnings
```

Expected: no diff from fmt, no clippy warnings.

- [ ] **Step 7: Commit**

```bash
git add src/pages.rs src/pages_tests.rs
git commit -m "Add Range Display and fmt_ranges for verbose output"
```

---

## Task 2: Reassign `-v` and add `--verbose` parsing

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/cli_tests.rs`

- [ ] **Step 1: Update existing tests and add verbose tests**

In `src/cli_tests.rs`, locate the `help_and_version_short_circuit` test (around line 98) and remove the line that asserts `-v` → `Version`. The test should read:

```rust
#[test]
fn help_and_version_short_circuit() {
    assert_eq!(parse_args(&["-h"]), Ok(Command::Help));
    assert_eq!(parse_args(&["a.pdf", "--help", "-o"]), Ok(Command::Help));
    assert_eq!(parse_args(&["--version"]), Ok(Command::Version));
    assert_eq!(parse_args(&["-V"]), Ok(Command::Version));
}
```

Append at end of file:

```rust
#[test]
fn verbose_short_and_long() {
    let parse_one = |arg: &str| match parse_args(&["a.pdf", "--count-pages", arg]).unwrap() {
        Command::Run { verbose, .. } => verbose,
        other => panic!("expected Run, got {other:?}"),
    };
    assert!(parse_one("-v"));
    assert!(parse_one("--verbose"));
}

#[test]
fn verbose_defaults_false() {
    match parse_args(&["a.pdf", "--count-pages"]).unwrap() {
        Command::Run { verbose, .. } => assert!(!verbose),
        other => panic!("expected Run, got {other:?}"),
    }
}

#[test]
fn verbose_is_position_independent() {
    for args in [
        &["-v", "a.pdf", "--count-pages"][..],
        &["a.pdf", "-v", "--count-pages"][..],
        &["a.pdf", "--count-pages", "-v"][..],
        &["a.pdf", "-o", "w.pdf", "--verbose"][..],
    ] {
        match parse_args(args).unwrap() {
            Command::Run { verbose, .. } => assert!(verbose, "args = {args:?}"),
            other => panic!("expected Run, got {other:?}"),
        }
    }
}

#[test]
fn verbose_and_quiet_coexist() {
    match parse_args(&["a.pdf", "--count-pages", "-q", "-v"]).unwrap() {
        Command::Run { quiet, verbose, .. } => {
            assert!(quiet);
            assert!(verbose);
        }
        other => panic!("expected Run, got {other:?}"),
    }
    match parse_args(&["a.pdf", "--count-pages", "-v", "-q"]).unwrap() {
        Command::Run { quiet, verbose, .. } => {
            assert!(quiet);
            assert!(verbose);
        }
        other => panic!("expected Run, got {other:?}"),
    }
}

#[test]
fn verbose_rejects_inline_value() {
    use CliError::UnexpectedValue;
    assert_eq!(
        parse_args(&["a.pdf", "--count-pages", "-v=foo"]),
        Err(UnexpectedValue("--verbose"))
    );
    assert_eq!(
        parse_args(&["a.pdf", "--count-pages", "--verbose=foo"]),
        Err(UnexpectedValue("--verbose"))
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --quiet verbose 2>&1 | tail -20
```

Expected: compilation errors — `verbose` field is missing on `Command::Run`.

- [ ] **Step 3: Update `src/cli.rs`**

(a) In the `Command::Run { … }` variant (around line 47), add `verbose: bool`:

```rust
pub enum Command {
    Run {
        inputs: Vec<Input>,
        output: Option<String>,
        count_pages: bool,
        count_bytes: bool,
        quiet: bool,
        verbose: bool,
    },
    Help,
    Version,
}
```

(b) In the `Parser` struct (around line 71), add `verbose: bool`:

```rust
struct Parser<'a> {
    args: &'a [String],
    pos: usize,
    inputs: Vec<Input>,
    output: Option<String>,
    count_pages: bool,
    count_bytes: bool,
    quiet: bool,
    verbose: bool,
    options_done: bool,
}
```

(c) In `Parser::new`, add `verbose: false` to the struct literal.

(d) In `Parser::step`, change the `--version` arm to drop `-v` and add a new `-v` / `--verbose` arm. Replace this single line:

```rust
            "-V" | "-v" | "--version" => return Ok(Some(Command::Version)),
```

with:

```rust
            "-V" | "--version" => return Ok(Some(Command::Version)),
```

Then, in the same `match` (after the `-q` / `--quiet` arm and before the catch-all `_ if opt.starts_with('-') …`), add:

```rust
            "-v" | "--verbose" => {
                Self::reject_value("--verbose", inline)?;
                self.verbose = true;
            }
```

(e) In `finish()` (around line 213), propagate `verbose`:

```rust
        Ok(Command::Run {
            inputs: self.inputs,
            output: self.output,
            count_pages: self.count_pages,
            count_bytes: self.count_bytes,
            quiet: self.quiet,
            verbose: self.verbose,
        })
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test --quiet verbose help_and_version 2>&1 | tail -20
```

Expected: all listed tests pass.

- [ ] **Step 5: Build the full crate**

```bash
cargo build --quiet 2>&1 | tail -20
```

Expected: a compile error in `src/main.rs` from the `Command::Run { … }` destructure not binding `verbose`. This is expected — Task 6 will wire it; for now keep going.

To silence the build break without touching main, update the destructure in `src/main.rs` `run()` (around line 167) to ignore the new field with `..`:

```rust
        Command::Run {
            inputs,
            output,
            count_pages,
            count_bytes,
            quiet,
            ..
        } => {
```

Now:

```bash
cargo build --quiet 2>&1 | tail -5
cargo test --quiet 2>&1 | tail -5
```

Expected: build succeeds, all tests pass.

- [ ] **Step 6: Format and lint**

```bash
cargo fmt
cargo clippy --quiet -- -D warnings
```

Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add src/cli.rs src/cli_tests.rs src/main.rs
git commit -m "Parse -v/--verbose and reassign -v from --version"
```

---

## Task 3: `fmt_header_index` helper

**Files:**
- Modify: `src/main.rs`
- Modify: `src/main_tests.rs`

- [ ] **Step 1: Add failing tests**

Append to `src/main_tests.rs`:

```rust
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
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --quiet fmt_header_index 2>&1 | tail -10
```

Expected: compile error — `fmt_header_index` is undefined.

- [ ] **Step 3: Implement in `src/main.rs`**

Add at end of `main.rs`, just before the `#[cfg(test)]` block:

```rust
/// Format `[i/total]` with `i` right-justified to the width of `total`.
fn fmt_header_index(i: usize, total: usize) -> String {
    let width = total.to_string().len();
    format!("[{i:>width$}/{total}]")
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test --quiet fmt_header_index 2>&1 | tail -10
```

Expected: all three tests pass.

- [ ] **Step 5: Format and lint**

```bash
cargo fmt
cargo clippy --quiet -- -D warnings
```

Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs src/main_tests.rs
git commit -m "Add fmt_header_index helper for verbose header alignment"
```

---

## Task 4: `load_sources` verbose logging

**Files:**
- Modify: `src/main.rs`
- Modify: `src/main_tests.rs`

- [ ] **Step 1: Add failing tests**

Append to `src/main_tests.rs`:

```rust
fn write_tiny_pdf(n: usize, name: &str) -> std::path::PathBuf {
    let mut doc = tiny_doc(n);
    let path = std::env::temp_dir().join(format!("pdfcat-{name}-{}.pdf", std::process::id()));
    let _ = std::fs::remove_file(&path);
    doc.save(&path).unwrap();
    path
}

#[test]
fn load_sources_verbose_logs_header_and_detail_with_pages() {
    let path = write_tiny_pdf(5, "load-verbose-pages");
    let inputs = vec![cli::Input {
        path: path.to_str().unwrap().to_string(),
        ranges: Some(pages::parse_ranges("1,3").unwrap()),
    }];
    let mut log = Vec::new();
    load_sources(&inputs, true, &mut log).unwrap();
    let s = std::str::from_utf8(&log).unwrap();
    let expected = format!(
        "[1/1] {} -p 1,3\n      5 pages total, 2 selected\n",
        path.to_str().unwrap()
    );
    assert_eq!(s, expected);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn load_sources_verbose_uses_all_when_ranges_absent() {
    let path = write_tiny_pdf(3, "load-verbose-all");
    let inputs = vec![cli::Input {
        path: path.to_str().unwrap().to_string(),
        ranges: None,
    }];
    let mut log = Vec::new();
    load_sources(&inputs, true, &mut log).unwrap();
    let s = std::str::from_utf8(&log).unwrap();
    let expected = format!(
        "[1/1] {}\n      3 pages total, all\n",
        path.to_str().unwrap()
    );
    assert_eq!(s, expected);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn load_sources_verbose_pads_header_index() {
    let path = write_tiny_pdf(2, "load-verbose-padded");
    let p = path.to_str().unwrap().to_string();
    let inputs: Vec<cli::Input> = (0..10)
        .map(|_| cli::Input { path: p.clone(), ranges: None })
        .collect();
    let mut log = Vec::new();
    load_sources(&inputs, true, &mut log).unwrap();
    let s = std::str::from_utf8(&log).unwrap();
    assert!(s.contains(&format!("[ 1/10] {p}\n")), "got: {s}");
    assert!(s.contains(&format!("[10/10] {p}\n")), "got: {s}");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn load_sources_silent_when_verbose_false() {
    let path = write_tiny_pdf(2, "load-silent");
    let inputs = vec![cli::Input {
        path: path.to_str().unwrap().to_string(),
        ranges: None,
    }];
    let mut log = Vec::new();
    load_sources(&inputs, false, &mut log).unwrap();
    assert!(log.is_empty());
    let _ = std::fs::remove_file(&path);
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --quiet load_sources_verbose load_sources_silent 2>&1 | tail -20
```

Expected: compile errors — `load_sources` does not take `(verbose, log)` and `pages` / `cli` module names need to be accessible. Since `main_tests.rs` is included via `#[path = "main_tests.rs"] mod tests;` inside `main.rs`, `super::cli::Input` and `super::pages::parse_ranges` are reachable via `use super::*;` (already at line 1).

- [ ] **Step 3: Extend the `pages` use in `src/main.rs`**

Replace the existing import line near the top:

```rust
use pages::{PageSpecError, resolve_ranges};
```

with:

```rust
use pages::{PageSpecError, fmt_ranges, resolve_ranges};
```

- [ ] **Step 4: Replace `load_sources` in `src/main.rs`**

Replace the entire `load_sources` function (currently the last function in the file before the `#[cfg(test)]` block — was around lines 192–217 in the original) with:

```rust
/// Load each input document and resolve its page selection against the actual
/// page count. When `verbose` is true, write a header line to `log` *before*
/// loading each file (so failures are attributable) and a detail line after.
fn load_sources(
    inputs: &[Input],
    verbose: bool,
    log: &mut impl Write,
) -> Result<Vec<(Document, Vec<u32>)>, Error> {
    let mut sources = Vec::with_capacity(inputs.len());
    let total_inputs = inputs.len();
    let indent_width = fmt_header_index(1, total_inputs).len() + 1;
    for (idx, input) in inputs.iter().enumerate() {
        if verbose {
            let head = fmt_header_index(idx + 1, total_inputs);
            match &input.ranges {
                Some(ranges) => {
                    writeln!(log, "{head} {} -p {}", input.path, fmt_ranges(ranges))
                }
                None => writeln!(log, "{head} {}", input.path),
            }
            .map_err(Error::ReportIo)?;
            log.flush().map_err(Error::ReportIo)?;
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
        if verbose {
            let indent = " ".repeat(indent_width);
            let count_str = if input.ranges.is_none() {
                "all".to_string()
            } else {
                format!("{} selected", selected.len())
            };
            writeln!(log, "{indent}{total} pages total, {count_str}")
                .map_err(Error::ReportIo)?;
        }
        sources.push((doc, selected));
    }
    Ok(sources)
}
```

- [ ] **Step 5: Stub the call site so the build passes**

`load_sources` is called from `run()` (formerly as `load_sources(&inputs)`). Final wiring lives in Task 6; for now, just keep the build green. Replace that line with:

```rust
            let mut sink = io::sink();
            let sources = load_sources(&inputs, false, &mut sink)?;
```

(The `verbose` field on `Command::Run` stays unused — already silenced by the `..` in the destructure from Task 2.)

- [ ] **Step 6: Run tests to verify they pass**

```bash
cargo test --quiet load_sources 2>&1 | tail -20
```

Expected: the four new tests pass.

- [ ] **Step 7: Run the full test suite**

```bash
cargo test --quiet 2>&1 | tail -5
```

Expected: no regressions.

- [ ] **Step 8: Format and lint**

```bash
cargo fmt
cargo clippy --quiet -- -D warnings
```

Expected: clean.

- [ ] **Step 9: Commit**

```bash
git add src/main.rs src/main_tests.rs
git commit -m "Emit per-input header and detail lines from load_sources"
```

---

## Task 5: `execute_run` verbose logging and unified output path

**Files:**
- Modify: `src/main.rs`
- Modify: `src/main_tests.rs`

- [ ] **Step 1: Update existing `execute_run` test calls for the new signature**

Every existing test in `src/main_tests.rs` calls `execute_run(&mut doc, output, count_pages, count_bytes, quiet, &mut report)`. The new signature is `execute_run(&mut doc, output, count_pages, count_bytes, quiet, verbose, &mut report, &mut log)`. Use `sed`-style updates by hand: insert `false, ` after `quiet, ` and `, &mut Vec::new()` before the closing paren — but `Vec::new()` needs a binding. Update each call to one of these forms:

```rust
let mut log = Vec::new();
execute_run(&mut doc, output, count_pages, count_bytes, quiet, false, &mut report, &mut log).unwrap();
assert!(log.is_empty());
```

Concretely, update these tests:

- `execute_run_count_pages_only_writes_no_file`
- `execute_run_count_bytes_only_matches_serialized_size`
- `execute_run_both_flags_emit_pages_then_bytes`
- `execute_run_writes_file_when_output_given`
- `execute_run_quiet_count_pages_omits_label`
- `execute_run_quiet_count_bytes_omits_label`
- `execute_run_quiet_both_emits_two_bare_numbers`
- `execute_run_quiet_no_counts_emits_nothing`

For each, add `let mut log = Vec::new();` before the call, insert `false, ` in the args, and append `, &mut log` at the end, then add `assert!(log.is_empty());` after the existing assertions.

Example diff for `execute_run_count_pages_only_writes_no_file`:

```rust
#[test]
fn execute_run_count_pages_only_writes_no_file() {
    let mut doc = tiny_doc(3);
    let mut report = Vec::new();
    let mut log = Vec::new();
    execute_run(&mut doc, None, true, false, false, false, &mut report, &mut log).unwrap();
    assert_eq!(report, b"pages: 3\n");
    assert!(log.is_empty());
}
```

Apply the equivalent change to the other seven tests.

- [ ] **Step 2: Add new tests for verbose paths**

Append to `src/main_tests.rs`:

```rust
#[test]
fn execute_run_verbose_logs_merged_and_wrote_with_bytes() {
    let mut doc = tiny_doc(4);
    let tmp = std::env::temp_dir().join(format!(
        "pdfcat-exec-verbose-{}.pdf",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&tmp);
    let path = tmp.to_str().unwrap().to_string();

    let mut report = Vec::new();
    let mut log = Vec::new();
    execute_run(
        &mut doc,
        Some(&path),
        false,
        false,
        false,
        true,
        &mut report,
        &mut log,
    )
    .unwrap();

    let on_disk = std::fs::metadata(&tmp).unwrap().len();
    let s = std::str::from_utf8(&log).unwrap();
    let lines: Vec<&str> = s.lines().collect();
    assert_eq!(lines.len(), 2, "got: {s}");
    assert_eq!(lines[0], "merged: 4 pages");
    let wrote_prefix = format!("wrote {path} (");
    assert!(lines[1].starts_with(&wrote_prefix), "got: {}", lines[1]);
    assert!(lines[1].ends_with(" bytes)"), "got: {}", lines[1]);
    let inside = lines[1]
        .trim_start_matches(&wrote_prefix)
        .trim_end_matches(" bytes)");
    let reported: u64 = inside.parse().unwrap();
    assert_eq!(reported, on_disk);
    assert!(report.is_empty(), "report should be empty: {report:?}");

    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn execute_run_verbose_no_output_skips_wrote_line() {
    let mut doc = tiny_doc(2);
    let mut report = Vec::new();
    let mut log = Vec::new();
    execute_run(
        &mut doc, None, false, true, false, true, &mut report, &mut log,
    )
    .unwrap();
    let s = std::str::from_utf8(&log).unwrap();
    let lines: Vec<&str> = s.lines().collect();
    assert_eq!(lines, vec!["merged: 2 pages"]);
    let r = std::str::from_utf8(&report).unwrap();
    assert!(r.starts_with("bytes: "), "got: {r}");
}

#[test]
fn execute_run_verbose_with_count_bytes_still_writes_one_wrote_line() {
    let mut doc = tiny_doc(3);
    let tmp = std::env::temp_dir().join(format!(
        "pdfcat-exec-verbose-cb-{}.pdf",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&tmp);
    let path = tmp.to_str().unwrap().to_string();

    let mut report = Vec::new();
    let mut log = Vec::new();
    execute_run(
        &mut doc,
        Some(&path),
        false,
        true,
        false,
        true,
        &mut report,
        &mut log,
    )
    .unwrap();

    let log_s = std::str::from_utf8(&log).unwrap();
    let report_s = std::str::from_utf8(&report).unwrap();
    let on_disk = std::fs::metadata(&tmp).unwrap().len();

    // stderr (log) has the verbose lines.
    assert!(log_s.starts_with("merged: 3 pages\n"), "log: {log_s}");
    assert!(
        log_s.contains(&format!("wrote {path} ({on_disk} bytes)\n")),
        "log: {log_s}"
    );

    // stdout (report) has the labelled byte count from --count-bytes.
    assert_eq!(report_s, format!("bytes: {on_disk}\n"));

    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn execute_run_quiet_and_verbose_coexist() {
    let mut doc = tiny_doc(2);
    let tmp = std::env::temp_dir().join(format!(
        "pdfcat-exec-qv-{}.pdf",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&tmp);
    let path = tmp.to_str().unwrap().to_string();

    let mut report = Vec::new();
    let mut log = Vec::new();
    execute_run(
        &mut doc,
        Some(&path),
        true,
        true,
        true,  // quiet
        true,  // verbose
        &mut report,
        &mut log,
    )
    .unwrap();

    let on_disk = std::fs::metadata(&tmp).unwrap().len();
    let report_s = std::str::from_utf8(&report).unwrap();
    let log_s = std::str::from_utf8(&log).unwrap();

    // quiet: stdout has bare numbers (one per line).
    let mut report_lines = report_s.lines();
    assert_eq!(report_lines.next(), Some("2"));
    let bytes_line = report_lines.next().unwrap();
    assert_eq!(bytes_line.parse::<u64>().unwrap(), on_disk);
    assert_eq!(report_lines.next(), None);

    // verbose: stderr has labelled lines.
    assert!(log_s.starts_with("merged: 2 pages\n"), "log: {log_s}");
    assert!(
        log_s.contains(&format!("wrote {path} ({on_disk} bytes)\n")),
        "log: {log_s}"
    );

    let _ = std::fs::remove_file(&tmp);
}
```

- [ ] **Step 3: Run tests to verify they fail**

```bash
cargo test --quiet execute_run 2>&1 | tail -30
```

Expected: compile errors — the signature mismatch and the new test helpers reference behaviour that does not exist yet.

- [ ] **Step 4: Rewrite `execute_run` in `src/main.rs`**

Replace the existing function (lines 95–150) with:

```rust
fn execute_run(
    merged: &mut Document,
    output: Option<&str>,
    count_pages: bool,
    count_bytes: bool,
    quiet: bool,
    verbose: bool,
    report: &mut impl Write,
    log: &mut impl Write,
) -> Result<(), Error> {
    if verbose {
        writeln!(log, "merged: {} pages", merged.get_pages().len())
            .map_err(Error::ReportIo)?;
    }

    if count_pages {
        write_count(report, "pages", merged.get_pages().len(), quiet)?;
    }

    match (output, count_bytes) {
        (Some(path), need_bytes) => {
            // Always go through a CountingWriter so the verbose `wrote` line
            // can include the byte count even when --count-bytes is absent.
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
            if need_bytes {
                write_count(report, "bytes", w.count(), quiet)?;
            }
            if verbose {
                writeln!(log, "wrote {path} ({} bytes)", w.count())
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
            write_count(report, "bytes", w.count(), quiet)?;
        }
        (None, false) => {
            // Only page count requested; nothing more to do.
        }
    }

    Ok(())
}
```

Note: the `(Some(_), false)` arm has been folded into `(Some(_), need_bytes)` so writing via `CountingWriter` is unconditional whenever there is an output file. `merged.save(path)` is no longer used.

- [ ] **Step 5: Update the caller in `run()`**

In `run()`, find the `execute_run(&mut merged, ...)` call (around line 178) and pass the two new arguments. Use the same temporary stub as Task 4 — full wiring is Task 6. Replace the body of the `Command::Run { ... }` arm with this stub:

```rust
        Command::Run {
            inputs,
            output,
            count_pages,
            count_bytes,
            quiet,
            ..
        } => {
            let mut sink = io::sink();
            let sources = load_sources(&inputs, false, &mut sink)?;
            let mut merged = merge::merge(sources)?;
            let stdout = io::stdout();
            let mut report = stdout.lock();
            execute_run(
                &mut merged,
                output.as_deref(),
                count_pages,
                count_bytes,
                quiet,
                false,
                &mut report,
                &mut sink,
            )
        }
```

- [ ] **Step 6: Run tests to verify they pass**

```bash
cargo test --quiet 2>&1 | tail -10
```

Expected: all tests pass.

- [ ] **Step 7: Format and lint**

```bash
cargo fmt
cargo clippy --quiet -- -D warnings
```

Expected: clean.

- [ ] **Step 8: Commit**

```bash
git add src/main.rs src/main_tests.rs
git commit -m "Emit merged/wrote verbose lines; always count bytes when writing"
```

---

## Task 6: Wire `run()` to actual stderr / sink

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Replace the stub in `run()`**

Replace the `Command::Run { ... } => { ... }` arm with:

```rust
        Command::Run {
            inputs,
            output,
            count_pages,
            count_bytes,
            quiet,
            verbose,
        } => {
            let stdout = io::stdout();
            let mut report = stdout.lock();
            let stderr = io::stderr();
            if verbose {
                let mut log = stderr.lock();
                let sources = load_sources(&inputs, true, &mut log)?;
                let mut merged = merge::merge(sources)?;
                execute_run(
                    &mut merged,
                    output.as_deref(),
                    count_pages,
                    count_bytes,
                    quiet,
                    true,
                    &mut report,
                    &mut log,
                )
            } else {
                let mut sink = io::sink();
                let sources = load_sources(&inputs, false, &mut sink)?;
                let mut merged = merge::merge(sources)?;
                execute_run(
                    &mut merged,
                    output.as_deref(),
                    count_pages,
                    count_bytes,
                    quiet,
                    false,
                    &mut report,
                    &mut sink,
                )
            }
        }
```

Why two arms instead of one? `io::stderr().lock()` and `io::sink()` are different types, and we want zero overhead when verbose is off without resorting to a `Box<dyn Write>`. The two arms keep both branches monomorphic.

- [ ] **Step 2: Smoke-test by hand**

```bash
cargo build --release --quiet 2>&1 | tail -5
```

Expected: clean build.

Create a one-page PDF using the test helper indirectly — or just verify behaviour on an existing PDF if available. The verbose run should print to stderr while stdout stays clean:

```bash
# If no test PDF is at hand, skip this step; the unit tests already cover it.
target/release/pdfcat --help | head -2
```

Expected: help prints from stdout (sanity check that the build runs).

- [ ] **Step 3: Run the full test suite**

```bash
cargo test --quiet 2>&1 | tail -5
```

Expected: all tests pass.

- [ ] **Step 4: Format and lint**

```bash
cargo fmt
cargo clippy --quiet -- -D warnings
```

Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs
git commit -m "Route verbose log to stderr in run()"
```

---

## Task 7: Update `help.txt` and `README.md`

**Files:**
- Modify: `src/help.txt`
- Modify: `README.md`

- [ ] **Step 1: Update `src/help.txt`**

In the `USAGE` line (line 4), append `[-v]` after `[-q]`:

```
    pdfcat <INPUT> [-p <PAGES>] [<INPUT> [-p <PAGES>] ...] [-o <OUTPUT>] [--count-pages] [--count-bytes] [-q] [-v]
```

Between the existing `-q, --quiet` row and the `--` row, insert:

```
    -v, --verbose           Print progress to stderr: per-input header
                            and detail lines, the merged page count, and
                            the byte count written. Independent of -q.
```

- [ ] **Step 2: Update `README.md`**

In the synopsis code block (around line 16), update the option summary to include `[-v]`:

```
pdfcat <INPUT> [-p <PAGES>] [<INPUT> [-p <PAGES>] ...] [-o <OUTPUT>] [--count-pages] [--count-bytes] [-q] [-v]
```

In the options table, add a row immediately before the `--` row:

```
| `-v`, `--verbose` | Print progress (per-input header/detail lines, `merged:`, `wrote ... (B bytes)`) to stderr. Independent of `-q`. |
```

In the "Behaviour" bulleted section, append a new bullet:

```
- `-v` / `--verbose` writes a progress log to stderr: one
  `[i/N] <path>[ -p <spec>]` header line per input (flushed before
  loading so failures are attributable), an indented detail line with
  the page totals, a `merged: <M> pages` line after merging, and (if
  writing a file) `wrote <path> (<B> bytes)`. This log is independent
  of `-q`; the two flags may be combined.
```

- [ ] **Step 3: Verify help renders correctly**

```bash
cargo run --quiet -- --help | tail -20
```

Expected: the `-v, --verbose` row is present and aligned with the others.

- [ ] **Step 4: Commit**

```bash
git add src/help.txt README.md
git commit -m "Document -v/--verbose in help and README"
```

---

## Task 8: Pre-PR cleanup and final verification

**Files:**
- Delete: `docs/superpowers/specs/2026-05-16-verbose-option-design.md`
- Delete: `docs/superpowers/plans/2026-05-16-verbose-option.md`

- [ ] **Step 1: Run full pre-commit checks**

```bash
cargo fmt --check
cargo clippy --quiet -- -D warnings
cargo test --quiet
```

Expected: clean fmt, no clippy warnings, all tests pass.

- [ ] **Step 2: Self-review via subagent**

Per `.claude/CLAUDE.md`, run a subagent review of the diff. Use the general-purpose subagent:

> Prompt: "Review the diff between `main` and `feature/verbose-option` for the pdfcat repo. The change adds a `-v`/`--verbose` flag. Check: (1) the new verbose stderr output matches the spec in `docs/superpowers/specs/2026-05-16-verbose-option-design.md`; (2) the `-v` reassignment from `--version` to `--verbose` is consistent (no leftover references in help/README/tests); (3) `execute_run` always uses `CountingWriter` when an output path is given; (4) verbose output is flushed before any fallible PDF load. Report any issues in under 200 words."

Address any issues raised; commit fixes if any.

- [ ] **Step 3: Remove dev docs**

```bash
git rm docs/superpowers/specs/2026-05-16-verbose-option-design.md
git rm docs/superpowers/plans/2026-05-16-verbose-option.md
# If the docs/superpowers tree is now empty, also remove the empty dirs:
rmdir docs/superpowers/specs docs/superpowers/plans docs/superpowers docs 2>/dev/null || true
git add -A
```

- [ ] **Step 4: Commit cleanup**

```bash
git commit -m "Remove in-tree design and plan docs before PR"
```

- [ ] **Step 5: Final check**

```bash
git log --oneline main..HEAD
cargo test --quiet
```

Expected: a clean commit list and all tests pass. Branch is ready for `commit-commands:commit-push-pr`.
