# README quickstart and CLI error guidance

Date: 2026-05-17
Status: approved (brainstorming)

## Goal

Make pdfcat easier to pick up by surfacing three canonical commands
("quickstart") in:

1. The top of `README.md`, before the build instructions.
2. The `--help` output (`src/help.txt`), as part of the `USAGE` block.
3. The stderr output whenever the CLI fails to parse the command line
   (including the no-argument case).

Secondary goal: shorten the help output by adopting shorter primary
spellings for the count flags and removing the now-redundant `EXAMPLES`
section from the help.

## Quickstart content

A single block of text, reused verbatim in README, `--help`, and CLI
error output:

```
Quickstart:
  # Concatenate x, y, z into out.pdf
  pdfcat x.pdf y.pdf z.pdf -o out.pdf

  # Take pages 1-3, 5, and 7-end of x.pdf into out.pdf
  pdfcat x.pdf -p -3,5,7- -o out.pdf

  # Print the page count of x.pdf
  pdfcat x.pdf --npages
```

The block ends with a trailing newline. Indentation is two spaces so the
block reads naturally both standalone (CLI error output) and embedded
under the existing `USAGE:` header in the help text.

## Source of truth

- New file `src/quickstart.txt` containing the block above.
- `src/help.txt` is split into two files:
  - `src/help_head.txt` -- program tagline and `USAGE:` block.
  - `src/help_tail.txt` -- `OPTIONS:` table and `PAGE SPEC:` section.
  - The previous `EXAMPLES:` section is removed.
- `src/cli.rs`:
  - `pub const QUICKSTART: &str = include_str!("quickstart.txt");`
  - `pub const HELP: &str = concat!(`
    `    include_str!("help_head.txt"),`
    `    include_str!("quickstart.txt"),`
    `    include_str!("help_tail.txt"),`
    `);`

The single `quickstart.txt` file is the single source of truth; the
`HELP` constant embeds it via `concat!`, and the error path prints it
directly.

### Whitespace around the embedded block

- `quickstart.txt` starts with `Quickstart:` and ends with a single
  trailing newline (no surrounding blank lines).
- `help_head.txt` ends with a blank line (i.e. two trailing newlines)
  so the embedded block is visually separated from the `USAGE:` block.
- `help_tail.txt` starts with a blank line (a leading newline before
  `OPTIONS:`) so the `OPTIONS:` block sits one blank line below the
  embedded block.
- The error path prints `QUICKSTART` between two `eprintln!()` blank
  lines, producing the same visual spacing in stderr output.

## Primary spelling for the count flags

The current primary spelling `--count-pages` / `--count-bytes` is
replaced by `--npages` / `--nbytes` for display purposes. The old
spellings stay as aliases (no behavioural change). All sites that
display these flags switch:

- `src/help.txt` (now `help_tail.txt`) `OPTIONS:` table.
- `README.md` options table.
- `README.md` `## Behaviour` section.
- `README.md` `## Examples` examples that use these flags.

The Rust parser still accepts every existing alias. Internal field names
(`count_pages: bool`, `count_bytes: bool` on `Command::Run`) are left
unchanged because they describe the semantic action.

## `pdfcat --help` output (post-change)

```
pdfcat - concatenate PDFs and extract pages

USAGE:
    pdfcat <INPUT> [-p <PAGES>] [<INPUT> [-p <PAGES>] ...] [-o <OUTPUT>] [--npages] [--nbytes] [-q] [-v]

    At least one of -o, --npages, --nbytes must be given.

Quickstart:
  # Concatenate x, y, z into out.pdf
  pdfcat x.pdf y.pdf z.pdf -o out.pdf

  # Take pages 1-3, 5, and 7-end of x.pdf into out.pdf
  pdfcat x.pdf -p -3,5,7- -o out.pdf

  # Print the page count of x.pdf
  pdfcat x.pdf --npages

OPTIONS:
    -h, --help              Print this help
    -V, --version           Print version
    -o, --output <FILE>     Output file
    -p, --pages <SPEC>      Page selection for the preceding input file
                            (aliases: --page, -pp)
    --npages                Print the merged page count to stdout
                            (aliases: --count-pages, --count-page,
                            --page-count, --num-pages, --npage, ...)
    --nbytes                Print the merged byte count to stdout
                            (aliases: --count-bytes, --count-byte,
                            --byte-count, --num-bytes, --nbyte, ...)
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

(No `EXAMPLES:` section.)

## CLI error output

`src/main.rs`:

- Remove the early `if args.is_empty()` branch. The empty-argv case will
  fall through to `CliError::NoInputs`.
- On `Err(Error::Cli(_))`, write to stderr:
  - `pdfcat: <error>` (existing behaviour)
  - blank line
  - `QUICKSTART` (which already starts with `Quickstart:` and ends with
    a newline)
  - blank line
  - `Run 'pdfcat --help' for details.`
- On other (runtime) errors, keep the current behaviour: just
  `pdfcat: <error>`.

Example for an unknown option:

```
pdfcat: unknown option: --foo

