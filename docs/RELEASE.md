# Releasing Ghostlight 1.0

This is the release plan for the current 1.0 implementation. The completed 0.8 publication record
lives in [`business/PUBLICATION-PACKET-0.8.md`](business/PUBLICATION-PACKET-0.8.md), and the active
harvest lives in [`0.8/HARVEST.md`](0.8/HARVEST.md). The old raw-binary packages and implementation
cannot be relabeled as 1.0, but their tests, platform facts, compatibility model, release safety,
and publication lessons are inputs to this pipeline.

No release action is authorized merely by this document. Tags, pushes, packages, store
submissions, registry mutations, website publication, and external messages require explicit owner
approval.

## Process rule

A release step must do at least one of these:

- prevent a failure already observed in Ghostlight or its delivery chain;
- prove a promise a user will depend on; or
- make a published failure safer to diagnose or recover.

Keep candidate verification, immutable artifact publication, store reconciliation, and optional
directory work independent. Do not recreate the 0.8 master release conductor, make a trust-footer
date a build gate, commit generated checksums after tagging, or hold a release for an optional
directory submission.

## Release unit

One Ghostlight version comprises:

- `ghostlight`, including the orchestrator, tray, native shell, and bundled workbench;
- sibling `ghostlight-mcp-connector`;
- sibling `ghostlight-browser-connector`;
- the mandatory checksum-bound `ghostlight` npm launcher, whose bare command remains an MCP stdio
  edge and whose subcommands reach the native orchestrator;
- a platform-native package that installs and removes the browser native-messaging registration;
- two portable archives, one-line installers, a self-contained Windows Claude Desktop MCPB, and
  candidate-derived Scoop and WinGet metadata;
- the independently delivered but contract-matched `Ghostlight in Browser` adapter; and
- checksums, GitHub build-provenance attestations, SBOM, license notices, source archive, and
  release notes.

The desktop and service are one executable. The connectors are deliberately stable independent
process shores, but a package must ship a tested sibling set. The extension keeps its established
name, store identity, artwork, settings, and permissions.

## Candidate gates

Before spending a candidate build, inspect release access without exposing credential values:

```powershell
pwsh -File scripts/check-release-access.ps1 -Online
```

The command is read-only. GitHub and npm access are required for their publication channels. MCP
Registry and Chrome API credentials enable optional automation and are reported without becoming
release blockers. Chrome can always be submitted through its Developer Dashboard, which is the
fallback used by 0.8. API V2 automation requires `CWS_CLIENT_ID`, `CWS_CLIENT_SECRET`,
`CWS_REFRESH_TOKEN`, `CWS_ITEM_ID`, and `CWS_PUBLISHER_ID`. If desired, a revoked refresh token can
be replaced with `scripts/get-cws-refresh-token.ps1`; the helper validates OAuth state and PKCE,
stores the token in the machine-local credential file, and never prints it.

