# Ghostlight 1.0 release-readiness audit -- 2026-08-13

Status: source gate passed; release gate not yet passed

Implementation revision tested: `f1c6dab1d6c0298c3de368b2db85324ef826d5ed`.
This record was added afterward with no implementation change.

Environment: Windows x86_64, Rust/Cargo 1.95.0, Node 24.7.0, PowerShell 7.6.0, and Tauri CLI
2.11.0. Build outputs are local and gitignored. Nothing was tagged, pushed, signed, submitted, or
published.

## Passed locally

| Gate | Result |
| --- | --- |
| Formatting | `cargo fmt --all -- --check` passed. |
| Rust lint | Workspace/all-target clippy passed with warnings denied. |
| Rust tests | 184 passed: 149 orchestrator library, 1 launch mode, 30 bridge, and 4 MCP connector. |
| Extension tests | 99 passed. |
| JavaScript syntax | All 105 JavaScript and module files under extension, workbench UI, and tests parsed. |
| Isolated build | All four workspace packages built under `.target-release-audit`. |
| Process topology | Service/relay reconnect, MCP catalog/call, unknown-effect, recording, and audit journey passed. |
| CLI | Governed result, non-zero refusal, caller-owned batch session, and CLI-attributed audit passed. |
| PowerShell journey | Separate-process open/list/read/JPEG capture/close passed against the isolated stack. |
| Workbench surface | Startup fault containment and the recovered stale-path Update action passed. |
| Historical evidence | Regeneration reproduced 1,355 tests plus 34 Lightbox scenarios byte-for-byte. |
| Recovery disposition | All 1,389 entries and all 34 scenarios are covered by the checked recovery matrix. |
| Dependency policy | `cargo deny check licenses bans sources` passed. |
| Advisory gate | `cargo audit` exited zero with no unallowed vulnerability error; residual warnings are below. |
| Public truth | Offline check reported source 1.0.0, observed public product 0.8.0, and observed public adapter 0.8.0. |
| Extension artifact | Two builds were byte-identical: SHA-256 `8bc54c93e454a74fd85585af9a06e51381cd7a949f7a9dd58013b9c145dd943b`. |
| Windows bundle build | Unsigned NSIS completed from one locked release build. |
| Windows bundle contents | Exact `ghostlight.exe`, MCP connector, and browser connector present; no staging names leaked. |
| Release native-host check | Read-only check named the exact sibling connector and four missing browser registrations. |

The local unsigned NSIS output was
`target/x86_64-pc-windows-msvc/release/bundle/nsis/Ghostlight_1.0.0_x64-setup.exe`, 3,230,201
bytes, SHA-256 `9f00f60eab0aa7121b28ee223c269e6acf7e37dd0d1c40fc07e239252f4e7074`.
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
| GitHub workflow execution | NOT RUN. The four recovery commits are local and have not been pushed. |
| Linux Debian candidate | NOT RUN in a native Linux builder. |
| macOS arm64/x64 candidates | NOT RUN in native macOS builders. |
| Windows package install | NOT RUN. Building and inspecting NSIS does not prove its hooks or lifecycle. |
| Package signing/attestation | NOT RUN on any platform. |
| SBOM and immutable release checksums | NOT BUILT for a 1.0 publication candidate. |
| Clean install | NOT RUN on Windows, macOS, or Linux ordinary-user machines. |
| 0.8 -> 1.0 upgrade | NOT RUN. Stale manifest, harness, and supervisor migration still need live proof. |
| Login/reboot demand-start | NOT RUN on any packaged platform. |
| Uninstall ownership | NOT RUN against installed native packages. |
| Native tray/window/notification | NOT RUN interactively. |
| Store adapter | NOT SUBMITTED and no matching 1.0 store build exists publicly. |
| Visible browser acceptance | NOT RUN against the package and matching store adapter. |
| Public MCP harness matrix | NOT RUN in three public harnesses against the signed candidate. |
| Public reconciliation | NOT APPLICABLE until approved immutable artifacts exist. |

The Linux L1-L9 record remains [`linux-live-lifecycle.md`](linux-live-lifecycle.md), with every row
still `NOT RUN`. That is the next high-value platform gate. It must start from public 0.8 for the
upgrade row and retain only content-free evidence.

## Release decision

Do not publish 1.0 from this state. The source and local Windows build are credible inputs to a
candidate, but they are not a signed, installed, upgraded, uninstalled, or visibly exercised
cross-platform product. Run the manual build-only workflow after the owner chooses to push, then
perform the native and visible gates one platform at a time. Keep publication channels independent.
