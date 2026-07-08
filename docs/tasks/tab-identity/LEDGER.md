# tab-identity batch -- LEDGER

Durable execution record. One task = one code commit + one ledger commit. Update after EVERY
task (or block); this file is the single source of truth for batch progress.

## RESUME HERE

Next task: **T3** (`T3-stable-session-guid.md`). Base: T2 landed at `293dfd1`.

## Task table

| Task | Status | Code commit | Notes |
|---|---|---|---|
| T1 managed-surface predicate | done | 31049f2 | |
| T2 down-classifier | done | 293dfd1 | |
| T3 stable session guid | pending | - | |
| T4 envelope guid + session ops | pending | - | |
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
