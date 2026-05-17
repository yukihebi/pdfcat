# README quickstart & CLI error guidance — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Surface a reusable three-command Quickstart block at the top
of `README.md`, inside `pdfcat --help`, and on every CLI parse error
(including the no-argument case). Switch the primary spelling of the
counters to `--npages` / `--nbytes` and drop the now-redundant
`EXAMPLES` block from `--help`.

**Architecture:** A single source-of-truth file `src/quickstart.txt`
is embedded both into `HELP` (via `concat!` of three include_str!
fragments) and into the new `cli_error_message` helper that
`main.rs` calls for `CliError`. Other runtime errors keep their
current short message.

**Tech Stack:** Rust 2024 edition, `lopdf`, `thiserror`. No new
dependencies. All tests live in sibling `*_tests.rs` files per the
existing repo pattern.

**Spec:** `docs/superpowers/specs/2026-05-17-readme-quickstart-design.md`

**Branch:** `feature/readme-quickstart` (already checked out).

---

## File map

- Create:
  - `src/quickstart.txt`
  - `src/help_head.txt`
  - `src/help_tail.txt`
- Modify:
  - `src/cli.rs` — new `QUICKSTART` constant, new `cli_error_message`
    helper, `HELP` rebuilt from `concat!`
  - `src/cli_tests.rs` — tests for `QUICKSTART`, `HELP`, and
    `cli_error_message`
  - `src/main.rs` — drop early empty-argv branch, route `CliError`
    through `cli_error_message`
  - `README.md` — add `## Quickstart`, rework `## Usage` / `## Examples`
    / `## Behaviour` to use the new primary spellings
- Delete:
  - `src/help.txt`

---

## Pre-commit checklist for every task

Before each `git commit` step, run in sequence:

```sh
cargo fmt
cargo clippy -- -D warnings
cargo test
```

All three must be clean. If anything fails, fix it before the commit
step. (CLAUDE.md mandates this; the plan does not repeat the commands
in every step but they are part of every commit.)

---

### Task 1: Add `quickstart.txt` and `QUICKSTART` constant

**Files:**
- Create: `src/quickstart.txt`
- Modify: `src/cli.rs:6-8` (extend the existing constants block)
- Test: `src/cli_tests.rs` (append at end of file)

- [ ] **Step 1: Append failing test to `src/cli_tests.rs`**

Add this test at the bottom of the file:

```rust
#[test]
fn quickstart_block_has_canonical_shape() {
    let q = QUICKSTART;
    assert!(q.starts_with("Quickstart:\n"), "got: {q:?}");
    assert!(q.contains("pdfcat x.pdf y.pdf z.pdf -o out.pdf"));
    assert!(q.contains("pdfcat x.pdf -p -3,5,7- -o out.pdf"));
    assert!(q.contains("pdfcat x.pdf --npages"));
    assert!(q.ends_with('\n'));
}
```

- [ ] **Step 2: Run the new test to verify it fails**

Run: `cargo test --lib quickstart_block_has_canonical_shape`
Expected: compile error — `cannot find value 'QUICKSTART'`.

- [ ] **Step 3: Create `src/quickstart.txt`**

Write this exact content (note the leading `Quickstart:` line, two-space
indentation, and a single trailing newline):

```
Quickstart:
  # Concatenate x, y, z into out.pdf
  pdfcat x.pdf y.pdf z.pdf -o out.pdf

  # Take pages 1, 2, 3, 5, and 7-end of x.pdf into out.pdf
  pdfcat x.pdf -p -3,5,7- -o out.pdf

  # Print the page count of x.pdf
  pdfcat x.pdf --npages
```

- [ ] **Step 4: Add `QUICKSTART` constant in `src/cli.rs`**

After the existing `pub const HELP: &str = include_str!("help.txt");`
line, insert:

```rust
pub const QUICKSTART: &str = include_str!("quickstart.txt");
```

- [ ] **Step 5: Run the new test to verify it passes**

