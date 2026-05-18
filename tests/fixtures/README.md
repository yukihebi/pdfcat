# Test fixtures

Treat these PDFs as opaque inputs. Tests must not depend on
producer-specific quirks (e.g. exact `/Producer` strings); rely only on
structural facts (page count, page size, presence/absence of features).

To add a new fixture, drop the PDF here and append one line below noting
where it came from.

## Provenance

- `1page.pdf` — LuaLaTeX (`ltjsarticle`), one page with a large "PAGE1"
  body. PDF 1.7, ~5 KB. Built with `SOURCE_DATE_EPOCH=0` for byte-stable
  output.
- `3pages.pdf` — LuaLaTeX (`ltjsarticle`), three pages labelled
  "PAGE1"/"PAGE2"/"PAGE3". PDF 1.7, ~7 KB. Built with
  `SOURCE_DATE_EPOCH=0`.
- `with-outline.pdf` — LuaLaTeX (`ltjsarticle`) + `hyperref` with two
  `\section`s. Has `/Outlines`. PDF 1.7. Used to verify that
  `DROPPED_CATALOG_KEYS` are stripped during merge.
- `v1.4.pdf` — LuaLaTeX (`ltjsarticle`) with `\pdfvariable
  minorversion=4`. Single page. PDF 1.4. Used to verify that `merge`
  picks the highest PDF version among inputs.
- `inherited-resources.pdf` — **Synthesized** via
  `cargo run --example gen_synthetic_fixtures`. Two-page document whose
  single `/Pages` node carries `/Resources` (a Type1 Helvetica font);
  leaves omit `/Resources` and inherit it. Used to verify
  `INHERITABLE` flatten + `STALE_PAGES_KEYS` strip.
- `0page.pdf` — **Synthesized** via
  `cargo run --example gen_synthetic_fixtures`. Catalog + `/Pages` with
  `/Count 0` and empty `/Kids`. Used to verify the `NoPages` error
  branch in `runner::load_one_source`.
