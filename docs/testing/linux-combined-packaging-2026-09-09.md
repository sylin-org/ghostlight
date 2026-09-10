# Combined Linux package validation, 2026-09-09

The focused combined-candidate round passed. No product or packaging-script change
was needed. This is package and installed-browser evidence, not release approval.

## Candidate and build

Exact source: `0129ff15bf0c6348a78e8c0d2e4e1d2716695ae8`, tree
`a8f014c5cd53c0acaab5f24980f24cb72f6e3a7f`, service/package 1.3.5.
The ordinary repository branch is `codex/fleet-test-01-final-packaging`.
All 1,045 staged Git blobs matched before and after packaging; Cargo.lock was
unchanged. The candidate includes bounded startup/readiness, selected connector
copy retry, and explicit Flatpak activation.

The builder was Ubuntu 22.04.2 with glibc 2.35, Rust/Cargo 1.95.0, Tauri CLI
2.11.0 and PowerShell 7.6.5. Workspace release outputs were rebuilt offline with
the locked dependency set; only dependency build caches were reused. Prepared
Ubuntu 22.04, Debian 12 and Ubuntu 24.04 roots were reused with fresh candidate
user state. These are disposable namespace fixtures, not pristine OS installs.
The prior ac1becab artifacts, failed attempts and installed paths remain intact.

The [candidate manifest](linux-combined-packaging-2026-09-09.json) records package
payloads, modes, binary hashes, builder identity and an explicit evidence allowlist.
The local evidence root is `.tmp/linux-local/combined-0129ff15/`.

## Required gates

Formatting, warnings-denied Clippy, all 564 Rust tests and all 222 extension tests
passed. A fresh debug build passed the real process journey with its binary
directory explicitly selected. The real selected-image executable-lock regression
in `tests/linux/dev-loop-copy-retry.ps1` also passed.

## Artifacts

- `Ghostlight_1.3.5_amd64.deb`
  SHA-256: `9c02034a1c0accff01190537a9ee074e5686d2f116c6becf1edb31bac9651cb6`
- `ghostlight-v1.3.5-x86_64-unknown-linux-gnu.tar.gz`
  SHA-256: `ac2a3e2b930c8ac2d2c2492a66e96f94ad061de71b13baa6db4ed0dbbc98f7f7`

All three baseline siblings require at most GLIBC_2.34, have no RPATH/RUNPATH,
and resolve their dependencies in both consumers. The portable archive contains
exactly three executable siblings and three legal files. A second independent
packager process produced an identical archive. Debian payload, desktop entry,
manuals, legal files, native registrations and conffiles passed inspection.
The authority differs between raw/portable and Debian only by Tauri's expected
three-byte UNK-to-DEB bundle marker. No Flatpak activation service or grant is
bundled. This evidence does not expand the supported platform floor.

## Debian 12 and Ubuntu 24.04.4 consumers

| Check | Debian 12 | Ubuntu 24.04.4 |
| --- | --- | --- |
| Install, remove, reinstall, purge and dpkg verification | Pass | Pass |
| Owned registration repair and foreign registration preservation | Pass | Pass |
| Retained MCP client across 1.3.4-to-1.3.5 upgrade | Pass | Pass |
| Four corrected-context clients cold-start one ready authority | 5,925 ms | 5,933 ms |
| Same four clients recover after authority termination | 1,526 ms | 1,588 ms |
| Portable install, uninstall, reinstall and foreign-state preservation | Pass | Pass |
| Native defaults leave Flatpak state unchanged | Pass | Pass |
| Explicit Flatpak activation refuses /usr/bin without mutation | Pass | Pass |

Lifecycle tests ran the actual installed executable as an ordinary user under
Xvfb and a private session bus. Runtime permissions were 0600. Owned stale native
registration was repaired, foreign same-name registration was preserved, and no
per-user desktop shadow was added. Runtime history survived package removal.

