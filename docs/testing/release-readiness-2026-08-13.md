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
| GitHub workflow execution | NOT RUN. The recovery commits are local and have not been pushed. |
| Linux Debian candidate | NOT RUN in a native Linux builder. |
| macOS arm64/x64 candidates | NOT RUN in native macOS builders. |
| Windows package install | NOT RUN. Building and inspecting NSIS does not prove its hooks or lifecycle. |
| Package signing/attestation | Platform signing NOT RUN. Workflow provenance is implemented but has not run on GitHub. |
| SBOM and immutable release checksums | Assembly logic passed locally; a real cross-platform 1.0 publication candidate is NOT BUILT. |
| Clean install | NOT RUN on Windows, macOS, or Linux ordinary-user machines. |
| 0.8 -> 1.0 upgrade | NOT RUN. Stale manifest, harness, and supervisor migration still need live proof. |
| Login/reboot demand-start | NOT RUN on any packaged platform. |
| Uninstall ownership | NOT RUN against installed native packages. |
| Native tray/window/notification | NOT RUN interactively. |
| Store adapter | NOT SUBMITTED and no matching 1.0 store build exists publicly. |
| Visible browser acceptance | NOT RUN against the package and matching store adapter. |
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
