# pdfcat `--verbose` option — design

Date: 2026-05-16
Branch: `feature/verbose-option`

## Goal

Add a `-v` / `--verbose` flag that streams human-readable progress to stderr
while pdfcat runs, so the user can see per-input details, the merged page
count, and the bytes written.

## CLI

- New flag: `-v`, `--verbose`. Takes no value (`-v=foo` → `UnexpectedValue`).
- Independent from `-q` / `--quiet`. Both may be given simultaneously: `-q`
  controls the stdout numeric output (existing), `-v` controls the stderr
  progress log (new). They never interact.
- Position-independent like every other flag.
- `Command::Run` gains a `verbose: bool` field. `Parser` gains a matching
  field and a match arm for `-v` / `--verbose` using `reject_value`.
- No new `CliError` variant; existing `UnknownOption` / `UnexpectedValue`
  suffice.

## Output format (stderr, `-v` only)

```
[i/N] <path>[ -p <normalized-spec>]
      <total> pages total, <selected> selected
... (repeated per input)
merged: <M> pages
wrote <output-path> (<B> bytes)
```

### Header line (per input)

- Form: `[<i>/<N>] <path>[ -p <spec>]`.
- `<i>` is right-justified to the width of `<N>`. For N = 10 the lines line
  up as `[ 1/10]`, `[ 2/10]`, …, `[10/10]`.
- `<path>` is `Input.path` verbatim.
- `-p <spec>` appears only when `Input.ranges` is `Some`. `<spec>` is the
  re-formatted token list (see "Range Display" below); multiple `-p` flags
  on the same input collapse into one comma-separated spec.
- Emitted **before** loading the PDF, then `flush()`ed, so that if loading
  fails the user sees which file caused the failure.

### Detail line (per input)

- Form: `<indent><total> pages total, <selected> selected`, where:
  - `<total>` is the document's page count.
  - `<selected>` is `<N>` when a `-p` selection is given, or the literal
    `all` when ranges are `None`.
- `<indent>` matches the width of `[<i>/<N>] `, i.e. `4 + 2 * width(N)`
  spaces. For N = 10 that is 8 spaces.

### Merged line

- Form: `merged: <M> pages`.
- `<M>` is `merged.get_pages().len()` after `merge::merge` returns.
- Emitted immediately after merging.

### Wrote line

- Only when `-o <path>` is given.
- Form: `wrote <path> (<B> bytes)`, **always** including the byte count.
- `<B>` comes from the `CountingWriter` that wraps the output file.
- When no `-o` is given (counts-only mode), no wrote line is emitted —
  pdfcat wrote no file.

### Range Display (re-formatting `Vec<Range>`)

`Range { start, end }` formats as:

| Case | Output |
| --- | --- |
| `end == Some(s)` and `s == start` | `start` |
| `end == None` | `start-` |
| `end == Some(e)` (general) | `start-e` |

Note: the parser already normalises `-N` into `Range { start: 1, end: Some(N) }`,
so the re-formatter emits `1-N` rather than `-N`. This is by design.

The full spec string is the per-token formatting joined by `,`. Order and
duplicates are preserved.

### Example

```
$ pdfcat a.pdf -p 1,1,2 -p 4- b.pdf c.pdf -p -3 -o out.pdf --count-bytes -v
[1/3] a.pdf -p 1,1,2,4-
      10 pages total, 9 selected
[2/3] b.pdf
      5 pages total, all
[3/3] c.pdf -p 1-3
      8 pages total, 3 selected
merged: 17 pages
wrote out.pdf (234567 bytes)
```

(stdout still receives `pages: …` / `bytes: …` from `--count-pages` /
`--count-bytes` if requested, independently.)

## Implementation

### `cli.rs`

- `Command::Run { …, verbose: bool }`.
- `Parser` gets a `verbose: bool` field and a new match arm:
  ```rust
  "-v" | "--verbose" => {
      Self::reject_value("--verbose", inline)?;
      self.verbose = true;
  }
  ```
- `finish()` propagates `verbose` into `Command::Run`.

### `pages.rs`

- `impl Display for Range` per the table above.
- `pub fn fmt_ranges(ranges: &[Range]) -> String` that joins each
  `Range` with `,`. Used by the verbose header.

### `main.rs`

