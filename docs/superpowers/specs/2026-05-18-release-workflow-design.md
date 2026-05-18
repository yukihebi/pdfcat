# Release workflow + README restructure — design

Date: 2026-05-18
Branch: `feature/release-workflow`
First release tag (planned): `v1.0.0`

## Motivation

- `README.md` says `cargo build --release` but does not tell readers how to obtain
  a usable `pdfcat`. The Quickstart command examples assume the binary is already
  on PATH.
- There is no GitHub release pipeline. Users without a Rust toolchain have no way
  to get the tool.
- The project is ready for a `1.0.0` cut.

## Scope

1. Restructure `README.md`:
   - Quickstart: keep usage examples, add a link to the Releases page as the
     primary install path for general users.
   - Drop the standalone `Build` section.
   - Add a `From source` section at the end that consolidates everything Rust:
     rustup prerequisite, `cargo install`, `cargo build`/`run`, `cargo
     fmt`/`clippy`/`test`.
2. Add `.github/workflows/release.yml` that, on `v*` tag push:
   - Builds release binaries for four targets.
   - Packages them and publishes a draft GitHub Release with SHA256 sums.
3. Bump `Cargo.toml` `version` from `0.1.0` to `1.0.0`.

Out of scope: crates.io publishing, ARM Linux target, signing.

## README structure (after)

```
# pdfcat
(badge, one-paragraph description)

## Quickstart
- "Download the latest binary from the Releases page and place it on your PATH."
- existing pdfcat command examples (unchanged)

## Usage
## Page spec
## Examples
## Behaviour
## Limitations

## From source
- rustup prerequisite (rustc 1.85+ for edition 2024)
- cargo install --git https://github.com/yukihebi/pdfcat.git
- cargo install --path .
- cargo build --release  /  cargo run --release -- ...
- cargo fmt / cargo clippy / cargo test
```

## Release workflow design

### Trigger

```yaml
on:
  push:
    tags: ['v*']
```

Tag-based only. No `workflow_dispatch`.

### Target matrix

| target triple                  | runner        | archive |
| ------------------------------ | ------------- | ------- |
| `x86_64-unknown-linux-gnu`     | ubuntu-latest | tar.gz  |
| `aarch64-apple-darwin`         | macos-latest  | tar.gz  |
| `x86_64-apple-darwin`          | macos-13      | tar.gz  |
| `x86_64-pc-windows-msvc`       | windows-latest| zip     |

`macos-13` is the last Intel-runner image; `macos-latest` is now ARM.

### Jobs

**Job `build` (matrix)**

1. `actions/checkout@v6`
2. `dtolnay/rust-toolchain@stable` with `targets: <triple>`
3. `Swatinem/rust-cache@v2`
4. Verify tag matches `Cargo.toml`:
   - Extract `${GITHUB_REF_NAME}` (e.g., `v1.0.0`), strip leading `v`.
   - Extract `version = "X.Y.Z"` from `Cargo.toml`.
   - Fail the job if they differ.
   - (Unix step; on Windows the same check runs in PowerShell or bash via
     git-bash. Keep it simple: a single bash step works on `windows-latest`.)
5. `cargo build --release --target <triple>`
6. Stage the binary into a versioned directory:
   - dirname: `pdfcat-${GITHUB_REF_NAME}-<triple>`
   - contents: just the `pdfcat[.exe]` binary. README/LICENSE bundling is
     deferred (see "Open questions" below — no LICENSE file exists yet).
7. Archive:
   - Unix: `tar -C <stage_parent> -czf <dir>.tar.gz <dir>`
   - Windows: `Compress-Archive` into `<dir>.zip`
8. Compute SHA256:
   - Unix: `shasum -a 256 <archive> > <archive>.sha256`
   - Windows: `(Get-FileHash <archive> -Algorithm SHA256).Hash + "  " + <name>`
9. `actions/upload-artifact@v4` for both `<archive>` and `<archive>.sha256`.

**Job `release` (needs: build)**

1. `actions/download-artifact@v4` (no name → downloads all artifacts into
   per-artifact subdirs).
2. Flatten artifacts into a single dir.
3. Concatenate all `*.sha256` files into one `SHA256SUMS` (sorted by filename).
4. `softprops/action-gh-release@v2` with:
   - `files: <flat>/pdfcat-v*-*.{tar.gz,zip}` and `SHA256SUMS`
   - `draft: true`
   - `generate_release_notes: true`
   - `fail_on_unmatched_files: true`
   - `name: ${{ github.ref_name }}`
5. `permissions: { contents: write }` at the job level.

### Release artifacts (per tag `v1.0.0`)

```
pdfcat-v1.0.0-x86_64-unknown-linux-gnu.tar.gz
pdfcat-v1.0.0-aarch64-apple-darwin.tar.gz
pdfcat-v1.0.0-x86_64-apple-darwin.tar.gz
pdfcat-v1.0.0-x86_64-pc-windows-msvc.zip
SHA256SUMS
```

### Release operation flow

1. Bump `Cargo.toml` `version`, `cargo build` to refresh `Cargo.lock`, commit.
2. Open PR → merge to `main`.
3. `git tag vX.Y.Z && git push origin vX.Y.Z` on `main`.
4. Workflow builds 4 archives and opens a **draft** Release.
5. Maintainer reviews the auto-generated notes (and on `v1.0.0`, overwrites with
   a short summary of pdfcat's basic features), then clicks Publish.

The `draft: true` setting is permanent, not first-release-only: every release
gets a review gate before going public.

## Cargo.toml change

```toml
version = "1.0.0"
```

`cargo build` after the bump updates `Cargo.lock` (`pdfcat 0.1.0` → `1.0.0`).

## Testing the workflow before v1.0.0

The workflow only fires on tag push, so dry-running on a branch is awkward.
Options:
- Push a throw-away tag like `v0.9.0-test` on a temporary commit, then delete
  the tag + draft release if anything is wrong. The version-check step would
  fail unless `Cargo.toml` matches — for a test run, accept that and adjust the
  check to be skippable, or temporarily match the version.
- Recommended: skip dry-run; rely on careful review of the workflow file and
  the existing CI's confidence in `cargo build --release`. The draft release
  acts as the safety net — nothing is public until manually published.

## Risks / edge cases

- **`macos-13` lifecycle**: GitHub will eventually deprecate Intel runners.
  When that happens, drop `x86_64-apple-darwin` (or accept a manual build).
- **Tag/version drift**: the version-check pre-step catches the common mistake
  of forgetting to bump `Cargo.toml`.
- **Windows archive format**: PowerShell's `Compress-Archive` is the simplest
  choice; users can extract with built-in tooling.
- **Action versions**: pin `softprops/action-gh-release` to a major (`@v2`),
  let Dependabot bump on minor releases.

## Out of scope (deferred)

- Publishing to crates.io.
- ARM Linux (`aarch64-unknown-linux-gnu`) cross-compilation.
- Code signing / notarization for macOS.
- Homebrew tap / scoop bucket.

## Open questions

- **No LICENSE file.** The repo has no `LICENSE` and `Cargo.toml` has no
  `license` field. Publishing a 1.0.0 binary without a license leaves users in
  an ambiguous legal state ("all rights reserved" by GitHub ToS default). The
  maintainer should decide on a license (MIT / Apache-2.0 / MIT OR Apache-2.0
  / etc.) before tagging `v1.0.0`. Out of scope for this PR's workflow design
  but flagged here so it is not forgotten. Implementation plan will not block
  on this; the LICENSE decision can land in a follow-up.
