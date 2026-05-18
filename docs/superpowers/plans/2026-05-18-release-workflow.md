# Release workflow + README restructure — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restructure `README.md` to surface a Releases-based Quickstart and consolidate Rust-toolchain instructions into a single `From source` section, and add a tag-triggered GitHub Actions workflow that builds 4-target binaries and publishes a draft Release with SHA256SUMS.

**Architecture:**
- `README.md`: Quickstart points at GitHub Releases; standalone `Build` section is removed; a new `From source` section at the end gathers rustup prerequisite, `cargo install`, `cargo build/run`, and dev checks.
- `.github/workflows/release.yml`: tag push (`v*`) → `verify` job (tag vs `Cargo.toml`) → `build` matrix (4 targets) → `release` job that flattens artifacts, generates `SHA256SUMS`, and publishes a **draft** Release with auto-generated notes.

**Tech Stack:** GitHub Actions, `actions/checkout@v6`, `dtolnay/rust-toolchain@stable`, `Swatinem/rust-cache@v2`, `actions/upload-artifact@v4`, `actions/download-artifact@v4`, `softprops/action-gh-release@v2`. Bash on Linux/macOS, PowerShell on Windows.

---

## File structure

- Modify: `README.md` (Quickstart prelude; remove Build; add From source)
- Create: `.github/workflows/release.yml`
- Delete (final task): `docs/superpowers/specs/2026-05-18-release-workflow-design.md` and `docs/superpowers/plans/2026-05-18-release-workflow.md` (CLAUDE.md squash-merge policy)

No source code changes. No new tests. Validation is performed by review and (on first tag push) by observing the workflow's draft Release.

---

## Task 1: README — Quickstart prelude (Releases link)

**Files:**
- Modify: `README.md:9-23` (current Quickstart section)

- [ ] **Step 1: Insert Releases-link paragraph before the Quickstart code block**

The current section reads:

```markdown
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
```

Edit `README.md` to insert a new paragraph immediately after `## Quickstart` and before the ```sh fence:

```markdown
## Quickstart

