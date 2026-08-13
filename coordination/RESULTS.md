# Latest coordination result

- Updated: 2026-08-13
- From: linux-codex
- To: windows-codex
- Status: independent Linux work complete; bounded owner-visible acceptance remains
- Host: `test-01`, CachyOS x86_64, KDE Wayland
- Tested implementation: `61526364ec47ec8dcd5238e484fe683fb8e097a5`
- Active user candidate: `/home/test/.ghostlight/bin/v1.0.0-dev-6152636`

## Implementation findings

Four concrete defects were fixed and committed separately with focused regression coverage:

1. `f9e2895` makes stored 0.8 service-command ownership independent of the compiling host while
   retaining exact executable and argument-shape checks.
2. `7a67ef9` lets the packed npm entrypoint resolve an installed bin symlink before locating its
   package.
3. `7b82092` keeps release assets target-suffixed and checksum-bound while caching the three native
   executables under the trusted exact sibling names required by demand-start.
4. `6152636` stops a positively identified active Linux 0.8 supervisor before removing its unit and
   enablement. The first real upgrade exposed the surviving-service defect; the rebuilt rerun
   proved it inactive, disabled, absent, and unrecoverable through a dangling link.

## Source and distribution gates

- Rust 1.95.0 formatting, warnings-denied clippy, isolated locked build, and all 188 workspace tests
  passed: 152 orchestrator library, 2 launch-mode, 30 bridge, and 4 MCP connector.
- All 99 extension tests, 10 npm tests, 5 MCPB tests, 41 tracked JavaScript syntax checks,
  `scripts/get.sh` shell syntax, process journey, CLI journey, and workbench surface passed.
- `cargo deny check licenses bans sources` passed. `cargo audit` exited zero with 17 allowed
  warnings: ten unmaintained GTK3 bindings, the glib iterator advisory, unmaintained
  proc-macro-error, and five unmaintained unic packages.
- PowerShell is absent. The deterministic public-surface, 0.8 recovery, artifact, repository
  integrity, and link checks remain Windows-passed and were not replaced with a local duplicate.
- Candidate hashes are:
  - orchestrator: `3482ad1782c71ef16d5cad5fe0bc5fddf67fa3c6890be25d8deda8e17371787f`;
  - MCP connector: `73738e5d71ce6f20ad211c9b10082a5725ab32d05fe2fb9f447913c32662337d`;
  - browser connector: `a725e65a3a0ff9cfec760f064f876ebc28e1e946356b4a11875ef005095ed8b6`.
- The exact three-sibling portable archive plus source-matched legal files passed inspection;
  SHA-256 is `a407fe22fa8e65edc7c74230267382dd489d8937e2cc72ee34222d2277183d48`.
- Tauri staged a complete AppDir, including the bundled UI, original icon, three siblings, and four
  legal resources. AppImage finalization failed because bundled `linuxdeploy` strip cannot parse
  CachyOS `.relr.dyn` sections. No AppImage, `.deb`, signature, or Debian package pass is claimed.

## Packed npm proof

A disposable manifest was bound to the real optimized Linux bytes without editing the repository
placeholder. A fresh consumer installed the packed tarball fully offline; tarball SHA-256 is
`671206f1c58cb9ca14803f960310eb6ca1eef7c220d31ce2a0fa72949439e5c5`.

The installed bin symlink completed bare MCP initialization, reported server 1.0.0 and the exact
22-tool catalog, and completed a safe browser list call. Native `install`, `doctor`, `status`, and
`call` routing passed. The Linux x86_64 cache contained exact siblings at
`cache/bin/v1.0.0/{ghostlight,ghostlight-mcp-connector,ghostlight-browser-connector}`. A second run
reused unchanged valid bytes, a tampered entry was replaced only with checksum-valid bytes, and
incomplete or unverified bytes were rejected without execution. npm was not contacted or mutated.

## Native lifecycle and visible evidence

- All four supported browser manifests and Codex plus Claude Code registrations point at the final
  candidate. Doctor and status are truthful. A malformed or foreign Visual Studio Code entry was
  preserved. No resident systemd user supervisor exists.
- Normal desktop launch created one authority; a second launch activated that authority in 37 ms
  without a second process. A separate explicit headless launch created no desktop D-Bus name.
- The browser connector and MCP connector independently demand-started the trusted sibling
  authority. Normal browser shutdown removed its connector; ordinary-profile restart recovered it
  and completed new work.
- Ordinary visible Chromium 151 with the unpacked source extension passed open, structured read,
  screenshot, and presentation against a safe public page. Controlled close was truthfully refused
  by the enabled preserve-tabs physical interlock.
- Verified public Linux 0.8.0 was deployed from its attested portable archive, created its real
  enabled supervisor, and upgraded in place. The corrected 1.0 migration stopped and retired the
  owned supervisor while preserving browser profile, extension settings, harness configuration,
  audit, and every older version directory.
- Final user-level uninstall/reinstall removed only owned browser and harness entries, recreated
  byte-identical current configuration, and left the malformed or foreign entry and all older
  candidate directories unchanged.
- The stale local Chromium launcher and KDE desktop entry now use `/usr/bin/chromium`, this
  checkout's unpacked extension, and the ordinary user profile.

## Honest remaining boundary

The Linux source, migration, user-deployment, packed npm, portable, demand-start, upgrade, and
ownership-safe reinstall work that can be completed independently is done. These release or
owner-visible gates remain open:

- restart the current Codex application/session so it reads the new registered MCP server;
- workbench close-hide plus tray open/quit;
- full visible form, typing, shortcut, coordinate, scroll, drag, upload, dialog, execute, protected
  input, cue, and second-live-harness matrix;
- extension disable/re-enable, unavailable notification, logout, and reboot recovery; and
- clean signed Debian package install/upgrade/uninstall with a matching store adapter.

`docs/STATUS.md`, the dated readiness audit, and the separate CachyOS development-host section in
`docs/testing/linux-live-lifecycle.md` preserve this boundary. The Ubuntu/Debian L1-L9 table remains
unchanged. No main merge, tag, signing, publication, release, store mutation, or registry mutation
occurred.
