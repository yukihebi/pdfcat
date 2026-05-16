# Quiet (numeric-only) output Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `-q` / `--quiet` flag that suppresses the `pages: ` / `bytes: ` labels from the existing count flags so scripts can consume the numbers directly. Also reject `=value` on the value-less `--count-*` / `--quiet` flags (previously silently ignored).

**Architecture:** Thread one `bool` (`quiet`) from CLI parsing through `Command::Run` into `execute_run`. Output formatting branches on it: `pages: N` / `bytes: N` when `false`, bare `N` when `true`. A new `CliError::UnexpectedValue(&'static str)` covers the misuse of value-less flags.

**Tech Stack:** Rust 2024 edition, `lopdf`, `thiserror`. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-05-16-quiet-numeric-output-design.md`

---

## File map

- `src/cli.rs` — modify. Add `quiet: bool` to `Parser` and `Command::Run`. Add a `-q`/`--quiet` match arm. Add `CliError::UnexpectedValue` and a `reject_value` helper used by every value-less flag arm.
- `src/main.rs` — modify. Destructure `quiet` from `Command::Run`. Pass it to `execute_run`. Branch `writeln!` calls on it.
- `src/cli_tests.rs` — modify. Add parsing tests for `-q` / `--quiet`, position-independence, `=value` rejection.
- `src/main_tests.rs` — modify. Add `execute_run` tests for `quiet=true`.
- `src/help.txt` — modify. Add `-q, --quiet` line.
- `README.md` — modify. Options table row, behaviour bullet, example.

---

### Task 1: Plumb `quiet` through CLI types

Add the field everywhere it must exist, drive it from `-q` / `--quiet`. No output-format change yet — `execute_run` just ignores the new value for now.

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/main.rs`
- Modify: `src/cli_tests.rs`

- [ ] **Step 1: Add failing CLI tests for `-q` / `--quiet`**

Add to `src/cli_tests.rs` (append):

```rust
#[test]
fn quiet_short_and_long() {
    let parse_one = |arg: &str| match parse_args(&["a.pdf", "--count-pages", arg]).unwrap() {
        Command::Run { quiet, .. } => quiet,
        other => panic!("expected Run, got {other:?}"),
    };
    assert!(parse_one("-q"));
    assert!(parse_one("--quiet"));
}

#[test]
fn quiet_defaults_false() {
    match parse_args(&["a.pdf", "--count-pages"]).unwrap() {
        Command::Run { quiet, .. } => assert!(!quiet),
        other => panic!("expected Run, got {other:?}"),
    }
}

#[test]
fn quiet_is_position_independent() {
    let want = match parse_args(&["a.pdf", "--count-pages", "-q"]).unwrap() {
        Command::Run { quiet, .. } => quiet,
        other => panic!("expected Run, got {other:?}"),
    };
    assert!(want);
    let want = match parse_args(&["-q", "a.pdf", "--count-pages"]).unwrap() {
        Command::Run { quiet, .. } => quiet,
        other => panic!("expected Run, got {other:?}"),
    };
    assert!(want);
}

#[test]
fn quiet_with_only_output_parses() {
    match parse_args(&["a.pdf", "-o", "w.pdf", "-q"]).unwrap() {
        Command::Run {
            quiet,
            count_pages,
            count_bytes,
            output,
            ..
        } => {
            assert!(quiet);
            assert!(!count_pages);
            assert!(!count_bytes);
            assert_eq!(output.as_deref(), Some("w.pdf"));
        }
        other => panic!("expected Run, got {other:?}"),
    }
}

#[test]
fn quiet_alone_is_no_action() {
    assert_eq!(parse_args(&["a.pdf", "-q"]), Err(CliError::NoAction));
}
```

- [ ] **Step 2: Run the new tests and observe failure**

Run: `cargo test --quiet quiet_`
Expected: compile error — `Command::Run` has no field `quiet`.

- [ ] **Step 3: Add `quiet` to types and parsing**

In `src/cli.rs`:

In the `Command::Run` variant, add the field:

```rust
pub enum Command {
    Run {
        inputs: Vec<Input>,
        output: Option<String>,
        count_pages: bool,
        count_bytes: bool,
        quiet: bool,
    },
    Help,
    Version,
}
```

In `Parser`, add the field:

```rust
struct Parser<'a> {
    args: &'a [String],
    pos: usize,
    inputs: Vec<Input>,
    output: Option<String>,
    count_pages: bool,
    count_bytes: bool,
    quiet: bool,
    options_done: bool,
}
```

In `Parser::new`, initialise it:

