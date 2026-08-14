# Ghostlight 1.0 release-readiness audit -- 2026-08-13

Status: source gate passed; release gate not yet passed

Implementation revision tested: `54802f89d4133c0ac42f8062376a84808003ed9e`.

Environment: Windows x86_64, Rust/Cargo 1.95.0, Node 24.7.0, PowerShell 7.6.0, and Tauri CLI
2.11.0. Build outputs are local and gitignored. Nothing was tagged, pushed, signed, submitted, or
published.

## Passed locally

| Gate | Result |
| --- | --- |
| Formatting | `cargo fmt --all -- --check` passed. |
| Rust lint | Workspace/all-target clippy passed with warnings denied. |
| Rust tests | 187 passed: 151 orchestrator library, 2 launch mode, 30 bridge, and 4 MCP connector. |
| Extension tests | 99 passed. |
| npm launcher | 8 offline tests passed, including cache-tamper replacement and rejected unverified bytes. |
| Claude Desktop MCPB | 5 launcher tests passed; two complete packages were byte-identical. |
| JavaScript syntax | All 41 tracked JavaScript and module files parsed. |
| Isolated build | All four workspace packages built under `.target-release-audit`. |
| Process topology | Service/relay reconnect, MCP catalog/call, unknown-effect, recording, and audit journey passed. |
| CLI | Governed result, non-zero refusal, caller-owned batch session, and CLI-attributed audit passed. |
| PowerShell journey | Separate-process open/list/read/JPEG capture/close passed against the isolated stack. |
| Workbench surface | Startup fault containment and the recovered stale-path Update action passed. |
| Historical evidence | Regeneration reproduced 1,355 tests plus 34 Lightbox scenarios byte-for-byte. |
| Recovery disposition | All 1,389 entries and all 34 scenarios are covered by the checked recovery matrix. |
| Artifact recovery | All 810 mature 0.8 files are named and dispositioned; the four high-value missing release artifacts are restored on current seams. |
| Repository integrity | All 681 tracked files were readable, local documentation links resolved, source versions aligned, and no new ASCII exception appeared. |
| Dependency policy | `cargo deny check licenses bans sources` passed. |
| Advisory gate | `cargo audit` exited zero with no unallowed vulnerability error; residual warnings are below. |
| Public truth | Offline check reported source 1.0.0, observed public product 0.8.0, and observed public adapter 0.8.0. |
| Extension artifact | Two builds with exact Apache and MIT texts were byte-identical: SHA-256 `47a7cb7b715d14de991266f3602ecf6f166fd967623c4e7980f58a2afc3c47c3`. |
| Windows bundle build | Unsigned NSIS completed from one locked release build. |
| Windows bundle contents | Exact three sibling executables and four source-matched legal files present; no staging names leaked. |
| Release native-host check | Read-only check named the exact sibling connector and four missing browser registrations. |
| Development swap | Exact-path plan found only the repository service; isolated build/lock/copy/cleanup passed without touching that service. |
| Candidate assembler | Four real component SBOMs plus synthetic cross-platform input passed exact 27-artifact assembly and verification. |
| Distribution entry points | npm, MCPB, four portable archives, one-line installers, and Homebrew/Scoop/WinGet metadata were constructed and checked from candidate hashes. |
| Publication planning | Chrome, GitHub, npm, and MCP plans made no mutation and named the missing or mismatched prerequisites. |

The local unsigned NSIS output was
`target/x86_64-pc-windows-msvc/release/bundle/nsis/Ghostlight_1.0.0_x64-setup.exe`, 3,240,136
bytes, SHA-256 `3d694ea78ee0dcf9589654b48223a32ba60d619dda544c69746d432178f1352c`.
It is build evidence, not a release artifact.

## Native Linux development-host record

A separate CachyOS x86_64 run tested implementation revision
`61526364ec47ec8dcd5238e484fe683fb8e097a5` with Rust 1.95.0, Node 22.22.1, npm 10.9.4,
Chromium 151.0.7922.137, KDE, and an ordinary visible Wayland session. This is source,
user-candidate, and portable-distribution evidence. CachyOS cannot satisfy the clean Ubuntu/Debian
package, signature, package-manager, login, or reboot rows.

