# Release preflight -- fdcea1b3cb61d06280337cbdc4cfa621750c5466

```text
date_utc: 2026-09-04T19:42:12Z
source_revision: fdcea1b3cb61d06280337cbdc4cfa621750c5466
tree_dirty: true
toolchain: rustc 1.95.0 (59807616e 2026-04-14); node v24.7.0
host: Microsoft Windows 10.0.26200
```
Windows source preflight for the 1.3.3 service and 1.1.1 adapter candidate. The frozen source is
`fe5b9de8`; this preflight head is its docs-only freeze descendant. The dirty-tree flag is this
evidence file plus the untracked `.zcode/` session cache. No tracked product or packaging file was
dirty.

## Fringe-stability review

Diff `06f96cc0..fe5b9de8` touches the shared bridge only at the existing negotiated browser
capability seam and touches the extension only for physical composed-DOM observation, frame
routing, geometry, and their tests. The MCP connector and browser connector are unchanged. The
integration Fix stays inside the orchestrator's existing harness application service and
workbench command. The extension remains policy-free, and no product feature was placed in a
connector or relay.

## Rows completed around this runner

- `scripts/check-public-surfaces.ps1` (offline): source service 1.3.3, public service 1.3.2, source
  adapter 1.1.1, and public adapter 1.0.0, as intended before publication.
- `scripts/check-repository-integrity.ps1`: 894 tracked files readable, links valid, service
  stamps aligned, 26 historical ASCII exceptions unchanged, permission justifications complete,
  and the capability matrix green.
- `scripts/check-release-access.ps1 -Online`: GitHub, npm, Chrome Web Store API V2, and MCP
  Registry publication access all valid; no credential value was printed.
- The workspace suite contains 436 passing Rust tests and the extension suite contains 171 passing
  tests. The isolated process, CLI, PowerShell, and workbench journeys all passed without changing
  the machine's native-host registration or leaking a process.
- The real browser journey against `https://sylin.org/ghostlight/demo/iframe/` was completed before
  the freeze with the unpacked 1.1.1 source reloaded. It covered composed full-page read, document
  inspect, find, text wait, shadow-hosted iframe target geometry and screenshot, semantic form fill,
  submission, completion wait, and completion read.

## Stage results

| Stage | Result | Detail |
| --- | --- | --- |
| cargo fmt --all -- --check | PASS |  |
| cargo clippy --workspace --all-targets -D warnings | PASS |     Checking ghostlight-mcp-connector v1.3.3 (F:\Replica\NAS\Files\repo\github\sylin-org\browser-mcp\crates\mcp-connector);     Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.40s |
| cargo test --workspace | PASS | test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s;  |
| extension tests (npm test) | PASS | ? todo 0; ? duration_ms 183.795 |
| npm launcher tests (packaging/npm) | PASS | ? todo 0; ? duration_ms 94.0563 |
| MCPB launcher tests | PASS | ? todo 0; ? duration_ms 56.2844 |
| shell script syntax (sh -n scripts/*.sh) | SKIP | no shell on this host; CI runs sh -n |
| isolated workspace build (F:\Replica\NAS\Files\repo\github\sylin-org\browser-mcp\.target-release-1.3.3) | PASS |    Compiling webview2-com v0.38.2;     Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 24s |
| machine registration snapshot | PASS | real native-host registration captured before the journeys |
| process journey | PASS | [ghostlight.exe] Ghostlight 1.0 ready on local ports 59792 and 59793; process journey ok: reconnect -> open/read/find/flow(execute/article/tree/wheel/upload/drop/guarded) -> screenshot/region/chain -> recording -> close -> pinned no-adapter refusal |
| CLI journey | PASS | [ghostlight] Ghostlight 1.0 ready on local ports 57750 and 57751; cli journey ok: demand-free call -> governed result -> cli-attributed audit -> batch session -> channel refusal |
| CLI PowerShell journey | PASS | ; powershell journey ok: separate processes, one session, open/list/read/capture/close |
| workbench surface | PASS | ; workbench surface ok: a broken panel costs its own panel and nothing else |
| machine state guard (registration + leaked processes) | PASS | registration unchanged; no process leaked from the isolated target |
| policy grammar | PASS | ; policy grammar ok: the readback matches what the matcher does |
| capability matrix (behavior evidence map) | PASS | capability matrix ok: 21 COMPLETE rows, 4 SUPERSEDED rows, all evidenced |
| JavaScript syntax (ui/app.js, preview server) | PASS |  |
| freeze binding (docs/release/freeze.json) | PASS |  |
| dependency gates (deny licenses/bans/sources + audit) | PASS | bans ok, licenses ok, sources ok; URL:       https://rustsec.org/advisories/RUSTSEC-2024-0429;  |

## Rows outside this runner (complete by hand or in CI)

| Row | Where it runs |
| --- | --- |
| Dependency license/ban/source/advisory detail | CI dependency gate on the frozen revision |
| Repository truth, documentation links, ASCII policy | CI repository-integrity job |
| Complete 0.8 recovery disposition | tracked matrix plus release-environment lanes |
| Clean Windows/Linux install, upgrade, uninstall | release-environment machines (owner) |
