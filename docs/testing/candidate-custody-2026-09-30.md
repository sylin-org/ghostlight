# 1.3.12 candidate custody -- 2026-09-30

Status: PUBLISHED. GitHub, npm metadata, and the official MCP Registry independently serve 1.3.12.
Chrome adapter 1.3.12 was published separately. Website and public consumer verification are
recorded below as they complete.

## Build

- Workflow run: [36722844820](https://github.com/sylin-org/ghostlight/actions/runs/36722844820)
  (`Build release candidate`), all seven jobs green.
- Frozen source: `d67048bbe0914485771a2640ea0eef3e4b02ccaa`, bound by tag `v1.3.12`.
  The [freeze declaration](../release/freeze.json) is committed separately at `40b71065`.
- Service and Chrome adapter version 1.3.12, 18 artifacts. The quality gate, Windows NSIS and
  Linux Debian packaging, Debian 12 and Ubuntu 24.04 package lifecycle smokes, candidate assembly,
  five component CycloneDX SBOMs, and GitHub attestations passed.
- This release includes the previously unpublished 1.3.11 service changes. The
  [release notes](../release/notes-v1.3.12.md) describe the complete update from service 1.3.10.
  Adapter protocol major 3 requires the matching 1.3.12 service and Chromium adapter.

## Custody

The downloaded bundle at `.tmp/release-1.3.12/candidate/` passed
`scripts/verify-custody.ps1 -IncludeProvenance`: exact freeze binding, deep candidate checks,
the 18-artifact roster, SHA-256 recomputation, and GitHub provenance for all six raw binaries.
The separate copy at `.tmp/release-1.3.12/candidate-custody/` passed the candidate and hash checks.
Original verification output is retained in `.tmp/release-1.3.12/custody-original.log`.

The candidate manifest SHA-256 is
`4763b5bcda8e9030185befb0fa305f734b0e19cdcd24a1360dc99035b0db1bbc`.
Its extension ZIP is byte-identical to the uploaded 1.3.12 submission that is now published.
This byte comparison does not attest the extension bytes loaded in the development browser.
The GitHub publisher verified provenance for all 20 release files, then independently downloaded
the draft assets and compared their names and hashes before publication.

## Installed Windows candidate evidence

The three exact Windows executables from the candidate were copied into the existing selected
`target/release` development installation without rebuilding. The deployment preserved the
installation selection and native-host registration bytes, held both deployment locks, stopped
only exact selected executable paths, and verified destination hashes against the candidate.
Previous binaries remain in the ignored deployment backup. The candidate service started hidden,
and the browser reconnected automatically and reached Ready. No browser restart or extension reload
was needed for that reconnect.

Installed SHA-256 values:

- `ghostlight.exe`:
  `ee0ab7d887c6bb45fbc8ee76c4f465c38e8d2b0dcd983dce18959b4992e6370b`.
- `ghostlight-mcp-connector.exe`:
  `0b9ce152c05d0710cd9ec6c18e359b87eecbd4ee5f14c6b67017b76e7de5216d`.
- `ghostlight-browser-connector.exe`:
  `a2af2cbe936061dc92bcc64838854fc4c51e0bd0b82dabd168702f4ef877a2df`.

The unmodified `tests/live-journey.mjs` rerun passed all 15 checks and included a successful
invocation of every one of the 24 advertised tools through the installed MCP connector, selected
service, registered native host, and installed MV3 adapter. It checked retained ordinary and shadow
editor values, embedded documents, truthful failed compositions, keyboard and pointer effects,
byte-exact uploads, dialogs, diagnostics, zoom and scroll, recording delivery and erase, history,
workspace listing, the Sylin framed form, and captures. Its source fingerprint remained unchanged.
Evidence: `.tmp/release-1.3.12/installed-browser-candidate-rerun.json` and the matching `.log`.

The first run is retained separately in `.tmp/release-1.3.12/installed-browser-candidate.json`
and the matching `.log`. It failed on the first `browser_read` after a successful new localhost
navigation with `document_unavailable`, `effect: none`, and `repeat_safe: true`. No source,
service, adapter, or configuration change separated that failure from the diagnostic and rerun.
The independent `.tmp/release-1.3.12/fixture-diagnostic.json` records immediate successful reads
and control inspection on both localhost and 127.0.0.1, with actual fixture requests and one
inspected document. The original failure's specific cause remains unproven. The successful rerun
does not establish a cold-start race or erase the failed evidence.

The focused installed credential check also passed. Unacknowledged fill and targeted typing
returned authorization guidance with no input effect; subsequent work proceeded without a session
hold. Explicitly acknowledged fill, targeted typing, and focused typing succeeded, and separate
page evaluations returned `true` for the retained-value checks. This used synthetic fixture input
and did not submit a form. Evidence and the exact driver are retained at
`.tmp/release-1.3.12/credential-candidate.json` and `.tmp/release-1.3.12/credential-candidate.mjs`.

Earlier source and real Chromium component checks cover passive page feedback, absence of page
buttons and input interception, and removal of obsolete owned presentation roots. Those checks
have component scope. The installed journey above proves the candidate binaries in the existing
Windows development stack; it does not prove clean-machine installation, packaged upgrade/removal,
the running adapter's store-byte identity, or visible Linux browser and desktop acceptance.

## Publish-relevant hashes

- Chrome adapter `ghostlight-extension-v1.3.12.zip`:
  `9d5521b3f34562051ea6b07d9d5d0e07267ff6d7bacf274bd38234d43659a25d`.
- npm launcher `ghostlight-1.3.12.tgz`:
  `b92e1aa2c33bfab4dccb2fe788f4ee9e5270e3949d6895cf74cd609b682b3da7`.
- Windows installer:
  `50d3ccbe04648294236f6a26317f5fd1948e84df8a2b7b4ecf835a2d48a5b043`.
- Windows portable archive:
  `c801ca7608109acbdfbeec59911d7479ccd2dfede78a3998da2306e34a321ad8`.
- Debian package:
  `2e78aca77fee0af4d666f7bb6b19577b5bd428dfefab995252251f8a7a46e4ed`.
- Linux portable archive:
  `435ee484773370a715fb8b5e66610e3afbd2dfa645f14e0248efaacc6d8d177e`.
- MCPB:
  `1b93411bc9a33f34e3df0b73f8fee9366f94d8aa491d870b8e50dc3bb03d22da`.

## Publication

- GitHub release [v1.3.12](https://github.com/sylin-org/ghostlight/releases/tag/v1.3.12) is public
  with all 20 files, published at 14:04:04 UTC.
- npm accepted the exact candidate tarball once, then reported asynchronous processing. Public
  version and `latest` endpoints first returned 1.3.12 at 14:15:13 UTC. The first fresh consumer
  attempt encountered a cached tarball 404. A cache-busted public download at 14:17:00 UTC matched
  the candidate SHA-256; the ordinary consumer path remained pending at that observation.
- The official MCP Registry published `org.sylin/ghostlight 1.3.12` after npm metadata became
  observable. An independent latest-version query confirmed that exact record.
- The public Chrome CRX matches all 38 non-manifest submitted files. Its manifest differs only
  by Chrome's official update URL, and its sole extra file is Chrome verification metadata.
  See [the store record](chrome-store-submission-2026-09-30.md).
- Scoop metadata is derived from the exact Windows portable archive hash above. WinGet remains
  at its earlier published 1.3.6; no WinGet submission is claimed here.

Website reconciliation and the ordinary public npm consumer result remain to be recorded.
