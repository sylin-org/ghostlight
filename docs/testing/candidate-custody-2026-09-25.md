# 1.3.10 candidate custody -- 2026-09-25

Status: PUBLISHED. Service 1.3.10 and unchanged Chrome adapter 1.3.8 were assembled, attested,
downloaded, verified, and published at the owner's direction.

## Build

- Workflow run: [36149531433](https://github.com/sylin-org/ghostlight/actions/runs/36149531433)
  (`Build release candidate`), all seven jobs green.
- Frozen source: `6078e5425442aa0f0235baad52468b0bab57cf85`, bound by immutable tag
  `v1.3.10`. The freeze declaration is committed separately at `1a11934c`.
- Candidate version 1.3.10, Chrome adapter version 1.3.8, 18 artifacts. The quality gate, Windows
  NSIS and Ubuntu Debian packaging, Debian 12 and Ubuntu 24.04 lifecycle smokes, candidate
  assembly, CycloneDX SBOM generation, and attestations passed.
- Before CI, the freshly built Windows binaries passed 23 native lifecycle checks. Both connectors
  and the desktop authority exited successfully under simulated system shutdown, including the
  worst-case Tauri teardown ordering. The full process journey also passed against 1.3.10.

## Custody

The downloaded bundle passed `scripts/verify-custody.ps1 -IncludeProvenance`: freeze binding,
deep candidate checks, the exact 18-artifact roster, SHA-256 recomputation, and GitHub provenance
for the raw binaries. The GitHub publisher verified provenance for all 20 release files, then
re-downloaded the draft and compared exact names and hashes before publication.

The first draft-publication attempt stopped safely while GitHub's attestation index had not yet
resolved one SBOM. The release remained a draft. The same constrained provenance query passed once
the index settled, and the complete all-file gate was rerun unchanged before publication.

## Publish-relevant hashes

- Chrome adapter `ghostlight-extension-v1.3.8.zip`:
  `d071e2b7aa48d0b3df9649efbf41c422584088f3b16b00ca27350a0d5619667a`.
- npm launcher `ghostlight-1.3.10.tgz`:
  `4d6168683a4fb2427a4b4a16b43bd3e1946e5e852faa5c4ce985832609065c3f`.
- Windows installer:
  `8562dbf75d08750207a237d3313d7758bd289a0c430d244c0da0be5a20994067`.
- Windows portable archive:
  `1fb295e86400771a386b45839087dfb4f42a4bbf8b94819058744dce66677a00`.
- Debian package:
  `21aa9735db9320fa009238d2cdd9e5e1f70a33312c8e421e19eb8a91308f1425`.
- Linux portable archive:
  `4d833535ef7db72c12e4ba4a3be6f63c8e6500b08b6f8ad545e4641172bd974b`.
- MCPB:
  `2bfb8aa9bc06b06be49355a7010be10e03d3e629c78c3e7fadf9cb74650dff40`.

## Publication

- GitHub release [`v1.3.10`](https://github.com/sylin-org/ghostlight/releases/tag/v1.3.10) is
  public with all 20 files and the exact source tag.
- npm `ghostlight@1.3.10` is public with the `latest` tag and candidate-bound tarball above.
- The official MCP Registry published `org.sylin/ghostlight 1.3.10` after npm 1.3.10 became
  publicly observable.
- Chrome adapter 1.3.8 and Firefox adapter 1.3.9 are unchanged. No browser-store upload or
  submission occurred for this service-only release.
- Website commit
  [`eb08c37f`](https://github.com/sylin-org/website/commit/eb08c37ffbf74d5932a7d3094d5fc8ec63c76470)
  is live at `https://sylin.org/ghostlight/` and names service 1.3.10.
- The online public-surface check reports GitHub, npm, Chrome's update feed, the official MCP
  Registry, and the website in agreement.

The [Windows public consumer smoke](windows-public-1.3.10-smoke-2026-09-25.md) independently
downloaded all three Windows executables through the public npm launcher, verified their candidate
hashes, and ran the 1.3.10 CLI from an isolated Ghostlight state directory.
