# Real-PDF Test Migration — Design

## Goal

Move tests in `src/merge_tests.rs` and `src/runner_tests.rs` off of
in-memory synthesized `Document` factories (`tiny_doc`, `doc_with_widths`,
`doc_with_inherited_box`) and onto fixtures loaded via
`Document::load(...)`. This exercises lopdf's parser and gives each test
a fixture that directly expresses the structural property it checks.

The audit driving this work is by branch: every branch in `merge.rs` and
`runner.rs` that the implementation deliberately considers must have a
test. Existing tests are not deleted unless their helpers go unused after
migration; the migration replaces *how* a test obtains its `Document`,
not *what* the test asserts.

## Out of scope

- `pages_tests.rs`, `cli_tests.rs` — pure-data tests, no PDF I/O.
- Test consolidation / table-driven simplification — separate PR.
- Black-box integration tests via `main::run` — separate PR if ever.

## Test inventory

### `merge_tests.rs` (10 → 11)

| # | Test | Status | Fixture |
|---|------|--------|---------|
| 1 | `concatenates_pages_in_order` | migrate | `1page.pdf` + `3pages.pdf` |
| 2 | `selects_and_reorders_pages` | migrate | `3pages.pdf`, select `[3,1,2]` |
| 3 | `duplicate_page_gets_a_fresh_id` | migrate | `3pages.pdf`, select `[1,1,2]` |
| 4 | `duplicate_then_more_inputs_keeps_ids_disjoint` | migrate | `1page.pdf` + `3pages.pdf` (with selections) |
| 5 | `keeps_supporting_objects_but_drops_outlines` | migrate | `with-outline.pdf` |
| 6 | `inherited_media_box_is_flattened_onto_pages` → rename to `inheritable_attrs_flattened_and_stale_keys_stripped` | migrate | `inherited-resources.pdf` |
| 7 | `merge_with_no_sources_panics` | unchanged | none |
| 8 | `info_dictionary_is_carried_over` | migrate | `1page.pdf` (already has `/Info`) |
| 9 | `no_catalog_error_carries_first_input_path` | unchanged | none (constructed `Document`) |
| 10 | `empty_selection_yields_zero_page_document` | migrate | any fixture, select `[]` |
| 11 | `picks_highest_pdf_version` **(NEW)** | add | `3pages.pdf` (1.5) + `v1.7.pdf` |

### `runner_tests.rs` (30 → 32)

Existing 30 tests all retained. Migration touches the 12 `tiny_doc(n)`
call sites and the 4 `write_tiny_pdf(n)` call sites; the two helpers are
then deleted.

| Group | Count | Status | Fixture / notes |
|-------|-------|--------|-----------------|
| `execute_run_*` | 12 | migrate | `1page.pdf` or `3pages.pdf`; adjust hard-coded page-count assertions to fixture-actual values |
| `load_sources_*` | 4 | migrate | as above |
| `load_one_source_missing_file_*` | 1 | unchanged | error path, no PDF |
| `load_one_source_corrupt_file_*` | 1 | unchanged | uses tempfile + raw bytes |
| `atomic_write_*` | 2 | unchanged | not PDF-shaped |
| `counting_writer_*` | 2 | unchanged | unit test of helper |
| `fmt_header_index_*` | 3 | unchanged | unit test of helper |
| `verbose_log_*` | 5 | unchanged | unit test of `VerboseLog` methods |
| `load_one_source_no_pages_returns_no_pages` **(NEW)** | 1 | add | `0page.pdf` |
| `load_one_source_page_selection_out_of_range` **(NEW)** | 1 | add | `3pages.pdf`, select `[10]` |

### `pages_tests.rs`, `cli_tests.rs`

Unchanged. Out of scope for this PR.

## Helper changes

### Removed (dead after migration)

- `tiny_doc` (runner_tests.rs)
- `doc_with_widths` (merge_tests.rs)
- `doc_with_inherited_box` (merge_tests.rs)
- `page_widths` (merge_tests.rs)
- `write_tiny_pdf` (runner_tests.rs) — callers can use fixture paths directly

### Added

- `page_content_bytes(doc, page_id) -> Vec<u8>` in `merge_tests.rs`:
  resolve a page's `/Contents` reference, clone the stream, decompress,
  return its bytes. Used to verify page ordering by content identity
  instead of MediaBox width.

### Retained

