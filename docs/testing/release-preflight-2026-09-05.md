# Release preflight -- b00199889dacf61a4b1f91e36a7024403f09a410

```text
date_utc: 2026-09-05T20:52:49Z
source_revision: b00199889dacf61a4b1f91e36a7024403f09a410
tree_dirty: true
toolchain: rustc 1.95.0 (59807616e 2026-04-14); node v24.7.0
host: Microsoft Windows 10.0.26200
```
Windows source preflight for service 1.3.4 and Chrome adapter 1.1.1. The frozen source is
`768ee7383da1988a2d6b0217812e23d3fe580680`; this preflight head adds only its freeze declaration.
The dirty-tree flag is the pre-existing untracked `.zcode/` session cache. No tracked product or
packaging file was dirty. The runner passed 18 stages, failed none, and deferred shell syntax to
the Linux release workflow because this host has no `sh` on PATH.

## Fringe-stability review

The local-destination fix stays inside the orchestrator's governance decision and effective-policy
projection. It removes address-specific restrictions; neither connector, the shared bridge, nor
the extension changes for that fix. Relative to public 1.3.2, the inherited 1.3.3 changes touch the
bridge only at its negotiated browser-capability seam and the extension only for physical
composed-DOM observation, frame routing, geometry, and their tests. Both connectors remain
unchanged. The extension remains policy-free.

## Checks completed around this runner

- The workspace suite has 437 passing Rust tests. The extension, npm launcher, and MCPB suites
  have 171, 10, and 4 passing tests respectively.
- The process journey proves local HTTP(S) open/read with an explicit localhost policy denial
  as its negative control, then completes the existing reconnect and browser mechanism journey.
- CLI, PowerShell, policy grammar, and workbench journeys pass. Native-host registration is
  unchanged, and no process leaked from the isolated target.
- Dependency license, ban, source, and advisory gates pass with the existing 17 accepted
  GTK/Tauri-chain allowances.
- Offline public truth checks distinguish source 1.3.4 from the still-public service 1.3.2 and
  adapter 1.0.0. Repository integrity and documentation links pass; the 26 historical ASCII
  exceptions are unchanged.
- Publication access is valid for GitHub, npm, Chrome API V2, and the MCP Registry. Adapter 1.1.1
  has cleared staged review; its ZIP is unchanged from the held 1.3.3 candidate.

## Active live graph

The 1.3.4 orchestrator was deployed through `scripts/dev-loop.ps1 -Action Deploy -NoStart`.
Existing connectors demand-started the replacement and reconnected without replacement. The
live executable and isolated release build have the same SHA-256:
`4552c35cb6df93584d5211d236b0ce5d7a01648737f13e6e76b6aa48c939c816`.

`tests/live-journey.mjs`, pinned to the deployed release directory, passed localhost and literal
loopback HTTP open/read, explicit local host denial, composed document read/inspect/find/wait,
framed hover, target screenshot, semantic form fill and completion, full screenshot, and region
and chained-region screenshots. The browser's preserve-tabs setting kept the final demo tab as
expected. The already-connected MCP tool returned the new effective-policy projection with zero
policy layers and only the non-HTTP(S) scheme ceiling. This is a development-host browser proof;
it does not claim a new clean-machine installed-product lane.

## Stage results

| Stage | Result | Detail |
| --- | --- | --- |
| cargo fmt --all -- --check | PASS |  |
| cargo clippy --workspace --all-targets -D warnings | PASS |     Checking ghostlight v1.3.4 (F:\Replica\NAS\Files\repo\github\sylin-org\browser-mcp\crates\orchestrator);     Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.20s |
| cargo test --workspace | PASS | test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s;  |
| extension tests (npm test) | PASS | ? todo 0; ? duration_ms 235.2667 |
| npm launcher tests (packaging/npm) | PASS | ? todo 0; ? duration_ms 114.0611 |
| MCPB launcher tests | PASS | ? todo 0; ? duration_ms 60.8066 |
| shell script syntax (sh -n scripts/*.sh) | SKIP | no shell on this host; CI runs sh -n |
| isolated workspace build (F:\Replica\NAS\Files\repo\github\sylin-org\browser-mcp\.target-ghostlight-1.0) | PASS |    Compiling ghostlight-mcp-connector v1.3.4 (F:\Replica\NAS\Files\repo\github\sylin-org\browser-mcp\crates\mcp-connector);     Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.41s |
| machine registration snapshot | PASS | real native-host registration captured before the journeys |
| process journey | PASS | [ghostlight.exe] Ghostlight 1.0 ready on local ports 50781 and 50782; process journey ok: reconnect -> open/read/find/flow(execute/article/tree/wheel/upload/drop/guarded) -> screenshot/region/chain -> recording -> close -> pinned no-adapter refusal |
| CLI journey | PASS | [ghostlight] Ghostlight 1.0 ready on local ports 57511 and 57512; cli journey ok: demand-free call -> governed result -> cli-attributed audit -> batch session -> channel refusal |
| CLI PowerShell journey | PASS | ; powershell journey ok: separate processes, one session, open/list/read/capture/close |
| workbench surface | PASS | ; workbench surface ok: a broken panel costs its own panel and nothing else |
| machine state guard (registration + leaked processes) | PASS | registration unchanged; no process leaked from the isolated target |
| policy grammar | PASS | ; policy grammar ok: the readback matches what the matcher does |
| capability matrix (behavior evidence map) | PASS | capability matrix ok: 21 COMPLETE rows, 4 SUPERSEDED rows, all evidenced |
| JavaScript syntax (ui/app.js, preview server) | PASS |  |
| freeze binding (docs/release/freeze.json) | PASS |  |
| dependency gates (deny licenses/bans/sources + audit) | PASS | bans ok, licenses ok, sources ok; ; warning: 17 allowed warnings found |

## Evidence outside this runner

| Row | Result and scope |
| --- | --- |
| Linux source and shell syntax | PASS in release run 33991341425 at the exact frozen source |
| Repository truth, documentation links, ASCII policy | PASS in ordinary CI and the candidate quality gate |
| Historical 0.8 recovery | Retained in Git history; the current release uses repository integrity and candidate package checks |
| Debian install, upgrade, uninstall | PASS on Debian 12 and Ubuntu 24.04 in the candidate workflow |
| Windows public launcher | PASS against published 1.3.4; all three binaries verified and doctor returned the expected version |
| Clean installed Windows/Linux desktop lanes | Not repeated for this patch; source, development-host, launcher, and CI package proofs do not claim those physical lanes |

See the [custody and publication record](candidate-custody-2026-09-05.md) for public artifact hashes,
channel reconciliation, and website verification.