Download the latest binary from the
[Releases page](https://github.com/yukihebi/pdfcat/releases) and place it
on your `PATH`. (To install from source instead, see [From source](#from-source).)

```sh
# Concatenate x, y, z into out.pdf
...
```

The shell block and the `Run pdfcat --help...` paragraph are unchanged.

- [ ] **Step 2: Verify the diff is exactly the inserted paragraph**

Run: `git diff README.md`
Expected: a single hunk that adds the two-line paragraph + blank line; no other changes.

---

## Task 2: README — Remove standalone `Build` section

**Files:**
- Modify: `README.md:25-30` (current Build section)

- [ ] **Step 1: Remove the entire `## Build` section**

Delete this block (including the blank line that follows it):

```markdown
## Build

```sh
cargo build --release
# binary at target/release/pdfcat
```

```

After removal, `## Quickstart` is followed directly by `## Usage`.

- [ ] **Step 2: Verify removal**

Run: `git diff README.md`
Expected: hunks for both Task 1 (insert Releases paragraph) and Task 2 (Build section deletion). `grep -n '^## Build$' README.md` returns nothing.

---

## Task 3: README — Add `From source` section at the end

**Files:**
- Modify: `README.md` (append after `## Limitations`)

- [ ] **Step 1: Append the `From source` section**

Add the following at the end of `README.md`, after the last bullet of `## Limitations`:

```markdown

## From source

Requires a Rust toolchain (rustc 1.85+ for edition 2024); install via
[rustup](https://rustup.rs/) if needed.

```sh
# Install (binary goes to ~/.cargo/bin/pdfcat)
cargo install --git https://github.com/yukihebi/pdfcat.git
# or from a local checkout
cargo install --path .

# Build / run without installing
cargo build --release            # binary at target/release/pdfcat
cargo run --release -- x.pdf -o out.pdf

# Dev checks
cargo fmt
cargo clippy
cargo test
```
```

- [ ] **Step 2: Verify the section anchor matches the Quickstart link**

Run: `grep -n '^## From source$' README.md`
Expected: exactly one match. The Quickstart link `[From source](#from-source)` (added in Task 1) must resolve to this heading.

- [ ] **Step 3: Commit all README changes**

```bash
git add README.md
git commit -m "Restructure README: Releases-based Quickstart and consolidated From source section"
```

Expected: one new commit; `git status` is clean.

---

## Task 4: Create the release workflow

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Write `.github/workflows/release.yml`**

Create the file with this exact content:

```yaml
name: Release

on:
  push:
    tags: ['v*']

permissions:
  contents: read

jobs:
  verify:
    name: verify tag matches Cargo.toml
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
      - name: Check version
        shell: bash
        run: |
          tag_version="${GITHUB_REF_NAME#v}"
          cargo_version=$(grep -E '^version *= *' Cargo.toml | head -1 | sed -E 's/.*"([^"]+)".*/\1/')
          echo "tag=$tag_version cargo=$cargo_version"
          if [ "$tag_version" != "$cargo_version" ]; then
            echo "::error::Tag ${GITHUB_REF_NAME} (version $tag_version) does not match Cargo.toml version $cargo_version"
            exit 1
          fi

  build:
    name: build ${{ matrix.target }}
    needs: verify
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        include:
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-latest
            archive: tar
          - target: aarch64-apple-darwin
            os: macos-latest
            archive: tar
          - target: x86_64-apple-darwin
            os: macos-13
            archive: tar
          - target: x86_64-pc-windows-msvc
            os: windows-latest
            archive: zip
    steps:
      - uses: actions/checkout@v6
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - uses: Swatinem/rust-cache@v2
        with:
          key: ${{ matrix.target }}

      - name: Build
        run: cargo build --release --target ${{ matrix.target }}

      - name: Stage artifacts (Unix)
        if: matrix.archive == 'tar'
        shell: bash
        run: |
          stage="pdfcat-${GITHUB_REF_NAME}-${{ matrix.target }}"
          mkdir -p "$stage"
          cp "target/${{ matrix.target }}/release/pdfcat" "$stage/"
          cp LICENSE README.md "$stage/"
          tar -czf "${stage}.tar.gz" "$stage"
          shasum -a 256 "${stage}.tar.gz" > "${stage}.tar.gz.sha256"

      - name: Stage artifacts (Windows)
        if: matrix.archive == 'zip'
        shell: pwsh
        run: |
          $stage = "pdfcat-$env:GITHUB_REF_NAME-${{ matrix.target }}"
          New-Item -ItemType Directory -Path $stage | Out-Null
          Copy-Item "target/${{ matrix.target }}/release/pdfcat.exe" $stage
          Copy-Item LICENSE,README.md $stage
          Compress-Archive -Path $stage -DestinationPath "$stage.zip"
          $hash = (Get-FileHash "$stage.zip" -Algorithm SHA256).Hash.ToLower()
          "$hash  $stage.zip" | Out-File -FilePath "$stage.zip.sha256" -Encoding ascii -NoNewline

      - uses: actions/upload-artifact@v4
        with:
          name: pdfcat-${{ matrix.target }}
          path: |
            pdfcat-${{ github.ref_name }}-${{ matrix.target }}.tar.gz
            pdfcat-${{ github.ref_name }}-${{ matrix.target }}.tar.gz.sha256
            pdfcat-${{ github.ref_name }}-${{ matrix.target }}.zip
            pdfcat-${{ github.ref_name }}-${{ matrix.target }}.zip.sha256
          if-no-files-found: ignore

  release:
    name: release
    needs: build
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - uses: actions/download-artifact@v4
        with:
          path: dist

      - name: Flatten artifacts and build SHA256SUMS
        shell: bash
        run: |
          mkdir -p flat
          find dist -type f \( -name '*.tar.gz' -o -name '*.zip' \) -exec cp {} flat/ \;
          # Each per-file .sha256 already has "HASH  FILENAME" format; sort by filename for determinism.
          find dist -type f -name '*.sha256' -exec cat {} \; | LC_ALL=C sort -k2 > flat/SHA256SUMS
          ls -la flat/

      - name: Publish draft release
        uses: softprops/action-gh-release@v2
        with:
          name: ${{ github.ref_name }}
          files: |
            flat/pdfcat-*.tar.gz
            flat/pdfcat-*.zip
            flat/SHA256SUMS
          draft: true
          generate_release_notes: true
          fail_on_unmatched_files: true
```

Notes embedded in the design that the implementer should preserve:
- `fail-fast: false` on the build matrix so one target failing does not cancel the others (debugging the workflow is easier).
- `permissions: contents: write` is scoped to the `release` job only; the rest of the workflow runs with default `contents: read`.
- The `verify` job uses `grep`+`sed` rather than `cargo pkgid` because the latter requires Cargo.lock to be present and may be brittle in fresh checkouts.
- The Windows `Out-File -NoNewline` avoids appending a CRLF that would confuse shasum-style verification.

- [ ] **Step 2: Verify file is parseable YAML and validate via actionlint if available**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))"`
Expected: no output (success).

Then: `command -v actionlint >/dev/null && actionlint .github/workflows/release.yml || echo "(actionlint not installed — skipping)"`
Expected: either no output (clean) or "(actionlint not installed — skipping)".

If actionlint reports errors, fix them inline before continuing.

- [ ] **Step 3: Sanity-grep the workflow for the expected pieces**

Run each and confirm it returns at least one match:

```bash
grep -E "tags: \['v\*'\]" .github/workflows/release.yml
grep -E "matrix:" .github/workflows/release.yml
grep -E "x86_64-unknown-linux-gnu" .github/workflows/release.yml
grep -E "aarch64-apple-darwin" .github/workflows/release.yml
grep -E "x86_64-apple-darwin" .github/workflows/release.yml
grep -E "x86_64-pc-windows-msvc" .github/workflows/release.yml
grep -E "draft: true" .github/workflows/release.yml
grep -E "generate_release_notes: true" .github/workflows/release.yml
grep -E "softprops/action-gh-release" .github/workflows/release.yml
grep -E "SHA256SUMS" .github/workflows/release.yml
```

Expected: every grep returns a match (no missing pieces).

- [ ] **Step 4: Commit the workflow**

```bash
git add .github/workflows/release.yml
git commit -m "Add release workflow: build 4 targets and publish draft release on v* tag"
```

---

## Task 5: Pre-commit verification (full CLAUDE.md checks)

**Files:** none modified

- [ ] **Step 1: Run the full CLAUDE.md pre-commit gauntlet**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Expected:
- fmt: no output
- clippy: no warnings, "Finished" line
- test: all tests pass (84 currently)

If any step fails, fix in a new commit (do not amend).

- [ ] **Step 2: Sub-agent self-review of the substantive changes**

Dispatch a code-review subagent with this prompt:

> Review the staged-and-committed changes on branch `feature/release-workflow` against `main`. Two artifacts: `README.md` restructure (Releases-based Quickstart + new From source section, Build section removed) and a new `.github/workflows/release.yml` (tag-push -> 4-target matrix build -> draft Release with SHA256SUMS). Look for: (a) workflow YAML correctness — paths, matrix conditions, action versions, permissions scoping, Windows/Unix shell consistency; (b) README anchor link `#from-source` resolves; (c) any unintended file changes. Report concrete issues only.

Apply any concrete fixes the subagent identifies (as new commits, not amends).

---

## Task 6: Final cleanup — remove dev docs before opening PR

**Files:**
- Delete: `docs/superpowers/specs/2026-05-18-release-workflow-design.md`
- Delete: `docs/superpowers/plans/2026-05-18-release-workflow.md`

CLAUDE.md: "PRはsquash mergeされるので中間コミットに開発ドキュメントを含めることは構わないが，PRを出す時はREADMEやコメント等にまとめ，ポインタ含め削除しておくこと．"

- [ ] **Step 1: Remove both dev-doc files**

```bash
git rm docs/superpowers/specs/2026-05-18-release-workflow-design.md
git rm docs/superpowers/plans/2026-05-18-release-workflow.md
# If docs/superpowers/{specs,plans} are now empty, remove them too:
rmdir docs/superpowers/plans docs/superpowers/specs docs/superpowers docs 2>/dev/null || true
git status
```

Expected: both files staged for deletion; no other unintended changes.

- [ ] **Step 2: Verify no surviving pointers to the dev docs**

Run: `grep -rn 'release-workflow-design\|2026-05-18-release-workflow' --include='*.md' .`
Expected: no matches.

- [ ] **Step 3: Commit the cleanup**

```bash
git commit -m "Remove dev docs before PR (CLAUDE.md squash-merge policy)"
```

- [ ] **Step 4: Final state check**

```bash
git log --oneline main..HEAD
git diff --stat main..HEAD
```

Expected log (commits added on top of `932d0f1` LICENSE/metadata + `3b6351a` design doc update already on the branch):
- `Restructure README: ...` (Task 3)
- `Add release workflow: ...` (Task 4)
- (any review-fix commits from Task 5)
- `Remove dev docs before PR ...` (Task 6)

Expected `git diff --stat main..HEAD` files touched (post-cleanup):
- `Cargo.toml` (already committed earlier on the branch)
- `Cargo.lock`
- `LICENSE` (new)
- `README.md`
- `.github/workflows/release.yml` (new)

No files under `docs/superpowers/`.

---

## Post-plan release sequence (out of plan, for reference)

After the PR is squash-merged to `main`:

1. `git checkout main && git pull`
2. `git tag v1.0.0`
3. `git push origin v1.0.0`
4. Workflow opens a draft Release. Maintainer reviews/edits the auto-generated notes (and on this first release, overwrites with a short basic-features summary) and clicks **Publish**.

If the version check fails: delete the tag (`git push --delete origin v1.0.0 && git tag -d v1.0.0`), fix `Cargo.toml`, merge, and re-tag.
