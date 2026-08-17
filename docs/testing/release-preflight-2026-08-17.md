# Ghostlight 1.0 local release preflight -- 2026-08-17

Status: pre-freeze source pass; not candidate or release approval

Baseline source revision: `eb7cf4edf271ca81bb292df3d43b313be35265ba` on `dev`, equal to
`origin/dev` at the start of the run.

The working tree also contained the new release-checklist documentation being validated by this
run. G0 has not frozen the final source revision, so this evidence reduces uncertainty but does not
check G1 in the release checklist. Candidate-bound, clean-machine, visible-browser, publication,
and public-state gates remain open.

## Results

| Gate | Result |
| --- | --- |
| Formatting | `cargo fmt --all -- --check` passed. |
| Rust lint | Locked workspace/all-target Clippy passed with warnings denied. |
| Rust tests | 356 passed: 307 orchestrator library, 10 orchestrator binary, 33 bridge, and 6 MCP connector. |
| Extension | 116 tests passed. |
| npm launcher | 10 tests passed. |
| Claude Desktop MCPB | 4 tests passed. |
| Fresh build | The complete workspace built into `.target-ghostlight-1.0`. |
| Process journey | Fresh isolated binaries passed reconnect, open/read, recording, close, and pinned no-adapter refusal. |
| Native CLI journey | Demand-free call, governed result, CLI-attributed audit, batch session, and channel refusal passed. |
| PowerShell CLI journey | Separate-process open, list, read, JPEG capture, and close passed against the fresh isolated binaries. |
| Workbench surface | All 42 assertions passed, including the current product-card integration roster and panel failure containment. |
| Policy grammar | All maintained host-pattern, capability-label, and browser-startup grammar assertions passed. |
| JavaScript syntax | All 43 tracked JavaScript, module, and CommonJS files under the release-owned surfaces parsed. |
| Script syntax | All PowerShell release scripts parsed and all 5 shell scripts passed `bash -n`. |
| Dependency policy | `cargo deny check licenses bans sources` passed. |
| Advisory policy | `cargo audit` exited zero with exactly the 17 accepted GTK/Tauri-chain warnings and no unallowed advisory. |
| Repository integrity | 797 tracked files were readable; tracked local documentation links and version alignment passed; the historical ASCII exception set remained fixed at 25. |
| 0.8 recovery | All 1,388 inventory entries remained covered in 12 reviewed groups; all 34 Lightbox scenarios retained explicit dispositions. |
| Offline public truth | Correctly reported source 1.0.0, public product 0.8.0, and public adapter 0.8.0. |

## Boundaries

This run did not perform or claim:

- a frozen source revision or provenance-bound candidate;
- GitHub CI or the manual candidate workflow;
- package construction or package lifecycle;
- a clean installed-Windows journey;
- Ubuntu GNOME Wayland L1-L9;
- a matching-store-adapter visible browser journey;
- the public MCP harness matrix;
- online public-surface reconciliation or release-access inspection;
- any tag, push, release, store, registry, package-manager, website, or external action.

Release-access inspection was deliberately not run because its optional credential discovery may
read machine-local state, which was not authorized for this session.
