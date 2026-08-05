# Protocol-versioned MCP edge LEDGER

Durable progress for the ADR-0096 break-and-rebuild implementation.

## RESUME HERE

- State: ADR-0096 implementation, closed-transport recovery, and ADR-0098 browser-shore topology
  correction are complete and live-verified in the working tree
- Current phase: complete
- Next: owner review and intentional commit/release sequencing. The conformance-runner fork side
  quest remains paused; no fork or publication happened in this batch.

## Phases

| Phase | Scope | Status |
|---|---|---|
| P1 | Pin bridge, work, workspace, and catalog contracts | implemented |
| P2 | Neutral service execution, authority, and cancellation | implemented |
| P3 | `mcp_2025_11_25` and `mcp_2026_07_28` edge handlers | implemented and verified |
| P4 | Three-executable runtime, installer, and distribution cutover | implemented |
| P5 | Full verification, current-truth docs, and adversarial review | complete |

## Notes

- The repository already contains uncommitted MCPB and directory-submission work. This batch
  extends overlapping files in place and does not treat those edits as its own.
- The official conformance server runner currently accepts an HTTP URL, not a stdio command. It
  was not run against Ghostlight's shipping transport. Direct protocol evidence is immutable
  dated-schema/spec-driven review plus exact transcript, neutral-service, and real-process tests.
  Do not claim a conformance-runner result; add that external gate if the runner gains compatible
  transport support.
- The working tree contains architecture assertions for the exact three-executable topology,
  neutral service vocabulary, edge/core dependency direction, PID-free routing, browser-only
  relay, and removal of the old core `mcp` directory.
- The final naming amendment renames the two shores to `ghostlight-mcp-connector` and
  `ghostlight-browser-connector`, with matching crate directories. It adds no process, alias,
  protocol type, or state machine. The installer treats the immediately prior `ghostlight-mcp`
  path and the older `ghostlight-relay --role agent` path as stale migration inputs.
- The adversarial minimality pass removed the process-global role marker. Executable entry points
  and crate dependencies now carry that boundary without a second runtime identity mechanism.
- The adversarial runtime pass found and closed late-settlement and browser-shore races without a
  new job/result subsystem. The same per-call future drains after one outward `outcome_unknown`;
  existing pending and scheduler state carry exact executor-generation quarantine proof; one
  safety lock orders hold/panic against final enqueue; the browser writer is bounded; browser
  restart purges stale workspace tab ownership; and stale detach cannot erase replacement focus.
- Browser extension routing uses only `WorkspaceId` in the compatibility `guid` field. Human
  labels are presentation/audit data, current tool/group frames omit the former top-level
  presentation/routing `clientKey`, and two workspaces with the same client name stay isolated.
  A nested scheduler resource may retain the old `clientKey` wire spelling for adapter skew, but
  its value is `WorkspaceId`, not the human label.
- Input tab ids are verification-only. Unknown and cross-workspace ids fail identically before a
  browser frame. Only exact, successful creator results may atomically add membership, and that
  settlement happens synchronously at the browser shore before the result escapes. The Lightbox
  fixtures now earn their tab inventory through that path instead of inventing ids.
- The relay restart red-team reproduced an intermittent Windows stale-pipe opening failure. Dial,
  relay hello, and cached browser identity are now one bounded reconnect attempt. A deterministic
  transport regression and 12 consecutive real three-executable restart scenarios cover it.
- The live connector-name cutover exposed a distinct host boundary: an MCP client may retain
  cached tool declarations after its host-owned stdio connector exits. The date-named MCP shores
  now carry one shared recovery instruction, install names the reconnect requirement, and doctor
  reports the existing aggregate live-edge count without inventing per-client state. A standalone
  connector is not recovery for a closed client transport and may create a different workspace.
- ADR-0098 supersedes ADR-0097's window-qualified implementation after the live test showed that
  moving a whole group invalidated the service's stale native-window assumption. The service now
  owns logical workspace and tab authority plus browser-profile routing only. The extension owns
  all live Chrome placement, follows user moves, and carries one workspace topology record. The
  asynchronous `group_request` path, service-side native-window state, stale-window retry, and the
  separate extension grouping module are removed.

## Final gate evidence (2026-08-04)

- `cargo fmt --all -- --check`, strict locked workspace clippy, and a locked workspace build pass
  in the isolated `.target-adr96` target directory. The only three product executables are
  `ghostlight`, `ghostlight-mcp-connector`, and `ghostlight-browser-connector`.
- `cargo test --locked --no-fail-fast --workspace` passes. Notable suites include 11 architecture
  boundary tests, 728 core tests, 48 MCP-edge tests, and 76 transport tests.
- Lightbox passes all 31 real-process scenarios. The repaired inventory scenarios cover
  redaction, late extension wait, parent audit, and two-client multiplex; browser relay restart
  passes in the full run and in the 12/12 stress run above.
- The extension suite passes 164 tests. The npm launcher passes 7, the MCPB launcher passes 5, and
  Anthropic's pinned `@anthropic-ai/mcpb@2.1.2` validator accepts the manifest.
- Public-surface consistency, changed JavaScript syntax, PowerShell parsing, `scripts/get.sh`
  parsing, changed/new JSON parsing, `git diff --check`, and tracked/untracked ASCII scans pass.
- The official MCP conformance server runner remains HTTP-URL-only and was not claimed as stdio
  evidence. No commit, release, registry publication, directory post, or external comment was
  made by this implementation batch.
- The connector-name follow-up passes strict workspace Clippy, the full Rust workspace, all 31
  Lightbox scenarios, 164 extension tests, 7 npm tests, 5 MCPB launcher tests, Anthropic's pinned
  MCPB validator, formatting, syntax, and diff hygiene. A machine-local install cutover and live
  transcript are recorded only in `local/MACHINE-STATE.md`.
- The closed-transport recovery follow-up passes formatting, strict workspace Clippy, the complete
  fast-tier Rust workspace, 48 focused MCP-edge tests, 28 focused doctor tests, and 13 focused
  installer tests. A read-only live doctor probe rendered one connected browser, two aggregate
  point-in-time MCP edges, and no false service orphan/client attribution. The affected Codex edge
  was not replaced with a standalone connector; its client-owned reconnect remains the live gate.
- The ADR-0097 follow-up passes all 168 extension tests, changed JavaScript syntax checks, strict
  workspace Clippy, and the complete Rust workspace suite. The fresh service build is live and
  healthy. The first live pass exposed and then closed asynchronous group-id replacement: adoption
  now occurs only before a workspace has a live mapping, and workspace labels are first-capture-
  wins. The corrected service is redeployed; the second pass awaits an extension reload.

## ADR-0098 gate evidence (2026-08-05)

- Focused browser-topology tests pass 14/14 and the complete extension suite passes 158 tests after
  removing the obsolete grouping module and its ten tests. Changed JavaScript syntax checks pass.
- Focused core workspace tests pass 10/10. Formatting, diff hygiene, strict locked workspace
  Clippy, the complete locked Rust workspace suite, and all 31 real-process Lightbox scenarios
  pass. The repository release build is live and healthy.
- After the unpacked extension reload, `tabs_context_mcp(createIfEmpty:true)` created tab
  5541182382 in group 399144999. The user moved the entire group to another Chrome window. One
  `tabs_context_mcp(createIfEmpty:false)` call returned the exact same tab and group, proving that
  the browser shore followed live placement without creating another artifact or asking the
  service to recover a native-window pin. A subsequent addressed `navigate` reached that same tab
  and loaded `https://example.com/` successfully.
