# 1.3.6 candidate custody -- 2026-09-12

Status: PUBLISHED. Service 1.3.6 and unchanged Chrome adapter 1.1.4 were assembled, attested,
downloaded, verified, and published at the owner's direction.

## Build

- Workflow run: [34705695934](https://github.com/sylin-org/ghostlight/actions/runs/34705695934)
  (`Build release candidate`), all seven jobs green.
- Frozen source: `fa4edd8891d08cbc5e7f2af09dbcd1c0b7c019a9`, bound by immutable tag `v1.3.6`.
- Candidate version 1.3.6, adapter version 1.1.4, 18 artifacts. Windows NSIS and Ubuntu Debian
  packaging, Debian 12 and Ubuntu 24.04 lifecycle smokes, assembly, and attestations passed.
- PR [#89](https://github.com/sylin-org/ghostlight/pull/89) merged as
  `a331230f5e17563e742c683d832ba287455326be` after all 11 jobs in ordinary CI run
  [34707865338](https://github.com/sylin-org/ghostlight/actions/runs/34707865338) passed.

## Custody

The downloaded bundle and an independent held copy both passed
`scripts/verify-custody.ps1 -IncludeProvenance`: freeze binding, the exact 18-artifact roster,
byte lengths, SHA-256 recomputation, and GitHub provenance for every release file. The GitHub
publisher then re-downloaded all 20 release files and compared exact hashes before publication.

## Publish-relevant hashes

- Chrome adapter `ghostlight-extension-v1.1.4.zip`:
  `dbc145b62bd2107b627588cdfcb24ac98b558db0cec136dfb4ee146ea73608db`.
- npm launcher `ghostlight-1.3.6.tgz`:
  `2d843e9426bd64410c9fbd47c0f017dfb5c327065d2fe6b6c52138bbb48f9f7b`.
- Windows installer:
  `ae4575302f854566269795bd63a30ba2e40eee3f4fd5c78b5fc769baee2c56d4`.
- Windows portable archive:
  `9f53a8f796df8469709fb8debc797f12bf9057c6d4d064694605b9d25cef09dd`.
- Debian package:
  `d3ca24eb665e52f5e3c1446c37c2f0d05e41c90b699074099b26945b978f53ce`.
- Linux portable archive:
  `2d002f1cd1862c35d2eb2af38f7ef6e49b7c14330cba487afe5e956c0ad5275d`.
- MCPB:
  `e29869300ac31f454c69336c9e8365a74921fa28d6dc6d1b28e19f773381f492`.

## Publication

- GitHub release [`v1.3.6`](https://github.com/sylin-org/ghostlight/releases/tag/v1.3.6) is public
  with all 20 files and the exact source tag.
- npm `ghostlight@1.3.6` is public with the `latest` tag. An independent registry download has
  the candidate SHA-256 above.
- The official MCP Registry returns `org.sylin/ghostlight 1.3.6` as active and latest. Its package
  record points to npm 1.3.6.
- Chrome adapter 1.1.4 is unchanged and remains public at 100 percent distribution. No Chrome
  upload, submission, or browser restart occurred for this service release.
- `packaging/scoop/ghostlight.json` resolves directly to the public Windows archive and records
  the candidate hash above. Central Scoop Extras eligibility remains unchanged.

The public website, GitHub Pages redirect and WinGet submission are separate downstream publication
steps. Their receipts and the public consumer smoke belong in this record after completion.
