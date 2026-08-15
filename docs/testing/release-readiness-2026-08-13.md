# Ghostlight 1.0 release-readiness audit -- 2026-08-13

Status: source gate passed; release gate not yet passed

Windows implementation revision tested: `b292bb22766686f7a07d8ffb75194867e5e94c70`.

Environment: Windows x86_64, Rust/Cargo 1.95.0, Node 24.7.0, PowerShell 7.6.0, and Tauri CLI
2.11.0. Build outputs are local and gitignored. The tested implementation was pushed to `dev`;
nothing was tagged, signed, submitted, or published.

## Passed locally

| Gate | Result |
| --- | --- |
| Formatting | `cargo fmt --all -- --check` passed. |
| Rust lint | Workspace/all-target clippy passed with warnings denied. |
| Rust tests | 194 passed: 158 orchestrator library, 2 launch mode, 30 bridge, and 4 MCP connector. |
| Extension tests | 100 passed. |
| npm launcher | 10 offline tests passed, including cache-tamper replacement and rejected unverified bytes. |
| Claude Desktop MCPB | 4 launcher tests passed. |
| JavaScript syntax | All 41 tracked JavaScript and module files parsed. |
| Isolated build | All four workspace packages built under `.target-release-audit`. |
| Process topology | Service/relay reconnect, MCP catalog/call, unknown-effect, recording, and audit journey passed. |
| CLI | Governed result, non-zero refusal, caller-owned batch session, and CLI-attributed audit passed. |
| PowerShell journey | Separate-process open/list/read/JPEG capture/close passed against the isolated stack. |
| Workbench surface | Startup fault containment and the recovered stale-path Update action passed. |
| Historical evidence | Regeneration reproduced 1,354 tests plus 34 Lightbox scenarios byte-for-byte. |
| Recovery disposition | All 1,388 entries and all 34 scenarios are covered by the checked recovery matrix. |
| Artifact recovery | All 809 in-scope mature 0.8 files are named and dispositioned; the four high-value missing release artifacts are restored on current seams. |
| Repository integrity | All tracked files were readable, local documentation links resolved, source versions aligned, and no new ASCII exception appeared. |
| Dependency policy | `cargo deny check licenses bans sources` passed. |
| Advisory gate | `cargo audit` exited zero with no unallowed vulnerability error; residual warnings are below. |
| Public truth | Offline check reported source 1.0.0, observed public product 0.8.0, and observed public adapter 0.8.0. |
| Extension artifact | Two builds with exact Apache and MIT texts were byte-identical: SHA-256 `47a7cb7b715d14de991266f3602ecf6f166fd967623c4e7980f58a2afc3c47c3`. |
| Windows bundle build | Unsigned NSIS completed from one locked release build. |
| Windows bundle contents | Exact three sibling executables and four source-matched legal files present; no staging names leaked. |
| Windows package lifecycle | Disposable silent install, browser and MCP registration, doctor, idempotent reinstall, ownership-safe double uninstall, NSIS uninstall, and exact cleanup passed. |
| Windows desktop lifecycle | Release showed no console; startup minimized one exact Tauri workbench; activation, Close containment, and recreation passed with one authority. |
| Release native-host check | Read-only check named the exact sibling connector and four missing browser registrations. |
| Development swap | Exact-path plan found only the repository service; isolated build/lock/copy/cleanup passed without touching that service. |
| Candidate assembler | Four real component SBOMs plus synthetic Windows and Linux input passed exact 17-artifact assembly and verification. |
| Distribution entry points | npm, Windows MCPB, two portable archives, one-line installers, and Scoop/WinGet metadata were constructed and checked from candidate hashes. |
| Publication planning | Chrome, GitHub, npm, and MCP plans made no mutation and named the missing or mismatched prerequisites. |
| GitHub CI | Push run `31809913114` passed all nine Windows, Linux, extension, process, supply-chain, release-truth, and formatting jobs at `de4392db`. |

The rebuilt local unsigned NSIS output was
`.target-windows-package/x86_64-pc-windows-msvc/release/bundle/nsis/Ghostlight_1.0.0_x64-setup.exe`,
3,292,239 bytes, SHA-256
`100093627d781b1a4e0c8cc481d974e63fbce3939ad2383384c74f8915acb4d9`.
It is build evidence, not a release artifact.

## Native Windows development-host record

The Windows x86_64 lane used a real unsigned NSIS install under an exact disposable directory on
the development host. It did not simulate user files or registry state.

| Gate | Result |
| --- | --- |
| First-run handoff | First usable npm install opened the service-first walkthrough once; repeat, dry-run, `--no-open`, and CI paths stayed non-interactive. |
| Browser registration | Chrome, Edge, Brave, and Chromium all pointed at the installed sibling browser connector. |
| MCP registration | Codex, Claude Code, Claude Desktop, Cursor, Visual Studio Code, Windsurf, Zed, OpenCode, and Crush pointed directly at the installed native MCP connector. |
| Installer idempotency | A second all-client install changed zero bytes across all nine config files. |
| Runtime chain | Doctor found three siblings, four current browsers, nine installed clients, and a reachable service. A real stdio initialize selected MCP revision `2025-11-25`. |
| Desktop startup | Exact HWND inspection found one visible minimized `Ghostlight` / `Tauri Window`, no visible console, and one authority. |
| Desktop recovery | Second launch restored and focused the existing view; native Close destroyed only the view; a later launch recreated it. |
| Ownership-safe removal | First uninstall removed the four browser keys and nine owned MCP entries. The second changed zero config bytes and no installed connector reference remained. |
| Native uninstaller | NSIS removed all package-owned files and its uninstall record. Test-created runtime and empty audit files were removed only after exact-path verification. |