Run: `cargo test --lib quickstart_block_has_canonical_shape`
Expected: PASS.

- [ ] **Step 6: Run pre-commit checklist**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: clean, all tests pass.

- [ ] **Step 7: Commit**

```sh
git add src/quickstart.txt src/cli.rs src/cli_tests.rs
git commit -m "Add reusable QUICKSTART block"
```

---

### Task 2: Split `help.txt` and switch primary spelling

This task replaces the single `help.txt` with `help_head.txt` +
`quickstart.txt` + `help_tail.txt` (concatenated at compile time),
removes the `EXAMPLES:` block, and switches the primary spelling of
the counters to `--npages` / `--nbytes`.

**Files:**
- Create: `src/help_head.txt`, `src/help_tail.txt`
- Delete: `src/help.txt`
- Modify: `src/cli.rs` (replace the `HELP` definition)
- Test: `src/cli_tests.rs` (append three tests)

- [ ] **Step 1: Append failing tests to `src/cli_tests.rs`**

Add at the bottom of the file:

```rust
#[test]
fn help_embeds_quickstart_verbatim() {
    assert!(HELP.contains(QUICKSTART), "HELP does not contain QUICKSTART");
}

#[test]
fn help_drops_examples_section() {
    assert!(!HELP.contains("EXAMPLES:"), "HELP still contains EXAMPLES:");
}

#[test]
fn help_uses_npages_nbytes_as_primary() {
    // Primary spellings appear as left-column entries in the OPTIONS
    // table, so they are followed by at least one space.
    assert!(HELP.contains("    --npages "), "missing --npages primary row");
    assert!(HELP.contains("    --nbytes "), "missing --nbytes primary row");
    // Old spellings are kept as aliases.
    assert!(HELP.contains("--count-pages"));
    assert!(HELP.contains("--count-bytes"));
}
```

- [ ] **Step 2: Run new tests to verify they fail**

Run: `cargo test --lib help_`
Expected: at least two of the three FAIL (current `HELP` has
`EXAMPLES:` and does not embed `QUICKSTART`).

- [ ] **Step 3: Create `src/help_head.txt`**

Exact content (ends with a blank line, i.e. two trailing newlines, so
that the embedded Quickstart sits below an empty visual gap):

```
pdfcat - concatenate PDFs and extract pages

USAGE:
    pdfcat <INPUT> [-p <PAGES>] [<INPUT> [-p <PAGES>] ...] [-o <OUTPUT>] [--npages] [--nbytes] [-q] [-v]

    At least one of -o, --npages, --nbytes must be given.

```

(The file must end with the blank line above the closing fence.)

- [ ] **Step 4: Create `src/help_tail.txt`**

Exact content (starts with a single blank line so `OPTIONS:` is one
empty line below the Quickstart block):

```

OPTIONS:
    -h, --help              Print this help
    -V, --version           Print version
    -o, --output <FILE>     Output file
    -p, --pages <SPEC>      Page selection for the preceding input file
                            (aliases: --page, -pp)
    --npages                Print the merged page count to stdout
                            (aliases: --count-pages, --count-page,
                            --page-count, --page-counts, --num-pages,
                            --num-page, --npage)
    --nbytes                Print the merged byte count to stdout
                            (aliases: --count-bytes, --count-byte,
                            --byte-count, --byte-counts, --num-bytes,
                            --num-byte, --nbyte)
    -q, --quiet             Omit the `pages: ` / `bytes: ` labels and
                            print just the numbers (one per line)
    -v, --verbose           Print progress to stderr: per-input header
                            and detail lines, the merged page count, and
                            the byte count written. Independent of -q.
    --                      Treat every following argument as an input file

PAGE SPEC (1-based, comma-separated, trailing comma optional):
    N        page N
    -N       pages from the first to N (inclusive)
    N-       pages from N to the last (inclusive)
    N-M      pages N to M (inclusive)
    e.g.  -p 1        -p -2,4-        -p 1-3,5
```

