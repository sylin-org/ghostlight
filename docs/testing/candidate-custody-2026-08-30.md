# 1.3.0 candidate custody -- 2026-08-30

Status: custody taken 2026-08-30 and publication proceeded the same day at the owner's
direction. The candidate was assembled, attested, downloaded, and verified from two
independent local copies before any channel advanced.

## Build

- Workflow run: [33333813230](https://github.com/sylin-org/ghostlight/actions/runs/33333813230)
  ("Build release candidate", dispatched on `dev`), all seven jobs green: quality gate,
  Windows NSIS and Debian native packages, Debian 12 and Ubuntu 24.04 package lifecycle
  smokes, deterministic extension artifact, and the attested candidate bundle.
- Candidate source revision: `7b92562521d72f6c03378aeccc4441c0bcb57d59`, pinned by
  `docs/release/freeze.json`. The freeze was first declared at the release-preparation head
  `dfcf8c4a`; the two commits between them (the freeze declaration and the preflight
  evidence) are docs-only, so the G1 preflight evidence
  [release-preflight-2026-08-30](release-preflight-2026-08-30.md) covers the candidate
  revision unchanged.
- Candidate version 1.3.0, 18 artifacts (six raw binaries, two native packages, two portable
  archives, the deterministic extension ZIP, the npm launcher, the MCPB, and five component
  SBOMs), each bound by SHA-256 in `SHA256SUMS` and attested by GitHub build provenance for
  the raw binaries.

## Verification results

`scripts/verify-custody.ps1 -IncludeProvenance` against both local copies, each after an
independent download or copy:

- freeze binding: PASS (manifest sourceRevision == `docs/release/freeze.json`);
- deep candidate checks: PASS (18 artifacts, exact roster and coordinates, every hash);
- SHA256SUMS recomputation: PASS (all 18 lines rehashed from bytes);
- GitHub provenance attestations: PASS for all six raw binaries.

Local custody: two verified copies under the machine-local
`F:\Replica\NAS\Files\ghostlight-custody\v1.3.0-run33333813230\copy-a` and `copy-b`
(paths are machine-local working state; not secrets).

## Publish-relevant asset hashes

- Extension ZIP `ghostlight-extension-v1.1.0.zip`, SHA-256
  `ce59185e271b2daad7a05c01db261036dfbb48f48bfe2f25790f079ba13bb5ce`. This is byte-identical
  to the store-approved adapter 1.1.0 revision submitted on 2026-08-30, exactly as the
  deterministic packager promises; the Chrome Web Store needs no action for this line, and
  the pending staged review already carries these bytes.
- npm `ghostlight-1.3.0.tgz`, SHA-256
  `9c818de3569f5178b4b7d027a8c48f175c2ac955496148e709f1a6fdc1fbc576`.
- Windows NSIS `ghostlight-v1.3.0-x86_64-pc-windows-msvc-setup.exe`, SHA-256
  `49fc1b0ade232c0df8962bd4d8b33649f339192fea5e74dc0fde33c83cdf9509`.
- Debian `ghostlight-v1.3.0-x86_64-unknown-linux-gnu.deb`, SHA-256
  `455ec9af64f3313a43ce423edbfdc875e5a245d74a604784d5ed5c09b961f12f`.
- MCPB `ghostlight-v1.3.0.mcpb`, SHA-256
  `d1c6663f7c274c5d7c92ab4f533658dc258e711dd922b20de9f27407ddc50afa`.