- `page_ids(doc) -> Vec<ObjectId>` — still needed for the
  "duplicate page gets a fresh id" tests.
- `siblings_of(target) -> Vec<OsString>` — for `atomic_write_*` tests.

## Page identity strategy

LaTeX-produced fixtures have a uniform `/MediaBox`, so width can no
longer mark page identity. Instead, each test that asserts page ordering
extracts the `/Contents` stream of each merged page and compares to the
corresponding source page's `/Contents` stream. `merge` carries content
streams byte-for-byte (only the page dict's `/Parent` and inherited
attributes are rewritten), so byte equality after decompression is a
stable identity for "this merged page came from that source page".

The `page_content_bytes` helper centralises this. Tests assert:

```rust
let src1 = Document::load("tests/fixtures/3pages.pdf").unwrap();
let want: Vec<_> = [3, 1, 2].iter().map(|&n| {
    let id = src1.get_pages()[&n];
    page_content_bytes(&src1, id)
}).collect();

let merged = merge(...).unwrap();
let got: Vec<_> = merged.get_pages().values()
    .map(|&id| page_content_bytes(&merged, id))
    .collect();
assert_eq!(got, want);
```

## Fixture corpus

| Fixture | Provenance | Used by |
|---------|------------|---------|
| `1page.pdf` | LuaLaTeX (`ltjsarticle`), 1 page (existing) | runner load tests; merge concat / info |
| `3pages.pdf` | LuaLaTeX (`ltjsarticle`), 3 pages (existing) | runner load tests; merge selection / duplicate / version |
| `with-outline.pdf` | LuaLaTeX + `hyperref` + `\section`, 2 pages **(new)** | `keeps_supporting_objects_but_drops_outlines` |
| `inherited-resources.pdf` | Synthesized via lopdf **(new)** | `inheritable_attrs_flattened_and_stale_keys_stripped` |
| `0page.pdf` | Synthesized via lopdf **(new)** | `load_one_source_no_pages_returns_no_pages` |
| `v1.7.pdf` | LuaLaTeX with `\pdfvariable minorversion=7` **(new)** | `picks_highest_pdf_version` |

All LaTeX-produced fixtures are built with `SOURCE_DATE_EPOCH=0` for
byte-stable output.

Tests reference fixtures by relative path from the crate root
(`tests/fixtures/...`); Cargo runs tests with CWD set to the crate root,
so the relative path resolves without additional setup.

The `picks_highest_pdf_version` test merges `3pages.pdf` (PDF 1.5) with
`v1.7.pdf` and asserts the merged document's `version` is `"1.7"`.

## Synthetic fixture generation

A single example program produces both synthetic PDFs:

`examples/gen_synthetic_fixtures.rs`

Run on demand by a contributor regenerating fixtures:

```
cargo run --example gen_synthetic_fixtures
```

Each PDF is written under `tests/fixtures/`. The example is documented
in `tests/fixtures/README.md` alongside per-fixture provenance entries.

`inherited-resources.pdf` structure: a 2-page document where the single
`/Pages` node carries `/Resources` (with at least one entry, e.g. a
trivial `/Font` dict so identity is observable), and each leaf `/Page`
object omits `/Resources`. The merged document must (a) have each leaf
page carry the inherited `/Resources` after flattening, and (b) have
the rebuilt `/Pages` node free of `/Resources` (verifying both the
INHERITABLE flatten loop and the STALE_PAGES_KEYS strip loop with one
attribute).

`0page.pdf` structure: a minimal catalog plus a `/Pages` node with
`/Count 0` and empty `/Kids`. Saved to disk and loaded back via
`Document::load`, then passed through `load_one_source`, which should
return `Error::NoPages`.

## LaTeX-produced fixture sources

The `.tex` sources for `with-outline.pdf` and `v1.7.pdf` are *not*
committed (per the established policy of treating fixtures as opaque
inputs). Provenance is recorded in `tests/fixtures/README.md` with
enough detail to recreate equivalent fixtures if ever needed.

## Validation

- `cargo test`: all 81 existing tests retained pass + 3 new tests pass = 84 total.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.
- Subagent self-review before commit, per `CLAUDE.md`.
- No fixture leaks personal information (re-run the grep audit used for
  the original two PDFs against the new ones).

## Branching

Work proceeds on `refactor/real-pdf-tests` (already branched from
`main`, where the tempfile PR has been merged). Single PR against
`main` at the end.
