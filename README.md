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