- [ ] **Step 5: Replace `HELP` definition in `src/cli.rs`**

Change:

```rust
pub const HELP: &str = include_str!("help.txt");
```

to:

```rust
pub const HELP: &str = concat!(
    include_str!("help_head.txt"),
    include_str!("quickstart.txt"),
    include_str!("help_tail.txt"),
);
```

- [ ] **Step 6: Delete the old `src/help.txt`**

Run: `git rm src/help.txt`

- [ ] **Step 7: Run new tests to verify they pass**

Run: `cargo test --lib help_`
Expected: all three PASS.

- [ ] **Step 8: Visually inspect the rendered `--help` output**

Run: `cargo run --quiet -- --help`
Expected: the help text contains, in order, the tagline, `USAGE:`
block, a blank line, the `Quickstart:` block, a blank line, the
`OPTIONS:` table (with `--npages` and `--nbytes` as the primary rows),
the `PAGE SPEC` section, and NO `EXAMPLES:` section.

- [ ] **Step 9: Run pre-commit checklist**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: clean, all tests pass.

- [ ] **Step 10: Commit**

```sh
git add src/cli.rs src/cli_tests.rs src/help_head.txt src/help_tail.txt
git rm src/help.txt   # already staged by Step 6, but harmless if re-run
git commit -m "Embed Quickstart in --help; switch primary to --npages/--nbytes"
```

---

### Task 3: Add `cli_error_message` helper

**Files:**
- Modify: `src/cli.rs` (add a new `pub fn`)
- Test: `src/cli_tests.rs` (append one test)

- [ ] **Step 1: Append failing test to `src/cli_tests.rs`**

Add at the bottom of the file:

```rust
#[test]
fn cli_error_message_appends_quickstart_and_help_hint() {
    let err = CliError::UnknownOption("--foo".to_string());
    let msg = cli_error_message(&err);
    assert!(
        msg.starts_with("pdfcat: unknown option: --foo\n"),
        "unexpected prefix: {msg:?}"
    );
    assert!(msg.contains("\n\nQuickstart:\n"));
    assert!(msg.contains("pdfcat x.pdf y.pdf z.pdf -o out.pdf"));
    assert!(
        msg.ends_with("\nRun 'pdfcat --help' for details.\n"),
        "unexpected suffix: {msg:?}"
    );
}
```

- [ ] **Step 2: Run the new test to verify it fails**

Run: `cargo test --lib cli_error_message_appends_quickstart_and_help_hint`
Expected: compile error — `cannot find function 'cli_error_message'`.

- [ ] **Step 3: Add the helper in `src/cli.rs`**

Add this function just below the `QUICKSTART` constant:

```rust
/// Format a CLI parse error with the Quickstart block and a pointer
/// to `--help`, ready to write to stderr.
pub fn cli_error_message(err: &CliError) -> String {
    format!("pdfcat: {err}\n\n{QUICKSTART}\nRun 'pdfcat --help' for details.\n")
}
```

- [ ] **Step 4: Run the new test to verify it passes**

Run: `cargo test --lib cli_error_message_appends_quickstart_and_help_hint`
Expected: PASS.

- [ ] **Step 5: Run pre-commit checklist**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: clean, all tests pass.

- [ ] **Step 6: Commit**

```sh
git add src/cli.rs src/cli_tests.rs
git commit -m "Add cli_error_message helper bundling Quickstart"
```

---

### Task 4: Wire `main.rs` to the helper

`main.rs` is not unit-tested in this repo, so verification is by
running the binary and inspecting stderr.

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Edit `src/main.rs`**

Replace the existing `main` function:

```rust
fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
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
```

with:

```rust
fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Error::Cli(ref cli_err)) => {
            eprint!("{}", cli::cli_error_message(cli_err));
            ExitCode::FAILURE
        }
        Err(err) => {
            eprintln!("pdfcat: {err}");
            ExitCode::FAILURE
        }
    }
}
```

