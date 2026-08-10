# Releasing Ghostlight 1.0

This is the release plan for the current 1.0 implementation. The completed 0.8 publication record
lives in [`business/PUBLICATION-PACKET-0.8.md`](business/PUBLICATION-PACKET-0.8.md); its scripts,
package launchers, adapter compatibility, and registry state are historical evidence, not a 1.0
pipeline.

No release action is authorized merely by this document. Tags, pushes, packages, store
submissions, registry mutations, website publication, and external messages require explicit owner
approval.

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
cargo build --workspace --target-dir .target-ghostlight-1.0
node tests/process-journey.mjs
node --check crates/orchestrator/ui/app.js
node --check tests/workbench-preview-server.mjs
```

The completed feature diff must remain empty under `crates/mcp-connector`,
`crates/browser-connector`, `crates/bridge`, and `extension` for ADR-0102.

### Desktop and package

For Windows, macOS, and Linux:

1. Build a native release bundle with the original Ghostlight icon and bundled local UI.
2. Verify code signature or platform attestation and published checksum.
3. Install as an ordinary user on a clean machine.
4. Verify tray launch, open/hide/quit, headless fallback, global search, plural snapshots, Checkup,
   notifications, harness Check/Install/Uninstall, JSONC/TOML preservation, and no remote WebView
   access.
5. Verify native messaging points at the packaged sibling browser connector.
6. Upgrade from the latest supported public release without clobbering unrelated state.
7. Uninstall and prove only Ghostlight-owned files, registrations, desktop entries, and selected
   harness entries are removed. Record the audit-retention choice.

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

After all gates are evidenced and the owner approves:

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