- New helper `fn fmt_header_index(i: usize, total: usize) -> String` that
  returns `"[<i right-justified>/<total>]"`, unit-testable.
- `execute_run` signature becomes:
  ```rust
  fn execute_run(
      merged: &mut Document,
      output: Option<&str>,
      count_pages: bool,
      count_bytes: bool,
      quiet: bool,
      verbose: bool,
      report: &mut impl Write,  // stdout-side
      log: &mut impl Write,     // stderr-side
  ) -> Result<(), Error>;
  ```
- `load_sources` signature becomes:
  ```rust
  fn load_sources(
      inputs: &[Input],
      verbose: bool,
      log: &mut impl Write,
  ) -> Result<Vec<(Document, Vec<u32>)>, Error>;
  ```
  Per input:
  1. If `verbose`, write the header line and `log.flush()`.
  2. `Document::load` + page-count check + `resolve_ranges`.
  3. If `verbose`, write the detail line.
- `run()` produces `log` based on the flag:
  - `verbose = true` → `let mut log = io::stderr().lock();`
  - `verbose = false` → `let mut log = io::sink();`
  Then passes `log` (and `verbose`) into both helpers.
- `execute_run` writes the `merged: <M> pages` line at its start (before
  the count-pages / output handling) when `verbose` is true. This keeps
  every verbose stderr line that depends on the merged document
  observable through `log` in `execute_run`'s unit tests.
- In `execute_run`, when `output.is_some()`, *always* go through a
  `CountingWriter` (regardless of `count_bytes`) so the verbose wrote
  line can include the byte count. The two output-bearing arms collapse
  to one. When `count_bytes` is true the byte count also goes to
  `report` as before; the new `wrote` line uses the same count.

### Help / README

- Add `-v, --verbose` row to `src/help.txt` and the README options table.
- Add a Behaviour bullet describing the stderr log and that it is
  independent of `-q`.

## Testing

### `pages_tests.rs`

- `Range::Display`: single, `start-`, `start-end`.
- `fmt_ranges`: multi-token join, order/duplicates preserved
  (`[1,1,2,4-]`), no trailing comma.

### `cli_tests.rs`

- `-v` alone sets `verbose = true`.
- `--verbose` alone sets `verbose = true`.
- `-v=foo` → `UnexpectedValue("--verbose")`.
- `-q -v` and `-v -q` both yield `quiet = true, verbose = true`.
- `-v` accepted at various positions (before inputs, between flags,
  after `-o`).

### `main_tests.rs`

- Update every existing call to `execute_run` to add the new `verbose`
  and `log` arguments. For `verbose = false`, pass `&mut Vec::new()` as
  `log` and assert it is empty.
- `fmt_header_index` width tests:
  - N=3: `fmt_header_index(1, 3) == "[1/3]"`.
  - N=10: `fmt_header_index(1, 10) == "[ 1/10]"`, `fmt_header_index(10, 10)
    == "[10/10]"`.
- `execute_run` with `verbose = true` and `-o <tmp>`:
  - `log` contains `merged: <M> pages\n` and `wrote <tmp> (<B> bytes)\n`.
  - `B` matches the on-disk size.
- `execute_run` with `verbose = true` and no output (counts only):
  - `log` contains `merged: <M> pages\n` but no `wrote` line.
- `execute_run` with `verbose = false`: `log` is empty.
- `execute_run` with `verbose = true`, `-o <tmp>`, and `count_bytes = false`:
  the `(B bytes)` is still present.
- `load_sources` verbose path (creating a tempfile PDF via `tiny_doc`):
  - `log` contains the header `[1/1] <path> -p 1\n` followed by the
    detail line `      <total> pages total, 1 selected\n`.
  - With no `-p`, the detail line uses `, all`.
- `-q -v` end-to-end via `execute_run`: stdout (`report`) has unlabeled
  numbers, stderr (`log`) has the standard verbose lines.

## Process note

Per `.claude/CLAUDE.md`, in-progress design docs may live on the branch
but must be removed before the final PR is opened (the substance migrates
to README / inline comments). This file should be deleted in the last
commit on `feature/verbose-option` before raising the PR.

## Out of scope

- Multiple verbosity levels (`-vv`, etc.).
- Warnings about dropped PDF features (Outlines, AcroForm, Names, …).
- TTY-aware streaming / carriage-return progress overwriting.
- Colour output.