### Source

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm test --prefix extension
npm test --prefix packaging/npm
node --test packaging/mcpb/test/launcher.test.js
cargo audit
cargo deny check licenses bans sources
cargo build --workspace --target-dir .target-ghostlight-1.0
node tests/process-journey.mjs
node --check crates/orchestrator/ui/app.js
node --check tests/workbench-preview-server.mjs
pwsh -NoProfile -File scripts/check-public-surfaces.ps1
pwsh -NoProfile -File scripts/check-0.8-recovery.ps1
pwsh -NoProfile -File scripts/package-extension.ps1
```

CI runs the Rust, extension, and process tiers on Windows and Linux. The extension artifact
uses a fixed file order and timestamp, so identical input produces identical ZIP bytes. The ordinary
public truth check is offline and deterministic. `scripts/check-public-surfaces.ps1 -Online` is a manual
reconciliation against GitHub, npm, the Chrome update feed, the official MCP Registry, and the
canonical website; a transient external outage does not make an otherwise valid source commit
fail CI.

The completed feature diff must remain empty under `crates/mcp-connector`,
`crates/browser-connector`, `crates/bridge`, and `extension` for ADR-0102.

### Desktop and package

For Windows and Linux:

1. Build a native release bundle with the original Ghostlight icon and bundled local UI.
2. Verify the package contains byte-exact Apache, MIT, commercial-module, and plain-language
   licensing files, then verify the published checksum and GitHub build-provenance attestation.
3. Install as an ordinary user on a clean machine.
4. Verify tray launch, open/hide/quit, headless fallback, global search, plural snapshots, Status,
   notifications, MCP integrations connect/disconnect, JSONC/TOML preservation, and no remote
   WebView access.
5. Verify native messaging points at the packaged sibling browser connector.
6. Upgrade from the latest supported public release without clobbering unrelated state.
7. Uninstall and prove only Ghostlight-owned files, registrations, desktop entries, and selected
   harness entries are removed. Record the audit-retention choice.

Windows release binaries statically link the Microsoft Visual C++ runtime through
`.cargo/config.toml`. The clean-machine proof must still start every packaged executable on a
machine without a separately installed Visual C++ Redistributable; the build flag is a prevention,
not a substitute for the journey.

The manual `Build release candidate` workflow builds Windows NSIS and Linux Debian candidates from
one locked workspace build. It stages the two connectors as Tauri sidecars,
inspects every native package for the exact three-executable sibling
set, builds deterministic portable archives, prepares the checksum-bound npm tarball and
self-contained MCPB, builds the deterministic extension archive and one pinned CycloneDX SBOM for
each of the four workspace components, then assembles one 17-artifact candidate unit:

- two native packages;
- two portable archives;
- six versioned raw binaries;
- the Chromium adapter;
- four component SBOMs;
- the npm launcher tarball; and
- the Claude Desktop MCPB.

`release-candidate.json` binds 17 normalized artifact coordinates, exact byte lengths, SHA-256
values, the version, the full source revision, and release status. `SHA256SUMS` is independently
recomputed by `scripts/check-release-candidate.ps1`. GitHub Actions creates build-provenance
attestations for every file in the candidate unit. The workflow uploads the unit for fourteen days.
It does not tag, platform-sign, publish, or mutate a store.

This deliberately retains the 0.8 trust model. Ghostlight has no Windows Authenticode certificate
and does not require one for 1.0. Artifact integrity comes from the immutable checksums and GitHub's
keyless build-provenance attestations, not a maintainer-held or platform code-signing key.

`ghostlight native-host check|install|uninstall` is the package-facing registration seam. It covers
Chrome, Edge, Brave, and Chromium; repairs missing or Ghostlight-owned stale state; and leaves
malformed or foreign state untouched. The 1.0 package installs no Run key, scheduled task, or
systemd user service. The connectors demand-start the orchestrator.

### Browser and MCP journeys

Run the accepted matrix in [`1.0/ACCEPTANCE.md`](1.0/ACCEPTANCE.md) with a visible ordinary browser
profile. Include two supported Chromium families where available and at least three supported MCP
harnesses. Exercise concurrent sessions, screenshots, semantic and coordinate input, file upload,
dialogs, scripts, governed denial, blocked close, group reuse across windows, child-tab adoption,
orchestrator restart, browser restart, extension reload, and unknown-effect non-replay.

### First success

Complete [`testing/greenfield-first-success.md`](testing/greenfield-first-success.md) using the exact
provenance-verified candidate and matching store adapter. Source-build success does not substitute
for this gate.

## Publication sequence

After all gates are evidenced and the owner approves, advance one independently recoverable channel
at a time:

1. Freeze versions and compatibility; build each release artifact from the approved commit.
2. Verify artifacts independently, including exact embedded UI/icon bytes and native-host paths.
3. Publish the matching browser adapter through deferred store publication if the store supports
   it.
4. Publish provenance-attested platform packages and immutable source/binary release assets.
5. Publish the exact candidate-bound npm tarball. Then publish candidate-derived Scoop and
   WinGet metadata only after their referenced portable assets are observable.
6. Publish MCP Registry metadata only after the exact npm coordinate is observable.
7. Reconcile store feeds and public compatibility from independently downloaded artifacts.
8. Update `docs/public-status.json`, README release language, trust review stamps, website copy,
   distribution records, and changelog from observed public state.
9. Run one public install-to-first-task smoke per platform.

Never claim a platform, store, package manager, or compatibility combination before the public
artifact is independently observable.

Chrome upload and submission are separate explicit operations. `scripts/publish-extension.ps1`
defaults to `Plan` and makes no request. `Upload` and `Submit` each require both the named action
and `-Execute`; submission defaults to `STAGED_PUBLISH` so review approval does not silently make
the adapter public. The script uses Chrome Web Store API V2, validates the package version and hash,
and refuses warned or taken-down items before submission. The plan always prints the exact manual
Developer Dashboard workflow, and the access check reports stale API credentials without making
them a release blocker. Missing or broken API automation never blocks the release or store
submission.

GitHub release creation and publication are also separate. `scripts/publish-github-release.ps1`
defaults to `Plan`; `CreateDraft` and `PublishDraft` each require `-Execute`. It requires an existing
remote `v<version>` tag at the candidate source revision, verifies every GitHub provenance
attestation against this repository and the release workflow, and re-downloads every draft asset
for an exact hash comparison before publication. It never creates a tag.

`scripts/publish-npm.ps1` defaults to `Plan`. Publication requires `Publish -Execute`, the exact
tarball SHA-256, a candidate that contains that same npm asset, an observable matching GitHub
release, and a valid GitHub provenance attestation for the tarball. The launcher carries all six
raw-binary hashes, rechecks cached binaries on every invocation, and refuses downloads outside the
fixed GitHub release host chain. npm publication is mandatory for 1.0; MCP Registry publication
remains downstream of it.

The MCP Registry is downstream of the public npm coordinate in `server.json`.
`scripts/publish-mcp-registry.ps1` defaults to an offline `Plan` and reports why the current
metadata is not publishable. `Publish` requires `-Execute`, an exact verified candidate with matching
versions, a publicly observable npm package at that exact version, successful official publisher
validation, and the recovered DNS credential. It logs out in a `finally` block. Registry failure
cannot hold up or roll back any other publication channel.

## Rollback

Published versions and tags remain immutable. If a release is defective, mark the affected channel
clearly and publish a higher corrected version. Browser stores generally require forward version
movement, so an adapter rollback is a higher-version code correction. Preserve evidence and do not
rewrite a failed candidate as a pass.
