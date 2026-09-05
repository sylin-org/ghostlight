# 1.3.4 candidate custody -- 2026-09-05

Status: PUBLISHED. Service 1.3.4 and Chrome adapter 1.1.1 were assembled, attested, downloaded,
and verified from two independent local copies, then published at the owner's direction.

## Build

- Workflow run: [33991341425](https://github.com/sylin-org/ghostlight/actions/runs/33991341425)
  (`Build release candidate`, dispatched on `v1.3.4`), all seven jobs green.
- Frozen source: `768ee7383da1988a2d6b0217812e23d3fe580680`, bound by the immutable `v1.3.4` tag
  and `docs/release/freeze.json`. The tag includes the local-destination fix and the unpublished
  1.3.3 changes; it does not alter the existing 1.3.3 tag or draft.
- Candidate version 1.3.4, adapter version 1.1.1, 18 assets. Windows NSIS and Ubuntu Debian
  packaging, Debian 12 and Ubuntu 24.04 lifecycle smokes, assembly, and attestations passed.
- Ordinary CI passed all nine jobs on `dev`
  ([33991354064](https://github.com/sylin-org/ghostlight/actions/runs/33991354064)) and promoted
  `main` ([33991652325](https://github.com/sylin-org/ghostlight/actions/runs/33991652325)).
- The [Windows preflight and active browser proof](release-preflight-2026-09-05.md) cover the
  same product source.

## Custody

Two independent candidate downloads passed `scripts/verify-custody.ps1 -IncludeProvenance`:
freeze binding, exact 18-asset roster, byte lengths, SHA-256 values, checksum recomputation, and
GitHub provenance for all six raw binaries. Comparing relative names and hashes found all 20
candidate files byte-identical. The package-manager metadata artifact was also retained.
Custody paths are machine-local and are deliberately absent from this record.

The first verification attempt against the second copy reported that GitHub CLI's public-good
verifier could not initialize. A sequential retry passed with the same bytes and normal verifier;
no integrity or provenance check was disabled.

## Publish-relevant hashes

- Chrome adapter `ghostlight-extension-v1.1.1.zip`:
  `1a955726153884243e86e7845b09a783c97ffe6a3f660628f97f43550bd2d2e7`.
- npm launcher `ghostlight-1.3.4.tgz`:
  `21334523423ff22a05b6f4468d86b46eae98119ae2f8cbf0b37fc59b67a18817`.
- Windows installer:
  `2c2225a75e208e0164b79d92b413b1b156ce9bd557162e97157009308aed250b`.
- Debian package:
  `30e525d6cd21da30c6569e713a53e9e892adb8b1f0f2aaf325086a0c76eea401`.
- MCPB:
  `bc815b9235873e78bf0f63ee674a9ef5b043e79d6c1c22bce2257090ecc967d1`.

The adapter ZIP is byte-identical to the approved staged 1.1.1 revision from the held 1.3.3
candidate. Its compatibility row now includes service 1.3.4.

## Publication

- GitHub release [`v1.3.4`](https://github.com/sylin-org/ghostlight/releases/tag/v1.3.4) is public.
  The publisher checked all 20 provenance attestations against the exact repository, release
  workflow, and frozen source. It re-downloaded and hash-compared every draft asset before
  publication.
- npm `ghostlight@1.3.4` is public and carries the `latest` tag. An independently downloaded
  public tarball has the candidate SHA-256 above. The public launcher downloaded and verified
  all three Windows executables; `doctor --json` reported version 1.3.4 and three ready siblings.
  The existing development-tree browser registration remained `owned_elsewhere`.
- The official MCP Registry returns `org.sylin/ghostlight 1.3.4` as latest. The official publisher
  validated the metadata, authenticated with the existing DNS credential, published, and logged
  out. No credential value was written to the repository.
- Chrome API V2 reported the approved adapter `PUBLISHED`, and the public update feed serves
  1.1.1. An independently downloaded CRX matches all 33 non-manifest candidate files. Every
  candidate manifest field is identical; Chrome adds its official update URL and verification
  metadata. No new adapter source or permission change was needed for this release.
- The website release fallback was committed and pushed at
  `a2d64732f5eeb2fa1467ce5e135abce19523faf5`. The public origin serves asset revision
  `a2d64732f5ee`, service 1.3.4, and adapter 1.1.1. Its clean build checked 36 HTML pages,
  1,221 internal references, 9 agent documents, the decision aid, and all 54 imported playground
  files. The Windows checkout initially produced CRLF-only Koan example mismatches; rebuilding
  from the exact committed LF template bytes passed unchanged checks. The published website
  commit changes only Ghostlight's release fallback.
  A connected-browser read also confirmed both published versions on the visible Ghostlight
  page. The first read reported `browser_wrong_profile` while the connection changed after store
  publication; the same read succeeded when the connection settled, without changing the session
  or relaxing its host restriction.
- `scripts/check-public-surfaces.ps1 -Online` passes: GitHub, npm, Chrome's public update feed,
  the official MCP Registry, and the canonical website agree.

Scoop and WinGet metadata remain retained candidate artifacts; this release made no bucket
submission. The older 1.3.3 tag and private draft retain their original bytes as historical custody.
