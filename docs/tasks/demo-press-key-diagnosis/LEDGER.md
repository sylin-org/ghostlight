# Foundry demo press_key diagnosis

Status: IN PROGRESS. Started 2026-08-24. This ledger is the authority on where the
investigation stands. It records what was eliminated, what was established, and the exact
next step, so any agent can resume after a context wipe.

## The question

`scripts/demo-foundry.ps1` fails deterministically at the `key to end` beat
(`browser_press_key`, key End, target the Release name textbox) with status failed and the
summary "The browser disconnected before anything happened." Standalone press_key calls,
short repros, and big-GIF repros all pass. Four-plus failures, always the same beat, at
multiple pacings.

## MACHINE-LOCAL STATE -- read before resuming (as of 2026-08-24 ~10:50 local)

- The live authority has been swapped for diagnosis: `target/release/ghostlight.exe` is an
  instrumented dev build. Pristine backup: `target/release/ghostlight.exe.pre-diag.bak`.
  RESTORE THE BACKUP BEFORE ANY RELEASE WORK OR COMMIT OF BINARIES.
- Temporary source instrumentation (marked `TEMPORARY DIAGNOSTIC` in comments) exists in
  `crates/orchestrator/src/browser/mod.rs` and `crates/orchestrator/src/work/mod.rs`.
  Revert with `git checkout -- crates/orchestrator`. NEVER COMMIT IT.
- Throwaway scripts live outside the tree in `C:/Users/onose/AppData/Local/Temp/opencode/`:
  `demo-foundry-probe.ps1` (story variant: frame + zoom beats removed, probe scroll +
  roster read before press_key, timestamps, prints FACTS on unexpected status) and
  `poll-relay.ps1` (concurrent roster poller). Trace log: `liveness-trace.log` there too.
- One authority, one browser: Google Chrome, id `browser_c21b2f55e68e4276b6c35b02efde0beb`,
  attended. The extension reconnects to a restarted authority automatically within seconds
  and keeps the same browser id.

## Eliminated hypotheses (with evidence)

1. Two authorities / PATH mismatch between demo and probes. One authority; demo resolved
   `target/release` (first existing candidate).
2. A second connected browser serving polls while the demo pins another. Roster shows
   exactly one browser at all times.
3. Transport dead at the failure moment. Concurrent polls succeeded 190 ms before and after
   failures. Caveat learned later: `browser_tabs list` never crosses the liveness gate or
   the wire, so polls only prove the authority process was up.
4. Window-resize and zoom beats prime the failure. Probe variant with both removed still
   fails at the same beat.
5. GIF size / save-replay encode teardown. A 13-second replay repro passed.
6. Discard teardown. `-KeepRecording` still fails.
7. Liveness machinery (stale flag, 45 s ack timeout, heartbeat ticks). Instrumented trace
   across a full failing run: zero stale transitions, every ack within 1.2 s, all ticks
   clean, connection healthy through and after the failure.
8. Workspace pin resolution (`choose_browser`). The traced refusal arm in
   `work/mod.rs::target_browser` never fired; in-session scroll + roster succeed
   milliseconds before press_key fails.
9. Browser identity change mid-story. Authority restart + reconnect kept the same id.

## Established facts

- The sentence "The browser disconnected before anything happened." is NOT evidence of
  disconnection. It is the fallthrough rendering at `work/mod.rs` (~line 1283):
  `Refusal::BrowserStopped` is produced for EVERY BrowserError that survives the routing,
  effect-unknown, and cancellation checks -- including `Primitive`, `Protocol`,
  `CapabilityVersion`, `RecoveryFailed`, and `DisconnectedAfterDispatch`.
- Instrumented run 2026-08-24 10:34:38: press_key dispatched three sub-commands; all three
  piggyback heartbeats were acknowledged within milliseconds; total orchestrator time was
  32 ms; CLI facts carried `"reason":"browser_primitive_failed"`.
  `browser_reason()` maps that string to `BrowserError::Primitive(_)` -- an ADAPTER ERROR
  FRAME from the extension. The extension processed the command and answered with an error.
- The extension's error message text is discarded end-to-end: `adapter_error` folds it into
  `Primitive(message)`, the terminal rendering emits only summary + reason string, and the
  audit record stores neither. Nobody can see what the extension actually said. This is a
  real product defect independent of the demo (finding #1).
- Earlier runs (previous session) showed CLI facts `"reason":"browser_disconnected"`,
  which maps ONLY to true `DisconnectedBeforeDispatch`. So there may be two distinct modes:
  an earlier genuine pre-disconnect mode, and tonight's primitive mode. Unverified whether
  they share a root cause.
- `browser_record save` crosses zero liveness probes during its 8-16 s encode: recording
  export does not ride `call_inner`.
- Controlled-tab listing is session/workspace-scoped, which is why concurrent poll sessions
  saw zero tabs while the demo had one open.
- Post-completion page state is the new suspect: press_key targets the Release name textbox
  AFTER `Complete release packet` transitioned the page. Every passing repro skipped the
  completion beats. Leading hypothesis: the extension legitimately errors on the key
  dispatch against post-completion page state (focus/interactability/target staleness),
  and the misleading fallthrough sentence disguised it as a disconnect.

## Next steps, in order

1. Add tracing of adapter error frames (code, message, effect_unknown) in
   `read_adapter`, and a Debug dump of the error in the work fallthrough terminal builder.
   Rebuild, redeploy over `target/release/ghostlight.exe`.
2. Minimal repro: drive the story through `complete release packet` standalone, then issue
   the identical press_key. Capture the extension's actual message from the trace.
3. Decide the fix split: (a) presentation must render `Primitive` honestly and carry the
   message; (b) whatever the extension-side cause turns out to be; (c) re-examine whether
   the earlier true-disconnect mode still exists once (a) stops masking everything.
4. Restore `target/release/ghostlight.exe.pre-diag.bak`, revert instrumentation, run gates
   (`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`, `npm test` from `extension/`, `node --check` on changed
   extension JS), commit fixes and docs separately.
