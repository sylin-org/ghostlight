# 1.3.3 candidate custody -- 2026-09-04

Status: HELD. The 1.3.3 service and 1.1.1 adapter candidate was assembled, attested, downloaded,
and verified from two independent local copies. Service publication waits for Chrome adapter 1.1.1
to clear staged review.

## Build

- Workflow run: [33912620937](https://github.com/sylin-org/ghostlight/actions/runs/33912620937)
  (`Build release candidate`, dispatched on `dev`), all seven jobs green.
- Frozen candidate source: `fe5b9de8815b9c20be4828d1b9d8e95a204fb350`, bound by
  `docs/release/freeze.json` and covered by the
  [2026-09-04 preflight](release-preflight-2026-09-04.md).
- Candidate version 1.3.3, adapter version 1.1.1, 18 assets. The quality gate, Windows NSIS and
  Ubuntu Debian builds, Debian 12 and Ubuntu 24.04 lifecycle smokes, candidate assembly, and
  provenance-attestation job all passed.

## Custody

Two independent downloads under the durable machine-local custody root passed
`scripts/verify-custody.ps1 -IncludeProvenance`: freeze binding, exact 18-asset roster, deep
candidate checks, SHA256SUMS recomputation, and GitHub provenance for all six raw binaries. A
relative-name and SHA-256 comparison found all 20 candidate files byte-identical between copies.
The machine-local custody path is deliberately not recorded in the repository.

## Publish-relevant hashes

- Chrome adapter `ghostlight-extension-v1.1.1.zip`:
  `1a955726153884243e86e7845b09a783c97ffe6a3f660628f97f43550bd2d2e7`.
- npm launcher `ghostlight-1.3.3.tgz`:
  `1e271db60388ad7e989164423d59ab24349bea7cb0edf2fac2ff8fab350c9a83`.
- Windows installer:
  `59a046a6c2937caaba1624fe9de6d6ecf6d59b75c940cc98140e683a96a12f15`.
- Debian package:
  `e8f3e92b2cc41b6cee33b29128e9e016dd1b57a42c234987330dbf3860af07a9`.
- MCPB:
  `4a8f3e1297b3014c3c73630e8058d10bd18f24501e4748efceef178fac0b158c`.

## Chrome staged review

Before replacement, the store served adapter 1.0.0 and held approved adapter 1.1.0 staged. With
the owner's publication direction, the stale staged submission was canceled. The exact custody
ZIP above uploaded successfully as adapter 1.1.1 and was submitted with `STAGED_PUBLISH`. The API
reported `PENDING_REVIEW`; public adapter 1.0.0 remains unchanged until review clears and the
staged revision is explicitly published.

## Prepared service publication

The lightweight remote tag `v1.3.3` points exactly at frozen source `fe5b9de8`. A private GitHub
draft contains the candidate's exact 18 assets plus `release-candidate.json` and `SHA256SUMS`; its
creator verified provenance for all 20 files before upload. npm and MCP Registry publication have
not run. The GitHub draft remains unpublished while Chrome review is pending.
