# 1.3.5 candidate custody -- 2026-09-11

Status: PUBLISHED. Service 1.3.5 and Chrome adapter 1.1.4 were assembled, attested, downloaded,
verified, and published at the owner's direction.

## Build

- Workflow run: [34564815508](https://github.com/sylin-org/ghostlight/actions/runs/34564815508)
  (`Build release candidate`), all seven jobs green.
- Frozen source: `88aa7c3c8485ba9ce97cdc19d0270650407112bc`, bound by immutable tag `v1.3.5`.
- Candidate version 1.3.5, adapter version 1.1.4, 18 artifacts. Windows NSIS and Ubuntu Debian
  packaging, Debian 12 and Ubuntu 24.04 lifecycle smokes, assembly, and attestations passed.
- The quality job passed formatting, warnings-denied Clippy, workspace tests, extension tests,
  repository integrity, the parent-lifecycle regression, and the live process and CLI journeys.

The candidate includes the exact-process lifecycle fix. Both connectors allow unlimited healthy
inactivity and exit when the exact process that spawned them ends. Windows binds pid to creation
time. Linux binds parent pid to `/proc` start time. The regression keeps connector stdin open after
the parent exits and proves cleanup without using an inactivity timer.

## Custody

The downloaded bundle passed `scripts/verify-custody.ps1 -IncludeProvenance`: candidate binding,
the exact 18-artifact roster, byte lengths, SHA-256 recomputation, and GitHub provenance for every
release file. The GitHub publisher then verified provenance and re-downloaded all 20 release files
for exact hash comparison before publication.

## Publish-relevant hashes

- Chrome adapter `ghostlight-extension-v1.1.4.zip`:
  `dbc145b62bd2107b627588cdfcb24ac98b558db0cec136dfb4ee146ea73608db`.
- npm launcher `ghostlight-1.3.5.tgz`:
  `635672322b045bc221c31d62f3807cb213b66ed99889c33f94c4ba4c3e593f28`.
- Windows installer:
  `662da39833dd1bd946b46ed8a5be70a5e6152a48e9136e88f04324d6fad022ea`.
- Debian package:
  `160daed9bd8fbc8372770516eb80ea2f9a3da5e14a216464edecae50c0dd9c78`.
- MCPB:
  `f96b482fa138d84383497021564ac5bcc3508bff4952dc309d01b1e5f758eda5`.

## Publication

- GitHub release [`v1.3.5`](https://github.com/sylin-org/ghostlight/releases/tag/v1.3.5) is public
  with all 20 candidate files and exact source tag.
- npm `ghostlight@1.3.5` is public with the `latest` tag. An independently downloaded public
  tarball has the candidate SHA-256 above.
- The official MCP Registry returns `org.sylin/ghostlight 1.3.5` as latest. Publication followed
  an intentional Ed25519 DNS-key rotation: the old apex proof was replaced, public DNS returned
  only the new proof, and the official publisher authenticated, validated, published, and logged
  out. The private key remains only in the owner-authorized ignored credential file.
- Chrome adapter 1.1.4 is public at 100 percent distribution. Google's API and public update feed
  independently report it, as recorded in
  [adapter custody](adapter-1.1.4-2026-09-10.md).

Candidate-derived Scoop and WinGet metadata remain unpublished. A public clean-install smoke on
the newly returned second Windows machine remains pending until Codex registers it as a connected
host with a Ghostlight project. Existing Windows, CachyOS, Bluefin, Alpine, Debian 12, and Ubuntu
24.04 evidence remains bounded by its dated reports; this publication does not widen those claims.
