# Ghostlight 0.8 publication state on 2026-08-13

Status: read-only observation

This is a fresh observation of the shipped 0.8 publication. It changes no external channel. The
original `docs/business/PUBLICATION-PACKET-0.8.md` remains the record of what was known on
2026-08-07.

## Canonical release

- Version: `0.8.0`.
- Release commit: `993135b048b60622157266b53b21f1719c9df4b3`.
- Reconciled public source commit: `95468758ab56b38da8b5ea5b717d51642c8cd56d`.
- GitHub release: <https://github.com/sylin-org/ghostlight/releases/tag/v0.8.0>.
- Release workflow run: `31152001239`, completed successfully.
- GitHub API assets: 38. The web UI also counts the two automatic source archives and displays
  `Assets 40`.

The 38 explicit assets comprise four platform archives, twelve raw executables, per-artifact
SHA-256 files, the extension ZIP, MCPB bundle, SBOM, and sorted `SHA256SUMS`.

## Public channels

| Channel | Observed state |
| --- | --- |
| GitHub release | v0.8.0 public, signed commit verified, explicit artifacts still downloadable. |
| npm | `ghostlight@0.8.0` remains the public package and `latest`. |
| Chrome Web Store | `Ghostlight in Browser` 0.8.0 is public under item `lejccfmoeogmhemakeknjjdhkfkgncdl`. |
| Official MCP Registry | `org.sylin/ghostlight` 0.8.0 is active and latest. |
| GitHub MCP catalog | Search returns `Sylin Ghostlight` by `sylin-org`. |
| Canonical website | <https://sylin.org/ghostlight/> presents the 0.8 product and install path. |
| Homebrew | The Sylin tap carries the 0.8.0 formula and immutable release hashes. |
| Scoop | Public `main` carries the direct 0.8.0 manifest and Windows archive hash. |
| WinGet | PR <https://github.com/microsoft/winget-pkgs/pull/413601> is merged. Validation, moderator review, and publication completed. |
| mcpservers.org | <https://mcpservers.org/servers/sylin-org/ghostlight> is live. |
| Glama | <https://glama.ai/mcp/servers/sylin-org/ghostlight> is live but needs correction; see below. |

## Open and deferred destinations

| Destination | Observed state |
| --- | --- |
| Cline marketplace | Issue <https://github.com/cline/mcp-marketplace/issues/1989> remains open. |
| awesome-mcp-servers | PR <https://github.com/punkpeye/awesome-mcp-servers/pull/11306> remains open. |
| PulseMCP | No Ghostlight result was found after the stated daily/weekly ingestion window. Recheck before contacting anyone. |
| mcp.so | Not submitted because its path required a fee. Spending still requires owner approval. |
| Claude directory | The released MCPB exists, but the observed MIT-only form rule conflicted with the complete open-core bundle. No inquiry was sent. |
| OpenAI public directory | The observed form required a public production HTTPS MCP endpoint. Ghostlight remains intentionally local-only. |
| Native Edge Add-ons | Intentionally deferred because individual enrollment exposed the owner's home contact address. Edge can use the Chrome store path. |

## Drift found

### WinGet record

The 2026-08-07 publication packet says the WinGet PR was pending. It later merged and published.
The historical packet stays unchanged; this observation supplies the correction.

### Glama

Glama reports local-only hosting metadata, but its rendered page mixes current repository material
with stale 0.6/0.7-era install fragments. It also offers an install/deploy action whose hosted
presentation does not fit Ghostlight's local-browser boundary. Treat the listing as drift, not as
an approved distribution mechanism.

Before asking Glama to refresh:

1. Confirm the canonical repository and website text are correct.
2. Capture the exact stale fragments without private browser state.
3. Ask only for a metadata refresh or removal of the misleading hosted action.
4. Do not publish a remote Ghostlight transport to satisfy a directory UI.

### Website search surfaces

The canonical Ghostlight page is current for 0.8. Search results for other Sylin routes still expose
older embedded Ghostlight snippets. A publication check must scan the whole deployed site, not only
the canonical route.

## Publication lessons for 1.0

- Keep public 0.8 facts in `docs/public-status.json` until 1.0 is actually observable.
- Generate candidate metadata separately from observed-public metadata.
- Publish downstream manifests only after their exact immutable package exists.
- Reconcile every channel from independently downloaded public bytes.
- Let one failed channel remain failed or pending; do not block unrelated channels.
- Treat directory-provided hosted-install actions as a trust-boundary review item.
- Record distribution and reception separately.
- No external edit, comment, submission, or refresh is implied by this document.