No Ghostlight process, browser registration, disposable install directory, or default runtime file
remained. This passes the development-host package lane. It does not claim a provenance-verified
clean-machine install, public-0.8 package upgrade, login/reboot behavior, notification delivery, or
matching store-adapter browser acceptance.

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

## Current-tree Linux source follow-up -- 2026-08-15

The source committed with this follow-up was rerun on the same CachyOS x86_64 development host
with Rust and Cargo 1.95.0, Node 22.22.1, and npm 10.9.4. This is current-tree source evidence. It
does not change the Debian L1-L9 table or any public-release state.

| Gate | Result |
| --- | --- |
| Formatting and lint | `cargo fmt --all -- --check` and locked workspace/all-target Clippy with warnings denied passed. |
| Rust tests | 264 passed: 227 orchestrator library, 2 launch mode, 31 bridge, and 4 MCP connector. |
| JavaScript and shell | 103 extension, 10 npm, and 4 MCPB tests passed; all 42 tracked JavaScript/module files and all shell scripts parsed; policy grammar and all 30 workbench-surface assertions passed. |
| Fresh builds | Locked debug and optimized workspace builds completed in one new isolated target directory. |
| Process surfaces | Fresh debug binaries passed reconnect, unknown-effect, recording, audit, CLI governance, batch, and schema-3 channel-refusal journeys. |
| Dependency policy | License, bans, and source checks passed. `cargo audit` exited zero with the exact 17 allowed warnings below. |

The follow-up also removed the direct unmaintained `rustls-pemfile` dependency, used rustls's own
PEM iterator for the managed HTTPS CA pin, scoped the reviewed `CDLA-Permissive-2.0` allowance to
`webpki-roots`, and kept the Windows-only managed-path environment import off Linux.

PowerShell is absent on this host, so the PowerShell CLI journey and PowerShell release-truth
scripts were not rerun. This rolling CachyOS host also lacks the exact pinned Ubuntu/Debian package
environment and package inspection tools. No Debian artifact or lifecycle pass is claimed.

## Residual dependency warnings

RustSec reported 17 warnings in target-specific transitive dependencies:

- ten unmaintained GTK3 binding packages and one `glib` iterator unsoundness advisory in the
  Linux Tauri/WebKit/tray dependency chain;
- five unmaintained `unic-*` packages below Tauri's URL pattern dependency; and
- the unmaintained `proc-macro-error` below the GTK3 macro chain.

These are warnings under the current `cargo audit` gate, not silently clean output. Ghostlight does
not call the named `glib::VariantStrIter` API directly, but the dependency is present in the Linux
desktop graph. Recheck the graph and available Tauri upgrade before publishing Linux. Do not describe
the dependency scan as warning-free.

## Not run and therefore still blocking release

| Gate | State |
| --- | --- |
| Linux Debian candidate | NOT RUN. CachyOS source/user-candidate and portable proof does not substitute for it. |
| Package provenance | Workflow provenance is implemented but has not run on GitHub. Ghostlight follows the 0.8 checksum plus keyless GitHub attestation model; platform code signing is not a gate. |
| SBOM and immutable release checksums | Assembly logic passed locally; a real cross-platform 1.0 publication candidate is NOT BUILT. |
| Clean install | NOT RUN on Windows or Linux ordinary-user machines. |
| 0.8 -> 1.0 upgrade | Development user-path proof passed on CachyOS; packaged Ubuntu/Debian upgrade remains NOT RUN. |
| Login/reboot demand-start | NOT RUN on any packaged platform. |
| Uninstall ownership | Windows development-host NSIS removal and the CachyOS user path passed; clean-machine candidate removal remains NOT RUN. |
| Native tray/window/notification | Windows native window lifecycle passed by exact HWND inspection. Tray interaction and notification delivery remain NOT RUN interactively. |
| Store adapter | NOT SUBMITTED and no matching 1.0 store build exists publicly. |
| Visible browser acceptance | PARTIAL in ordinary-profile Chromium with the unpacked source adapter; package plus matching store adapter remains NOT RUN. |
| Public MCP harness matrix | NOT RUN in three public harnesses against the provenance-verified candidate. |
| Public reconciliation | NOT APPLICABLE until approved immutable artifacts exist. |

Release access is ready for the mandatory GitHub and npm channels. The MCP DNS key and publisher
also exist. Chrome API automation is not configured because the stored refresh token is revoked or
expired and `CWS_PUBLISHER_ID` is missing; this is not a release blocker because the 0.8 manual
Developer Dashboard path remains supported. No Windows signing credential is expected or required.
The 0.8 Linux SSH identity was recovered, but its recorded host name was not resolvable at the time
of this report.

The Linux L1-L9 record remains [`linux-live-lifecycle.md`](linux-live-lifecycle.md), with every row
still `NOT RUN`. That is the next high-value platform gate. It must start from public 0.8 for the
upgrade row and retain only content-free evidence.

## Release decision

Do not publish 1.0 from this state. The source and unsigned Windows development-host install are
credible inputs to a candidate, but they are not a provenance-verified clean-machine, upgraded,
login/reboot, or visibly exercised cross-platform product. Run the manual build-only workflow after
the owner chooses to push, then perform the remaining native and visible gates one platform at a
time. Keep publication channels independent.
