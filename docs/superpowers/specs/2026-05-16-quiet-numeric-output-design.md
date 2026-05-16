# Quiet (numeric-only) output for `--count-pages` / `--count-bytes`

Status: approved
Date: 2026-05-16

## Motivation

`--count-pages` and `--count-bytes` (added in PR #3) print labeled lines
(`pages: N` / `bytes: N`). Scripts that want to consume the number
must strip the label first:

```sh
N=$(pdfcat x.pdf --count-pages | cut -d' ' -f2)
```

A `--quiet` modifier that suppresses the label lets the same data flow
directly into shell variables, pipes, and downstream tools, and shortens
human-facing output when the label is redundant.

## Interface

New flag: `-q`, `--quiet`.

- A modifier for `--count-pages` / `--count-bytes`; it changes the form
  of those lines from `pages: N` / `bytes: N` to bare `N`.
- Order is preserved: when both counts are requested, pages comes first,
  then bytes, one per line.
- Position-independent — may appear anywhere in the argument list, like
  the existing `--count-*` flags.
- Takes no value. `--quiet=foo` is rejected with an error. Since the
  `--count-*` family is also value-less, this change is applied to
  them at the same time: `--count-pages=foo` and friends, which were
  previously silently accepted, now also error. `--help`/`--version`
  remain as they are (silent), matching common convention for those
  flags.
- No aliases. `-q` and `--quiet` are short and idiomatic enough.
- Used alone (no `--count-*`), `-q` is a no-op. This is intentional so
  that scripts can pass `-q` unconditionally. `NoAction` is unchanged:
  at least one of `-o`, `--count-pages`, `--count-bytes` must still be
  given.

### Output matrix

| Flags                                  | Output            |
| -------------------------------------- | ----------------- |
| `--count-pages`                        | `pages: 42`       |
| `--count-bytes`                        | `bytes: 12345`    |
| `--count-pages --count-bytes`          | two lines: `pages: 42`, `bytes: 12345` |
| `--count-pages -q`                     | `42`              |
| `--count-bytes -q`                     | `12345`           |
| `--count-pages --count-bytes -q`       | two lines: `42`, `12345` |
| `-q` only (with `-o`)                  | nothing on stdout |

## Implementation

Smallest change that fits the existing structure: thread one extra
`bool` through the command.

### `src/cli.rs`

- Add `quiet: bool` to `Parser` (default `false`) and to
  `Command::Run`.
- Add a match arm for `-q` / `--quiet` next to the existing
  `--count-*` arms; set `self.quiet = true`.
- Add a new error variant `CliError::UnexpectedValue(&'static str)`
  ("`{name}` does not take a value"). In the match arms for the
  value-less flags (`--count-pages`, `--count-bytes`, `--quiet`, and
  their aliases), reject an inline `=value` by returning this error,
  using a stable label per family (e.g. `"--count-pages"`,
  `"--count-bytes"`, `"--quiet"`). `-h`/`-V` arms are untouched.
- `finish()` is unchanged: `NoAction` still triggers when none of
  `-o`, `--count-pages`, `--count-bytes` is given. `quiet` does not
  count as an action.

### `src/main.rs`

- Add `quiet: bool` to `execute_run`'s signature, plumbed from
  `Command::Run`.
- Replace the two labeled `writeln!` calls with a conditional:
  - `if quiet { writeln!(report, "{n}") } else { writeln!(report, "pages: {n}") }`
  - similarly for `bytes`.
- No other logic changes; the byte-counting path (`CountingWriter`,
  `io::sink`) is identical.

### `src/help.txt`

Add one line:

```
    -q, --quiet             Omit the `pages: ` / `bytes: ` labels and
                            print just the numbers (one per line)
```

### `README.md`

- Add `-q, --quiet` to the options table.
- Add one bullet to the *Behaviour* section noting that `-q` strips
  the labels.
- Add one example, e.g. `N=$(pdfcat x.pdf --count-pages -q)`.

## Tests

### `src/cli_tests.rs`

- `-q` and `--quiet` both set `quiet = true`.
- Position-independent: parse succeeds with `-q` before and after the
  input file.
- `-q` combined with `--count-pages` produces `Command::Run { quiet: true, .. }`.
- `-q` with only `-o` parses to `Command::Run { quiet: true, output: Some(..), count_pages: false, count_bytes: false }`.
- `-q` with nothing else returns `CliError::NoAction`.
- `--quiet=anything` returns `CliError::UnexpectedValue("--quiet")`.
- `--count-pages=anything` (and any alias) returns
  `CliError::UnexpectedValue("--count-pages")`; same for
  `--count-bytes`.
- `--help=foo` / `--version=foo` still parse to `Command::Help` /
  `Command::Version` (unchanged behavior).

### `src/main_tests.rs`

Drive `execute_run` with a `Vec<u8>` report writer:

- `count_pages=true, count_bytes=false, quiet=false` → `pages: N\n`.
- `count_pages=true, count_bytes=false, quiet=true`  → `N\n`.
- `count_pages=true, count_bytes=true,  quiet=true`  → `N\nM\n` in
  page-then-bytes order.
- `count_pages=false, count_bytes=false, quiet=true` (with `-o`) →
  empty report.

`merge_tests.rs` and `pages_tests.rs` are not affected.

## Out of scope

- A separator other than newline between `pages` and `bytes` in
  quiet mode (e.g. space, tab, JSON). Newline is enough for scripting
  and matches the existing two-line shape.
- Suppressing or reformatting error messages. `--quiet` only affects
  the count lines on stdout.
- Aliases for `--quiet`.
