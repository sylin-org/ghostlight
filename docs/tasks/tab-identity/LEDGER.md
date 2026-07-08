# tab-identity batch -- LEDGER

Durable execution record. One task = one code commit + one ledger commit. Update after EVERY
task (or block); this file is the single source of truth for batch progress.

## RESUME HERE

Next task: **T5** (`T5-client-name-titles.md`). Base: T4 landed at `7ee9b06`.

## Task table

| Task | Status | Code commit | Notes |
|---|---|---|---|
| T1 managed-surface predicate | done | 31049f2 | |
| T2 down-classifier | done | 293dfd1 | |
| T3 stable session guid | done | fb88795 | build-order note (deviation 1) |
| T4 envelope guid + session ops | done | 7ee9b06 | deviations 1-3 |
| T5 client-name titles + errors | pending | - | |
| T6 liveness + pruning + changelog | pending | - | |

## Per-task log

(Append one entry per task: commit hash, verification results, and EVERY deviation from the task
file/PINS, numbered. A BLOCKED entry carries the failed precondition or error text verbatim and
your reasoning, then the batch HALTS per BOOTSTRAP.)

### T1 -- managed-surface predicate (ADR-0047 D1) -- DONE

- Code commit: `31049f2`.
- STOP preconditions: both passed (all anchors present verbatim; `GhostlightGrouping` did not yet
  export `managedGroupIds`/`isManagedGroupId` -- grep found no matches anywhere under extension/).
- Changes made exactly per PINS P1: added `managedGroupIds` + `isManagedGroupId` pure fns and
  extended the export object in `grouping.js`; rewrote the stale "additive/never touched" header
  claim and the stale `sessionGroups` comment to cite ADR-0047 D1; rewired `service-worker.js`
  destructure line, `groupTabs` body (union over managed ids), and `inGroup`'s final membership
  line to `isManagedGroupId(...)`; require line + two pinned tests appended to the test file.
- Verification (V-ALL, all green): `node --check` on both JS files OK; `node --test
  tests/extension/grouping.test.js` = 3 pass (the 2 new + the pre-existing); `cargo fmt --check`
  OK; `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo test --workspace
  --no-fail-fast` = 43 suites `test result: ok`, 0 failed; `cargo check --target
  x86_64-unknown-linux-gnu --workspace --all-targets` OK. All three edited files verified pure
  ASCII.
- Deviations from task/PINS: NONE.
- Note (not a deviation): git emitted the usual "CRLF will be replaced by LF" advisory for
  `service-worker.js` -- a pre-existing repo line-ending condition, no content impact.

### T2 -- relay down-classifier (ADR-0047 D6) -- DONE

- Code commit: `293dfd1`.
- STOP preconditions: both passed. The `down` arm text matched the quoted block verbatim
  (`let down = async { match tokio::io::copy(ipc_read, client_out).await { ... } }`);
  `grep -rn "copy_service_to_client" crates/ src/` returned nothing. `RelaySide` confirmed a
  two-variant enum; a `#[cfg(test)] mod tests` already existed (no structural addition needed).
- Changes exactly per PINS P2: added the private `copy_service_to_client` async fn (doc comment
  verbatim) right after `relay_session`; replaced the `down` arm with
  `let down = copy_service_to_client(ipc_read, client_out);`; appended the three pinned
  `#[tokio::test]`s with local `FailingReader`/`FailingWriter`; APPENDED the ADR-0045 amendment
  section `## Amendment (2026-07-08, ADR-0047 D6): down-relay error classification` (existing
  lines untouched).
- Test-assertion mechanism: used `assert!(matches!(..., RelaySide::ServiceClosed))` /
  `RelaySide::ClientClosed` rather than `assert_eq!`, since `RelaySide` derives neither
  `PartialEq` nor `Debug` and the task fences forbid unrelated changes; `matches!` satisfies the
  pinned "returns RelaySide::X" assertion without touching the enum. (Judgment-free: the pin
  states the expected variant, not the macro.)
- Verification (all green): `cargo fmt --check` OK; `cargo clippy --workspace --all-targets --
  -D warnings` exit 0 (verified via exit code, not just tail); `cargo test -p
  ghostlight-transport` = 60 passed incl. the 3 new; `cargo test --workspace --no-fail-fast` = 43
  `test result: ok`, 0 failed, `adapter_reconnects_across_a_service_restart_without_a_client_reload`
  green; `cargo check --target x86_64-unknown-linux-gnu --workspace --all-targets` OK. Both files
  pure ASCII.
- Deviations from task/PINS: NONE beyond the `matches!` choice noted above (a transcription
  choice, not a semantic deviation).

### T3 -- stable per-process SessionGuid (ADR-0047 D2) -- DONE

- Code commit: `fb88795`.
- STOP preconditions: all passed. Every anchor present; `grep "fn adapter_hello"` empty;
  `spawn_adapter` did NOT set `GHOSTLIGHT_DEBUG` (the line-60 `GHOSTLIGHT_DEBUG` belongs to
  `service_cmd`), so per the task's sanctioned edit I added `.env("GHOSTLIGHT_DEBUG", "1")` to
  `spawn_adapter`. Confirmed constants for the pinned test: `HUB_PROTO: u32 = 1`,
  `ROLE_ADAPTER = "adapter"`.
- Changes exactly per PINS P3: extracted `adapter_hello(guid)`; `try_connect_once` gained the
  `guid` param and dropped its local mint; `connect_and_handshake` gained the `guid` param and
  threads it to both call sites; `relay_adapter` mints ONE guid before the loop, emits the pinned
  note, and passes `&session_guid` into `connect_and_handshake`; rewrote the two stale
  doc-comment passages to cite ADR-0047 D2; added `hello_carries_the_caller_guid`; extended the
  restart integration test (mint-note count == 1, reconnect-note count >= 1) leaving the 5s-gap
  test untouched; APPENDED the ADR-0045 D2 amendment.
