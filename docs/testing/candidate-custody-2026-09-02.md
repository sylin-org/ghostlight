# 1.3.2 candidate custody -- 2026-09-02

Status: HELD. The candidate was assembled, attested, downloaded, and verified from two
independent local copies. No channel has advanced; publication is owner-gated.

## Build

- Workflow run: [33643387463](https://github.com/sylin-org/ghostlight/actions/runs/33643387463)
  ("Build release candidate", dispatched on `dev`), all seven jobs green.
- Candidate source revision: `456395414fb86ec59cc7a43a50af57dce8c265cf`, pinned by
  `docs/release/freeze.json`. The preflight evidence
  [release-preflight-2026-09-02](release-preflight-2026-09-02.md) covers this revision (its head
  `ab886c1f` is the docs-only freeze commit). This candidate carries ADR-0150: demand-start
  identity follows the runtime override, so a floating launcher entry can route a development
  machine at its real authority.
- Candidate version 1.3.2, 18 artifacts, SHA256SUMS-bound and provenance-attested for the raw
  binaries.

## Verification results

`scripts/verify-custody.ps1 -IncludeProvenance` against both local copies: freeze binding,
deep candidate checks, SHA256SUMS recomputation, and GitHub provenance all PASS, and the two
copies are byte-identical (`diff -r` clean). Local custody:
`F:\Replica\NAS\Files\ghostlight-custody\v1.3.2-run33643387463\copy-a` and `copy-b`.

## Publish-relevant asset hashes

- Extension ZIP `ghostlight-extension-v1.1.0.zip`, SHA-256
  `ce59185e271b2daad7a05c01db261036dfbb48f48bfe2f25790f079ba13bb5ce` -- byte-identical to the
  approved store adapter 1.1.0 revision; no store action for this line.
- npm `ghostlight-1.3.2.tgz`, SHA-256
  `ecdbedbc4b1cb3907cfed97639830a91f82a5a2dccab609db9c128264509af97`.
- Windows NSIS setup, SHA-256
  `b3e47f7533a04b946e2a49bfcc47ec0c401f1358ee49191d4f1702e371db4045`.
- Debian package, SHA-256
  `2c5b81f9562ab7fa736daf3268e134d68a18264d11abc9c71b7dd8cd9d9e2fc1`.
- MCPB, SHA-256 `0d2446f47be99e0231bbdf4956be8f5510d7445f7b38d1cb557b0e90b6634331`.