- [ ] **Step 2: Verify the unknown-option error path manually**

Run: `cargo run --quiet -- --foo`
Expected stderr (verbatim):

```
pdfcat: unknown option: --foo

Quickstart:
  # Concatenate x, y, z into out.pdf
  pdfcat x.pdf y.pdf z.pdf -o out.pdf

  # Take pages 1, 2, 3, 5, and 7-end of x.pdf into out.pdf
  pdfcat x.pdf -p -3,5,7- -o out.pdf

  # Print the page count of x.pdf
  pdfcat x.pdf --npages

Run 'pdfcat --help' for details.
```

Exit code: `1` (FAILURE). Confirm with: `echo $?`.

- [ ] **Step 3: Verify the no-argument path manually**

Run: `cargo run --quiet`
Expected stderr: the same Quickstart block as Step 2, but with the
first line replaced by:

```
pdfcat: no input files (need at least one PDF)
```

Exit code: `1`.

- [ ] **Step 4: Verify a runtime error path is unchanged**

Run: `cargo run --quiet -- nonexistent.pdf -o /tmp/out.pdf`
Expected stderr: a single line of the form
`pdfcat: nonexistent.pdf: cannot open: ...` (no Quickstart block).
Exit code: `1`.

- [ ] **Step 5: Verify the `--help` path still works**

Run: `cargo run --quiet -- --help`
Expected stdout: the full HELP text from Task 2, exit code `0`.

- [ ] **Step 6: Run pre-commit checklist**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: clean, all tests pass.

- [ ] **Step 7: Commit**

```sh
git add src/main.rs
git commit -m "Route CLI parse errors through cli_error_message"
```

---

### Task 5: Rewrite `README.md`

This task is documentation-only. There is no automated test; the
acceptance check is reading the rendered file and confirming the
sections match the spec.

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Replace `README.md` with the content below**

Use the `Write` tool to overwrite the file with this exact content:

````markdown
# pdfcat

