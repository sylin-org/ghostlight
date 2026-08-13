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
- a platform-native package that installs and removes the browser native-messaging registration;
- the independently delivered but contract-matched `Ghostlight in Browser` adapter; and
- checksums, signatures/attestations, SBOM, license notices, source archive, and release notes.

The desktop and service are one executable. The connectors are deliberately stable independent
process shores, but a package must ship a tested sibling set. The extension keeps its established
name, store identity, artwork, settings, and permissions.

## Candidate gates

### Source

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm test --prefix extension
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

CI runs the Rust, extension, and process tiers on Windows, macOS, and Linux. The extension artifact
uses a fixed file order and timestamp, so identical input produces identical ZIP bytes. The ordinary
public truth check is offline and deterministic. `scripts/check-public-surfaces.ps1 -Online` is a manual
reconciliation against GitHub, npm, the Chrome update feed, the official MCP Registry, and the
canonical website; a transient external outage does not make an otherwise valid source commit
fail CI.

The completed feature diff must remain empty under `crates/mcp-connector`,
`crates/browser-connector`, `crates/bridge`, and `extension` for ADR-0102.

### Desktop and package

For Windows, macOS, and Linux:

1. Build a native release bundle with the original Ghostlight icon and bundled local UI.
2. Verify code signature or platform attestation and published checksum.
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

The manual `Build release candidate` workflow builds unsigned Windows NSIS, Linux Debian, and
macOS application/disk-image candidates from one locked workspace build. It stages the two
connectors as Tauri sidecars, inspects every native package for the exact three-executable sibling
set, builds the deterministic extension archive and one pinned CycloneDX SBOM for each of the four
workspace components, then assembles one nine-artifact candidate unit. `release-candidate.json`
binds normalized artifact names,
byte lengths, SHA-256 values, version, and the full source revision; `SHA256SUMS` is independently
recomputed by `scripts/check-release-candidate.ps1`. The workflow uploads the unit for fourteen
days. It does not tag, sign, publish, or mutate a store.

`ghostlight native-host check|install|uninstall` is the package-facing registration seam. It covers
Chrome, Edge, Brave, and Chromium; repairs missing or Ghostlight-owned stale state; and leaves
malformed or foreign state untouched. The 1.0 package installs no Run key, scheduled task,
launchd agent, or systemd user service. The connectors demand-start the orchestrator.

### Browser and MCP journeys

Run the accepted matrix in [`1.0/ACCEPTANCE.md`](1.0/ACCEPTANCE.md) with a visible ordinary browser
profile. Include two supported Chromium families where available and at least three supported MCP
harnesses. Exercise concurrent sessions, screenshots, semantic and coordinate input, file upload,
dialogs, scripts, governed denial, blocked close, group reuse across windows, child-tab adoption,
orchestrator restart, browser restart, extension reload, and unknown-effect non-replay.

### First success

Complete [`testing/greenfield-first-success.md`](testing/greenfield-first-success.md) using the signed
candidate and matching store adapter. Source-build success does not substitute for this gate.

## Publication sequence

After all gates are evidenced and the owner approves, advance one independently recoverable channel
at a time:

1. Freeze versions and compatibility; build each release artifact from the approved commit.
2. Verify artifacts independently, including exact embedded UI/icon bytes and native-host paths.
3. Publish the matching browser adapter through deferred store publication if the store supports
   it.
4. Publish signed platform packages and immutable source/binary release assets.
5. Publish package-manager and MCP-registry metadata only after their referenced assets exist.
6. Reconcile store feeds and public compatibility from independently downloaded artifacts.
7. Update `docs/public-status.json`, README release language, trust review stamps, website copy,
   distribution records, and changelog from observed public state.
8. Run one public install-to-first-task smoke per platform.

Never claim a platform, store, package manager, or compatibility combination before the public
artifact is independently observable.

## Rollback

Published versions and tags remain immutable. If a release is defective, mark the affected channel
clearly and publish a higher corrected version. Browser stores generally require forward version
movement, so an adapter rollback is a higher-version code correction. Preserve evidence and do not
rewrite a failed candidate as a pass.
