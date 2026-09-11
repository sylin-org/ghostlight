# Fleet wrap and release preparation -- 2026-09-09

September 10 update: this record describes the original wrap. Google subsequently
served 1.1.2, and the owner rolled back to 1.1.1 code under Google's new version
1.1.3. The unsubmitted local 1.1.3 ZIP below is superseded and must not be uploaded.
The fixes now use 1.1.4; see [the new custody record](adapter-1.1.4-2026-09-10.md).

Status: Prepared locally; no public release or live deployment in this wrap-up.
The owner stopped the next Linux round before dispatch and authorized preparing
the bump/deployment, with package publication conditional on Google's publication
of the pending extension. That condition is not met.

## Frozen scope and versions

- Completed integrated fleet source/evidence: `94e5387f435c685dcc41e64fc775208641c9c34f`.
  Last product runtime change: `0129ff15bf0c6348a78e8c0d2e4e1d2716695ae8`.
- Service/package stays at the already prepared, unpublished 1.3.5. There is no
  reason to skip that version merely because more fixes joined its candidate.
- Extension source advances from 1.1.2 to 1.1.3 for the later human-browsing fixes.
  Its compatibility row covers service 1.3.5. This wrap changes no runtime code.
- The submitted 1.1.2 ZIP remains unchanged, SHA-256
  `38d4cc9be45a43494b0803adaa1a7657fff237efd527d067241c5a23a38116d5`.
- New local artifact: `dist/ghostlight-extension-v1.1.3.zip`, SHA-256
  `4392dd130bfca57cf6fabb6250146dc1a0d6148509cdc7cb1089a44ab2aa166b`.
  It is not uploaded, approved, published or provenance-attested.
  All 39 ZIP entries match source; the manifest differs only by removal of the
  development key. The prior submitted ZIP hash was independently rechecked.
- Public GitHub/npm/MCP metadata remains service 1.3.4. Both the
  [Chrome listing](https://chromewebstore.google.com/detail/ghostlight-in-browser/lejccfmoeogmhemakeknjjdhkfkgncdl)
  and Google's live update feed were checked during this wrap and still show 1.1.1.
  This proves public distribution, not the private review state of 1.1.2.
- The original submission disabled automatic publication. Approval could therefore
  leave 1.1.2 staged. Even a published 1.1.2 would not deliver the later fixes now
  captured in 1.1.3; do not conflate those archive identities.

## Evidence retained

The [completed fleet board](fleet-focused-round-2026-09.md) links exact reports,
candidate hashes and original failures. Its completed scope is:

| Environment | Established evidence | Remaining boundary |
| --- | --- | --- |
| CachyOS/KDE | Installed native Chromium and Brave, typing/readback; exact Ubuntu-built GNU portable and Debian candidates; Debian 12 and Ubuntu 24.04 lifecycle/retained-client recovery | Namespace/private-display consumers do not prove a fresh GNOME desktop |
| Alpine/KDE | Combined dynamic-musl build, installed configured Codex cold start, normal native Chromium and workbench | Native debug prototype, not a published APK; no new combined-candidate reboot |
| Bluefin/GNOME | Real Flatpak Chromium cold start, open/read, authority/relay recovery and owned registration removal/reinstall | Home-resident/default-root scope; new permission needs a fresh browser sandbox |
| Windows | Exact installed NSIS upgrade, sibling hashes, actual Codex cold start and retained VS Code recovery | Browser adapter unavailable on the test machine; held-file uninstall remains unverified |

Linux combined source gates passed 564 Rust and 222 extension tests; Windows
passed 537 Rust and 222 extension tests. Process and installed reports name their
tested binaries. The extension version-only change does not turn those reports
into installed 1.1.3 or matching-store acceptance.

Wrap validation on `codex/release-wrap-1.3.5`: formatting, Clippy, all 537 Windows
Rust tests, all 222 extension tests and offline public/version compatibility pass.
Repository integrity passes for 1,051 tracked files, local links, unchanged ASCII
exceptions and the evidenced capability matrix. Read-only installed Doctor reports
service 1.3.5 Ready with its browser connected; this is the existing live stack,
not proof of a deployment of the wrap candidate.
No extension JavaScript changed in this wrap. The artifact check above validates
the actual newly numbered ZIP rather than an older adapter archive.

Both earlier fleet monitors were deleted. No new assignments or monitor were
created. The [six-topic Linux plan](linux-offerings-round-2026-09.md) is deferred,
including APK/Arch/RPM packaging, system-package Flatpak, container clients, broader
setup repair, Ubuntu GNOME and distribution metadata. It adds no support claim.

## Deployment and publication handoff

1. Use this wrap's committed source. Do not rebuild an old fleet checkout or
   substitute a historical package with the same 1.3.5 version label.
2. The local dev-loop plan verified the ordinary repository's `target/release`
   installation. Build affected siblings into `.target-dev-loop` and deploy only
   through `scripts/dev-loop.ps1`. Shared startup source changed during the fleet
   round, so inspect installed custody before selecting the sibling set. The plan
   itself stopped no process and changed no registration or live file.
3. Reload the current unpacked adapter through an allowed browser control and
   verify the normal installed graph. Preserve human tabs and browser profiles;
   a fresh disposable navigation avoids stale content-script receivers.
4. Build the exact candidate through the existing release workflow and retain its
   checksums/provenance. Local package reports do not establish CI attestation.
   Keep remaining clean-machine, matching-store and Windows browser/uninstall
   limits explicit; do not rewrite the old passing evidence as broader coverage.
5. Recheck Google's public feed and the staged submission. Prepare the 1.1.3 store
   submission separately from unchanged 1.1.2. No store mutation was made here.
   Once the owner's publication condition and actual candidate gates are met,
   use the existing release tooling and reconcile observed public metadata.

The owner branch, public tags, package registries and store submission remain
unchanged. No recurring publication monitor was requested or created by this wrap.