Quickstart:
  # Concatenate x, y, z into out.pdf
  pdfcat x.pdf y.pdf z.pdf -o out.pdf

  # Take pages 1-3, 5, and 7-end of x.pdf into out.pdf
  pdfcat x.pdf -p -3,5,7- -o out.pdf

  # Print the page count of x.pdf
  pdfcat x.pdf --npages

Run 'pdfcat --help' for details.
```

Exit code stays `ExitCode::FAILURE`.

## README restructure

New section order:

1. Title and CI badge (unchanged).
2. One-paragraph description (unchanged).
3. **`## Quickstart`** -- new. Contains the same three commands with the
   same one-line comments as the QUICKSTART block, formatted as a
   fenced shell block.
4. `## Build` (unchanged).
5. `## Usage` -- command synopsis updated to use `--npages` / `--nbytes`
   in the form line and the "at least one of" sentence. Options table
   rewritten so `--npages` and `--nbytes` are the primary spellings and
   the legacy spellings appear in the aliases column.
6. `### Page spec` (unchanged).
7. **`## Examples`** -- rewritten as a spec-oriented list. Each entry
   has a short title describing the feature being demonstrated, then
   the command. Examples that overlap with the Quickstart block are
   removed.
8. `## Behaviour` -- flag name occurrences updated to `--npages` /
   `--nbytes`.
9. `## Limitations` (unchanged).

### Examples section (post-restructure)

- **Combine ranges from multiple inputs** -- shows that `-p` binds to
  the preceding input only.
  `pdfcat x.pdf -p 1-3 y.pdf -p 5- -o out.pdf`
- **Count without writing** -- shows that `--npages` and `--nbytes` can
  run without `-o` and that both can be combined; expected stdout is
  one `pages: N` line followed by one `bytes: M` line.
  `pdfcat x.pdf y.pdf --npages --nbytes`
- **Numeric-only output for scripts** -- shows `-q` dropping labels.
  ```
  N=$(pdfcat x.pdf --npages -q)
  ```
- **Write and report at the same time** -- shows `-o` combined with a
  counter.
  `pdfcat x.pdf -p 1-3 -o out.pdf --nbytes`
- **File name starting with `-`** -- shows the `--` separator.
  `pdfcat -o out.pdf -- -scan.pdf`
- **Verbose progress** -- shows that `-v` emits a progress log to
  stderr.
  `pdfcat x.pdf y.pdf -o out.pdf -v`

## Tests

- `src/cli_tests.rs` gains a small block of tests:
  - `QUICKSTART` contains the three exact command lines.
  - `HELP` contains the full `QUICKSTART` substring (guards against the
    `concat!` order being changed accidentally).
  - `HELP` no longer contains `"EXAMPLES:"`.
- No new tests for `main.rs` error output. The runtime print behaviour
  is straightforward and a future integration test (`assert_cmd` /
  `Command::cargo_bin`) could be added separately if desired.

## Files touched

- New: `src/quickstart.txt`, `src/help_head.txt`, `src/help_tail.txt`.
- Edited: `src/cli.rs`, `src/main.rs`, `src/cli_tests.rs`, `README.md`.
- Deleted: `src/help.txt`.

## Non-goals

- Renaming the internal `count_pages` / `count_bytes` fields.
- Dropping any existing flag alias.
- Adding a single-dash spelling like `-npages` (the parser still
  treats anything starting with a single `-` as a short-flag cluster
  and rejects unknown ones).
- Reformatting help text beyond what the quickstart change requires.