```rust
fn new(args: &'a [String]) -> Self {
    Parser {
        args,
        pos: 0,
        inputs: Vec::new(),
        output: None,
        count_pages: false,
        count_bytes: false,
        quiet: false,
        options_done: false,
    }
}
```

In `step`, add a match arm between the `--count-bytes` arm and the catch-all `_ if opt.starts_with('-') ...` arm:

```rust
"-q" | "--quiet" => self.quiet = true,
```

In `finish`, include the field in the returned `Command::Run`:

```rust
Ok(Command::Run {
    inputs: self.inputs,
    output: self.output,
    count_pages: self.count_pages,
    count_bytes: self.count_bytes,
    quiet: self.quiet,
})
```

- [ ] **Step 4: Update `main.rs` to destructure the new field**

In `src/main.rs`, in `run()`, the existing `Command::Run` arm becomes:

```rust
Command::Run {
    inputs,
    output,
    count_pages,
    count_bytes,
    quiet: _quiet,
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
```

The `_quiet` prefix silences the unused-variable warning for this task only; Task 3 will start using it.

- [ ] **Step 5: Run tests and verify they pass**

Run: `cargo test --quiet`
Expected: all tests pass, no warnings about an unused `quiet` field on `Parser` (it is read by `finish`) and `_quiet` is fine.

- [ ] **Step 6: Commit**

```bash
git add src/cli.rs src/main.rs src/cli_tests.rs
git commit -m "cli: parse -q/--quiet and plumb through Command::Run"
```

---

### Task 2: Reject `=value` on value-less flags

Replace the existing "silently ignore" behaviour with a clear error for `--count-pages=foo`, `--count-bytes=foo`, `--quiet=foo`, and every alias. `--help` / `--version` remain silent (spec decision).

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/cli_tests.rs`

- [ ] **Step 1: Add failing tests for `=value` rejection**

Append to `src/cli_tests.rs`:

```rust
#[test]
fn value_less_flags_reject_inline_value() {
    use CliError::UnexpectedValue;
    assert_eq!(
        parse_args(&["a.pdf", "--count-pages=foo"]),
        Err(UnexpectedValue("--count-pages"))
    );
    assert_eq!(
        parse_args(&["a.pdf", "--num-pages=foo"]),
        Err(UnexpectedValue("--count-pages"))
    );
    assert_eq!(
        parse_args(&["a.pdf", "--count-bytes=foo"]),
        Err(UnexpectedValue("--count-bytes"))
    );
    assert_eq!(
        parse_args(&["a.pdf", "--nbytes=foo"]),
        Err(UnexpectedValue("--count-bytes"))
    );
    assert_eq!(
        parse_args(&["a.pdf", "--count-pages", "--quiet=foo"]),
        Err(UnexpectedValue("--quiet"))
    );
    assert_eq!(
        parse_args(&["a.pdf", "--count-pages", "-q=foo"]),
        Err(UnexpectedValue("--quiet"))
    );
}

