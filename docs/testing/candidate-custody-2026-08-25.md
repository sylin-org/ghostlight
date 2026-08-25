# 1.0 candidate custody -- 2026-08-25

Status: HELD. The G2 candidate is assembled, attested, downloaded, and verified from two
independent local copies.

## Build

- Workflow run: [32846030216](https://github.com/sylin-org/ghostlight/actions/runs/32846030216)
  ("Build release candidate", dispatched on `dev`).
- Candidate source revision: `994b6c85dcd7c8df74237cf329461d85ce49b13a`. Product bytes are
  identical to the ADR-0137
  feature commit `8779e11b`; the delta between them is CI tooling only (SBOM contract growth
  for the fifth crate and smoke-step instrumentation), verified empty over
  `crates extension packaging Cargo.toml Cargo.lock`.
- `docs/release/freeze.json` pins `994b6c85...` per the owner's build-when-done decision: the
  freeze machinery pins whichever revision becomes the candidate, nothing more.

## Journey to green (three real defects found and fixed at their seams)

1. `linux-package-smoke (ubuntu:24.04)` failed silently twice: the minimized cloud image ships
   dpkg `path-exclude` rules that strip `/usr/share/man`, so `dpkg --verify` flagged the
   package's declared manual pages as missing. debian:12 passed identically. Fix
   (`672c89ce`): the lifecycle smoke strips the image's excludes before install and asserts
   all three manual pages are present in the archive itself.
2. `candidate-bundle` failed: five component SBOMs where the contract expected four -- the new
   audited `ghostlight-win-peer` crate is a fifth workspace component. Fix (`22547dbc`,
   `994b6c85`): the candidate contract grew to 18 artifacts / five SBOMs across the workflow,
   assembly, deep checks, custody verifier, and this checklist.
3. Custody verifier bugs found on first real use: a success-path `$LASTEXITCODE` strict-mode
   trip (`e4d38920`) and checksum lines resolving against the candidate root instead of
   `assets/` (`fd47d3a9`).

## Verification results

`scripts/verify-custody.ps1 -IncludeProvenance` against both local copies:

- freeze binding: PASS (manifest sourceRevision == `docs/release/freeze.json`);
- deep candidate checks: PASS (18 artifacts, exact roster and coordinates, every hash);
- SHA256SUMS recomputation: PASS (all 18 lines rehashed from bytes);
- GitHub provenance attestations: PASS for all six raw binaries.

Local custody: two verified copies under repo-local `.target-g2-custody/release-candidate` and
`.target-g2-custody-copy` (paths are machine-local working state; not secrets).

## Extension ZIP vs the pending store review

The candidate extension ZIP is `ghostlight-extension-v1.0.0.zip`,
SHA-256 `9ae88e6729c830a9871802a39a2301c27c1d2baa00a2213332c310a7746a6db8`. It intentionally
does NOT match the `f7b9a6ad...` bytes under the pending store review: ADR-0137 changed the
service worker after that submission. G3 therefore requires uploading this exact ZIP and
resubmitting staged, replacing the stale review -- an owner-authorized store mutation.
