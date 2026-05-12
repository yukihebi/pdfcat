# pdfcat

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
pdfcat <INPUT> [-p <PAGES>] [<INPUT> [-p <PAGES>] ...] -o <OUTPUT>
```

Inputs are processed in the order given and concatenated. `-p`/`--pages`
selects pages from the *immediately preceding* input file; without it, the
whole file is used. `-o`/`--output` is required and may appear anywhere.

| Option | Description |
| --- | --- |
| `-h`, `--help` | Print help |
| `-V`, `--version` | Print version |
| `-o`, `--output <FILE>` | Output file (required) |
| `-p`, `--pages <SPEC>` | Page selection for the preceding input (aliases: `--page`, `-pp`) |
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
```

## Behaviour

- Pages keep their original size; mixing different page sizes is fine.
- Inherited page attributes (`Resources`, `MediaBox`, `CropBox`, `Rotate`) are
  flattened onto each page so geometry survives the merge.
- The document `/Info` (metadata) of the first input is carried over.

## Limitations

- Bookmarks (`/Outlines`), forms (`/AcroForm`), the name tree (`/Names`),
  page labels and the document open action are dropped. Link annotations that
  point into those structures may become dangling.
- Only the file given to `-o` is written; there is no multi-file split mode.