#[test]
fn help_and_version_still_accept_inline_value() {
    assert_eq!(parse_args(&["--help=foo"]), Ok(Command::Help));
    assert_eq!(parse_args(&["--version=foo"]), Ok(Command::Version));
}
```

- [ ] **Step 2: Run the new tests and observe failure**

Run: `cargo test --quiet value_less_flags_reject_inline_value`
Expected: compile error — `CliError::UnexpectedValue` does not exist.

- [ ] **Step 3: Add the error variant**

In `src/cli.rs`, add the variant to `CliError`:

```rust
#[derive(Debug, PartialEq, Eq, Error)]
pub enum CliError {
    #[error("unknown option: {0}")]
    UnknownOption(String),
    #[error("{0} expects a value")]
    MissingValue(&'static str),
    #[error("{0} does not take a value")]
    UnexpectedValue(&'static str),
    #[error("--output specified more than once")]
    DuplicateOutput,
    #[error("--pages must follow an input file")]
    PagesWithoutInput,
    #[error("must specify --output and/or --count-pages/--count-bytes")]
    NoAction,
    #[error("no input files")]
    NoInputs,
    #[error("invalid page spec `{spec}`: {source}")]
    BadPageSpec {
        spec: String,
        #[source]
        source: PageSpecError,
    },
}
```

- [ ] **Step 4: Add a helper and call it from every value-less arm**

In `src/cli.rs`, add a method on `Parser`:

```rust
fn reject_value(label: &'static str, inline: Option<&str>) -> Result<(), CliError> {
    if inline.is_some() {
        Err(CliError::UnexpectedValue(label))
    } else {
        Ok(())
    }
}
```

Replace the three value-less match arms in `step` so each rejects an inline value before setting its flag. The full updated block:

```rust
// No-value flags: any `=value` is rejected.
"--count-pages" | "--count-page" | "--page-count" | "--page-counts" | "--num-pages"
| "--num-page" | "--npages" | "--npage" => {
    Self::reject_value("--count-pages", inline)?;
    self.count_pages = true;
}
"--count-bytes" | "--count-byte" | "--byte-count" | "--byte-counts" | "--num-bytes"
| "--num-byte" | "--nbytes" | "--nbyte" => {
    Self::reject_value("--count-bytes", inline)?;
    self.count_bytes = true;
}
"-q" | "--quiet" => {
    Self::reject_value("--quiet", inline)?;
    self.quiet = true;
}
```

`-h`/`--help` and `-V`/`-v`/`--version` arms are unchanged.

- [ ] **Step 5: Run all tests and verify they pass**

Run: `cargo test --quiet`
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/cli.rs src/cli_tests.rs
git commit -m "cli: reject =value on --count-*/--quiet"
```

---

### Task 3: Quiet-mode output formatting

Make `execute_run` emit `42\n` instead of `pages: 42\n` (and `12345\n` instead of `bytes: 12345\n`) when `quiet` is set.

**Files:**
- Modify: `src/main.rs`
- Modify: `src/main_tests.rs`

- [ ] **Step 1: Add failing tests for quiet-mode output**

Append to `src/main_tests.rs`:

```rust
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
    let tmp = std::env::temp_dir().join(format!(
        "pdfcat-quiet-no-counts-{}.pdf",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&tmp);
    let path = tmp.to_str().unwrap().to_string();

    let mut report = Vec::new();
    execute_run(&mut doc, Some(&path), false, false, true, &mut report).unwrap();
    assert!(report.is_empty());

    let _ = std::fs::remove_file(&tmp);
}
```

The existing `execute_run` tests in `main_tests.rs` each call `execute_run(..., count_pages, count_bytes, &mut report)`. They need a `false` inserted before the `report` argument once `execute_run` grows the new parameter. Update each existing call site to add `false` as the new `quiet` argument:

- `execute_run_count_pages_only_writes_no_file`: `execute_run(&mut doc, None, true, false, false, &mut report)`
- `execute_run_count_bytes_only_matches_serialized_size`: `execute_run(&mut doc, None, false, true, false, &mut report)`
- `execute_run_both_flags_emit_pages_then_bytes`: `execute_run(&mut doc, None, true, true, false, &mut report)`
- `execute_run_writes_file_when_output_given`: `execute_run(&mut doc, Some(&path), false, true, false, &mut report)`

- [ ] **Step 2: Run the new tests and observe failure**

Run: `cargo test --quiet`
Expected: compile error — `execute_run` takes 5 arguments, callers pass 6. (We are about to make this true.)

- [ ] **Step 3: Update `execute_run` to accept and honour `quiet`**

In `src/main.rs`, replace the `execute_run` function with:

```rust
fn execute_run(
    merged: &mut Document,
    output: Option<&str>,
    count_pages: bool,
    count_bytes: bool,
    quiet: bool,
    report: &mut impl Write,
) -> Result<(), Error> {
    if count_pages {
        let n = merged.get_pages().len();
        if quiet {
            writeln!(report, "{n}").map_err(Error::ReportIo)?;
        } else {
            writeln!(report, "pages: {n}").map_err(Error::ReportIo)?;
        }
    }

    match (output, count_bytes) {
        (Some(path), true) => {
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
            let n = w.count();
            if quiet {
                writeln!(report, "{n}").map_err(Error::ReportIo)?;
            } else {
                writeln!(report, "bytes: {n}").map_err(Error::ReportIo)?;
            }
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
            let n = w.count();
            if quiet {
                writeln!(report, "{n}").map_err(Error::ReportIo)?;
            } else {
                writeln!(report, "bytes: {n}").map_err(Error::ReportIo)?;
            }
        }
        (None, false) => {
            // Only page count requested (or nothing at all); nothing more to do.
        }
    }

    Ok(())
}
```

In `run()`, update the `Command::Run` arm to pass `quiet` through (drop the `_quiet` prefix):

```rust
Command::Run {
    inputs,
    output,
    count_pages,
    count_bytes,
    quiet,
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
        quiet,
        &mut report,
    )
}
```

- [ ] **Step 4: Run all tests and verify they pass**

Run: `cargo test --quiet`
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs src/main_tests.rs
git commit -m "main: emit bare numbers when --quiet is set"
```

---

### Task 4: Help text and README

Document the new flag for end-users.

**Files:**
- Modify: `src/help.txt`
- Modify: `README.md`

- [ ] **Step 1: Add `-q, --quiet` to `src/help.txt`**

Insert a new line in the OPTIONS section, after the `--count-bytes` block and before the `--` line:

```
    -q, --quiet             Omit the `pages: ` / `bytes: ` labels and
                            print just the numbers (one per line)
```

Add one example to the EXAMPLES section (after the existing `--count-pages` example):

```
    pdfcat x.pdf --count-pages -q
```

- [ ] **Step 2: Update `README.md`**

In the *Usage* synopsis line, extend the bracketed options:

```
pdfcat <INPUT> [-p <PAGES>] [<INPUT> [-p <PAGES>] ...] [-o <OUTPUT>] [--count-pages] [--count-bytes] [-q]
```

In the options table, add one row after the `--count-bytes` row:

```
| `-q`, `--quiet` | Omit the `pages: ` / `bytes: ` labels; print just the numbers (one per line). |
```

In the *Behaviour* section, replace:

```
- `--count-pages` and `--count-bytes` print one labeled line each to
  stdout (in `pages → bytes` order), and may be combined with `-o`
  or used on their own.
```

with:

```
- `--count-pages` and `--count-bytes` print one labeled line each to
  stdout (in `pages → bytes` order), and may be combined with `-o`
  or used on their own. Add `-q` / `--quiet` to drop the labels and
  emit just the numbers (one per line) for easier scripting.
```

In *Examples*, add after the existing `--count-pages` examples:

```sh
# Numbers only, ready to pipe or assign
N=$(pdfcat x.pdf --count-pages -q)
```

- [ ] **Step 3: Verify the binary still builds and the help renders**

Run: `cargo build --quiet && ./target/debug/pdfcat --help | grep -E '^\s*-q'`
Expected: a single line showing the new `-q, --quiet` row.

- [ ] **Step 4: Commit**

```bash
git add src/help.txt README.md
git commit -m "docs: document -q/--quiet flag"
```

---

### Task 5: Final verification

Run the project-wide checks the user lists in `CLAUDE.md`, plus a subagent self-review.

**Files:** none modified in this task.

- [ ] **Step 1: Format check**

Run: `cargo fmt --check`
Expected: no output, exit code 0. If it fails, run `cargo fmt` and commit the change with message `style: cargo fmt`.

- [ ] **Step 2: Clippy**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: no warnings, exit code 0.

- [ ] **Step 3: Full test suite**

Run: `cargo test --quiet`
Expected: all tests pass.

- [ ] **Step 4: Manual smoke check of the new behaviour**

Run, in a scratch directory with any small PDF named `a.pdf`:

```bash
./target/debug/pdfcat a.pdf --count-pages
./target/debug/pdfcat a.pdf --count-pages -q
./target/debug/pdfcat a.pdf --count-pages --count-bytes -q
./target/debug/pdfcat a.pdf --count-pages=foo
```

Expected output (with a 3-page PDF):

```
pages: 3
3
3
<some-byte-count>
pdfcat: error: --count-pages does not take a value
```

(The exact byte count varies; the important part is the structure.)

If `a.pdf` is not available, skip this step and note it in the handoff to the user.

- [ ] **Step 5: Dispatch a self-review subagent**

Use the Agent tool with `subagent_type: Explore` and prompt:

> Self-review the pending changes on branch `feature/quiet-output`. Read `docs/superpowers/specs/2026-05-16-quiet-numeric-output-design.md`, then read the diff against `main` (use `git diff main...HEAD`). Verify: (a) the new `-q`/`--quiet` flag is plumbed through `cli.rs` and `main.rs`; (b) `--count-pages=foo`, `--count-bytes=foo`, `--quiet=foo` all return `CliError::UnexpectedValue`; (c) `execute_run` emits bare numbers when `quiet` is set and labeled lines otherwise; (d) help text and README mention the new flag; (e) no dead code or stray TODOs. Report under 250 words: what looks correct, and any concrete issues to fix.

If the review reports issues, fix them inline (small enough to handle in this task) and commit with message `fix: address self-review`.

- [ ] **Step 6: Final commit checkpoint**

Run: `git status` and confirm the working tree is clean.
Run: `git log --oneline main..HEAD` and confirm the commit list reads top-to-bottom as the four task commits (plus any fixup).

The branch is now ready for the user to push and open a PR (see CLAUDE.md: development happens on a feature branch, PRs are squash-merged).