- DEVIATION 1 (verification-recipe gap, worked around; NOT a code change): the pinned T3
  verification lists `cargo test --test adapter_reconnect` and `cargo test --workspace` but NONE
  of the pinned commands rebuild the DELIVERABLE `target/debug/ghostlight-adapter-agent.exe` that
  `adapter_bin()` spawns by PATH (it is not referenced via `CARGO_BIN_EXE_*`, unlike the
  `ghostlight` bin). `cargo test --workspace` builds each crate's TEST harness, not the sibling
  deliverable bin, so the reconnect test first ran a stale (pre-T3) adapter and my new mint-note
  assertion failed (observed left:0 right:1; the surviving log_dir's adapter events file had the
  old notes but not the mint note; the on-disk exe was timestamped 16:10 and did not embed the
  new string). Fix: ran `cargo build --workspace` to refresh the deliverable bins, after which
  `cargo test --test adapter_reconnect` = 2 passed and `cargo test --workspace` = all green. The
  code is correct as pinned; only an extra `cargo build --workspace` step is needed before the
  reconnect test. RECOMMENDATION for the batch author: add `cargo build --workspace` to the T3
  verification block ahead of the reconnect test.
- Verification (all green after the build step): `cargo fmt --check` OK; clippy exit 0;
  `cargo test -p ghostlight-transport` = 61 passed incl. `hello_carries_the_caller_guid`;
  `cargo test --test adapter_reconnect` = 2 passed (mint-note + reconnect assertions live);
  `cargo test --workspace --no-fail-fast` = 43 `test result: ok`, 0 failed;
  `cargo check --target x86_64-unknown-linux-gnu --workspace --all-targets` OK. All three files
  pure ASCII.

### T4 -- guid on the tool envelope + session-scoped tab operations (ADR-0047 D3) -- DONE

- Code commit: `7ee9b06`. Nine owned source files, all pure ASCII.
- STOP preconditions: all passed. `grep '"guid"' browser.rs` matched only `request_group`/its
  doc + tests (the `tool_request` envelope in `call` had NO guid); `structuredContent` absent
  from server.rs; `LocalCtx` had exactly the five listed fields; `JsonRpcResponse.result` is
  `pub result: Option<Value>` (value-inspectable); the extension handler anchors were intact.
- Changes per PINS P4: `Browser::call(guid, tool, args)` + `"guid"` on the envelope;
  `run_tool_call`/`handle_tools_call` gained `guid: &str` after governance; `LocalCtx.guid`;
  `script.rs` PipelineRunner+bridge threading; `form_fill.rs` `run(.., guid, ..)` + 3 call sites;
  `server.rs` `SessionSeat` + `serve_session` seat + `handle_line(seat)` + the tabs_create
  response-claim spawn; `endpoint.rs` envelope oracle (`v["guid"] == "test-guid"`);
  `hub_multiplex.rs` `"session-a"`/`"session-b"`; extension `dispatch(.., guid)`,
  `tabsCreateLegacy`/`tabsContextLegacy` module fns, session-scoped `tabs_create_mcp`/
  `tabs_context_mcp`, `createTabInSessionGroup`, widened `tabContext(tabs, reportGroupId)`.
- DEVIATION 1 (mechanically forced): adding `guid` pushed `run_tool_call` and
  `futures_await_block` from 7 to 8 params, tripping `clippy::too_many_arguments` under
  `-D warnings`. Added `#[allow(clippy::too_many_arguments)]` (with a citing comment) to BOTH --
  the same sanctioned pattern the codebase already uses for `large_enum_variant` in server.rs
  (a pinned signature moves by transcription, not reshaping). No signature reshape.
- DEVIATION 2 (threading beyond the two explicitly-named pipeline call sites): the second
  production `browser.call` PINS references as "the navigate-landing region" lives in the helper
  `post_navigate_landing_check`; threaded a `guid: &str` param into it (and its run_tool_call
  call site) to reach that call. This is the pinned "thread the real guid; the compiler finds
  every site" path, not a new decision.
- DEVIATION 3 (bulk test-site mechanization): the P4 BLANKET TEST RULE names `"test-guid"` for
  every compile-flagged test `.call`/`handle_tools_call` site. pipeline.rs had 30 such
  `handle_tools_call` test calls (2 via aliased `&enforce_governance`/`&observe_governance`
  vars); inserted `"test-guid"` via one scripted regex pass (`&\w*governance,(\s+)Some\(` ->
  insert `"test-guid",`), verified count == 30 and 0 remaining, and confirmed the file stayed
  LF-only (no CRLF flip) with a localized diff. browser.rs's own 11 test sites + endpoint.rs +
  hub_multiplex done per the pinned literals.
- Also ran `cargo fmt` to normalize the new code (standard) and `cargo build --workspace` before
  the integration tests (the T3 deliverable-bin lesson; the pinned `cargo test` commands do not
  rebuild path-spawned sibling bins).
- Verification (all green): `node --check` OK; `node --test grouping.test.js` 3 pass; `cargo fmt
  --check` OK; clippy exit 0; `cargo test --workspace --no-fail-fast` = 43 `test result: ok`, 0
  failed; guardrails explicitly re-run green -- `all_open_golden` 3 (byte-identity intact),
  `tool_schema_fidelity` 10 (sacred surface intact), `serve_bridges_a_tool_call_over_the_real_ipc`
  ok (envelope-guid oracle), `hub_multiplex` 3, `hub_isolation` 2, `hub_queue` 2; `cargo check
  --target x86_64-unknown-linux-gnu --workspace --all-targets` OK.
