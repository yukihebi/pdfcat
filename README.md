# pdfcat

[![CI](https://github.com/yukihebi/pdfcat/actions/workflows/ci.yml/badge.svg)](https://github.com/yukihebi/pdfcat/actions/workflows/ci.yml)

A small command-line tool to concatenate PDF files and extract pages from
them. Written in pure Rust (using [`lopdf`](https://crates.io/crates/lopdf));
no native dependencies.

## Build

```sh
cargo build --release
# binary at target/release/pdfcat
```

## Usage

```
pdfcat <INPUT> [-p <PAGES>] [<INPUT> [-p <PAGES>] ...] [-o <OUTPUT>] [--count-pages] [--count-bytes] [-q]
```

Inputs are processed in the order given and concatenated. `-p`/`--pages`
selects pages from the *immediately preceding* input file; without it, the
whole file is used. `-o`/`--output` may appear anywhere; at least one of
`-o`, `--count-pages`, `--count-bytes` must be given.

| Option | Description |
| --- | --- |
| `-h`, `--help` | Print help |
| `-V`, `--version` | Print version |
| `-o`, `--output <FILE>` | Output file |
| `-p`, `--pages <SPEC>` | Page selection for the preceding input (aliases: `--page`, `-pp`) |
| `--count-pages` | Print the merged page count to stdout. Aliases: `--count-page`, `--page-count`, `--page-counts`, `--num-pages`, `--num-page`, `--npages`, `--npage` |
| `--count-bytes` | Print the merged byte count to stdout. Aliases: `--count-byte`, `--byte-count`, `--byte-counts`, `--num-bytes`, `--num-byte`, `--nbytes`, `--nbyte` |
| `-q`, `--quiet` | Omit the `pages: ` / `bytes: ` labels; print just the numbers (one per line). |
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

```sh
# Concatenate three PDFs
pdfcat x.pdf y.pdf z.pdf -o out.pdf

# Extract page 1 of x.pdf
pdfcat x.pdf -p 1 -o out.pdf

# x.pdf without its 3rd page, then all of y.pdf
pdfcat x.pdf -p -2,4- y.pdf -o out.pdf

# Pages 1-3 of x.pdf followed by pages 5 onward of y.pdf
pdfcat x.pdf -p 1-3 y.pdf -p 5- -o out.pdf

# An input whose name starts with '-'
pdfcat -o out.pdf -- -scan.pdf

# Inspect the result without writing a file
pdfcat x.pdf y.pdf --count-pages --count-bytes

# Write and report at the same time
pdfcat x.pdf -p 1-3 -o out.pdf --count-bytes

# Numbers only, ready to pipe or assign
N=$(pdfcat x.pdf --count-pages -q)
```

## Behaviour

- `--count-pages` and `--count-bytes` print one labeled line each to
  stdout (in `pages → bytes` order), and may be combined with `-o`
  or used on their own. Add `-q` / `--quiet` to drop the labels and
  emit just the numbers (one per line) for easier scripting.
- Pages keep their original size; mixing different page sizes is fine.
- Inherited page attributes (`Resources`, `MediaBox`, `CropBox`, `Rotate`) are
  flattened onto each page so geometry survives the merge.
- The document `/Info` (metadata) of the first input is carried over.

## Limitations

- Bookmarks (`/Outlines`), forms (`/AcroForm`), the name tree (`/Names`),
  page labels and the document open action are dropped. Link annotations that
  point into those structures may become dangling.
- Only the file given to `-o` is written; there is no multi-file split mode.
