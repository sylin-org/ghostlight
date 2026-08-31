# 1.3.1 candidate custody -- 2026-08-31

Status: PUBLISHED 2026-08-31. The candidate was assembled, attested, downloaded, and verified
from two independent local copies before any channel advanced.

## Build

- Workflow run: [33355735166](https://github.com/sylin-org/ghostlight/actions/runs/33355735166)
  ("Build release candidate", dispatched on `dev`), all seven jobs green.
- Candidate source revision: `0d7b77598bdefed3fdc51c161a06ed741dc3b5bd`, pinned by
  `docs/release/freeze.json`. The first freeze at the preparation head `71c10f95` predates the
  freeze declaration and preflight evidence, docs only; the G1 preflight evidence
  [release-preflight-2026-08-31](release-preflight-2026-08-31.md) covers the candidate revision
  unchanged.
- Candidate version 1.3.1, 18 artifacts, SHA256SUMS-bound and provenance-attested for the raw
  binaries.

## Verification results

`scripts/verify-custody.ps1 -IncludeProvenance` against both local copies: freeze binding,
deep candidate checks, SHA256SUMS recomputation, and GitHub provenance all PASS. Local custody:
`F:\Replica\NAS\Files\ghostlight-custody\v1.3.1-run33355735166\copy-a` and `copy-b`.

## Publish-relevant asset hashes

- Extension ZIP `ghostlight-extension-v1.1.0.zip`, SHA-256
  `ce59185e271b2daad7a05c01db261036dfbb48f48bfe2f25790f079ba13bb5ce` -- byte-identical to the
  approved store adapter 1.1.0 revision; no store action for this line.
- npm `ghostlight-1.3.1.tgz`, SHA-256
  `e291c27c229a1f266575d70bf2653ac7f1690733119e45fbfe95637699d81b4f`.
- Windows NSIS setup, SHA-256
  `ec36acd81aff3bc546f57b22b5eab1c327968d432383866ab6d406966829dd95`.
- Debian package, SHA-256
  `c8df18f1e360b2ff7c63a44c8424a0032a082d93724c7129538d078f6a65a59c`.
- MCPB, SHA-256 `51997e8c78d537de76340502bd720b442c3958d3bc9192002f75ce25ac09cc94`.
