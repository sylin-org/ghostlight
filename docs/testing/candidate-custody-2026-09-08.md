# Service 1.3.5 / adapter 1.1.2 preparation -- 2026-09-08

Status: EXTENSION PENDING REVIEW. The owner authorized bumping both revisions and submitting
the extension first, so Google review can run before package publication. Service/package 1.3.5
is a local source candidate. No service package, registry version, or GitHub release was published.

## Adapter artifact and submission

- Artifact: `dist/ghostlight-extension-v1.1.2.zip`, made by `scripts/package-extension.ps1`.
- SHA-256: `38d4cc9be45a43494b0803adaa1a7657fff237efd527d067241c5a23a38116d5`.
- All 38 ZIP entries were checked against source. Every non-manifest entry is byte-identical;
  the manifest differs only by removal of the development key. No permission was added by this
  revision bump. The actual dialog-opening fix was already exercised in installed Chrome.
- The source worker and manifest changes are retained with this record; the packager's ordinary
  fixed timestamps and entry order were used. This was a local extension build, not a
  provenance-attested full service release candidate.
- The authenticated Chrome Web Store developer dashboard accepted draft version 1.1.2 for
  existing item `lejccfmoeogmhemakeknjjdhkfkgncdl`, publisher Sylin.org.
- Before submission, the automatic-publication checkbox was unchecked and read back as false.
  Final confirmation said `Your extension was submitted for review`; item status was
  `Pending review`. Google noted that a staged approval expires 30 days after review passes.
- Existing listing copy, graphics, privacy fields, and distribution settings were not edited.
  No new publisher account or extension item was created. API credentials were absent from
  the default location on the refreshed machine; the owner signed in to the dashboard.

## Revision alignment and validation

The workspace and five local Cargo packages, Tauri manifest, npm package/checksum placeholder,
MCPB manifest, Debian changelog, and shipped manual version lines now name 1.3.5. The adapter
manifest names 1.1.2. Compatibility includes adapter 1.1.2 with services 1.3.4-1.3.5 and extends
the existing 1.1.1 row through 1.3.5. The public-status document and public MCP Registry manifest
remain at service 1.3.4 / adapter 1.1.1 until actual publication.

All 19 Windows hardening gates passed after the bumps, on unchanged source fingerprint:
`0b422b26a0f484858f260db2894dc72e87900929e7ca5c6cdbc4efcf5f34896d`.
Local report: `.tmp/hardening-suite/2026-09-08T17-19-06-584Z-36432/results.json`.
This includes formatting, clippy, Rust/extension/launcher tests, exact-source executable builds,
process recovery, native Windows desktop checks, and isolated Chromium journeys.
Offline public-surface and repository-integrity checks passed. Integrity checking also found
and repaired an assessment link to the retired sequence source by binding it to its historical
revision; the assessment itself was preserved.

The prior [installed integration evidence](pre-release-integration.md) exercises the same
dialog-handler code and all 23 tools through the installed native host. The local running
service was not replaced merely to change its version string. That live evidence is not an
attestation that the newly numbered service package was installed.

## Remaining release work

Google approval and explicit staged publication remain separate from this submission. The
service release still needs the exact candidate build/custody and outstanding Windows/Linux
package, clean-install, real-client, and governed installed-browser gates in the
[pre-release matrix](pre-release-integration.md). The original installation failure and retained
timing failures remain unresolved observations, not fixes claimed by this revision bump.