| Gate | Result |
| --- | --- |
| Formatting, lint, and build | Passed with one explicit fresh `.target-linux-1.0` directory and the locked graph. |
| Rust tests | 188 passed: 152 orchestrator library, 2 launch mode, 30 bridge, and 4 MCP connector. |
| Extension, npm, and MCPB tests | 99, 10, and 5 passed respectively. |
| JavaScript and shell syntax | All 41 tracked JavaScript/module files parsed; `scripts/get.sh` passed `sh -n`. |
| Process surfaces | Process reconnect/unknown-effect/recording and CLI governance/batch/refusal journeys passed; workbench surface passed. |
| Dependency policy | `cargo deny check licenses bans sources` passed; `cargo audit` exited zero with the same 17 allowed warnings below. |
| Candidate deployment | Exact three optimized siblings deployed under `~/.ghostlight/bin/v1.0.0-dev-6152636`; doctor found four current browser registrations plus current Codex and Claude Code entries. No user supervisor exists. |
| Portable/raw | Exact three siblings and source-matched legal files passed inspection. Portable archive SHA-256: `a407fe22fa8e65edc7c74230267382dd489d8937e2cc72ee34222d2277183d48`. |
| Packed npm launcher | Fresh offline install proved MCP initialization/catalog/safe call, CLI routing, cache reuse, tamper recovery, and rejection of incomplete or unverified bytes. Tarball SHA-256: `671206f1c58cb9ca14803f960310eb6ca1eef7c220d31ce2a0fa72949439e5c5`. |
| Public 0.8 upgrade | Attested public 0.8.0 was deployed, its real supervisor was created, and 1.0 retired it. A defect found in the first run was fixed with focused coverage before the passing rerun. |
| Ownership-safe reinstall | Owned browser and harness entries were removed and recreated identically; a malformed or foreign Visual Studio Code entry and all older version directories were unchanged. |
| Ordinary-profile browser | Visible open/read/screenshot/presentation, single-authority activation, both connector demand-start paths, and browser restart recovery passed. Model-driven close was truthfully blocked by the enabled local preserve-tabs interlock. |
| Linux native bundle | A complete AppDir was staged, but AppImage finalization failed because the bundled `linuxdeploy` strip tool does not understand CachyOS `.relr.dyn` sections. No `.deb`, AppImage, or package pass is claimed. |

PowerShell was absent. The deterministic public-surface, 0.8 recovery, artifact, integrity, and link
checks remain the Windows-passed results above and were not replaced by a home-grown Linux checker.
Full interactive form, typing, shortcut, coordinate, scroll, drag, upload, dialog, execution,
multi-harness, close/hide, tray, login, reboot, extension-disable, and notification-failure journeys
remain incomplete.

## Residual dependency warnings

RustSec reported 17 warnings in target-specific transitive dependencies:

- ten unmaintained GTK3 binding packages and one `glib` iterator unsoundness advisory in the
  Linux Tauri/WebKit/tray dependency chain;
- five unmaintained `unic-*` packages below Tauri's URL pattern dependency; and
- the unmaintained `proc-macro-error` below the GTK3 macro chain.

These are warnings under the current `cargo audit` gate, not silently clean output. Ghostlight does
not call the named `glib::VariantStrIter` API directly, but the dependency is present in the Linux
desktop graph. Recheck the graph and available Tauri upgrade before signing Linux. Do not describe
the dependency scan as warning-free.

## Not run and therefore still blocking release

| Gate | State |
| --- | --- |
| GitHub workflow execution | NOT OBSERVED for the combined Linux head in this record; inspect the triggered runs after the authorized `dev` push. |
| Linux Debian candidate | NOT RUN. CachyOS source/user-candidate and portable proof does not substitute for it. |
| macOS arm64/x64 candidates | NOT RUN in native macOS builders. |
| Windows package install | NOT RUN. Building and inspecting NSIS does not prove its hooks or lifecycle. |
| Package signing/attestation | Platform signing NOT RUN. Workflow provenance is implemented but has not run on GitHub. |
| SBOM and immutable release checksums | Assembly logic passed locally; a real cross-platform 1.0 publication candidate is NOT BUILT. |
| Clean install | NOT RUN on Windows, macOS, or Linux ordinary-user machines. |
| 0.8 -> 1.0 upgrade | Development user-path proof passed on CachyOS; packaged Ubuntu/Debian upgrade remains NOT RUN. |
| Login/reboot demand-start | NOT RUN on any packaged platform. |
| Uninstall ownership | Development user-path ownership proof passed on CachyOS; native package removal remains NOT RUN. |
| Native tray/window/notification | NOT RUN interactively. |
| Store adapter | NOT SUBMITTED and no matching 1.0 store build exists publicly. |
| Visible browser acceptance | PARTIAL in ordinary-profile Chromium with the unpacked source adapter; package plus matching store adapter remains NOT RUN. |
| Public MCP harness matrix | NOT RUN in three public harnesses against the signed candidate. |
| Public reconciliation | NOT APPLICABLE until approved immutable artifacts exist. |

Release access is only partly ready. GitHub and npm authentication are valid. The MCP DNS key and
publisher exist. Chrome's stored refresh token is revoked or expired and `CWS_PUBLISHER_ID` is
missing. Windows and Apple signing credentials were not found. The 0.8 Linux SSH identity was
recovered, but its `test-host-01` name is not currently resolvable.

The Linux L1-L9 record remains [`linux-live-lifecycle.md`](linux-live-lifecycle.md), with every row
still `NOT RUN`. That is the next high-value platform gate. It must start from public 0.8 for the
upgrade row and retain only content-free evidence.

## Release decision

Do not publish 1.0 from this state. The source and local Windows build are credible inputs to a
candidate, but they are not a signed, installed, upgraded, uninstalled, or visibly exercised
cross-platform product. Run the manual build-only workflow after the owner chooses to push, then
perform the native and visible gates one platform at a time. Keep publication channels independent.
