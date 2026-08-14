# M03: CI workflows (Windows-and-Linux matrix + release artifacts)

## Goal

ADR-0026 Decision 2: a CI gate (fmt, clippy -D warnings, test) across
Windows and Linux on every push and PR, plus a tag-triggered release job
that builds the `ghostlight` binary for the two shipping targets and
uploads artifacts.

## Authority

ADR-0026 Decision 2; 00-design.md "CI (m03)".

## Depends on

Nothing. STOP precondition: `.github/` does not exist. If it exists, STOP
(someone else created CI; do not merge configurations by judgment).

## Current behavior (verified 2026-07-03; re-read before editing)

- No .github/ directory. No rust-toolchain.toml; no rust-version key in
  Cargo.toml. Single crate `ghostlight`, edition 2021.
- The binary name is `ghostlight` (Cargo.toml [package] name), so release
  outputs are target/<target>/release/ghostlight (plus .exe on Windows).

## Required behavior

Create exactly two files with exactly this content.

### 1. .github/workflows/ci.yml

    name: CI

    on:
      push:
        branches: ["**"]
      pull_request:

    jobs:
      fmt:
        runs-on: ubuntu-latest
        steps:
          - uses: actions/checkout@v4
          - uses: dtolnay/rust-toolchain@stable
            with:
              components: rustfmt
          - run: cargo fmt --check

      test:
        strategy:
          fail-fast: false
          matrix:
            os: [ubuntu-latest, windows-latest]
        runs-on: ${{ matrix.os }}
        steps:
          - uses: actions/checkout@v4
          - uses: dtolnay/rust-toolchain@stable
            with:
              components: clippy
          - uses: Swatinem/rust-cache@v2
          - run: cargo clippy --all-targets -- -D warnings
          - run: cargo test

### 2. .github/workflows/release.yml

    name: Release artifacts

    on:
      push:
        tags: ["v*"]

    jobs:
      build:
        strategy:
          fail-fast: false
          matrix:
            include:
              - os: windows-latest
                target: x86_64-pc-windows-msvc
              - os: ubuntu-latest
                target: x86_64-unknown-linux-gnu
        runs-on: ${{ matrix.os }}
        steps:
          - uses: actions/checkout@v4
          - uses: dtolnay/rust-toolchain@stable
            with:
              targets: ${{ matrix.target }}
          - uses: Swatinem/rust-cache@v2
          - run: cargo build --release --target ${{ matrix.target }}
          - uses: actions/upload-artifact@v4
            with:
              name: ghostlight-${{ matrix.target }}
              path: |
                target/${{ matrix.target }}/release/ghostlight.exe
                target/${{ matrix.target }}/release/ghostlight
              if-no-files-found: error

## Constraints

Transcribe the YAML byte-for-byte (two-space indent, no tabs). No SPDX header
in YAML files (00-design.md). Do not add caching, badges, scheduling, or any
other job.

## Tests (all from repo root)

- `rg -c "dtolnay/rust-toolchain@stable" .github/workflows/ci.yml` prints `2`.
- `rg -c "windows-latest" .github/workflows/ci.yml` prints `1`.
- `rg -c "cargo clippy --all-targets -- -D warnings" .github/workflows/ci.yml`
  prints `1`.
- `rg -c "x86_64-pc-windows-msvc|x86_64-unknown-linux-gnu" .github/workflows/release.yml`
  prints `2`.
- `rg -c "if-no-files-found: error" .github/workflows/release.yml` prints `1`.
- YAML validity is confirmed live on the first push (GitHub parses it); locally
  the rg pins above plus a visual check that indentation is two-space and
  tab-free are sufficient. Note under "Notes for the reviewer" that the
  workflows are unvalidated until the first push.

## Verification

The rg assertions; ASCII diff scan; `cargo test` unchanged (no compiled
change). Ledger entry noting that the workflows are validated live on the
first push. Commit.

Commit subject: `ci: Windows-and-Linux gate (fmt, clippy, test) and tag-triggered release artifacts`

## Out of scope

Node/extension jobs (m05 and m06 own their own ci.yml additions); publishing
releases (artifacts only); toolchain pinning files; branch protection.
