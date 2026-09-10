# Integrated Linux package validation, 2026-09-09

## Candidate and custody

This focused test-01 round uses exact integrated baseline
`ac1becab78e5a8a6b89e553e82f6779d93dcb39c`, tree
`b6e0b7b3c5c84cb4fb82a3fc15fe5cd2d4395b88`, on
`codex/fleet-test-01-packaging` in the ordinary repository. Service/package remains
1.3.5 and source adapter 1.1.2. No new worktree was created. The previous active
installation and evidence remain under their existing paths.

The purpose is to close the earlier package evidence gap: the original test-01
Debian/Ubuntu artifacts contained a8cfd033, before the integrated fixes. This round
uses copied prepared Ubuntu 22.04.2/glibc 2.35 builder and Debian 12/Ubuntu 24.04
consumer roots under `.tmp/linux-local/integrated-ac1becab/`. Original roots and
artifacts remain intact. These are disposable package guests, not desktop acceptance.

## Checkpoint

- Formatting and warnings-denied workspace Clippy pass.
- All 542 Rust tests and 222 extension tests pass on the integrated baseline.
- All 1,029 staged source blobs match the exact integrated Git tree.
- Locked, offline Ubuntu release build is running. The cached toolchain is Rust
  1.95.0; local workspace package outputs are cleaned before rebuilding.
- Authority-only native deployment uses the existing dev-loop and existing live
  directory. Both connector and source adapter trees are unchanged from the previous
  tested installation. Browser smoke and final identities remain pending.
- Package content, ABI, lifecycle, retained-MCP upgrade and portable installer checks
  remain pending. No package or release readiness is claimed at this checkpoint.

The first build attempt ran before copying finished and stopped at the missing
Tauri command, before compilation. `build-precondition-failure.log` preserves it.
The successful source verification preceded the replacement build attempt.

No credentials were changed, no package asset was published, and no shared branch
was merged. Final results and candidate/evidence manifest will replace this checkpoint.