The upgrade test retained an initialized MCP connector while replacing public
1.3.4 with the candidate. The old authority remained 1.3.4 until explicit shutdown
and Open. The retained client then rediscovered 23 tools and completed policy
work against 1.3.5. Explicit Open participates in this upgrade lane; the separate
four-client test proves MCP-led cold startup.

During 11.5 seconds with desktop context stripped, four MCP clients produced a
maximum of one concurrent authority and three total attempts on each distro.
No runtime was published and no client initialized prematurely. Corrected-context
clients then became ready; the cold timings include the remaining cooldown.
After terminating the exact test authority, all four retained clients recovered
23-tool catalogs and completed fresh policy work with saved, effect-none results.
Each distro recorded five transient service-unavailable catalog replies while
connectors renegotiated. The helper polls read-only tools/list with a ten-second
deadline, following the existing process journey; it does not replay effects.

Portable checks covered actual Claude and Codex registration writers, the six
desktop environment names, owned commands/desktop/native paths, retained history,
and foreign client, command and native state. A seeded foreign Flatpak override
remained byte-identical. Native installation and startup added neither sandbox
registration nor activation grants. Explicit `--flatpak-chromium
--allow-flatpak-activation` with `/usr/bin` siblings refused the non-home-resident
layout before changing the fresh test home.

## Installed native browser smoke

All three siblings were rebuilt for CachyOS and deployed using
`scripts/dev-loop.ps1 -Action Deploy` to the preserved
`.tmp/fleet-test-01/target/release` installation. Their hashes match the fresh
native build and are recorded separately from the Ubuntu package binaries.
The unchanged source adapter was explicitly reloaded in both normal Chromium and
Brave because the native connector changed. Both browser root processes, normal
Default profiles and browser bindings were retained.

Two fresh real MCP sessions selected the browsers explicitly and passed local
fixture open, fill, targeted replacement typing, readback, independent script
verification and session tab isolation. All 23 tool schemas retained the required
history-storage result field. Host policy, client/native configuration and Flatpak
state were unchanged. Final health found one installed 1.3.5 authority, Ready,
no remaining guest processes and no remote-debugging listener. Test tabs remain;
no browser profile replacement or physical reboot was performed.

## Preserved fixture failures

Three initial fixture failures are retained beside the passing runs:

1. Debian's prepared root lacked a passwd record for uid 1000, so D-Bus failed
   before the client helper started. The wrapper adds a guest-only user when
   absent. Ubuntu's existing user was untouched.
2. Procfs exposed an outer PID while Python signals used guest PIDs. An ESRCH
   was a fixture identity mismatch, not authority death. The helper now matches
   PID namespaces, records procfs and NSpid identities plus start time, and
   signals only the exact guest process.
3. The helper queried a recovering connector before catalog renegotiation and
   incorrectly assumed a result. It now uses the existing bounded read-only
   catalog retry and retains transient errors in the passing evidence.

Each corrected startup run used a new candidate home. Original helper copies,
failure logs and results remain in the allowlist. No product change was needed.
The older ac1becab Ubuntu concurrent readiness failure remains in its own report.

## Reproduction and remaining scope

The allowlist includes `environment.sh`, `build-candidate.sh`, source verification,
artifact inspection, all consumer wrappers and the native smoke helper. Use the
existing `tests/linux/run.sh` guest drivers and these inputs in disposable roots;
select new fixture-home labels rather than reusing completed runtime state.
Builder package inventory and exact previous-package hash are retained. The
selected local `combined-evidence.tar.gz` contains only allowlisted evidence plus
this report and manifest; it excludes packages, profiles and raw runtime state.

These are local candidate builds without CI provenance attestation. This lane
has no fresh Ubuntu GNOME Wayland visible L1-L9 run against these exact packages
and a matching store adapter, no submitted-store-ZIP clean-machine acceptance,
and no physical reboot. Flatpak opt-in remains limited to home-visible binaries
and default data roots; this lane does not claim system-package Flatpak support
or repeat Bluefin's Flatpak browser acceptance. Desktop-stripped callers still
need corrected desktop context. No package, store or release publication occurred.
