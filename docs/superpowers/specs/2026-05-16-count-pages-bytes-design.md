# Design: page count and byte count output

Date: 2026-05-16
Branch: `feature/count-pages-bytes`

## Goal

Let users ask pdfcat to report the page count and/or byte count of the
merged result, independently of whether an output file is written.

Three usage modes must be supported:

1. Together with `-o` — write the file and also print counts.
2. Without `-o` — print counts without writing a file.
3. Page count and byte count are each independent; either alone or both.

## Scope

In scope:

- New CLI flags for page count and byte count of the merged result.
- Stdout reporting in a fixed, self-describing format.
- Adjusting the "must specify an action" CLI error so `-o` is no longer
  mandatory when at least one count flag is given.

Out of scope (for this change):

- Per-input reporting (a future `--verbose`-style flag may cover this).
- Any flag taking a value (these are boolean switches).
- Short single-letter aliases (`-c`, `-np`, etc.).

## CLI surface

Two new boolean flags. Either, both, or neither may be given.

### `--count-pages`

Print the number of pages in the merged result to stdout.

Accepted aliases (all equivalent):

| Form |
| --- |
| `--count-pages` (primary) |
| `--count-page` |
| `--page-count` |
| `--page-counts` |
| `--num-pages` |
| `--num-page` |
| `--npages` |
| `--npage` |

### `--count-bytes`

Print the byte count of the merged result (as it would be / is written
to disk) to stdout.

Accepted aliases (all equivalent):

| Form |
| --- |
| `--count-bytes` (primary) |
| `--count-byte` |
| `--byte-count` |
| `--byte-counts` |
| `--num-bytes` |
| `--num-byte` |
| `--nbytes` |
| `--nbyte` |

### Combination matrix

| `-o` | `--count-pages` | `--count-bytes` | Behavior |
| --- | --- | --- | --- |
| ✓ | – | – | Write file (current behavior). |
| ✓ | ✓ | – | Write file, print pages. |
| ✓ | – | ✓ | Write file, print bytes. |
| ✓ | ✓ | ✓ | Write file, print pages and bytes. |
| – | ✓ | – | Print pages (no file written). |
| – | – | ✓ | Print bytes (no file written). |
| – | ✓ | ✓ | Print pages and bytes (no file written). |
| – | – | – | Error: must specify `-o` and/or a count flag. |

Repeating a count flag is a no-op (the flags are idempotent). Repeating
`-o` remains a hard error (`DuplicateOutput`), as today.

## Output format

Counts go to **stdout**. Errors continue to go to stderr.

Format: one labeled line per requested count, in a fixed order
(pages → bytes), regardless of command-line order:

```
pages: <N>
bytes: <N>
```

Examples:

- `--count-pages` alone:
  ```
  pages: 5
  ```
- `--count-bytes` alone:
  ```
  bytes: 12345
  ```
- Both:
  ```
  pages: 5
  bytes: 12345
  ```

The order is fixed so callers do not have to track argv order to parse
the output. The labels make single-line output self-describing.

## Behavior details

### Page count

After all inputs are loaded and page selections resolved, the merged
document's page count is `merged.get_pages().len()`. No serialization
is required.

### Byte count

The byte count is "the size of the serialized merged PDF" — the same
number that `wc -c` would print on the file written with `-o`.

Computed by wrapping the output sink with a counting `Write` adapter:

- With `-o`: wrap the `File` writer; the count is incremented as bytes
  are written to disk. One pass; no extra allocation.
- Without `-o`: wrap `io::sink()`; bytes are produced and discarded as
  they are counted. No allocation, no I/O.

Both cases call `Document::save_to(&mut writer)` exactly once.

### Action requirement

Today: `-o` is mandatory; absence is `CliError::MissingOutput`.

New: at least one of `-o`, `--count-pages`, `--count-bytes` must be
present. The current `MissingOutput` variant is renamed/replaced by a
broader variant (e.g. `NoAction`) with a message such as:

```
must specify --output and/or --count-pages/--count-bytes
```

`NoInputs` is unchanged.

## Implementation sketch

CLI (`src/cli.rs`):

- `Command::Run` grows two `bool` fields, `count_pages` and
  `count_bytes`, plus the `output` becomes `Option<String>`.
- `Parser` recognizes every alias for each count flag and sets the
  corresponding bool. Repetition is allowed.
- `finish()` enforces "at least one action" and emits the new error
  variant when neither `-o` nor a count flag is given.

Main (`src/main.rs`):

- After merging:
  - If `count_pages`, print `pages: {n}` where `n = merged.get_pages().len()`.
  - Compose the output writer:
    - If `output.is_some()`, open the file.
    - Else, use `io::sink()`.
  - If `count_bytes`, wrap the writer in a counting adapter and call
    `save_to`; after, print `bytes: {n}`.
  - If `!count_bytes`, call `save_to` directly when `output.is_some()`,
    or skip saving entirely when output is `None`.

Counting writer:

- Small inline struct implementing `Write` that forwards to an inner
  writer and tracks total bytes written. Lives in the same module as
  the call site (likely `main.rs` or a new `src/counter.rs` if it
  grows).

## Help and docs

- `src/help.txt` gains entries for the two flags (primary names only;
  aliases noted briefly).
- `README.md`:
  - "Usage" line clarifies that `-o` is optional when a count flag is
    given.
  - New section or table row for `--count-pages` / `--count-bytes`.
  - Example for "inspect without writing" usage.

## Testing

- CLI parsing tests (in `cli.rs`):
  - Each alias parses to the same flag.
  - Both flags can be combined with each other and with `-o`.
  - Neither `-o` nor a count flag → new error variant.
  - `-o` alone still works.
- End-to-end behavior (integration-style, can live in `main.rs` tests
  or a new test file):
  - With `-o` only: file written; nothing on stdout.
  - With `--count-pages` only: no file written; correct page count on
    stdout.
  - With `--count-bytes` only: no file written; stdout byte count
    matches what `Document::save_to(&mut Vec::new())` produces.
  - With `-o` + `--count-bytes`: file is written and its on-disk size
    equals the reported byte count.
  - With both count flags: lines emitted in `pages → bytes` order
    regardless of argv order.

## Non-goals reaffirmed

- Per-input page or byte breakdown (deferred to a future verbose mode).
- Any new short single-letter flag.
- Machine-readable structured output (JSON, etc.).