[![CI](https://github.com/yukihebi/pdfcat/actions/workflows/ci.yml/badge.svg)](https://github.com/yukihebi/pdfcat/actions/workflows/ci.yml)

A small command-line tool to concatenate PDF files and extract pages from
them. Written in pure Rust (using [`lopdf`](https://crates.io/crates/lopdf));
no native dependencies.

## Quickstart

```sh
# Concatenate x, y, z into out.pdf
pdfcat x.pdf y.pdf z.pdf -o out.pdf

# Take pages 1, 2, 3, 5, and 7-end of x.pdf into out.pdf
pdfcat x.pdf -p -3,5,7- -o out.pdf

# Print the page count of x.pdf
pdfcat x.pdf --npages
```

Run `pdfcat --help` for details. The same Quickstart block is also
printed to stderr whenever the command line cannot be parsed.

## Build

```sh
cargo build --release
# binary at target/release/pdfcat
```

## Usage

```
pdfcat <INPUT> [-p <PAGES>] [<INPUT> [-p <PAGES>] ...] [-o <OUTPUT>] [--npages] [--nbytes] [-q] [-v]
```

Inputs are processed in the order given and concatenated. `-p`/`--pages`
selects pages from the *immediately preceding* input file; without it, the
whole file is used. `-o`/`--output` may appear anywhere; at least one of
`-o`, `--npages`, `--nbytes` must be given.

| Option | Description |
| --- | --- |
| `-h`, `--help` | Print help |
| `-V`, `--version` | Print version |
| `-o`, `--output <FILE>` | Output file |
| `-p`, `--pages <SPEC>` | Page selection for the preceding input (aliases: `--page`, `-pp`) |
| `--npages` | Print the merged page count to stdout. Aliases: `--count-pages`, `--count-page`, `--page-count`, `--page-counts`, `--num-pages`, `--num-page`, `--npage` |
| `--nbytes` | Print the merged byte count to stdout. Aliases: `--count-bytes`, `--count-byte`, `--byte-count`, `--byte-counts`, `--num-bytes`, `--num-byte`, `--nbyte` |
| `-q`, `--quiet` | Omit the `pages: ` / `bytes: ` labels; print just the numbers (one per line). |
| `-v`, `--verbose` | Print progress (per-input header/detail lines, `merged:`, `wrote ... (B bytes)`) to stderr. Independent of `-q`. |
| `--` | Treat every following argument as an input file |

### Page spec

A page spec is a comma-separated list (a trailing comma is allowed). Page
numbers are 1-based and ranges are inclusive:

| Token | Meaning |
| --- | --- |
| `N` | page `N` |
| `-N` | pages from the first up to `N` |
| `N-` | pages from `N` to the last |
| `N-M` | pages `N` through `M` |

Order and duplicates are preserved (`-p 3,1,1` yields pages 3, 1, 1 in that
order).

## Examples

Each example below demonstrates one feature beyond the Quickstart block.

```sh
# Combine ranges from multiple inputs (each -p binds to the preceding file)
pdfcat x.pdf -p 1-3 y.pdf -p 5- -o out.pdf

# Count without writing: prints `pages: N` and `bytes: M`, one per line
pdfcat x.pdf y.pdf --npages --nbytes

# Numeric-only output for scripts (no labels, one number per line)
N=$(pdfcat x.pdf --npages -q)

# Write and report at the same time
pdfcat x.pdf -p 1-3 -o out.pdf --nbytes

# An input whose name starts with `-`
pdfcat -o out.pdf -- -scan.pdf

# Verbose progress on stderr while writing
pdfcat x.pdf y.pdf -o out.pdf -v
```

## Behaviour

- `--npages` and `--nbytes` print one labeled line each to stdout
  (in `pages → bytes` order), and may be combined with `-o` or used
  on their own. Add `-q` / `--quiet` to drop the labels and emit
  just the numbers (one per line) for easier scripting.
- `-v` / `--verbose` writes a progress log to stderr: one
  `[i/N] <path>[ -p <spec>]` header line per input (flushed before
  loading so failures are attributable), an indented detail line with
  the page totals, a `merged: <M> pages` line after merging, and (if
  writing a file) `wrote <path> (<B> bytes)`. This log is independent
  of `-q`; the two flags may be combined.
- Pages keep their original size; mixing different page sizes is fine.
- Inherited page attributes (`Resources`, `MediaBox`, `CropBox`, `Rotate`) are
  flattened onto each page so geometry survives the merge.
- The document `/Info` (metadata) of the first input is carried over.

## Limitations

- Bookmarks (`/Outlines`), forms (`/AcroForm`), the name tree (`/Names`),
  page labels and the document open action are dropped. Link annotations that
  point into those structures may become dangling.
- Only the file given to `-o` is written; there is no multi-file split mode.
````

- [ ] **Step 2: Read the file back and sanity-check the section order**

Run: `cargo run --quiet -- --help | head -20`
This is unrelated to the README, but make sure no test was broken by
the README change.

Open `README.md` (e.g. via `cat README.md` or your editor) and confirm:
- The order is: title → badge → tagline → Quickstart → Build → Usage →
  Page spec → Examples → Behaviour → Limitations.
- The Quickstart code block contains the same three commands and
  comments as `src/quickstart.txt`.
- The Options table has `--npages` and `--nbytes` as primary entries
  (each with the old spellings listed as aliases).
- The `## Behaviour` section refers to `--npages` and `--nbytes`, not
  to `--count-pages` / `--count-bytes`.

- [ ] **Step 3: Run pre-commit checklist**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: clean, all tests pass.

- [ ] **Step 4: Commit**

```sh
git add README.md
git commit -m "Add Quickstart section; align README with new help and spellings"
```

---

### Task 6: Self-review via subagent and clean up

CLAUDE.md asks for a subagent self-review before each commit. We
batch a single review at the end of the branch so the reviewer can
look at the cumulative diff in one shot.

**Files:** none new.

- [ ] **Step 1: Dispatch a fresh review subagent**

Use the `Agent` tool with `subagent_type: "Explore"` (read-only review
is enough). Prompt:

> Review the diff of the current branch `feature/readme-quickstart`
> against `main`. The spec is at
> `docs/superpowers/specs/2026-05-17-readme-quickstart-design.md`. For
> each of the following, report whether it is satisfied and quote the
> specific file + line where you verified it:
>
> 1. `src/quickstart.txt` exists and contains the three canonical
>    commands with the expected comments.
> 2. `src/help.txt` no longer exists; `src/help_head.txt` and
>    `src/help_tail.txt` do, and `cli.rs` builds `HELP` via
>    `concat!` of head + quickstart + tail.
> 3. The OPTIONS table in `help_tail.txt` lists `--npages` and
>    `--nbytes` as the primary spellings with the old spellings as
>    aliases. The `EXAMPLES:` section is gone.
> 4. `cli.rs` exposes a `cli_error_message(&CliError) -> String`
>    that prepends `pdfcat: `, embeds `QUICKSTART`, and ends with
>    `Run 'pdfcat --help' for details.\n`.
> 5. `main.rs` routes `Error::Cli(_)` through `cli_error_message`
>    and lets other `Error` variants keep their short message.
> 6. The no-argument case (`args.is_empty()`) is no longer
>    short-circuited at the top of `main`; it now falls through to
>    `CliError::NoInputs`.
> 7. `README.md` has a `## Quickstart` section before `## Build`,
>    the Options table is rewritten with `--npages` / `--nbytes`
>    primary, the `## Examples` section is rewritten with the
>    feature-oriented examples listed in the spec, and the
>    `## Behaviour` section refers to `--npages` / `--nbytes`.
> 8. All references to `--count-pages` / `--count-bytes` in README
>    and help are listed only as aliases, never as primary names.
>
> Report any deviation as a bullet, quoting `file:line`. Keep the
> response under 300 words.

- [ ] **Step 2: Address any findings**

If the review surfaces deviations from the spec, fix them inline,
re-run the pre-commit checklist, and commit a fixup:

```sh
git add <files>
git commit -m "Address self-review: <short description>"
```

If the review is clean, skip.

- [ ] **Step 3: Final test run on a clean tree**

Run: `cargo fmt -- --check && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: every check clean.

- [ ] **Step 4: (Optional) Push and open PR**

This step is only performed if the user explicitly asks for the PR.
Do not push without confirmation.

```sh
git push -u origin feature/readme-quickstart
gh pr create --title "Add Quickstart guide to README, --help, and CLI errors" --body "..."
```

---

## Self-review checklist (plan author)

- **Spec coverage:**
  - Quickstart content → Task 1
  - Source of truth (`quickstart.txt`) → Task 1
  - `help.txt` split into head/quickstart/tail and HELP via `concat!` → Task 2
  - EXAMPLES section removed from HELP → Task 2
  - Primary spelling switched to `--npages` / `--nbytes` in HELP and
    README → Tasks 2 (HELP) and 5 (README)
  - Whitespace around the embedded block → Task 2 (Steps 3 and 4 fix
    the trailing blank line in head and leading blank line in tail)
  - CLI error output format → Task 3 (helper) + Task 4 (wiring)
  - No-args case falls through to `NoInputs` → Task 4 Step 1 + Step 3
  - README Quickstart, Usage, Examples, Behaviour updates → Task 5
  - Tests covering QUICKSTART, HELP, helper → Tasks 1, 2, 3
- **Placeholders:** none — every code block contains the actual code or
  command to run.
- **Type consistency:** the helper is named `cli_error_message` in
  Task 3 (definition), Task 4 (call site), and Task 6 (review prompt);
  the constants are `QUICKSTART` and `HELP` everywhere they appear.
