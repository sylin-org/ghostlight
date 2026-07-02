# Release-1 execution ledger

This file is the working memory of the unattended run defined in BOOTSTRAP.md.
The agent updates it before and after every task and commits it with each
task's changes. Humans read it to understand exactly what happened.

## RUN SUMMARY

Run completed 2026-07-02. All 18 tasks in the fixed sequence (T04, T06, T07, T01, T02, T03, T12,
T13, T14, T15, T08, T09, T10, T11, T18, T16, T17, T05) reached status `done`. Zero tasks blocked.

- Tasks done: 18 of 18 (100%). See the sequence table below for the full list; see the Task log
  for one entry per task with files touched, tests added, drift reconciled, and decisions made.
- Tasks blocked: none.
- Total commits made during this run: 18 one-per-task commits (T04 through T05, in execution
  order, all on branch `release-1-hardening`, none on `main`) plus this one
  (`chore(ledger): run summary`), for 19 total. Verified via
  `git log --oneline main..release-1-hardening` (18 feat commits before this summary commit) and
  `git log --oneline release-1-hardening ^main | wc -l` (18, matching).
- Quality gate re-verified at completion time (not just trusted from task logs): `cargo test`
  (91 tests: 80 unit + 4 mcp_protocol + 1 peer_death + 6 tool_schema_fidelity, all passing),
  `cargo clippy --all-targets -- -D warnings` (clean), `cargo fmt --check` (clean except the two
  pre-existing drifted files noted below, unchanged from every task's own log).
- Working tree: clean at completion (aside from the pre-existing, out-of-scope, untracked
  `docs/tasks/stage-2/*.md` files, which this run never touched, staged, or committed, per
  explicit instruction and ADR-0018).

### Anything a human must decide in the morning

1. **Pre-existing `cargo fmt` drift** in `src/policy/redact.rs` and `tests/tool_schema_fidelity.rs`
   (both reformat under the installed rustfmt 1.9.0, likely a rustfmt-version difference from
   whenever those two files were last formatted). No task in this run touched either file's
   substance; both were deliberately left as-is every time `cargo fmt` was run as a side effect of
   formatting other files, per the "one task = one commit, do not mix unrelated changes" rule. A
   human may want to run a dedicated repo-wide `cargo fmt` in its own commit at some point.
2. **T07's row-cap ambiguity** (see T07's own "Decisions made" log entry): whether the doctor
   subcommand's 6-row non-verbose session cap counts "(skipping unreadable state file: ...)" lines
   toward the cap the same as parsed rows. Implemented conservatively (cap applies to the first 6
   files in the newest-first list, parsed or unreadable; the trailing "(and N older...)" note only
   counts additional successfully-parsed sessions). Not unit-tested (out of the prompt's own
   required test list). A human who wants stricter/different row-cap behavior should treat this as
   a follow-up, not a bug.
3. **Every byte-exact string/format contract introduced by this run** (T01's three marker-line
   formats, T02's viewport-culling Note line, T03's get_page_text contract, T06's
   `[hop: ...] ... Next step: ...` format, T08's type-dispatch event contract, T09's click-event
   contract, T10's scroll-verify result strings, T11's zoom-result contract, T13's exception-text
   format, T14's network per-line format, T15's six zero-result strings, T18's background-capture
   contract, T16's javascript_tool contract, T17's tabId-fallback/valid-IDs messages, T05's two
   buffer-reset notice lines) is now load-bearing for the deferred browser tests in
   BROWSER-TESTS.md. None of this was verified against a real Chrome instance during this run (no
   live browser was available, per BOOTSTRAP.md ground rule 3) -- the human running
   BROWSER-TESTS.md top to bottom in the morning is the FIRST real-browser verification any of this
   run's 18 tasks has received. Treat any mismatch found there as a real bug to fix, not as this
   run having been wrong to defer it.
4. **`browser-mcp doctor`'s exit code is now truthful** (0 = healthy, 1 = at least one finding),
   a behavior change from before T07 (previously always 0). Any script that shells out to
   `browser-mcp doctor` and ignored its exit code should be aware it can now be 1.

### Reminders before running BROWSER-TESTS.md

1. Restart the MCP client (the binary was rebuilt multiple times across this run; a stale
   in-process copy would not reflect any of these 18 tasks' changes).
2. Reload the extension at `chrome://extensions` (every task touching `extension/service-worker.js`
   or `extension/content.js` needs a fresh load of the unpacked extension; a service worker that is
   still running the old code will not exercise any of T01, T02, T03, T05, T08, T09, T10, T11, T12,
   T13, T14, T15, T16, T17, or T18's changes).
3. Then run `docs/tasks/release-1/BROWSER-TESTS.md` top to bottom, in the order its entries appear
   (T04-1 through T05-9, appended in task-completion order across this run). Each entry names the
   task it verifies, what changed, exact steps, and the expected result.
4. Do not proceed into `docs/tasks/stage-2/` after BROWSER-TESTS.md; governance is a separate
   staged run by explicit project decision (ADR-0018).

## RESUME HERE

- All 18 tasks (T04, T06, T07, T01, T02, T03, T12, T13, T14, T15, T08, T09, T10, T11, T18, T16,
  T17, T05) are done. Nothing left in the fixed task sequence.
- Branch: release-1-hardening (create from main if absent).
- Last commit: feat(extension): T05 service-worker state recovery (this run)
- NEXT ACTION for a future call: run BOOTSTRAP.md's "Completion" section (verify the tree is
  clean, every task row has a final state, write the RUN SUMMARY section at the top of this
  file, commit `chore(ledger): run summary`, then stop). Do NOT execute any of the 18 tasks
  again; they are all done.
- Open concerns: pre-existing `cargo fmt` drift (unrelated to
  T04/T06/T07/T01/T02/T03/T12/T13/T18/T16/T17/T05) in `src/policy/redact.rs` and
  `tests/tool_schema_fidelity.rs` -- both reformat under the installed rustfmt 1.9.0 but were left
  untouched again because they are out of scope / forbidden. A whole-repo `cargo fmt --check` will
  report these two files; this has no bearing on any task in this run (none of them touched
  either file -- confirmed again for T05 via `git status --short -- '*.rs' src/ tests/`, empty).
  A human may want to run `cargo fmt` repo-wide in its own dedicated commit at some point; the
  completion pass should mention this in the RUN SUMMARY as something for a human to decide.

## Sequence and status

Order: T04, T06, T07, T01, T02, T03, T12, T13, T14, T15, T08, T09, T10, T11, T18, T16, T17, T05.

| # | Task | Title | Depends on | Status |
|---|------|-------|-----------|--------|
| 1 | T04 | Extension-channel warmup + bounded first-call wait | - | done |
| 2 | T06 | Hop-attributed error reporting | T04 (binary half) | done |
| 3 | T07 | Extend installer doctor with runtime/debug-state fusion | - | done |
| 4 | T01 | read_page structural pagination + caps | - | done |
| 5 | T02 | read_page viewport culling (filter=interactive) | - | done |
| 6 | T03 | get_page_text official semantics | - | done |
| 7 | T12 | Per-domain console/network buffer reset | - | done |
| 8 | T13 | Runtime.exceptionThrown capture | - | done |
| 9 | T14 | Network.loadingFailed status | - | done |
| 10 | T15 | Empty-result guidance notes | - | done |
| 11 | T08 | type via real keyDown/keyUp | - | done |
| 12 | T09 | Mouse click fidelity (clickCount sequence, buttons, force) | - | done |
| 13 | T10 | Scroll verify + scrollable-ancestor fallback | - | done |
| 14 | T11 | Real zoom region crop + coordinate-context update | - | done |
| 15 | T18 | Background-tab screenshot via clip+scale | T11 helpful, not required | done |
| 16 | T16 | javascript_tool REPL semantics + 50KB cap | - | done |
| 17 | T17 | Effective-tabId fallback + valid-ID errors | - | done |
| 18 | T05 | Service-worker state recovery (runs LAST) | after all service-worker tasks | done |

Status values: pending, in_progress, done, blocked (with reason in the log).

## Task log

Append one entry per task using this template. Newest at the bottom.

```
### T<NN> <title> -- <done|blocked> -- <timestamp>
- Commit: <hash or n/a>
- Files touched:
- Tests added:
- Drift reconciled: (prompt facts that no longer matched the code, and what was actually true)
- Decisions made: (conservative choices taken without a human, and why)
- Notes for later tasks:
- Browser checks queued: (section ids added to BROWSER-TESTS.md, or none)
```

### T04 Extension-channel warmup + bounded first-call wait -- done -- 2026-07-02
- Commit: (recorded after commit; see git log for `feat(mcp): T04 ...`)
- Files touched: src/browser.rs, src/mcp/server.rs, tests/mcp_protocol.rs,
  docs/tasks/release-1/BROWSER-TESTS.md, docs/tasks/release-1/LEDGER.md
- Tests added:
  - src/browser.rs: `wait_connected_times_out_without_a_connection`,
    `wait_connected_wakes_when_the_extension_attaches`
  - tests/mcp_protocol.rs: `tools_call_waits_for_a_late_extension_and_notes_the_wait` (new);
    updated `initialize_tools_list_and_tool_call_over_stdio` to assert the exact bounded-timeout
    message instead of a substring match
- Drift reconciled: none of consequence. The prompt's line-number references had already drifted
  slightly from the working tree (e.g. exact line numbers for `is_connected`/`attach` moved by a
  few lines versus the prompt's "lines 64-66" etc.), but every function name, doc comment, and
  code shape the prompt described was present and matched; all snippets in the prompt were used
  essentially verbatim.
- Decisions made:
  - Left `src/policy/redact.rs` and `tests/tool_schema_fidelity.rs` untouched even though a
    repo-wide `cargo fmt` reformatted both (pre-existing rustfmt drift unrelated to this task,
    likely a rustfmt-version difference from whenever those files were last formatted). Reverted
    those two files with `git checkout --` after running `cargo fmt`, and verified fmt cleanliness
    on only the files this task actually touched via `rustfmt --check src/browser.rs
    src/mcp/server.rs tests/mcp_protocol.rs` (clean). `tests/tool_schema_fidelity.rs` was run
    unchanged via `cargo test` and still passes (6/6). See "Open concerns" above; flagging for the
    run summary / a human to decide on a dedicated repo-wide fmt pass later.
  - In `handle_tools_call`'s error arm, `append_wait_note` is called after the `isError` insertion
    (order between the two does not matter per the prompt; only that the note is the last content
    block, which it is since `isError` is a sibling key, not a content entry).
  - The new integration test's fake extension sleeps 1000ms (well inside the 5000ms
    `FIRST_CALL_WAIT_MS` window) before connecting, matching the prompt's spec exactly.
- Notes for later tasks:
  - T06 (hop-attributed error reporting, binary half) touches error text in the same call path
    (`handle_tools_call` / `Browser::call`); the bounded-wait timeout message and the
    `(waited N.Ns ...)` note are new text surfaces introduced here -- do not clobber their exact
    wording (tests assert on exact strings).
  - `run` in src/mcp/server.rs is now concurrent: `tools/call` responses arrive via a single
    writer task fed by an mpsc channel, and out-of-order arrival relative to other in-flight
    `tools/call`s is expected and correct (correlated by JSON-RPC id). Any future change to the
    read loop must keep funneling all stdout writes through that one writer task (constraint 13
    in this prompt).
  - Pre-existing `cargo fmt` drift in `src/policy/redact.rs` and `tests/tool_schema_fidelity.rs`
    remains unfixed (see Open concerns above); a future task that legitimately edits either file
    will likely have its own diff intermixed with this reformatting the moment it runs `cargo
    fmt` -- reconcile deliberately (keep only the lines relevant to that task's own change, or do
    a clean dedicated repo-wide fmt commit first) rather than accepting the reformat silently.
- Browser checks queued: T04-1, T04-2, T04-3 in docs/tasks/release-1/BROWSER-TESTS.md.

### T06 Hop-attributed error reporting across the full dispatch path -- done -- 2026-07-02
- Commit: (recorded after commit; see git log for `feat(mcp): T06 ...`)
- Files touched: src/error.rs, src/lib.rs, src/browser.rs, src/mcp/server.rs, src/mcp/tools.rs,
  src/native/messages.rs, extension/service-worker.js, tests/mcp_protocol.rs,
  docs/tasks/release-1/BROWSER-TESTS.md, docs/tasks/release-1/LEDGER.md
- Tests added:
  - src/error.rs (`tool_error_tests` module): one Display test per variant (extension, invalid-
    request, binary, ipc, cdp, page) checking the exact `[hop: ...] ... Next step: ...` text;
    `from_extension_wire` mapping tests for `Some("cdp")`, `Some("page")`, `None`, and an unknown
    hop string; a `next_step(...)` override test.
  - src/browser.rs: updated `call_surfaces_a_tool_error` and `call_without_a_connection_fails_fast`
    to assert the `[hop: extension]` prefix (in addition to their prior substring checks); added
    `call_surfaces_a_cdp_tagged_tool_error_without_leaking_detail` (asserts `[hop: cdp]`, the CDP
    method text, and that `detail` never appears in the rendered message) and
    `call_surfaces_a_page_tagged_tool_error` (asserts `[hop: page]`).
  - src/mcp/tools.rs: `is_known_tool_recognizes_advertised_names`,
    `is_known_tool_rejects_unknown_names`.
  - tests/mcp_protocol.rs: strengthened the no-extension assertion in
    `initialize_tools_list_and_tool_call_over_stdio` to check the exact new hop-attributed text
    (superseding the old "after 5s..." wording); added `unknown_tool_name_is_rejected_before_dispatch`
    (no extension connected, sends `tools/call` for `bogus_tool`, asserts `[hop: invalid-request]`
    + "Unknown tool: bogus_tool", and asserts the round trip took well under the 5s extension-wait
    window, proving the pre-check runs before the wait/dispatch).
- Drift reconciled:
  - The prompt's "Current behavior" section describes `Browser::call` and `attach` as they existed
    BEFORE T04 landed (e.g. it does not mention the bounded first-call wait T04 added in
    `handle_tools_call`, or the concurrent per-call spawn/writer-task architecture). All the error-
    mapping sites the prompt names (`Browser::call`'s four failure arms, the `attach` read loop,
    `route_reply`) matched the actual code exactly aside from this omission; every function name,
    line-content shape, and message string cited was present and correct once cross-referenced
    against the real file.
  - The prompt's own Verification step 3 ("Chrome closed, call any tool" -> exactly
    "[hop: extension] Browser extension not connected. Next step: ...") and its Tests section
    ("strengthen the existing no-extension assertion ... starts with [hop: extension]") only make
    sense if the T04 bounded-wait timeout branch in `handle_tools_call` (which the prompt's Current
    Behavior section never mentions) is ALSO folded into the new hop-error contract, not left as
    its bespoke "Browser extension not connected after {}s. Check that Chrome is running ..."
    text. Reconciled by removing that bespoke early-return entirely: when the bounded wait times
    out, `waited` simply stays `None` and control falls through to `Browser::call`, which (being
    genuinely unconnected) fails fast with the canonical `ToolError::extension("Browser extension
    not connected")` -- one hop-attributed message to maintain, not two. This exactly produces the
    prompt's canonical example string and needed no separate formatting logic in
    `handle_tools_call`. No extra latency: the fallthrough call fails immediately (`sent` is
    `false`), it does not wait a second bounded window.
- Decisions made:
  - `ToolError` derives `Clone` (prompt did not say either way). Needed so `attach`'s read-loop-end
    drain can fan the same error out to every pending caller without hand-rolling a clone helper
    (thiserror variants here are plain owned `String` fields, so `Clone` is free and does not
    change `Display`/`Error` semantics). Rejected alternative: re-render `.to_string()` and wrap in
    a fresh `ToolError::ipc(...)` per pending caller -- discarded because that double-wraps the
    `[hop: ...] ... Next step: ...` text (the constraint says the hop/message/next-step strings are
    an exact contract; nesting them would violate it).
  - `attach`'s reader loop distinguishes `Ok(None)` vs `Err(e)` by `break`-ing a `let drain_err = ...`
    value out of the loop and running ONE shared post-loop cleanup+drain block, rather than
    duplicating the `outgoing = None` / `set_connected(false)` / `connected.send_replace(false)` /
    `writer.abort()` bookkeeping in both arms (the prompt's own pseudocode showed them as separate
    inline blocks; consolidated for DRY-ness per this repo's coding-style rule, with identical
    observable behavior).
  - `handle_tools_call`'s new unknown-tool pre-check and the pre-existing error arm now share one
    `error_result(ToolError) -> Value` helper (builds the `{content, isError:true}` shape from
    `err.to_string()`) instead of duplicating the `text_content` + `isError` insertion inline
    twice; not mentioned by the prompt but a direct, low-risk simplification.
  - `src/native/messages.rs` and the module doc at the top of `src/browser.rs` were both updated to
    document the new optional `hop`/`detail` wire fields (the prompt only explicitly required the
    `src/native/messages.rs` change in step 7, but leaving `browser.rs`'s own doc comment
    describing the old error-only wire shape would have made the two docs disagree).
  - Content-script error message trimming in `form_input` (drop exactly one trailing period before
    handing the message to `hopError`, per the prompt) was implemented as
    `msg.endsWith(".") ? msg.slice(0, -1) : msg`; verified against content.js's actual
    `setFormValue` error text ("Element ref_N not found or was garbage-collected.") which does end
    in a period, so this path is exercised by the documented example.
  - Left `src/policy/redact.rs` and `tests/tool_schema_fidelity.rs` untouched again (same pre-
    existing rustfmt-version drift noted by T04); reverted both with `git checkout --` after
    `cargo fmt` reformatted them as a side effect of formatting the crate root. Verified fmt
    cleanliness on exactly the files this task touched with
    `rustfmt --check --edition 2021 src/browser.rs src/error.rs src/mcp/server.rs src/mcp/tools.rs
    src/native/messages.rs tests/mcp_protocol.rs` (clean; `--edition 2021` is required when
    invoking `rustfmt` directly on individual files, otherwise it defaults to the 2015 edition and
    fails to parse `async fn`). `src/lib.rs` was excluded from that direct-file check because
    passing a crate root (a file with `pub mod ...` declarations) to standalone `rustfmt` makes it
    recurse into every reachable module -- including the two drifted files -- so `src/lib.rs`'s
    one-line diff was instead verified by inspection (`git diff -- src/lib.rs`).
- Notes for later tasks:
  - The dispatch order in `handle_tools_call` is now: extract name/args -> unknown-tool pre-check
    (`ToolError::invalid_request`, no extension wait) -> `dispatch::policy_check`/`dispatch::audit`
    -> bounded extension-channel wait (falls through to `Browser::call` on timeout, no separate
    message) -> `Browser::call`. Any future change to this function must preserve that the unknown-
    tool check runs before ANY extension-channel interaction (a later task's own test asserts this
    via elapsed-time).
  - The `[hop: <hop>] <message>. Next step: <next step>.` format, the six hop names, and every
    default next-step string are now a byte-exact contract asserted by multiple tests in
    src/error.rs, src/browser.rs, and tests/mcp_protocol.rs. Do not casually reword any of them.
  - `extension/service-worker.js` now has a `hopError(hop, message, detail)` helper (near `fail`);
    any NEW content-script-backed or CDP-backed failure site added by a later task should use it
    (`"cdp"` for `chrome.debugger.*` failures, `"page"` for content-script/DOM failures) rather than
    throwing a plain `Error`, to keep failures hop-attributed. Untagged throws still work (they
    fall back to the `extension` hop via `dispatch`'s catch), but lose the more specific
    attribution.
  - `extension/content.js` was NOT touched (out of scope per the prompt); its own error message
    text (e.g. the `setFormValue` "not found or was garbage-collected." string) is now surfaced
    verbatim (minus one trailing period) as the `[hop: page]` message text for `form_input`. If a
    later task changes content.js's error strings, the trailing-period-trim behavior in
    `form_input`'s handler in service-worker.js should be re-checked against the new text.
  - Four new/updated BROWSER-TESTS.md entries (T06-1..T06-4) depend on a live browser; T04-2's
    expected text was also corrected in place (it documented the now-superseded "after 5s ..."
    wording) rather than left stale, since a human running the checklist top-to-bottom would
    otherwise hit a real mismatch there.
- Browser checks queued: T06-1, T06-2, T06-3, T06-4 in docs/tasks/release-1/BROWSER-TESTS.md
  (T04-2's expected text was also updated in place; see above).

### T07 Doctor subcommand fusing debug state into one diagnosis -- done -- 2026-07-02
- Commit: (recorded after commit; see git log for `feat(cli): T07 ...`)
- Files touched: src/debug.rs, src/mcp/server.rs, src/main.rs, src/native/ipc.rs,
  src/install/mod.rs, src/lib.rs, src/doctor.rs (new), docs/tasks/release-1/BROWSER-TESTS.md,
  docs/tasks/release-1/LEDGER.md
- Tests added:
  - src/debug.rs: updated both existing `DebugSink::enabled(&dir)` calls to
    `enabled(&dir, "mcp-server")`; added `enabled_sink_records_role_and_client` (asserts
    `snap["role"] == "mcp-server"` and `snap["client"] == "claude-code 1.2.3"` after `set_client`
    + `flush`).
  - src/native/ipc.rs: `probe_reports_absent_for_an_unused_endpoint` (plain `#[test]`, pid-unique
    endpoint), `probe_reports_accepts_against_a_live_server` (`#[tokio::test]`, spawns `serve`,
    polls `probe_endpoint` via `spawn_blocking` until `Accepts` or a 5s deadline).
  - src/doctor.rs (new, `#[cfg(test)] mod tests`): `all_healthy_observations_produce_no_findings`,
    `unregistered_browser_and_client_each_produce_their_own_finding`,
    `absent_with_no_sessions_fires_exactly_rules_3_and_7_in_order`,
    `rejects_embeds_a_known_pid_and_falls_back_to_process_manager_without_one`,
    `accepts_with_no_server_session_fires_rule_5`,
    `accepts_with_a_disconnected_extension_distinguishes_never_connected_from_dropped`,
    `parse_session_extracts_full_new_format_fields`,
    `parse_session_defaults_role_and_client_for_old_format_files`,
    `parse_session_returns_none_for_garbage_or_a_missing_pid` -- all 9 cover every case the
    prompt's Verification/unit-test list named.
- Drift reconciled: none of consequence. Every function/struct/line-content the prompt named in
  "Current behavior" (src/main.rs's `DoctorArgs`/role dispatch/`build_debug_sink`, src/debug.rs's
  private helpers and `Snapshot`/`Inner`, src/install/mod.rs's old `run_doctor`/`DoctorOptions`,
  src/native/ipc.rs's `serve`/`connect`/`socket_path`/`pipe_path`, src/mcp/server.rs's
  `initialize` arm) matched the working tree exactly; only exact line numbers had drifted by a
  few lines from T04/T06 landing first, as the prompt itself warned they would.
- Decisions made:
  - `status_report()`'s old "debug state at <path> is unreadable" failure text is retired (folded
    into the new "no mcp-server debug state under <dir> (state files exist for other roles or are
    unreadable)" message when no file both parses AND has an mcp-server-or-absent role). The
    prompt's Part A.7 names exactly two new failure texts and says "everything else... keeps the
    existing messages" -- read as: the old two-branch "is unreadable" message (there were two
    identical `return format!(...)` arms for read-failure vs parse-failure) is not one of the
    messages being kept, since the prompt's replacement logic no longer distinguishes "newest file
    unreadable" from "no candidate at all" -- both simply produce no candidate. Grepped the repo
    first to confirm no test asserts on the old "is unreadable" string; none does.
  - The Debug-sessions row cap ("show at most 6 session rows... if more were parsed, `(and <n>
    older...)`") is ambiguous about whether "rows" includes "(skipping unreadable state file: ...)"
    lines in the cap-of-6 count. Implemented: the cap of 6 (non-verbose) applies to the *first 6
    files in the newest-first list* (parsed or unreadable, one row each), and the trailing "and <n>
    older" note counts only *additional successfully-parsed sessions* beyond what was shown (i.e.
    total-parsed-across-all-files minus parsed-shown-within-the-cap) -- so a run of unreadable
    files near the cap boundary can silently drop a couple of skip-lines without a trailing note,
    but a real session is never silently dropped without being counted in "older". This is not
    unit-tested (the prompt's own unit-test list only requires `findings` and `parse_session`
    coverage, not row-cap rendering) -- flagging for a human/future task if stricter behavior is
    wanted. The "extension last seen" line always scans the FULL parsed list (not just the shown,
    possibly-capped rows), by design, so it never goes stale under the cap.
  - `EndpointProbe`'s doc comment adds "(see [`probe_endpoint`])" to the prompt's literal text;
    this is elaboration only (not one of the byte-exact-contract strings like `ToolError`'s
    `Display` text), so it is not a deviation from any tested/asserted string.
  - `browser-mcp doctor`'s Verdict "no debug instrumentation found" (rule 7) and the `Absent`/
    `Rejects` rules (3/4) are independent findings per the prompt's own text ("fires in addition to
    rule 3 or 4"); implemented as unconditional pushes in sequence, not an `else`, matching that
    literally -- verified by `absent_with_no_sessions_fires_exactly_rules_3_and_7_in_order`.
  - Left `src/policy/redact.rs` and `tests/tool_schema_fidelity.rs` untouched again (same pre-
    existing rustfmt-version drift T04/T06 flagged); reverted both with `git checkout --` after
    `cargo fmt` reformatted them as a side effect of formatting the crate root. Verified fmt
    cleanliness on exactly the files this task touched with `rustfmt --check --edition 2021
    src/debug.rs src/install/mod.rs src/main.rs src/mcp/server.rs src/native/ipc.rs src/doctor.rs`
    (clean); `src/lib.rs`'s one-line diff (`pub mod doctor;`) was verified by inspection instead
    (same crate-root caveat as T04/T06).
- Notes for later tasks:
  - `DebugSink::enabled` now takes `(dir: &Path, role: &'static str)`, not just `(dir: &Path)`.
    `DebugSink::set_client(&self, client: &str)` and `DebugSink::ipc_note(&self, summary: &str)`
    are new public methods (both force a snapshot write). `frame_in`/`frame_out` now also refresh
    `updated_ms` (throttled via the new private `Inner::touch`), so a session that is only relaying
    frames (no MCP requests) no longer looks stale in `status`/`doctor`.
  - `Snapshot`/state-file JSON gained two additive fields: `role` (always present, "mcp-server" or
    "native-host") and `client` (present only after `set_client` was called; omitted via
    `skip_serializing_if` otherwise). Any later task reading `debug-state-*.json` by hand (tests,
    tooling) should tolerate both fields being absent (old-format files) as well as present.
  - `crate::debug::{now_ms, fmt_ms, session_state_files}` are now `pub(crate)` (were private) --
    available to any future in-crate module, not just `doctor`.
  - `browser_mcp::install::run_doctor` and `browser_mcp::install::DoctorOptions` are GONE (moved to
    `browser_mcp::doctor::run` / `browser_mcp::doctor::DoctorOptions`). `browser_mcp::install::
    host_file_path` is now `pub(crate)` (was private) so `doctor` can reuse it; `yesno` was deleted
    from `install::mod` (only caller was the removed `run_doctor`) -- `doctor.rs` has its own
    private `yn` helper, not shared.
  - `native::ipc::relay_native_host` signature changed: `(endpoint: &str)` ->
    `(endpoint: &str, debug: &crate::debug::DebugSink)`. Any future caller (there is currently only
    `main::run_native_host_role`) must pass a sink (use `DebugSink::disabled()` if none is wanted).
    New public `native::ipc::{EndpointProbe, probe_endpoint, endpoint_display}` (per-platform
    `#[cfg(windows)]`/`#[cfg(unix)]` implementations, like `serve`/`connect`) are synchronous (no
    tokio) and safe to call from `doctor`'s non-async context.
  - `main::run_native_host_role` now takes `(debug: bool)` and `main::build_debug_sink` now takes
    `(debug: bool, role: &'static str)`. The native-host role's debug sink is genuinely env-gated:
    Chrome inherits its own launch environment and never passes `--debug` to the process it spawns,
    so a native-host `debug-state-<pid>.json` only appears when Chrome ITSELF was started with
    `BROWSER_MCP_DEBUG=1` in its environment -- doctor's rule set intentionally never treats a
    missing native-host row as a problem by itself (see `doctor::findings`, which has no rule keyed
    on native-host presence at all).
  - `browser-mcp doctor`'s exit code is now truthful (0 = healthy/no findings, 1 = at least one
    problem line), a behavior change from before (old `run_doctor` always returned `Ok(())` ->
    exit 0 unconditionally). Any script that shells out to `browser-mcp doctor` and previously
    ignored its exit code should be aware it can now be 1.
  - Six new BROWSER-TESTS.md entries (T07-1..T07-6) depend on a live browser + a real MCP client
    session; while inserting them, also moved the pre-existing T04-3 entry (which a prior run had
    left stranded after the T06 block, out of task order) to sit directly after T06-4 and before
    the new T07 entries, restoring "in task order" top-to-bottom without altering T04-3's content.
- Browser checks queued: T07-1, T07-2, T07-3, T07-4, T07-5, T07-6 in
  docs/tasks/release-1/BROWSER-TESTS.md.

### T01 read_page structural pagination with element and char caps -- done -- 2026-07-02
- Commit: (recorded after commit; see git log for `feat(extension): T01 ...`)
- Files touched: extension/content.js, docs/tasks/release-1/BROWSER-TESTS.md,
  docs/tasks/release-1/LEDGER.md
- Tests added: none in the Rust sense (this task touches only extension JS, which has no test
  harness per project constraints). Verification performed instead:
  - `node --check extension/content.js` (syntax only).
  - A standalone throwaway Node script (not committed) that mirrored the pass-1/pass-2
    measure/emit algorithm in isolation (synthetic records with controlled `chars`/`show`
    values, not the real DOM helpers) to exercise: (a) everything-fits producing no markers,
    (b) a deep subtree overflowing and collapsing behind a marker while a LATER SIBLING at the
    same level still gets emitted (the breadth-over-depth property), (c) a subtree so large that
    even its own collapse marker does not fit, correctly halting the whole emit pass with no
    partial/gap output. All three matched the spec's described behavior exactly.
  - Full-file diff review confirming: (1) the diff is scoped entirely to lines inside
    `accessibilityTree` (verified via `git diff` hunk headers -- only three hunks, all within
    the function, nothing touched before or after it); (2) the per-line construction code (the
    element-line and select-option-line builders) was moved into pass 1 character-for-character
    unchanged, only the `add(...)` calls were replaced with direct string concatenation into
    `unit`; (3) the literal string `"... (truncated)"` no longer appears anywhere in the file
    (`grep -n "truncated"` returns nothing); (4) `cargo test` (all 91 tests across the workspace,
    including `tests/tool_schema_fidelity.rs`) passes unchanged, confirming no Rust surface was
    touched.
  - `git status --short -- '*.rs' src/ tests/` was empty throughout -- this task made zero Rust
    changes, exactly as the prompt's "Project context" predicted ("no Rust rebuild is required").
- Drift reconciled: none. Every line number, function name, and code shape the prompt's "Current
  behavior" section cited (accessibilityTree at lines 119-192, the `add` helper at 126-135, `walk`
  at 136-183, the ref_id re-rooting at 184-189, the service-worker forwarding at its cited lines)
  matched the actual working tree exactly -- this prompt's line numbers had not drifted at all
  from T04/T06/T07 (none of those touched extension/content.js).
- Decisions made:
  - Added an explicit `show` boolean field to each pass-1 record (not named in the prompt's field
    list: unit, ref, indent, children, unitChars, subtreeChars, elements) so pass 2 can branch on
    "is this record shown" without relying on `ref !== null` as an implicit proxy. This is an
    additive, non-observable implementation detail (does not change output), added for
    readability/robustness; every field the prompt DID require is present with the exact
    described semantics.
  - Kept the `collapsed` boolean flag in pass 2 even though no trailing-line decision reads it
    directly (only `capped` and `omitted > 0` gate the two trailing lines, per the prompt's own
    closing note "collapsed or stopped each imply omitted > 0, so this one condition covers every
    degraded outcome"). The prompt's Pass 2 preamble explicitly lists `collapsed` as required
    mutable state, so it is tracked for spec fidelity even though it is presently
    write-only; a future task could read it without restructuring the function.
  - `measure`'s guard-failure return value is `null` (a sentinel meaning "this node and its whole
    subtree do not exist in the render tree"), matching the original `walk`'s early-`return`
    semantics exactly: guard failure (depth exceeded, non-element node, `browser-mcp-` id,
    script/style/noscript/template tag, or the `filter==="interactive"` prune) skips the node AND
    everything under it, never just suppresses its own line. This was verified against the
    original code's control flow before writing pass 1, not assumed.
  - Did not special-case `<select>` records in pass 2 (no `if (tag === "select") ...` branch
    anywhere in `emit`). The "a select can never emit a marker, only stop" behavior the prompt
    describes falls out of the general algorithm automatically: a childless record (select's
    `children` is always `[]`, per the leaf rule preserved from pass 1) has
    `subtreeChars === unitChars`, so whenever rule 4 (does-not-fit) is reached for it,
    `unitChars` alone already exceeds `remaining`, which makes `unitChars + markerLine.length`
    exceed `remaining` too -- the marker-fits branch is therefore unreachable for any childless
    record, select or otherwise, without needing a dedicated check. Verified by direct algebraic
    reasoning (documented in the session) rather than assumed.
- Notes for later tasks:
  - T02 (viewport culling, filter=interactive) touches the SAME function
    (`accessibilityTree`/`measure` in extension/content.js) next. The `show` computation this task
    preserved verbatim is exactly what T02 will extend with position-in-viewport logic; do not
    reintroduce the old serialize-as-you-walk shape when adding that -- extend the `measure`
    function's guard/show logic in place, keep the pass-1/pass-2 split intact.
  - The three new literal line formats introduced here (`[subtree collapsed: ... to expand]`,
    `[element cap reached: ...]`, `[showing M of T elements; ...]`) are now a byte-exact contract
    of this file, same tier as T06's `[hop: ...]` contract in the Rust side -- do not reword them
    in a later task without updating this note and the T01 BROWSER-TESTS.md entries.
  - `MAX_ELEMENTS = 10000` is declared as a local `const` inside `accessibilityTree`, not at
    module scope -- there was no existing module-scope constant section in this file to join, and
    the prompt allowed either placement ("Declare it as a const at the top of accessibilityTree
    (or module scope next to the function)").
  - No `src/mcp/schemas/tools.json` edits were made or needed; `tests/tool_schema_fidelity.rs`
    passed unchanged (6/6), confirming the frozen `read_page` schema and its description (which
    still describes the now-superseded error-on-overflow behavior, deliberately -- see the
    prompt's Out of scope section) were left untouched.
- Browser checks queued: T01-1, T01-2, T01-3, T01-4, T01-5, T01-6 in
  docs/tasks/release-1/BROWSER-TESTS.md (appended after T07-6, preserving task order).

### T02 read_page viewport culling for filter=interactive -- done -- 2026-07-02
- Commit: (recorded after commit; see git log for `feat(extension): T02 ...`)
- Files touched: extension/content.js, docs/tasks/release-1/BROWSER-TESTS.md,
  docs/tasks/release-1/LEDGER.md
- Tests added: none in the Rust sense (extension JS has no test harness per project constraints).
  Verification performed instead:
  - `node --check extension/content.js` (syntax only).
  - Full re-read of the final `accessibilityTree`/`measure` function against every constraint in
    the prompt's Verification step 2: `intersectsViewport` exists with the exact strict-inequality
    formula given; `culled` is set only via `if (wouldShow && !show) culled = true;` (the
    wouldShow-but-not-shown case, and no other); the note string
    "Note: interactive results are limited to the current viewport; scroll or use filter=all for
    the full document." matches the contract character for character; line 152's early return
    (`if (filter === "interactive" && !isInteractive && !isContainer) return null;`) is byte-
    identical to before this task; `visible()` (lines 97-101) is untouched; the file is pure ASCII
    (confirmed by the BOOTSTRAP.md ASCII-scan command, empty output).
  - Traced the short-circuit algebra by hand: `show = wouldShow && (filter === "all" ||
    intersectsViewport(el))` -- when `filter === "all"`, the right operand short-circuits to `true`
    without evaluating `intersectsViewport`, so `show === wouldShow` always and `culled` can never
    become true for `filter=all` (satisfies "filter=all byte-identical, zero new
    getBoundingClientRect calls"). When `wouldShow` is `false` (excluded by role/name, interactive-
    ness, or `visible()`), the left operand of the outer `&&` is `false`, so JS never evaluates the
    right operand either -- `intersectsViewport` is only ever called for elements that would
    otherwise be shown, and `culled` is never set for any other exclusion reason.
  - `cargo test` (all 91 tests across the workspace, including `tests/tool_schema_fidelity.rs`,
    6/6) passes unchanged, confirming no Rust surface was touched.
  - `git status --short -- '*.rs' src/ tests/` was empty throughout -- this task made zero Rust
    changes, matching the prompt's "Project context" prediction ("no Rust rebuild is required").
  - `cargo clippy --all-targets -- -D warnings` clean; `cargo fmt --check` reports only the same
    two pre-existing drifted files noted by every prior task (see Decisions made below), neither of
    which this task touched.
- Drift reconciled: the prompt's entire "Current behavior" section describes the PRE-T01
  single-pass `walk()` function (a single `show` computation at "line 147", a `truncated` flag next
  to an `out` accumulator, `add(s)` for the character budget). T01 (which runs earlier in the fixed
  sequence and landed first) rewrote `accessibilityTree` into a two-pass `measure`/`emit` design
  with no `walk()` and no `add()`; `truncated` no longer exists (replaced by `capped`/`stopped`/
  `collapsed`/`omitted` in the pass-2 `emit` closure). Reconciled by mapping every required change
  onto its structural analog in the new code: (1) the exact `show` formula the prompt describes
  (`((filter === "all" && (r || n)) || (filter === "interactive" && isInteractive)) && isVisible`)
  is verbatim present inside `measure()` (pass 1) at the equivalent point in the walk -- this is
  where the culling logic was applied, unchanged from the prompt's literal formula. (2) `let culled
  = false;` was declared at the top of `accessibilityTree`, immediately after `const MAX_ELEMENTS =
  10000;` (not "next to `truncated`", which no longer exists) -- this is the earliest point in the
  new structure where `measure()` (defined and first invoked several lines later) can close over
  it; pass 2's own flags (`collapsed`/`stopped`/`capped`) are declared later, after pass 1 already
  ran, so `culled` could not sit next to them and still be visible to `measure()`. (3) the note-
  append logic was applied to the actual final `return` statement (now building `let result = out +
  ... ; if (culled) { result += ... } return result;`), which is the exact same statement the
  prompt calls "the return statement (currently line 191)" -- T01 did not change this statement's
  shape (it still ends the function with `out + Viewport line`), only what feeds into `out` earlier
  in pass 2. Every property the prompt requires of the new logic (show/culled semantics, note
  placement outside the char budget, filter=all short-circuit, the untouched early-return prune,
  the untouched children-descent code, the untouched `visible()`) was independently re-verified
  against the ACTUAL two-pass code, not assumed to still hold from the prompt's stale description.
- Decisions made:
  - Placed the one-line comment on `intersectsViewport` ("getBoundingClientRect is viewport-
    relative for every element, so this is correct at any scroll position and for position:fixed
    elements without special cases") wrapped as two short lines to stay under the file's existing
    line-length norms; this is the single comment the prompt's constraint 7 permits on the new
    helper ("At most one short comment on the new helper... is acceptable; more is not"). No
    comment was added at the `culled` declaration site beyond a short trailing note, and no
    comment was added at the `wouldShow`/`show`/`culled` lines inside `measure()` (the code is
    read as self-explanatory there, matching the prompt's own preference to express the logic in
    code rather than prose wherever possible).
  - Did not touch the pass-1 doc comment above `measure()` ("Same entry guards, same show
    computation, same recursion order as a single-pass walk would use...") even though "show
    computation" is now technically two lines (`wouldShow`/`show`) instead of one -- the comment's
    claim (this pass reproduces what a single-pass walk would compute) remains true; rewording it
    was not required by the prompt and would be an unrequested, unscoped comment change.
  - Left `src/policy/redact.rs` and `tests/tool_schema_fidelity.rs` untouched (same pre-existing
    rustfmt-version drift every prior task in this run has flagged). This task touched no Rust
    files at all, so there was nothing to run `cargo fmt` on and no reformatting side effect to
    revert this time; confirmed via `cargo fmt --check`, whose only reported diffs are in exactly
    those same two files, byte-for-byte the same diffs T01's log already described.
- Notes for later tasks:
  - The `intersectsViewport(el)` helper (declared directly after `visible()`, lines ~102-107) is
    now available to any later task in this file; it is intentionally NOT used by `find()` or
    `pageText()` per this task's Out of scope section -- do not wire it in elsewhere without a new
    task prompt actually requiring it.
  - `culled` and the `wouldShow`/`show` split inside `measure()` are now part of the same render-
    tree record shape T01 introduced (`{ unit, ref, indent, children, unitChars, subtreeChars,
    elements, show }`); `show` in that record still reflects the POST-culling decision (i.e. the
    viewport-aware value), so pass 2 (`emit`) automatically treats a culled element exactly like
    any other not-shown node (skip its own line, still walk its children) with zero changes to
    `emit` itself -- verified by inspection, not just assumed, since `emit` reads `record.show`
    directly.
  - The exact note string "Note: interactive results are limited to the current viewport; scroll
    or use filter=all for the full document." is now a byte-exact contract at the same tier as
    T01's three marker-line formats and T06's `[hop: ...]` contract -- do not reword it in a later
    task without updating this note and the T02 BROWSER-TESTS.md entries.
  - No `src/mcp/schemas/tools.json` edits were made or needed; `tests/tool_schema_fidelity.rs`
    passed unchanged (6/6), confirming the frozen `read_page` schema was left untouched, per this
    task's Constraints section.
- Browser checks queued: T02-1, T02-2, T02-3, T02-4 in docs/tasks/release-1/BROWSER-TESTS.md
  (appended after T01-6, preserving task order).

### T03 get_page_text official semantics -- done -- 2026-07-02
- Commit: (recorded after commit; see git log for `feat(extension): T03 ...`)
- Files touched: extension/content.js, extension/service-worker.js,
  docs/tasks/release-1/BROWSER-TESTS.md, docs/tasks/release-1/LEDGER.md
- Tests added: none in the Rust sense (this task touches only extension JS, which has no test
  harness per project constraints). Verification performed instead:
  - `node --check extension/content.js` and `node --check extension/service-worker.js` (syntax
    only), both clean.
  - Full diff review confirming: `PAGE_TEXT_SELECTORS` contains exactly the twelve selectors from
    the prompt's contract, in that exact order; `pageText` reads `el.innerText` /
    `document.body.innerText` only, with zero occurrences of `textContent` or `cloneNode` anywhere
    in the new code; the `body.length < 10` no-readable-content check runs strictly before the
    `body.length > maxChars` truncation check (verified by reading the `if`/`if` sequence); the
    header (`Source element: <sel>\n\n`), the no-readable-content message, and the truncation
    notice all match the prompt's contract strings character for character (only the `${bestSel}`/
    `${maxChars}` placeholders substituted); the old `Title:`/`URL:` lines are gone (grepped for
    `Title:` and `URL:` in the new `pageText` body -- zero matches); the service worker's
    `get_page_text` handler changed on exactly one line (the `content(...)` call), keeping the
    `inGroup` gate, the `text(...)` wrap, and the `"Could not extract page text."` fallback
    untouched; the content script's message-handler `case "pageText"` line is the only case
    touched.
  - `cargo test` (all 91 tests across the workspace -- 80 unit + 4 mcp_protocol + 1 peer_death + 6
    tool_schema_fidelity -- plus 0 doc-tests) passes unchanged, confirming no Rust surface was
    touched and the frozen `get_page_text` schema (including its `max_chars` advertisement) is
    intact.
  - `git status --short -- '*.rs' src/ tests/` was empty throughout -- this task made zero Rust
    changes, exactly as the prompt's "Project context" predicted ("no Rust rebuild is required").
  - `cargo clippy --all-targets -- -D warnings` clean (nothing to lint; no Rust changed).
  - `cargo fmt --check` reports only the same two pre-existing drifted files every prior task in
    this run has flagged (`src/policy/redact.rs`, `tests/tool_schema_fidelity.rs`); neither was
    touched by this task, and there was nothing to run `cargo fmt` on (zero Rust changes), so no
    reformatting side effect occurred this time.
- Drift reconciled: only line-number drift, exactly as the prompt itself warned ("re-verify before
  editing; line numbers may have drifted"). The prompt's Current-behavior section cited the
  `// --- Page text ---` section at content.js lines 194-204 and the message-handler case at line
  299; the actual working tree (after T01/T02 extended `accessibilityTree` earlier in the file)
  had them at lines 279-289 and 428 respectively. Likewise the prompt's service-worker handler
  citation (lines 484-488) was actually at lines 522-526. In every case the CODE SHAPE, selector
  list contents/order, and exact string literals the prompt described matched the working tree
  verbatim; only the line numbers had moved. No logic-level drift.
- Decisions made:
  - Reproduced the prompt's contract snippet verbatim (selectors, `normalizePageText`, `pageText`,
    the message-handler case, the service-worker bridge line) rather than paraphrasing, per the
    prompt's own instruction ("reproduce its behavior exactly... not logic, strings, or
    defaults"). No trivial formatting adjustments were needed; the snippet's style (2-space indent,
    double quotes for strings needing interpolation-safe quoting, template literals) already
    matched the surrounding file.
  - Kept the two comments from the contract snippet (`PAGE_TEXT_SELECTORS` selector-priority note,
    `normalizePageText` conservative-cleanup note) as the only comments added, per constraint 7
    ("the two short comments in the snippet above are the ceiling; do not add more"). No comment
    was added to `pageText` itself.
  - Left `src/policy/redact.rs` and `tests/tool_schema_fidelity.rs` untouched (same pre-existing
    rustfmt-version drift every prior task in this run has flagged). This task touched no Rust
    files at all, so there was nothing to run `cargo fmt` on and no reformatting side effect to
    revert this time; confirmed via `cargo fmt --check`, whose only reported diffs are in exactly
    those same two files, byte-for-byte the same diffs T01's and T02's logs already described.
- Notes for later tasks:
  - `get_page_text`'s output contract (`Source element: <sel>\n\n<body>`, the no-readable-content
    one-liner, and the `[Truncated at N characters. ...]` notice) is now a byte-exact contract at
    the same tier as T01's marker-line formats, T02's Note line, and T06's `[hop: ...]` contract --
    do not reword any of the three strings in a later task without updating this note and the T03
    BROWSER-TESTS.md entries.
  - `PAGE_TEXT_SELECTORS` and `normalizePageText` are module-scope (inside the content script's
    IIFE, alongside `accessibilityTree`'s helpers) and are NOT wired into `accessibilityTree`,
    `find`, or any other function -- per the prompt's Out of scope section, this task intentionally
    does not touch structural/interactive extraction, only free-text extraction.
  - `content(tabId, { type: "pageText", max_chars })` and the content script's `case "pageText"`
    now both pass `max_chars` through; `msg.max_chars` on the content-script side is validated
    entirely inside `pageText()` (any non-finite/non->=1 value silently falls back to 50000) -- the
    service worker performs zero validation of its own, by design (mechanism only, no policy in
    the extension).
  - No `src/mcp/schemas/tools.json` edits were made or needed; `tests/tool_schema_fidelity.rs`
    passed unchanged (6/6), confirming the frozen `get_page_text` schema (and its existing
    `max_chars` advertisement, already present before this task) was left untouched, per this
    task's Constraints section.
- Browser checks queued: T03-1, T03-2, T03-3, T03-4 in docs/tasks/release-1/BROWSER-TESTS.md
  (appended after T02-4, preserving task order).

### T12 Console/network buffers reset on same-tab domain change -- done -- 2026-07-02
- Commit: (recorded after commit; see git log for `feat(extension): T12 ...`)
- Files touched: extension/service-worker.js, docs/tasks/release-1/BROWSER-TESTS.md,
  docs/tasks/release-1/LEDGER.md
- Tests added: none in the Rust sense (this task touches only extension JS, which has no test
  harness per project constraints). Verification performed instead:
  - `node --check extension/service-worker.js` (syntax only), clean.
  - Full diff review confirming: `hostOf` matches the prompt's snippet verbatim; `tabHost` is a
    new module-level `Map` declared alongside the other buffer declarations; the persistent
    `chrome.tabs.onUpdated` listener and `bufferFor` match the prompt's snippets verbatim (byte
    for byte, including the exact reset/adopt/keep-as-is branching); `pushCapped` now routes
    through `bufferFor` and stays capped at 1000 via `buf.items.splice`; the attach closure in
    `ensureAttached` seeds `tabHost` right after `attached.set(tabId, { domains: new Set() })`
    inside its own try/catch that cannot fail the attach; `chrome.tabs.onRemoved` gained exactly
    one new line (`tabHost.delete(tabId);`); both read handlers resolve the tab's live hostname
    fresh via `chrome.tabs.get`, refresh `tabHost`, call `bufferFor`, and read `buf.items` before
    any filter/slice; the two `clear` lines were updated to the new `{ host, items: [] }` shape;
    grepped the two zero-entries strings ("No console messages matching the pattern." / "No
    network requests matching the pattern.") and both `[level] text` / `METHOD url -> status`
    format strings -- byte-identical to the pre-task code, confirmed via `git diff` (no lines
    inside either return statement's template literal changed).
  - `cargo test` (all 91 tests across the workspace, including `tests/tool_schema_fidelity.rs`,
    6/6) passes unchanged, confirming no Rust surface was touched.
  - `git status --short -- '*.rs' src/ tests/` was empty throughout -- this task made zero Rust
    changes, matching the prompt's "Build and test" note ("no Rust rebuild is needed").
  - `cargo clippy --all-targets -- -D warnings` clean (nothing to lint; no Rust changed).
  - `cargo fmt --check` reports only the same two pre-existing drifted files every prior task in
    this run has flagged (`src/policy/redact.rs`, `tests/tool_schema_fidelity.rs`); neither was
    touched by this task, and there was nothing to run `cargo fmt` on (zero Rust changes).
  - ASCII scan (the BOOTSTRAP.md python one-liner) on both edited files (`extension/service-
    worker.js`, `docs/tasks/release-1/BROWSER-TESTS.md`) returned empty lists.
- Drift reconciled: only line-number drift, as the prompt itself warned ("line numbers verified
  against extension/service-worker.js as of this writing" -- earlier tasks in this run had already
  landed and shifted them). The prompt cited the attach closure at "lines 58-61"; the actual
  working tree (after T04/T06/T07 landed) had it at lines 71-78 before this task's edit. The
  prompt cited `chrome.tabs.onRemoved` at "lines 125-133" and the buffering section
  (`chrome.debugger.onEvent`/`pushCapped`) at "lines 137-160"; actual lines were 146-181. The
  prompt cited the two read handlers at "lines 512-536"; actual lines were 554-578. In every case
  the function names, code shape, comment text, and the exact strings the prompt quoted (the
  console/network zero-entries strings, the two schema-description phrases it names in "Project
  context" for `src/mcp/schemas/tools.json`, which was not touched) matched the working tree
  verbatim; only line numbers had moved. No logic-level drift, and `src/mcp/schemas/tools.json`
  itself was never opened for edits (out of scope, and the prompt only cites it for context).
- Decisions made:
  - Kept the read-handler local variable names exactly as the prompt's own snippet uses them
    (`tab`, `host`, `buf`) in both `read_console_messages` and `read_network_requests`, even
    though each name is reused across the two independent handler functions -- there is no
    collision risk since each is its own function scope (verified by reading both handlers in
    full; neither had a pre-existing local named `tab`, `host`, or `buf`), and matching the
    prompt's snippet verbatim minimizes any risk of silently diverging from its documented
    semantics.
  - Placed the new `hostOf` function and the persistent `chrome.tabs.onUpdated` listener at the
    top of the "Console / network buffering" section (immediately after the section's `---`
    header comment, before the pre-existing `chrome.debugger.onEvent` listener), and placed the
    new `bufferFor` helper between the `chrome.debugger.onEvent` listener and `pushCapped` (which
    now calls it). The prompt names exact line ranges to touch but leaves placement of the four
    "new" additions (`hostOf`, `tabHost`, `bufferFor`, `chrome.tabs.onUpdated`) unspecified beyond
    "in the console/network buffering section near the chrome.debugger.onEvent listener" for the
    listener specifically; this placement satisfies that literally and keeps the whole
    buffer-ownership concern (hostname helper -> live tracking -> event-driven append -> ownership
    rule -> capped append) in one readable top-to-bottom block. `tabHost` itself was declared next
    to the other buffer declarations (line 20, after `screenshotCtx`), per the prompt's explicit
    instruction ("Add module-level state next to the buffer declarations").
  - In the `Network.responseReceived` branch of `chrome.debugger.onEvent`, call `bufferFor`
    directly (not `pushCapped`) to look up-or-reset the buffer before searching by `requestId`,
    exactly as the prompt's step 5 specifies; the not-found fallback still calls the existing
    `pushCapped(networkBuffer, tabId, {...})`, which internally calls `bufferFor` a second time --
    this second call is idempotent (the buffer's `host` was already resolved/adopted by the first
    call in this same event tick), so there is no double-reset or lost-append risk. Verified by
    tracing `bufferFor`'s branches by hand for this exact call sequence.
  - Left `src/policy/redact.rs` and `tests/tool_schema_fidelity.rs` untouched (same pre-existing
    rustfmt-version drift every prior task in this run has flagged). This task touched no Rust
    files at all, so there was nothing to run `cargo fmt` on and no reformatting side effect to
    revert this time; confirmed via `cargo fmt --check`, whose only reported diffs are in exactly
    those same two files, byte-for-byte the same diffs T01/T02/T03's logs already described.
- Notes for later tasks:
  - Both buffers are now `tabId -> { host, items: [...] }` instead of `tabId -> [...]`. Any later
    task reading `consoleBuffer`/`networkBuffer` directly (none of the remaining prompts in this
    run appear to) must go through `.items`, not treat the map's value as an array.
  - `bufferFor(map, tabId, host)` is the single choke point for "get or reset-or-adopt a buffer for
    this tab against this hostname"; both the event listener's append path (via `pushCapped`) and
    the two read handlers route through it. A later task adding a new event source that appends to
    either buffer should call `pushCapped`, not touch the maps directly.
  - `tabHost` is refreshed three ways (event-driven via `chrome.tabs.onUpdated`, seeded on attach,
    and refreshed fresh on every read-handler call via `chrome.tabs.get`) but is deliberately never
    persisted (no `chrome.storage`); a service-worker restart starts it empty again, same as
    `attached`/`consoleBuffer`/`networkBuffer`.
  - T15 (Empty-result guidance notes) will touch the exact same two zero-entries return strings
    this task deliberately left untouched ("No console messages matching the pattern." / "No
    network requests matching the pattern."); this task changed nothing about wording, only which
    entries are visible when those strings are chosen.
  - T13 (Runtime.exceptionThrown capture) and T14 (Network.loadingFailed status) both add new
    branches to the same `chrome.debugger.onEvent` listener this task modified. Any new branch
    that appends to `consoleBuffer` or `networkBuffer` must go through `pushCapped` (which now
    routes through `bufferFor`/`tabHost` automatically) to stay domain-scoped; do not append via a
    raw `map.get(tabId).items.push(...)` or reintroduce a bare-array buffer shape.
  - The accepted CDP-race limitation (a cross-domain navigation's main-document
    `Network.requestWillBeSent` can land in the old domain's buffer and be discarded on the next
    reset) is intentional per the prompt's Required-behavior item 8; do not "fix" it with
    `Page.frameNavigated`, `webNavigation`, or URL heuristics without a new task prompt requiring
    it.
- Browser checks queued: T12-1, T12-2, T12-3, T12-4, T12-5 in docs/tasks/release-1/BROWSER-TESTS.md
  (appended after T03-4, preserving task order).

### T13 Runtime.exceptionThrown capture -- done -- 2026-07-02
- Commit: (recorded after commit; see git log for `feat(extension): T13 ...`)
- Files touched: extension/service-worker.js, docs/tasks/release-1/BROWSER-TESTS.md,
  docs/tasks/release-1/LEDGER.md
- Tests added: none in the Rust sense (this task touches only extension JS, which has no test
  harness per project constraints). Verification performed instead:
  - `node --check extension/service-worker.js` (syntax only), clean.
  - A standalone throwaway Node script (not committed; deleted from scratchpad after use) that
    copied `exceptionText` verbatim and asserted 8 cases against it: the prompt's own worked
    example (`Error: boom` with url+lineNumber+4 call frames, confirming only the first 3 frames
    render, that an empty `functionName` becomes `<anonymous>`, and that frame line numbers are
    +1'd); a fully-empty `exceptionDetails` object producing the literal `"Uncaught exception"`
    fallback (never crashes); a thrown-primitive case (`exception.value` present, no
    `description`) using `String(value)` and ignoring `text`; a `text`-only fallback when no
    `exception` object exists at all; a `url` present with a non-numeric `lineNumber` correctly
    omitting the `:LINE` suffix (still emitting `(URL)`); a `lineNumber` present without a `url`
    being fully ignored (no location part at all, matching "only when url is a non-empty
    string"); a multi-line `description` reduced to only its first line; and an empty
    `callFrames` array producing no `[at ...]` suffix. All 8 passed.
  - Full diff review confirming: the new `Runtime.exceptionThrown` branch is an `else if`
    directly after the `Runtime.consoleAPICalled` branch inside the same
    `chrome.debugger.onEvent.addListener` callback (not a new listener); it stores via
    `pushCapped(consoleBuffer, tabId, { level: "exception", text: ... })`, the exact same call
    shape and buffer the consoleAPICalled branch already uses (which, per T12 having landed
    first, already routes through `bufferFor`/`tabHost` domain-scoping and the 1000-entry cap
    automatically -- no separate reset/keying logic was written for this task); `params.
    exceptionDetails || {}` guards the "missing details" case so `exceptionText` is never called
    with `undefined`; `exceptionText` is a pure function with no CDP calls, no `chrome.*` calls,
    and returns a single-line string in all 8 verified cases (uses `.split("\n")[0]` for the one
    documented multi-line source, `description`); nothing outside this one added function and
    one added `else if` branch changed (grepped `git diff --stat` -- one file, and the diff hunk
    boundaries in `service-worker.js` are exactly the two insertions described).
  - Confirmed `read_console_messages`'s `onlyErrors` filter (now inside the `handlers` object,
    not at the old cited line 518) still reads
    `["error", "exception"].includes(m.level)` -- byte-identical to the prompt's description, no
    edit needed or made.
  - `cargo test` (all 91 tests across the workspace, including `tests/tool_schema_fidelity.rs`,
    6/6) passes unchanged, confirming no Rust surface was touched.
  - `git status --short -- '*.rs' src/ tests/` was empty throughout -- this task made zero Rust
    changes, matching the prompt's "Build and test" note ("this task changes no Rust code").
  - `cargo clippy --all-targets -- -D warnings` clean (nothing to lint; no Rust changed).
  - `cargo fmt --check` reports only the same two pre-existing drifted files every prior task in
    this run has flagged (`src/policy/redact.rs`, `tests/tool_schema_fidelity.rs`); neither was
    touched by this task, and there was nothing to run `cargo fmt` on (zero Rust changes).
  - ASCII scan (the BOOTSTRAP.md python one-liner) on both edited files (`extension/service-
    worker.js`, `docs/tasks/release-1/BROWSER-TESTS.md`) returned empty lists.
- Drift reconciled: only line-number and buffer-shape drift, exactly as the prompt itself warned
  it might have ("If a separate task has already changed how the consoleAPICalled branch keys or
  clears the buffer by the time you start, mirror whatever that branch does"). T12 (which runs
  earlier in the fixed sequence and had already landed) changed `consoleBuffer` from a bare
  `tabId -> [...]` array to `tabId -> { host, items: [...] }`, and `pushCapped` now internally
  calls `bufferFor(map, tabId, tabHost.get(tabId))` to domain-scope/reset the buffer before
  appending. The prompt's own "Current behavior" section (lines 14, 48-60) describes the
  pre-T12 bare-array shape and cites `pushCapped(consoleBuffer, tabId, entry)` as the exact call
  pattern to reuse -- reconciled exactly as the prompt's own contingency text instructed: called
  `pushCapped(consoleBuffer, tabId, entry)` verbatim (unchanged call signature; `pushCapped`'s
  internal domain-scoping happens transparently) rather than writing any new keying/reset logic
  of this task's own. Line numbers had also drifted (prompt cited the listener at "137-154" and
  `pushCapped` at "155-160"; actual lines were 170-187 and 200-204 before this task's edit,
  matching T12's log which already noted the same listener block moved to 170-187) -- confirmed
  by reading the actual file before editing, not by trusting the prompt's numbers.
- Decisions made:
  - Named the helper `exceptionText` exactly as instructed, and placed it directly above
    `chrome.debugger.onEvent.addListener` (immediately after the `chrome.tabs.onUpdated`
    listener, which itself sits right after `hostOf`) -- "next to the listener" per the prompt,
    and hoisting order does not matter for a `function` declaration referenced only from within
    the listener body.
  - Read `details.exception` once into a local `exc` at the top of `exceptionText` (rather than
    repeating `details.exception` three times) -- a direct, low-risk readability simplification;
    every branch and precedence order (description-first, then value, then top-level text, then
    literal fallback) matches the prompt's four-way `else if` chain exactly.
  - The one comment permitted by constraint 7 ("a one-line comment noting that CDP line numbers
    are 0-based is acceptable") was placed directly on the `out += ... lineNumber + 1 ...` line
    inside `exceptionText`, where the +1 arithmetic actually happens; a second short doc-style
    comment was placed directly above the `exceptionText` function declaration itself
    (describing its three-part single-line output shape), matching this file's existing density
    of a one-to-two-line comment per non-trivial helper (for example `rescaleCoord`,
    `bufferFor`) rather than being uncommented or over-commented.
  - Verified `Array.isArray(details.stackTrace.callFrames)` (not just truthiness) before slicing,
    to defend against a malformed/absent `stackTrace` without throwing -- consistent with
    constraint requirement 1's "never crash the listener," even though the prompt's own spec
    text only says "exists and its callFrames is a non-empty array," which this check
    implements literally (an empty or non-array `callFrames` is treated as "no stack," never a
    crash).
  - Left `src/policy/redact.rs` and `tests/tool_schema_fidelity.rs` untouched (same pre-existing
    rustfmt-version drift every prior task in this run has flagged). This task touched no Rust
    files at all, so there was nothing to run `cargo fmt` on and no reformatting side effect to
    revert this time; confirmed via `cargo fmt --check`, whose only reported diffs are in exactly
    those same two files, byte-for-byte the same diffs T01/T02/T03/T12's logs already described.
- Notes for later tasks:
  - `exceptionText(details)` is a new pure helper in `extension/service-worker.js`, module-scope,
    taking a possibly-empty `exceptionDetails`-shaped object and returning a single-line string.
    It has no dependency on and is not called by anything outside the new `Runtime.
    exceptionThrown` branch; a later task should not need to touch it unless a new task prompt
    explicitly asks for it (for example, changing exception-text formatting is explicitly listed
    as out of scope for a "Formatting changes for other console levels" -- but note that clause
    is about OTHER levels, not this one; this task's own three-part text format for level
    `"exception"` is now itself a byte-exact contract at the same tier as T01's marker-line
    formats, T02's Note line, T03's get_page_text contract, and T06's `[hop: ...]` contract -- do
    not reword it in a later task without updating this note and the T13 BROWSER-TESTS.md
    entries).
  - T14 (Network.loadingFailed status) adds a new branch to the same `chrome.debugger.onEvent`
    listener this task modified (now ending after the `Runtime.exceptionThrown` branch, before
    `Network.requestWillBeSent`). Per this task's own Out-of-scope section, the
    `Network.requestWillBeSent`/`Network.responseReceived` branches and the network buffer were
    NOT touched; T14 should add its own `else if` branch in the same style, appending via
    `pushCapped(networkBuffer, tabId, ...)` to inherit T12's domain-scoping automatically, same
    as this task did for the console buffer.
  - T15 (Empty-result guidance notes) touches the same `read_console_messages` zero-entries
    string ("No console messages matching the pattern.") this task's Verification section
    exercises but does not modify; this task changed zero characters of that string or of the
    `[${m.level}] ${m.text}` render format in the handler.
  - The Runtime CDP domain continues to be enabled only lazily, on the first
    `read_console_messages` call per tab (unchanged); `Runtime.exceptionThrown` events for a tab
    only start flowing into the buffer once that domain has been enabled for that tab, exactly
    like `Runtime.consoleAPICalled` already did. No new domain-enable call was added anywhere.
- Browser checks queued: T13-1, T13-2, T13-3 in docs/tasks/release-1/BROWSER-TESTS.md (appended
  after T12-5, preserving task order).

### T14 Network.loadingFailed marks requests failed instead of eternally pending -- done -- 2026-07-02
- Commit: (recorded after commit; see git log for `feat(extension): T14 ...`)
- Files touched: extension/service-worker.js, docs/tasks/release-1/BROWSER-TESTS.md,
  docs/tasks/release-1/LEDGER.md
- Tests added: none in the Rust sense (this task touches only extension JS, which has no test
  harness per project constraints). Verification performed instead:
  - `node --check extension/service-worker.js` (syntax only), clean.
  - Full diff review confirming: the new `Network.loadingFailed` branch is an `else if` directly
    after the `Network.responseReceived` branch inside the same `chrome.debugger.onEvent.
    addListener` callback (not a new listener), guarded by `method === "Network.loadingFailed" &&
    params.requestId`, matching the sibling branches' guard style exactly; it looks up the entry
    via `bufferFor(networkBuffer, tabId, tabHost.get(tabId))` then `buf.items.find((r) =>
    r.requestId === params.requestId)`, the identical lookup pattern the `responseReceived` branch
    uses one branch above it; when found, `existing.status = 503` unconditionally, `existing.
    errorText = params.errorText` only inside `if (params.errorText)` (falsy-string guard covers
    both `undefined` and `""`, satisfying "only when a non-empty string"), and `existing.canceled =
    !!params.canceled` unconditionally; when NOT found, the branch does nothing (no `pushCapped`
    call, no synthetic entry) -- confirmed by reading the branch body, which has no code path after
    the `if (existing) { ... }` block. The `requestWillBeSent` and `responseReceived` branches
    above it are byte-identical to before this task (verified via the `git diff` hunk, which shows
    only an insertion after `responseReceived`'s closing line, no deletions in either sibling
    branch).
  - Renderer diff confirms the exact template expression from the prompt was used verbatim:
    `` `${r.method || "?"} ${r.url} ${r.status ? "-> " + r.status + (r.errorText ? " (" +
    r.errorText + ")" : "") : "(pending)"}` ``; traced by hand against the three documented cases
    (status+errorText -> `-> 503 (net::ERR_...)`; status, no errorText -> `-> 200` unchanged;
    status 0 -> `(pending)` unchanged, since `r.status` is falsy for `0` and the ternary's false
    branch is taken regardless of `errorText`). The group check, `ensureAttached`,
    `enableDomain(a.tabId, "Network")`, the `urlPattern` substring filter, the `limit` slice, the
    `clear` behavior, the `"\n"` join, and the empty-result message
    "No network requests matching the pattern." are all byte-identical to before this task (only
    the one template-literal line inside `.map(...)` changed; confirmed via `git diff` hunk
    boundaries).
  - The shape comment on the `networkBuffer` declaration was updated to list the two new fields
    (`errorText, canceled`) appended to the existing five.
  - `cargo test` (all 91 tests across the workspace, including `tests/tool_schema_fidelity.rs`,
    6/6) passes unchanged, confirming no Rust surface was touched.
  - `git status --short -- '*.rs' src/ tests/` was empty throughout -- this task made zero Rust
    changes, matching the prompt's "Build and test" note ("no Rust rebuild is required").
  - `cargo clippy --all-targets -- -D warnings` clean (nothing to lint; no Rust changed).
  - `cargo fmt --check` reports only the same two pre-existing drifted files every prior task in
    this run has flagged (`src/policy/redact.rs`, `tests/tool_schema_fidelity.rs`); neither was
    touched by this task, and there was nothing to run `cargo fmt` on (zero Rust changes).
  - ASCII scan (the BOOTSTRAP.md python one-liner) on both edited files (`extension/service-
    worker.js`, `docs/tasks/release-1/BROWSER-TESTS.md`) returned empty lists.
- Drift reconciled: the prompt's "Current behavior" section describes the buffer shape from BEFORE
  T12 landed (a bare `tabId -> [{ requestId, method, url, status, mimeType }]` array, with the
  prompt's own step 1 saying to "get the tab's array (`networkBuffer.get(tabId) || []`)"). T12
  (which runs earlier in the fixed sequence and had already landed) changed `networkBuffer` to
  `tabId -> { host, items: [...] }` and introduced `bufferFor`/`tabHost` for per-domain scoping;
  the `Network.responseReceived` branch immediately above the new code (which the prompt explicitly
  says to mirror -- "Look up the entry exactly like the responseReceived branch does") already used
  `bufferFor(networkBuffer, tabId, tabHost.get(tabId))` then `.items.find(...)`, not a bare
  `.get(tabId) || []`. Reconciled by copying the ACTUAL responseReceived lookup pattern verbatim
  (as the prompt itself instructed: mirror the sibling branch), not the prompt's stale
  `networkBuffer.get(tabId) || []` snippet. T13's own log entry had already flagged this exact
  handoff ("T14 should add its own else if branch in the same style, appending via
  pushCapped(networkBuffer, tabId, ...) to inherit T12's domain-scoping"); this task's branch does
  not append a NEW entry at all (constraint: no synthetic entry on a miss), so `pushCapped` is not
  called here -- only `bufferFor` (read path), which is the correct application of T12's
  domain-scoping to an update-in-place branch, not an append. Line numbers had also drifted (prompt
  cited the listener at "lines 137-154", `pushCapped` at "155-160", and the renderer at "line 535";
  actual lines before this task's edit were 196-215, 228-232, and 636 respectively, consistent with
  T12's and T13's logs which already noted the listener block's earlier moves).
- Decisions made:
  - Named the new branch's lookup buffer variable `buf` and the found record `existing`, matching
    the exact names the sibling `Network.responseReceived` branch already uses one block above --
    not mandated by the prompt but the natural, lowest-risk choice given the prompt's own
    instruction to look up "exactly like the responseReceived branch does."
  - `existing.errorText = params.errorText` is written inside `if (params.errorText)` (a plain
    truthiness check) rather than `if (typeof params.errorText === "string" && params.errorText.
    length > 0)`. Both are equivalent for this field in practice (CDP's `errorText` is always
    either a non-empty string or absent per the Network domain spec; there is no realistic path
    where it is `0`, `false`, or `null` instead of absent), and the prompt's own worked snippet
    (constraint 4's wording "whenever the event provided one") does not distinguish "absent" from
    "falsy," so the simpler truthiness form was kept for consistency with the surrounding file's
    style (every other optional-field guard in this file, e.g. `if (a.urlPattern)`, `if (a.
    pattern)`, uses plain truthiness, not an explicit type/length check).
  - Left `src/policy/redact.rs` and `tests/tool_schema_fidelity.rs` untouched (same pre-existing
    rustfmt-version drift every prior task in this run has flagged). This task touched no Rust
    files at all, so there was nothing to run `cargo fmt` on and no reformatting side effect to
    revert this time; confirmed via `cargo fmt --check`, whose only reported diffs are in exactly
    those same two files, byte-for-byte the same diffs T01/T02/T03/T12/T13's logs already
    described.
- Notes for later tasks:
  - `networkBuffer` entries can now carry two additional optional fields, `errorText` (string) and
    `canceled` (boolean), set only by the `Network.loadingFailed` branch. `canceled` is stored for
    data fidelity but deliberately never rendered by `read_network_requests` (per this task's own
    Constraints/Out-of-scope); a later task should not add rendering for it without a new task
    prompt explicitly requiring it.
  - The `read_network_requests` renderer's exact per-entry template
    (`<METHOD> <URL> -> <STATUS> (<ERRORTEXT>)` / `<METHOD> <URL> -> <STATUS>` / `<METHOD> <URL>
    (pending)`) is now a byte-exact contract at the same tier as T01's marker-line formats, T02's
    Note line, T03's get_page_text contract, T06's `[hop: ...]` contract, and T13's exception-text
    format -- do not reword any of the three cases in a later task without updating this note and
    the T14 BROWSER-TESTS.md entries.
  - T15 (Empty-result guidance notes) touches the same `read_network_requests` zero-entries string
    ("No network requests matching the pattern.") this task's Verification exercises but does not
    modify; this task changed zero characters of that string, and changed only the non-empty-result
    branch's per-line format (adding the optional ` (<errorText>)` suffix), not the empty-result
    branch.
  - `Network.loadingFailed` never creates a new buffer entry (only updates an existing one found by
    `requestId`); if `Network.requestWillBeSent` for that same request was dropped by a CDP race
    (the accepted cross-domain-navigation race T12's log already documents) or arrived in a
    different tab's buffer, the failure is silently dropped rather than rendered anywhere -- this
    matches the prompt's Required-behavior item 1's explicit "If no entry is found: do nothing," is
    not a bug introduced by this task, and should not be "fixed" by a later task without a new task
    prompt requiring synthetic-entry creation on a `loadingFailed` miss.
  - The Network CDP domain continues to be enabled only lazily, on the first `read_network_requests`
    call per tab (unchanged); `Network.loadingFailed` events for a tab only start flowing into the
    buffer once that domain has been enabled for that tab AND the corresponding
    `Network.requestWillBeSent` was already buffered, exactly matching how `responseReceived`
    already behaved. No new domain-enable call was added anywhere.
- Browser checks queued: T14-1, T14-2, T14-3, T14-4 in docs/tasks/release-1/BROWSER-TESTS.md
  (appended after T13-3, preserving task order).

### T15 Empty-result guidance notes for read_console_messages and read_network_requests -- done -- 2026-07-02
- Commit: (recorded after commit; see git log for `feat(extension): T15 ...`)
- Files touched: extension/service-worker.js, docs/tasks/release-1/BROWSER-TESTS.md,
  docs/tasks/release-1/LEDGER.md
- Tests added: none in the Rust sense (this task touches only extension JS, which has no test
  harness per project constraints). Verification performed instead:
  - `node --check extension/service-worker.js` (syntax only), clean.
  - Full diff review confirming the change is scoped to exactly the two zero-result return paths:
    in each handler, `const total = buf.items.length;` was inserted immediately after
    `const buf = bufferFor(...)` (the point where the buffer is first read, before any filter),
    the `let msgs = buf.items;` / `let reqs = buf.items;` line and every filter/limit/clear line
    below it are byte-identical to before this task (confirmed via `git diff` hunk boundaries --
    no lines between the `total` insertion and the final `return` were touched except the return
    itself); the single ternary `return text(...)` was replaced with an early `if (msgs.length)
    return text(...)` (non-empty branch's inner expression byte-identical to before, including the
    `errorText`/`(pending)` ternary in the network renderer) followed by a `primary` local (ternary
    on `total`) and a final `return text(`${primary}\nNote: ...`)`.
  - Traced the `clear` interaction by hand: `if (a.clear) ...Buffer.set(a.tabId, { host, items: []
    });` executes unconditionally before the `if (msgs.length)` / `if (reqs.length)` check, exactly
    as before this task -- `clear` still empties the buffer even on a zero-match call, satisfying
    Required-behavior's explicit requirement and the task's own T15-5 browser-check scenario.
  - Compared every one of the six required exact strings (two primary-line pairs, two note lines)
    character-for-character against the prompt's Required-behavior section: `"No console messages
    recorded for this tab."`, `` `${total} console message(s) recorded for this tab, but none
    matched your filter.` ``, `"Note: console tracking begins when this tool is first used on a
    tab. Reload the page to capture messages emitted during page load."`, `"No network requests
    recorded for this tab."`, `` `${total} network request(s) recorded for this tab, but none
    matched your filter.` ``, `"Note: network tracking begins when this tool is first used on a
    tab. Reload the page to capture requests made during page load, or interact with the page to
    trigger new requests."` -- all six match verbatim, joined as `${primary}\nNote: ...` (exactly
    one `\n` between the two lines, no trailing newline), matching the prompt's two worked
    examples byte for byte.
  - `cargo test` (all 91 tests across the workspace, including `tests/tool_schema_fidelity.rs`,
    6/6) passes unchanged, confirming no Rust surface was touched.
  - `git status --short -- '*.rs' src/ tests/` was empty throughout -- this task made zero Rust
    changes, matching the prompt's "Build and test" note ("no Rust rebuild is needed").
  - `cargo clippy --all-targets -- -D warnings` clean (nothing to lint; no Rust changed).
  - `cargo fmt --check` reports only the same two pre-existing drifted files every prior task in
    this run has flagged (`src/policy/redact.rs`, `tests/tool_schema_fidelity.rs`); neither was
    touched by this task, and there was nothing to run `cargo fmt` on (zero Rust changes).
  - ASCII scan (the BOOTSTRAP.md python one-liner) on both edited files (`extension/service-
    worker.js`, `docs/tasks/release-1/BROWSER-TESTS.md`) returned empty lists.
- Drift reconciled: the prompt's "Current behavior" section describes the pre-T12 buffer shape
  (`consoleBuffer`/`networkBuffer` as bare `tabId -> array` Maps, reading
  `consoleBuffer.get(a.tabId) || []` / `networkBuffer.get(a.tabId) || []` directly) and cites line
  numbers (512-536) that predate T12/T13/T14 landing. T12 (earlier in the fixed sequence) had
  already changed both buffers to `tabId -> { host, items: [...] }` with a `bufferFor` choke point,
  and the actual handlers (at lines 613-655 before this task's edit) read `buf.items` via
  `bufferFor(consoleBuffer, a.tabId, host)` / `bufferFor(networkBuffer, a.tabId, host)`, not the
  prompt's stale `.get(a.tabId) || []` snippet. T14 had also already changed the network renderer's
  non-empty branch to include the optional `(errorText)` suffix, which the prompt's own "Current
  behavior" quote for `read_network_requests` (line 42) does not show. Reconciled by: (1) capturing
  `total` from `buf.items.length` (the actual pre-filter source of truth in the current code) at
  the same conceptual point the prompt specifies ("where the buffer is first read, before any
  filter or limit"), rather than the prompt's literal `.get(a.tabId) || []` line, which no longer
  exists; (2) leaving the non-empty branch's inner expression (including T14's `errorText` suffix)
  completely untouched, since the prompt's Required behavior explicitly says "the non-empty
  branches ... must not change in any way" and only describes the zero-result path. Both T13's and
  T14's own "Notes for later tasks" entries had already flagged this exact handoff (this task
  "touches the same ... zero-entries string ... but does not modify" the non-empty format), which
  matched what was found in the actual code.
- Decisions made:
  - Captured `total` as `const total = buf.items.length;` immediately after `const buf =
    bufferFor(...)`, one line before the pre-existing `let msgs = buf.items;` / `let reqs =
    buf.items;` line -- this is the literal "point where the buffer is first read" in the current
    (post-T12) code, satisfying the prompt's instruction without needing to reinterpret which line
    counts as "first read" now that the code no longer matches the prompt's cited line 517/531.
  - Did not add a shared helper function for the two-line assembly (the prompt explicitly allowed
    either choice: "a small shared function ... is acceptable; a copy in each handler is also
    acceptable"). Chose the inline-copy option: the two note strings differ (console vs network
    wording) and the two primary-line pairs differ, so a shared helper would need 4 string
    parameters for marginal benefit inside a single ~90-line handlers object; duplicating four short
    lines per handler was judged lower-risk and easier to verify against the prompt's exact-string
    requirement than threading four parameters through a new function.
  - Used an early `if (msgs.length) return text(...);` (guard-clause style) instead of keeping a
    single top-level ternary that now has three branches -- not mandated by the prompt, but the
    lowest-risk way to keep the non-empty branch's inner expression textually identical to the
    pre-task code (a straight cut of the original ternary's true-branch, unmodified) while adding
    the two-variant zero-result logic below it without deeply nesting a ternary-of-ternaries on one
    line.
  - Left `src/policy/redact.rs` and `tests/tool_schema_fidelity.rs` untouched (same pre-existing
    rustfmt-version drift every prior task in this run has flagged). This task touched no Rust
    files at all, so there was nothing to run `cargo fmt` on and no reformatting side effect to
    revert this time; confirmed via `cargo fmt --check`, whose only reported diffs are in exactly
    those same two files, byte-for-byte the same diffs T01/T02/T03/T12/T13/T14's logs already
    described.
- Notes for later tasks:
  - The zero-result contract (six exact strings: two primary-line pairs keyed on whether `total` is
    0, and two note lines) is now a byte-exact contract at the same tier as T01's marker-line
    formats, T02's Note line, T03's get_page_text contract, T06's `[hop: ...]` contract, T13's
    exception-text format, and T14's network per-line format -- do not reword any of the six
    strings in a later task without updating this note and the T15 BROWSER-TESTS.md entries.
  - `total` in both handlers is a local `const` scoped to that one call, computed once from
    `buf.items.length` right after the buffer lookup and never mutated; it does not persist across
    calls and has no relationship to the 1000-entry cap in `pushCapped` (a tab with more than 1000
    buffered items would still report `total` as (at most) 1000, since `pushCapped` itself caps the
    array -- this task did not change that cap and the prompt's Out-of-scope section explicitly
    excludes touching it).
  - No remaining release-1 task (per BOOTSTRAP.md's fixed sequence: T08, T09, T10, T11, T18, T16,
    T17, T05) touches `read_console_messages` or `read_network_requests` again; this concludes the
    T12/T13/T14/T15 chain of changes to those two handlers and their shared buffers.
  - No `src/mcp/schemas/tools.json` edits were made or needed; `tests/tool_schema_fidelity.rs`
    passed unchanged (6/6), confirming the frozen `read_console_messages`/`read_network_requests`
    schemas were left untouched, per this task's Constraints section.
- Browser checks queued: T15-1, T15-2, T15-3, T15-4, T15-5, T15-6 in
  docs/tasks/release-1/BROWSER-TESTS.md (appended after T14-4, preserving task order).

### T08 computer type dispatches real keyDown/keyUp per character with Enter mapping -- done -- 2026-07-02
- Commit: (recorded after commit; see git log for `feat(extension): T08 ...`)
- Files touched: extension/service-worker.js, docs/tasks/release-1/BROWSER-TESTS.md,
  docs/tasks/release-1/LEDGER.md
- Tests added: none in the Rust sense (this task touches only extension JS, which has no test
  harness per project constraints). Verification performed instead:
  - `node --check extension/service-worker.js` (syntax only), clean.
  - A standalone throwaway Node script (not committed; written to the session scratchpad and not
    copied into the repo) that copied `keyCode`/`vkCode`/`charKeyInfo` verbatim from the new code
    and asserted: (a) all 95 printable ASCII characters (0x20-0x7E) resolve to a non-null
    `charKeyInfo` result with a non-zero `vk`; (b) `"\t"` and the accented `"e"` (U+00E9) both
    resolve to `null`; (c) `"\n"` maps to the exact `{ key: "Enter", code: "Enter", vk: 13,
    shift: false, text: "\r", unmodifiedText: "\r" }` object from the spec; (d) the prompt's own
    worked example (`"Ab1!;:\n"`) reproduces the exact seven-entry keydown sequence
    (`keydown|A|KeyA|1`, `keydown|b|KeyB|0`, `keydown|1|Digit1|0`, `keydown|!|Digit1|1`,
    `keydown|;|Semicolon|0`, `keydown|:|Semicolon|1`, `keydown|Enter|Enter|0`); (e) `"a\r\nb"`
    collapses to exactly one Enter between `a` and `b` (CRLF-skip logic); (f) `"cafe"` + accented
    `"e"` produces `dispatch, dispatch, dispatch, insertText` for the four characters, proving the
    fallback fires only for the non-ASCII character. All checks passed.
  - Full diff review confirming: `keyCode`/`vkCode` gained exactly one new branch each (`if
    (CODE_PUNCT[key]) return CODE_PUNCT[key];` / `if (VK_PUNCT[key]) return VK_PUNCT[key];`),
    inserted after the existing letter/digit branches and before the pre-existing fallback line,
    which is otherwise byte-identical to before this task; `pressKey`'s own body (including the
    reload-chord interception added by an earlier task) was not touched at all (confirmed via
    `git diff` hunk boundaries -- no lines inside `pressKey` appear in the diff); the `key` action
    case in `computer()` was not touched; `VK_PUNCT`, `CODE_PUNCT`, `SHIFT_BASE`, and
    `charKeyInfo` are new top-level declarations placed directly after `VK_NAMED` and `vkCode`
    respectively, matching the prompt's tables verbatim (including punctuation coverage and key
    order); the `type` case's guard line and success-return line are byte-identical to before this
    task (`if (!a.text) return text("text is required for type.");` and
    `` return text(`Typed ${a.text.length} character(s).`); ``, still using `a.text.length`, the
    raw string length, not the code-point count); the new loop body matches the prompt's exact
    code block (the `mods`/`evt` shape, the `...evt` spreads, the `text`/`unmodifiedText` fields
    only on `keyDown`) verbatim; no `try`/`catch` was added around any `cdp(...)` call in the new
    loop.
  - `cargo test` (all 91 tests across the workspace, including `tests/tool_schema_fidelity.rs`,
    6/6) passes unchanged, confirming no Rust surface was touched.
  - `git status --short -- '*.rs' src/ tests/` was empty throughout -- this task made zero Rust
    changes, matching the prompt's "Build and test" note ("no Rust rebuild is required").
  - `cargo clippy --all-targets -- -D warnings` clean (nothing to lint; no Rust changed).
  - `cargo fmt --check` reports only the same two pre-existing drifted files every prior task in
    this run has flagged (`src/policy/redact.rs`, `tests/tool_schema_fidelity.rs`); neither was
    touched by this task, and there was nothing to run `cargo fmt` on (zero Rust changes).
  - ASCII scan (the BOOTSTRAP.md python one-liner) on all three edited/touched files
    (`extension/service-worker.js`, `docs/tasks/release-1/BROWSER-TESTS.md`,
    `docs/tasks/release-1/LEDGER.md`) returned empty lists. Note: a first draft of the
    BROWSER-TESTS.md T08-2 entry accidentally typed the literal accented "cafe" word (copying the
    prompt's own prose) instead of describing the JSON escaped-accented-e sequence the prompt's
    Verification step 3 actually specifies for the MCP call argument; caught by the ASCII scan before
    committing and rewritten to describe the escape sequence in words instead of embedding the
    literal character.
- Drift reconciled: only line-number and one section-order drift, exactly as BOOTSTRAP.md warned
  ("line numbers verified at authoring time and DRIFT as earlier tasks land"). The prompt's
  "Current behavior" section cites `computer(a)` starting at line 357, the `type` case at lines
  390-395, `modifierBits`/`pressKey` context at lines 260-268/288-320, `keyCode` at 322-328,
  `VK_NAMED` at 330-334, `vkCode` at 335-342, and `KEY_MAP` at 253-259. The actual working tree
  (after T04/T06/T07/T12/T13/T14/T15 all landed earlier in the fixed sequence and grew the file
  from ~568 lines the prompt describes to 689 lines) had these at: `KEY_MAP` 341-347,
  `modifierBits` 348-357, `pressKey` 378-410, `keyCode` 412-418, `VK_NAMED` 420-424, `vkCode`
  425-432, `computer(a)` 447 (before this task's edits), `type` case 480-484. Every function name,
  table contents (VK_NAMED's exact 15 entries), and code shape the prompt describes matched the
  actual working tree verbatim once re-read at the drifted line numbers -- no logic-level drift.
  One additional thing the prompt's "Current behavior" section does not mention (because it
  predates this run's own earlier tasks): `pressKey` now contains a reload-chord interception
  block (ctrl/cmd+r, F5) inserted by an earlier task, sitting between the `key`/`modifiers`
  resolution and the `keyCode`/`vkCode` calls the prompt describes. This does not affect step 1's
  "do not change the body of pressKey" instruction (still honored exactly: zero lines of
  `pressKey` were touched) and does not affect anything else in this task's scope.
- Decisions made:
  - Placed `VK_PUNCT` and `CODE_PUNCT` directly after `VK_NAMED` (before `vkCode`), and
    `SHIFT_BASE`/`charKeyInfo` directly after `vkCode` (before `waitForLoad`), rather than
    scattering them by which function reads which table. This satisfies the prompt's "next to
    VK_NAMED" / "place it after vkCode" placement instructions literally, keeps every
    keyboard-mapping table and helper in one contiguous block, and is safe: `keyCode` (declared
    just above `VK_NAMED`) references `CODE_PUNCT` only inside its function body, which is
    evaluated on each call, long after all module-level `const` declarations have run -- there is
    no temporal-dead-zone issue since `keyCode` is never invoked during module initialization
    (verified by reading the file top to bottom: no top-level code calls `keyCode` or `vkCode`
    outside of `pressKey`/`charKeyInfo`, both of which only run in response to a `computer` tool
    call, long after the module has finished loading).
  - Used `if (CODE_PUNCT[key])` / `if (VK_PUNCT[key])` (plain truthiness) rather than an
    `Object.prototype.hasOwnProperty` or `!== undefined` check, matching the existing file's style
    for every other lookup-table branch in this exact function pair (e.g. `VK_NAMED[key] || 0`)
    and matching the prompt's own instruction to return the table's value "when present" -- every
    value in both tables is a non-empty string or a positive number, so truthiness is exactly
    equivalent to presence for this data.
  - `charKeyInfo`'s printable-ASCII guard (`if (ch < " " || ch > "~") return null;`) was placed
    after the newline/CR check, matching the prompt's own step ordering (newline mapping first,
    then the printable-ASCII guard, then the shift/base computation) rather than reordering for
    perceived efficiency; verified by hand that a surrogate-pair or astral code point from
    `Array.from` (length 2) still correctly returns `null` via this same string comparison (its
    leading UTF-16 code unit is always numerically greater than `"~"`, so the comparison holds
    without a separate length check).
  - Added exactly two short `//` comments beyond the prompt's own snippets: one above
    `charKeyInfo` describing its null/non-null contract (matching the file's existing
    one-line-doc-comment density on helpers like `pressKey`'s neighbors), and one inline comment
    on the CRLF-skip line inside the new loop. No comment was added on the `mods`/`evt`
    construction block or the two `cdp(...)` dispatch lines, since the prompt's own reference code
    block for that part carries no comments and constraint 7 sets those snippets as the ceiling.
  - Left `src/policy/redact.rs` and `tests/tool_schema_fidelity.rs` untouched (same pre-existing
    rustfmt-version drift every prior task in this run has flagged). This task touched no Rust
    files at all, so there was nothing to run `cargo fmt` on and no reformatting side effect to
    revert this time; confirmed via `cargo fmt --check`, whose only reported diffs are in exactly
    those same two files, byte-for-byte the same diffs T01/T02/T03/T12/T13/T14/T15's logs already
    described.
- Notes for later tasks:
  - `charKeyInfo(ch)` is a new pure helper in `extension/service-worker.js`, module-scope, used
    only by the new `type`-case loop. It has no dependency on and is not called by `pressKey` or
    the `key` action; a later task should not need to touch it unless a new task prompt explicitly
    requires it.
  - `keyCode`/`vkCode` now also cover the eleven US-QWERTY punctuation characters (plus Space for
    `keyCode`); this is a side effect `pressKey` (and therefore the `key` action) picks up for free
    on any single-character combo containing punctuation (for example a hypothetical `key` call
    with `text: ";"` now gets `code: "Semicolon"` / `vk: 186` instead of the old wrong
    `code: ";"` / `vk: 0`) -- this was explicitly called out as an intended side effect by the
    prompt's step 1, not a bug to "fix" differently in a later task.
  - The `type` action's per-character dispatch contract (real `keyDown`/`keyUp` pairs with the
    exact `evt` shape, the Shift-bit-only modifier approach, the `Input.insertText` fallback for
    non-printable-ASCII/control characters, and the `\r\n`-collapsing rule) is now a byte-exact
    behavioral contract at the same tier as T01's marker-line formats, T02's Note line, T03's
    get_page_text contract, T06's `[hop: ...]` contract, T13's exception-text format, T14's
    network per-line format, and T15's zero-result strings -- do not rework it in a later task
    without updating this note and the T08 BROWSER-TESTS.md entries.
  - No remaining release-1 task in the fixed sequence (T09, T10, T11, T18, T16, T17, T05) touches
    `pressKey`, `keyCode`, `vkCode`, `VK_NAMED`, `KEY_MAP`, or the `type`/`key` cases of
    `computer()` again, except T09 (mouse click fidelity) which is adjacent in the same `computer`
    dispatcher but touches only the `click`/`resolveCoords` mouse-event path, not keyboard code.
  - No `src/mcp/schemas/tools.json` edits were made or needed; `tests/tool_schema_fidelity.rs`
    passed unchanged (6/6), confirming the frozen `computer` schema (whose `type` action
    description does not describe implementation detail, only behavior) was left untouched, per
    this task's Constraints section.
- Browser checks queued: T08-1, T08-2, T08-3 in docs/tasks/release-1/BROWSER-TESTS.md (appended
  after T15-6, preserving task order).

### T09 Mouse click fidelity: incrementing clickCount sequence, buttons bitmask, force -- done -- 2026-07-02
- Commit: (recorded after commit; see git log for `feat(extension): T09 ...`)
- Files touched: extension/service-worker.js, docs/tasks/release-1/BROWSER-TESTS.md,
  docs/tasks/release-1/LEDGER.md
- Tests added: none in the Rust sense (this task touches only extension JS, which has no test
  harness per project constraints). Verification performed instead:
  - `node --check extension/service-worker.js` (syntax only), clean.
  - Full diff review confirming the change is scoped to exactly three spots: the two new
    module-level constants (`BUTTON_BITS`, `CLICK_GAP_MS`) placed directly after `KEY_MAP`; the
    body of `click(tabId, x, y, opts)`, whose signature, `opts.modifiers`/`opts.button`/
    `opts.clickCount` reads, and the surrounding functions (`modifierBits`, `resolveCoords`) are
    byte-identical to before this task; and the four `Input.dispatchMouseEvent` call sites inside
    the `left_click_drag` case, where only the params object gained `buttons`/`force` fields --
    coordinate rescaling, `moveCursor` calls, the 10-step interpolation loop bounds/formula, the
    16ms/40ms sleeps, and the `` `Dragged (${sx}, ${sy}) -> (${ex}, ${ey}).` `` result text are
    all untouched (confirmed via `git diff` hunk boundaries).
  - Traced the new `click()` loop by hand for N=1, 2, 3: one `mouseMoved` (buttons:0, force:0),
    then N press/release pairs with `clickCount` taking the values 1, then 1-2, then 1-2-3 in
    order (never a pair whose first clickCount is 2 or 3), each `mousePressed` carrying
    `buttons: bit, force: 0.5` and each `mouseReleased` carrying `buttons: 0, force: 0`, with a
    `CLICK_GAP_MS` sleep after the leading move, after every press, after every release, and
    between iterations (but not after the final release) -- matching the prompt's five-step
    algorithm exactly. Confirmed the call site in `computer()`'s click branch (`click(tabId, c[0],
    c[1], { button, clickCount, modifiers })`, where `clickCount` is 2 for `double_click` and 3
    for `triple_click`) was not touched, so `click()` still receives N and now expands it into the
    loop itself, as required.
  - Confirmed `BUTTON_BITS` is read via `BUTTON_BITS[button] || 0` inside `click()` (falls back to
    0 for an unrecognized button name, matching the file's existing lookup-table style elsewhere
    in this same file, e.g. `VK_NAMED[key] || 0`) and via the literal `BUTTON_BITS.left` in the
    three drag dispatch sites that always use the left button.
  - Grepped the diff for `clickCount` inside the `left_click_drag` case: zero matches, confirming
    the drag press/release events still omit `clickCount` entirely (CDP defaults it to 0), per the
    prompt's explicit "do not add clickCount to the drag path" instruction.
  - `cargo test` (all 91 tests across the workspace, including `tests/tool_schema_fidelity.rs`,
    6/6) passes unchanged, confirming no Rust surface was touched.
  - `git status --short -- '*.rs' src/ tests/` was empty throughout -- this task made zero Rust
    changes, matching the prompt's "Build and test" note ("the Rust binary is not rebuilt for this
    task").
  - `cargo clippy --all-targets -- -D warnings` clean (nothing to lint; no Rust changed).
  - `cargo fmt --check` reports only the same two pre-existing drifted files every prior task in
    this run has flagged (`src/policy/redact.rs`, `tests/tool_schema_fidelity.rs`); neither was
    touched by this task, and there was nothing to run `cargo fmt` on (zero Rust changes).
  - ASCII scan (the BOOTSTRAP.md python one-liner) on both edited files (`extension/service-
    worker.js`, `docs/tasks/release-1/BROWSER-TESTS.md`) returned empty lists.
- Drift reconciled: only line-number drift, exactly as the prompt itself warned ("all facts below
  were verified... 568 lines" -- earlier tasks in this run, especially T08, had already landed and
  grown the file to 740 lines before this task's edits). The prompt cited `sleep(ms)` at lines
  242-244, `modifierBits` at 260-269, `click()` at 270-277, the click branch of `computer(a)` at
  373-389, `left_click_drag` at 422-439, and the scroll `mouseWheel` dispatch at line 412; the
  actual working tree had these at `sleep` 330-332, `modifierBits` 348-357, `click()` 358-365,
  `computer(a)`'s click branch 495-511, `left_click_drag` 570-587, and the `mouseWheel` dispatch at
  553. In every case the function bodies, exact event sequences (`mouseMoved`, `sleep(40)`,
  `mousePressed`, `sleep(40)`, `mouseReleased` for `click()`; the ten-step interpolation for
  `left_click_drag`), and field lists the prompt described (button/clickCount/modifiers only, no
  buttons/force) matched the working tree verbatim -- no logic-level drift, only line numbers.
- Decisions made:
  - Placed `BUTTON_BITS` and `CLICK_GAP_MS` immediately after the `KEY_MAP` object literal (before
    `modifierBits`), matching the prompt's "near KEY_MAP" placement instruction literally while
    keeping both new declarations in the same "Input helpers" section as `sleep`, `modifierBits`,
    and `click` itself.
  - Kept the local variable name `clickCount` in `click(tabId, x, y, opts)` unchanged (still read
    from `opts.clickCount || 1`) and reused it directly as the loop's upper bound (`for (let i = 1;
    i <= clickCount; i++)`), rather than renaming it to `n`/`N` -- the prompt's own instruction was
    "keep reading... clickCount... from opts as today", and the loop variable `i` (the per-pair
    clickCount value dispatched on the wire) is kept distinct from the outer `clickCount` (the
    requested total), so there is no naming ambiguity: `clickCount` is always "how many pairs to
    send", `i` is always "this pair's CDP clickCount value".
  - Added exactly one comment (a two-line `//` note directly above the `for` loop) explaining that
    real N-clicks are N pairs with incrementing clickCount, matching constraint 7's guidance ("One
    short comment explaining the incrementing clickCount loop... is appropriate; do not comment
    each field"); no comment was added on the `bit` computation, the four field additions inside
    the loop, or any of the four drag dispatch sites (their `buttons`/`force` fields are read as
    self-explanatory next to the existing `button: "left"` field, matching the file's general
    preference for code-as-documentation over inline prose).
  - Left `src/policy/redact.rs` and `tests/tool_schema_fidelity.rs` untouched (same pre-existing
    rustfmt-version drift every prior task in this run has flagged). This task touched no Rust
    files at all, so there was nothing to run `cargo fmt` on and no reformatting side effect to
    revert this time; confirmed via `cargo fmt --check`, whose only reported diffs are in exactly
    those same two files, byte-for-byte the same diffs T01/T02/T03/T12/T13/T14/T15/T08's logs
    already described.
- Notes for later tasks:
  - `BUTTON_BITS` and `CLICK_GAP_MS` are now module-level constants available to any later task in
    this file. `BUTTON_BITS.middle` (4) exists as a constant only -- no middle-click action,
    parameter, or dispatch path was added anywhere, per the prompt's explicit out-of-scope note;
    a later task must not wire it in without its own task prompt requiring it.
  - `click()`'s new event-count behavior (1 pair for a single click, 2 pairs for double, 3 for
    triple, each with incrementing `clickCount`, plus `buttons`/`force` on every event) and the
    `left_click_drag` path's new `buttons`/`force` fields are now a byte-exact behavioral contract
    at the same tier as T08's type-dispatch contract, T01's marker-line formats, T02's Note line,
    T03's get_page_text contract, T06's `[hop: ...]` contract, T13's exception-text format, T14's
    network per-line format, and T15's zero-result strings -- do not rework the event sequence,
    field values, or timing in a later task without updating this note and the T09 BROWSER-TESTS.md
    entries.
  - The hover branch (`case "hover"` inside the click-family switch arm, still a bare `mouseMoved`
    with no `buttons`/`force`), the scroll `mouseWheel` dispatch (T10's target), and
    `resolveCoords`/`rescaleCoord` were all confirmed untouched, per this task's own out-of-scope
    list; T10 (scroll verify + scrollable-ancestor fallback) is the next task and touches the
    scroll/scroll_to cases of the same `computer()` switch, adjacent to but disjoint from this
    task's changes.
  - No `src/mcp/schemas/tools.json` edits were made or needed; `tests/tool_schema_fidelity.rs`
    passed unchanged (6/6), confirming the frozen `computer` schema (whose click-action
    descriptions do not describe implementation detail, only behavior) was left untouched, per this
    task's Constraints section.
- Browser checks queued: T09-1, T09-2, T09-3, T09-4, T09-5 in docs/tasks/release-1/BROWSER-TESTS.md
  (appended after T08-3, preserving task order).

### T10 Scroll verify + scrollable-ancestor fallback -- done -- 2026-07-02
- Commit: (recorded after commit; see git log for `feat(extension): T10 ...`)
- Files touched: extension/service-worker.js, docs/tasks/release-1/BROWSER-TESTS.md,
  docs/tasks/release-1/LEDGER.md
- Tests added: none in the Rust sense (this task touches only extension JS, which has no test
  harness per project constraints). Verification performed instead:
  - `node --check extension/service-worker.js` (syntax only), clean.
  - A standalone node one-liner that assembled both helpers' exact `expression` strings (same
    template-literal composition as the real code, including the shared `SCROLLABLE_FINDER_SNIPPET`
    interpolation) and parsed each with `new Function(...)` to confirm they are syntactically valid
    JS before ever reaching a real page (a function declaration inside an arrow-function block body
    is legal Annex B sloppy-mode JS; both parsed cleanly).
  - Full diff review confirming the change is scoped to exactly two spots: two new module-level
    helpers (`probeScrollState`, `directScrollFallback`) plus one new shared string constant
    (`SCROLLABLE_FINDER_SNIPPET`) inserted directly after `resolveCoords`; and the body of the
    `scroll` case in `computer()`'s switch, rewritten per the prompt's ten-step flow. Confirmed by
    hunk boundaries that `scroll_to` (the very next case), `left_click_drag`, `resolveCoords`,
    `moveCursor`, `rescaleCoord`, and the screenshot pipeline are byte-identical to before this task.
  - Traced the new `scroll` case by hand against the prompt's Result A/B/C/D contract: `before` is
    probed BEFORE `moveCursor`/dispatch (matching step order 1-2-3-4-5 exactly); `before === null`
    short-circuits to the legacy 250ms-sleep/Result-A path; otherwise a 200ms settle then a second
    probe, with `after === null` also short-circuiting to Result A without running the fallback (per
    the prompt's explicit "do not run the fallback when the re-read failed" rule); `windowMoved`/
    `elementMoved` use the literal 5px threshold with `after.elX/elY` and `before.elX/elY` coerced
    via `|| 0` only in the diff arithmetic (not in the `hasEl` gate); the fallback path returns
    Result D on `fb === null`, Result B on `fb.moved`, Result C otherwise -- all three fallback
    result strings, and the two Result-A call sites, reproduce the prompt's four verbatim templates
    exactly (`Scrolled ${dir} by ${amount}.`, the `(mouse wheel had no effect...)` suffix, and the
    two `Scroll ${dir} had no effect at (${c[0]}, ${c[1]}); ...` variants).
  - Confirmed `deltaX`/`deltaY`, the `amount * 100` magnitudes, the cap of 10, the `[0, 0]`
    coordinate default, the `moveCursor` call, and the `modifiers` pass-through on the wheel dispatch
    are byte-identical to the pre-task code (only the `before`/`after` probe calls were interleaved
    around the existing lines; grepped for `deltaX =`/`deltaY =`/`Math.min(a.scroll_amount` -- one
    match each, unchanged).
  - Confirmed via `git diff` that both new helpers wrap their entire body (including the `await
    cdp(...)` call) in `try { ... } catch { return null; }`, and that the failure check
    (`!r || r.exceptionDetails || !r.result || r.result.value === undefined`) matches the prompt's
    three named failure conditions (`cdp` rejects, `exceptionDetails` present, `result.value`
    missing) exactly -- neither helper can throw.
  - `cargo test` (all 91 tests across the workspace, including `tests/tool_schema_fidelity.rs`, 6/6)
    passes unchanged, confirming no Rust surface was touched.
  - `git status --short -- '*.rs' src/ tests/` was empty throughout -- this task made zero Rust
    changes, matching the prompt's "Build and test" note ("this task changes only extension
    JavaScript, so no Rust rebuild is required").
  - `cargo clippy --all-targets -- -D warnings` clean (nothing to lint; no Rust changed).
  - `cargo fmt --check` reports only the same two pre-existing drifted files every prior task in
    this run has flagged (`src/policy/redact.rs`, `tests/tool_schema_fidelity.rs`); neither was
    touched by this task, and there was nothing to run `cargo fmt` on (zero Rust changes).
  - ASCII scan (the BOOTSTRAP.md python one-liner) on both edited files (`extension/service-
    worker.js`, `docs/tasks/release-1/BROWSER-TESTS.md`) returned empty lists.
- Drift reconciled: only line-number drift, exactly as the prompt itself warned ("line numbers
  verified against the current tree" -- earlier tasks in this run, especially T08/T09, had already
  grown the file). The prompt cited the `scroll` case at lines 405-415, `resolveCoords` at 278-287,
  `cdp` at 114-117, `sleep` at 242-244, `text`/`textImage` at 207-212, `screenshot` at 215-239, and
  the screenshot-contract comment at line 356; the actual working tree had these at `scroll`
  556-565, `resolveCoords` 376-387, `cdp` 136-143, `sleep` 330-332, `text`/`textImage` 295-300,
  `screenshot` 303-327, and the screenshot-contract comment (worded slightly differently, as
  `// --- computer (13 actions; screenshots only on screenshot/scroll/zoom) ---`) at line 547. In
  every case the function bodies, exact event/params shapes, and the "only screenshot/scroll/zoom
  return an image" contract itself matched the working tree verbatim -- no logic-level drift, only
  line numbers and one comment's exact wording (still asserting the same contract).
- Decisions made:
  - Introduced one additional module-level constant, `SCROLLABLE_FINDER_SNIPPET` (a template-
    literal string holding the `findScrollable(px, py)` ancestor-walk function body), shared by both
    new helpers via string interpolation, rather than duplicating the walk predicate literally
    inside each helper's `expression` string. The prompt describes the predicate as "used by both
    helpers, inside the evaluated snippets" without mandating either duplication or extraction;
    sharing one definition means the two snippets cannot silently drift out of sync with each other,
    which better serves constraint 4 (the engine is truthful: the fallback's target-finding must
    exactly match what was measured in the probe, or the before/after comparison and the fallback's
    own target choice could disagree). Verified both resulting `expression` strings still parse as
    valid JS (see Tests added above) and that the walk semantics are identical to the prompt's
    two-part predicate (overflow-y/overflow-x auto-or-scroll on either axis, AND scrollHeight/
    clientHeight or scrollWidth/clientWidth overflow on either axis -- an OR of ORs, not a per-axis
    AND, matching the prompt's literal wording).
  - Rejected an earlier draft that built Result B via `` `${scrolled.slice(0, -1)} (mouse wheel...)` ``
    (stripping the trailing period off the cached Result-A string and appending the suffix) in favor
    of writing Result B as its own independent template literal. The slice-based version was byte-
    identical in output but harder to verify as byte-exact against the prompt's contract by
    inspection alone; constraint 4's "do not soften or merge them" reads most safely as "keep the
    four result strings independently legible", so the more literal form was kept despite the tiny
    duplication of `Scrolled ${dir} by ${amount}` between Result A and Result B.
  - Cached the Result-A message once as `const scrolled = \`Scrolled ${dir} by ${amount}.\`;` and
    reused it at its three call sites (before===null, after===null, windowMoved||elementMoved) since
    `dir`/`amount` do not change across those branches within one call -- this guarantees the three
    Result-A sites can never disagree with each other, and is a strict textual match for
    `` `Scrolled ${dir} by ${amount}.` `` (verified by direct string construction, not just visual
    inspection).
  - `elementMoved`'s null-to-0 coercion (`|| 0`) is applied only inside the absolute-difference
    arithmetic, never in the `before.hasEl && after.hasEl` gate itself -- per the prompt's exact
    wording ("treat null as 0 in the arithmetic"), so a probe that never found a scrollable ancestor
    (`hasEl: false`, `elX/elY: null`) cannot spuriously satisfy `elementMoved` via `0 - 0 > 5`
    (impossible) or, more importantly, cannot mask a real ancestor find/lose transition between the
    two probes (the `hasEl` gate requires both probes to have found one).
  - Left `src/policy/redact.rs` and `tests/tool_schema_fidelity.rs` untouched (same pre-existing
    rustfmt-version drift every prior task in this run has flagged). This task touched no Rust files
    at all, so there was nothing to run `cargo fmt` on and no reformatting side effect to revert this
    time; confirmed via `cargo fmt --check`, whose only reported diffs are in exactly those same two
    files, byte-for-byte the same diffs every prior task's log already described.
- Notes for later tasks:
  - `probeScrollState(tabId, x, y)` and `directScrollFallback(tabId, x, y, dx, dy)` are now
    module-level helpers available to any later task in this file; both are exception-safe (resolve
    to a value or `null`, never reject) and both round every interpolated coordinate/delta with
    `Math.round` before building their `expression` string, per constraint 12 ("interpolate only
    rounded numbers"). Neither is wired into any action besides `scroll` -- `scroll_to` was
    explicitly out of scope and was not touched (confirmed unchanged by diff).
  - `SCROLLABLE_FINDER_SNIPPET` is a raw JS-source string constant (not a callable function in the
    service worker's own scope); it only makes sense interpolated inside a `Runtime.evaluate`
    expression string. A later task adding a third scroll-related helper that needs the same
    ancestor-walk predicate should reuse this constant rather than redefining the walk logic again.
  - The four result texts (`Scrolled ${dir} by ${amount}.`, the fallback-used suffix, and the two
    no-effect variants) are now a byte-exact contract at the same tier as T09's click-event contract,
    T08's type-dispatch contract, T01's marker-line formats, T02's Note line, T03's get_page_text
    contract, T06's `[hop: ...]` contract, T13's exception-text format, T14's network per-line
    format, and T15's zero-result strings -- do not reword any of the four in a later task without
    updating this note and the T10 BROWSER-TESTS.md entries.
  - T11 (real zoom region crop + coordinate-context update) is next and touches the `zoom` case of
    the same `computer()` switch (adjacent to, but disjoint from, this task's `scroll` changes) plus
    likely `resolveCoords`/`rescaleCoord`/`screenshotCtx` for the coordinate-context update; it does
    not depend on T10 per the ledger's Depends-on column ("T11 helpful, not required" is actually
    listed the other way -- T18 depends helpfully on T11, T11 itself has no listed dependency), so no
    blocking concern here. The `scroll` case's shape (probe-before, dispatch, probe-after, branch on
    verified movement) is a new pattern in this file; T11 has no stated need to follow it, but a
    future task touching `zoom`'s own verification (if ever proposed) could look here for precedent.
- Browser checks queued: T10-1, T10-2, T10-3, T10-4 in docs/tasks/release-1/BROWSER-TESTS.md
  (appended after T09-5, preserving task order).

### T11 Real zoom region crop + coordinate-context update -- done -- 2026-07-02
- Commit: (recorded after commit; see git log for `feat(extension): T11 ...`)
- Files touched: extension/service-worker.js, docs/tasks/release-1/BROWSER-TESTS.md,
  docs/tasks/release-1/LEDGER.md
- Tests added: none in the Rust sense (this task touches only extension JS, which has no test
  harness per project constraints). Verification performed instead:
  - `node --check extension/service-worker.js` (syntax only), clean.
  - A standalone throwaway Node script (not committed; deleted from scratchpad after use) that
    copied `zoomScale` and the new `rescaleCoord` body verbatim and asserted: (a) eight regions
    from a tiny 1x1 box to a 3840x2160 box all produce an output within the 1568-token AND
    1568px-longest-side budget (magnifying small regions by over 1000x, downscaling a 4K-sized
    region to ~0.38x); (b) a full-screenshot context (offX=0, offY=0, regionW=vpW, regionH=vpH)
    reproduces the exact pre-task `rescaleCoord` formula numerically (confirming clicks after a
    plain screenshot behave identically to before this task); (c) a synthetic chained-zoom
    scenario (offX=100, offY=100, regionW=200, regionH=200 from a first zoom) maps the center of
    the zoomed image back to the center of the original 200x200 region ([200, 200]), confirming
    the offset-addition formula; (d) the no-context passthrough (`undefined` context) still
    rounds coordinates through unchanged, matching the pre-task behavior for a tab with no prior
    screenshot.
  - Full diff review confirming: the zoom case in `computer(a)` matches the prompt's exact
    validation/dispatch snippet verbatim (three error strings, the success template with the
    conditional "; clamped to the visible viewport" suffix); `zoomScale` was added directly after
    `targetDims`, byte-identical to the prompt's snippet (magnify-or-shrink scale search with the
    0.98 correction loop); `zoomScreenshot` was added directly after `screenshot()`, implementing
    all ten steps in order (`ensureAttached`, the combined viewport+scroll-offset `Runtime.
    evaluate` probe, the pre-overwrite `rescaleCoord` calls on both corners, the `[0,vpW]`/
    `[0,vpH]` clamp with a `clamped` flag, the `w<1||h<1` empty-region guard, `zoomScale`, the
    hide/sleep/capture-with-clip/show rhythm mirroring `screenshot()`'s, the re-encode-under-
    budget block mirroring lines 318-324 of `screenshot()` with the bitmap's own actual width/
    height as fallback-corrected dims, the final `screenshotCtx.set` with the new offset/region
    fields, and the `{ base64, x0, y0, x1, y1, clamped }` return shape); `rescaleCoord`'s body was
    replaced exactly as specified (the `|| c.vpW`/`|| c.vpH` fallbacks, the `offX`/`offY` addition);
    `screenshot()`'s only change is the single `screenshotCtx.set` line (now carrying `offX: 0,
    offY: 0, regionW: vpW, regionH: vpH`) -- confirmed via `git diff` hunk boundaries that no other
    line inside `screenshot()`, `probeViewport`, `targetDims`, `encodeJpeg`, `bytesFromBase64`, or
    `base64FromBytes` changed.
  - Confirmed exactly two new module-level functions were added (`zoomScale`, `zoomScreenshot`)
    and no new module-level state/constants beyond what the prompt's section 2 specifies (grepped
    `git diff` for new top-level `const`/`let` -- none outside the two function declarations
    themselves).
  - Confirmed the three other `rescaleCoord` consumers (`resolveCoords` at the coordinate branch,
    and the two `left_click_drag` endpoints) were not touched -- they automatically inherit the
    generalized offset-aware formula with zero changes to their own call sites, exactly as the
    prompt's Required-behavior intends.
  - `cargo test` (all 91 tests across the workspace, including `tests/tool_schema_fidelity.rs`,
    6/6) passes unchanged, confirming no Rust surface was touched and the frozen `computer` schema
    (whose `region` parameter and `zoom` action description were already present, per the prompt's
    Constraints) is intact.
  - `git status --short -- '*.rs' src/ tests/` was empty throughout -- this task made zero Rust
    changes, matching the prompt's "Build and test" note ("the Rust binary is not rebuilt for this
    task").
  - `cargo clippy --all-targets -- -D warnings` clean (nothing to lint; no Rust changed).
  - `cargo fmt --check` reports only the same two pre-existing drifted files every prior task in
    this run has flagged (`src/policy/redact.rs`, `tests/tool_schema_fidelity.rs`); neither was
    touched by this task, and there was nothing to run `cargo fmt` on (zero Rust changes).
  - ASCII scan (the BOOTSTRAP.md python one-liner) on all three edited files (`extension/service-
    worker.js`, `docs/tasks/release-1/BROWSER-TESTS.md`, `docs/tasks/release-1/LEDGER.md`) returned
    empty lists.
- Drift reconciled: only line-number drift and one guard-style detail, as the prompt itself warned
  ("line numbers verified at authoring time and DRIFT as earlier tasks land"). The prompt's
  "Current behavior" section cites `screenshotCtx` at line 16, budget constants at line 70,
  `probeViewport`/`targetDims`/`encodeJpeg`/`rescaleCoord` at lines 72-113, `screenshot()` at
  214-239, and the zoom case at 366-367 (in a 568-line file); the actual working tree (grown to
  840 lines by every earlier task in this run) had these at line 19, line 92, lines 94-127, lines
  303-327, and 557-558 respectively -- same shapes, only line numbers moved. One substantive detail
  had also drifted: the prompt's step 2 says to guard the new combined viewport+scroll probe "same
  guard style as probeViewport", quoting `probeViewport`'s OLD plain `throw new Error("failed to
  probe viewport")`. T06 (hop-attributed error reporting, earlier in the fixed sequence) had
  already changed `probeViewport`'s actual guard to `throw hopError("page", "failed to probe
  viewport")`. Reconciled literally per the prompt's own instruction ("same guard style as
  probeViewport") by using the ACTUAL current guard (`hopError("page", ...)`), not the prompt's
  stale quoted line -- this keeps the new probe's failure mode consistent with T06's hop-attributed
  error contract (a page-hop failure), which is what "same guard style as probeViewport" means once
  probeViewport itself changed.
- Decisions made:
  - The clamp step (Required-behavior item 4) was implemented as independent per-corner clamps
    (`x0 = clamp(rx0)`, `x1 = clamp(rx1)`, no min/max reordering of `rx0`/`rx1` against each other)
    exactly as the prompt's literal wording specifies ("Clamp to the viewport: x values to [0,
    vpW]... producing x0, y0, x1, y1"), rather than adding defensive min/max sorting of the two
    rescaled corners first. This is safe because `rescaleCoord` is a positive-scale affine map
    (verified by inspection: `rw`/`shotW`/`rh`/`shotH` are always positive), so it preserves the
    ordering guaranteed by the zoom case's own pre-check (`r[2] > r[0] && r[3] > r[1]`, in
    screenshot-space, before rescale) -- `rx1 > rx0` and `ry1 > ry0` always hold after rescale, so
    the unsorted per-corner clamp cannot silently produce an inverted region; verified numerically
    in the throwaway Node script (case (c) above) and reasoned about algebraically. A first draft
    did add defensive min/max sorting; removed it as unrequested complexity once the invariant was
    confirmed, per this task's own "do not comment every step" / minimal-diff spirit and to stay
    closest to the prompt's literal five-line clamp block.
  - Named the local variables in `zoomScreenshot` exactly as the prompt's own numbered steps name
    them (`vpW`, `vpH`, `sx`, `sy`, `rx0`/`ry0`/`rx1`/`ry1`, `x0`/`y0`/`x1`/`y1`, `w`/`h`, `s`,
    `cap`, `shotW`/`shotH`, `base64`) rather than introducing different names, to keep the
    implementation directly traceable against the prompt's ten-step description.
  - Placed `zoomScreenshot` directly after `screenshot()` and before the `--- Input helpers ---`
    section comment (which previously started immediately after `screenshot()`), giving the new
    function its own one-line section comment (`--- Zoom: capture a clipped, magnified region and
    record it as the tab's coordinate context ---`) rather than folding it under the existing
    `--- Screenshot pipeline ---` header -- matches this file's existing convention of one banner
    comment per logical block (`--- Screenshot pipeline ---`, `--- Input helpers ---`, etc.) and
    keeps the zoom capture rhythm visually distinct from the plain-screenshot rhythm it mirrors.
  - Added exactly the three comments constraint 7 names (one on `zoomScale`'s purpose, one on the
    `clip` scroll-offset addition noting clip is document-relative, and the two one-line additions
    to the `screenshotCtx`/`rescaleCoord` doc comments) plus one additional short comment on the
    `screenshotCtx.set` line inside `screenshot()` itself (noting that a full screenshot resets the
    zoom offset) -- judged to fall within "comments only for constraints the code cannot express"
    since the offset-reset behavior is exactly the kind of non-obvious invariant (why THIS specific
    line matters for T11's coordinate contract) that a future reader/task would otherwise have to
    rediscover by re-deriving it from `rescaleCoord`'s formula.
  - Left `src/policy/redact.rs` and `tests/tool_schema_fidelity.rs` untouched (same pre-existing
    rustfmt-version drift every prior task in this run has flagged). This task touched no Rust
    files at all, so there was nothing to run `cargo fmt` on and no reformatting side effect to
    revert this time; confirmed via `cargo fmt --check`, whose only reported diffs are in exactly
    those same two files, byte-for-byte the same diffs every prior task's log already described.
- Notes for later tasks:
  - `screenshotCtx` entries now always carry eight fields (`vpW, vpH, shotW, shotH, offX, offY,
    regionW, regionH`) after either a full screenshot or a zoom; `rescaleCoord`'s `|| c.vpW`/
    `|| c.vpH`/`|| 0` fallbacks keep it safe against any hypothetical caller that still constructs
    a context object without the new fields, but no code path in this file does that anymore (both
    writers -- `screenshot()` and `zoomScreenshot()` -- always write all eight fields).
  - `zoomScale(w, h)` and `zoomScreenshot(tabId, region)` are new module-level helpers; the only
    caller of either is the `zoom` case in `computer()`. A later task should not need to touch them
    unless a new task prompt explicitly requires it.
  - The zoom result-text contract (three exact validation error strings and the
    `` `Zoom region (${z.x0}, ${z.y0}) -> (${z.x1}, ${z.y1}) captured (jpeg${...}).` `` success
    template, including the exact "; clamped to the visible viewport" conditional suffix) is now a
    byte-exact contract at the same tier as every prior task's contract in this run (T01's
    marker-line formats, T02's Note line, T03's get_page_text contract, T06's `[hop: ...]`
    contract, T08's type-dispatch contract, T09's click-event contract, T10's scroll-verify
    strings, T13's exception-text format, T14's network per-line format, T15's zero-result
    strings) -- do not reword any of the four strings in a later task without updating this note
    and the T11 BROWSER-TESTS.md entries.
  - T18 (background-tab screenshot via clip+scale) is listed as "T11 helpful, not required" in the
    sequence table's Depends-on column; `zoomScreenshot`'s `Page.captureScreenshot` call already
    demonstrates the `clip`/`scale` parameter pattern T18 will likely need (document-relative clip
    origin via `sx + x0, sy + y0`, `scale` as a CSS-to-output-pixel multiplier) -- a later task
    implementing T18 can reuse or mirror this pattern rather than deriving it from scratch, but is
    not required to call `zoomScreenshot` itself (T18's own prompt should be read fresh for its
    exact requirements).
  - No `src/mcp/schemas/tools.json` edits were made or needed; `tests/tool_schema_fidelity.rs`
    passed unchanged (6/6), confirming the frozen `computer` schema (whose `region` parameter and
    `zoom` action description were already present before this task, per the prompt's Constraints
    section) was left untouched.
- Browser checks queued: T11-1, T11-2, T11-3, T11-4, T11-5, T11-6, T11-7, T11-8 in
  docs/tasks/release-1/BROWSER-TESTS.md (appended after T10-4, preserving task order).

### T18 Background-tab screenshot via clip+scale single-pass capture -- done -- 2026-07-02
- Commit: (recorded after commit; see git log for `feat(extension): T18 ...`)
- Files touched: extension/service-worker.js, docs/tasks/release-1/BROWSER-TESTS.md,
  docs/tasks/release-1/LEDGER.md
- Tests added: none in the Rust sense (this task touches only extension JS, which has no test
  harness per project constraints). Verification performed instead:
  - `node --check extension/service-worker.js` (syntax only), clean, both before and after every
    edit.
  - Full diff review confirming: `probeViewport`'s evaluated expression and return shape match the
    prompt's contract verbatim (`vis:document.visibilityState` added, `visible: (v.vis ||
    "visible") === "visible"`); the existing throw-on-missing-value guard is untouched; `screenshot`
    now returns `{ base64, note }` on every path; `targetDims` is called once, moved earlier so both
    the clipped and standard paths can read `w`/`h`; the HIDE_FOR_TOOL_USE/sleep(40) pair still runs
    unconditionally before either capture path exactly as before; the whole capture phase (clipped
    attempt, its quality-30 re-capture, and the standard/fallback capture) is wrapped in one
    try/finally whose finally fires `sendToTab(tabId, { type: "SHOW_AFTER_TOOL_USE" })` exactly
    once, not awaited, regardless of which path returns/throws; the clipped-path CDP params match
    the prompt's literal contract (`quality: 55` then `30` on oversize, `clip: { x: 0, y: 0, width:
    vpW, height: vpH, scale }`, `fromSurface: true`, `captureBeyondViewport: false`); the clipped
    success path records `screenshotCtx` and returns without ever decoding/measuring the image
    (grepped the new code block for `createImageBitmap` -- zero occurrences before the `return
    { base64: cap.data, note: "" }` line); a clipped-path rejection is caught, its message saved as
    `clipMsg`, and control falls through (no early return, no rethrow) to the standard capture,
    exactly as required; the standard capture's own params (`{ format: "jpeg", quality: 80,
    captureBeyondViewport: false }`) are byte-identical to the pre-task code; a standard-capture
    rejection on a visible tab (`clipMsg === null`) rethrows the original error object unchanged
    (preserving any `.hop` tag from `cdp`'s own `hopError("cdp", ...)` wrapping); a standard-capture
    rejection after a clipped-path failure throws a plain `new Error` (deliberately no `.hop`, per
    the prompt's literal snippet) with the exact combined-message template
    `` `screenshot of non-visible tab failed: clipped capture: ${clipMsg}; fallback capture:
    ${fbMsg}` ``; the canvas downscale block after the try/finally (raw-capture defaults, `encodeJpeg`
    at 0.55 then 0.3 over budget, silent keep-raw on canvas failure, the final
    `screenshotCtx.set(...)`) is byte-for-byte what it was before this task, confirmed via `git diff`
    hunk boundaries showing only the one-line move of `const { w, h } = targetDims(...)` and the
    `return base64;` -> `return { base64, note };` change inside that trailing section.
  - Confirmed every one of the SEVEN actual `await screenshot(tabId)` call sites in the file (not
    the three the prompt's stale line-number citations named -- see Drift reconciled) was updated to
    the `const shot = await screenshot(tabId); return textImage(shot.note ? caption + " " +
    shot.note : caption, shot.base64);` pattern, with each site's caption string byte-identical to
    its pre-task literal/template (grepped `git diff` for the caption text on each of the 7 sites;
    none changed). Confirmed the `zoom` case was NOT touched (it calls `zoomScreenshot`, a fully
    separate function introduced by T11, not `screenshot`) and needed no note-plumbing, per the
    reconciliation below.
  - Confirmed no other file references `screenshot(` as a bare-string-returning function:
    `extension/content.js` and `extension/agent-visual-indicator.js` have zero occurrences of
    `screenshot(` (grepped both).
  - `cargo test` (all 91 tests across the workspace, including `tests/tool_schema_fidelity.rs`,
    6/6) passes unchanged, confirming no Rust surface was touched and the frozen `computer` schema
    is intact.
  - `git status --short -- '*.rs' src/ tests/` was empty throughout -- this task made zero Rust
    changes, matching the prompt's "Build and test" note ("no Rust rebuild is required").
  - `cargo clippy --all-targets -- -D warnings` clean (nothing to lint; no Rust changed).
  - `cargo fmt --check` reports only the same two pre-existing drifted files every prior task in
    this run has flagged (`src/policy/redact.rs`, `tests/tool_schema_fidelity.rs`); neither was
    touched by this task, and there was nothing to run `cargo fmt` on (zero Rust changes).
  - ASCII scan (the BOOTSTRAP.md python one-liner) on both edited files (`extension/service-
    worker.js`, `docs/tasks/release-1/BROWSER-TESTS.md`) returned empty lists.
- Drift reconciled:
  - Line numbers had drifted throughout, exactly as the prompt itself warned ("locate the same code
    by the function names given here" once other tasks land). `screenshotCtx` was at line 16 (actual
    19), budget constants at line 70 (actual 92), `probeViewport` at 72-80 (actual 94-102),
    `screenshot()` at 215-239 (actual 312-337) -- all matched in shape and content once located by
    name, only line numbers moved.
  - The prompt's "Current behavior" section cites exactly THREE call sites of `screenshot(tabId)`:
    "Line 365" (the plain `screenshot` action), "Line 367" (the `zoom` action), and "Line 414"
    (`scroll`). T11 (real zoom region crop), which runs immediately before T18 in the fixed
    sequence, replaced the zoom call site entirely: `zoom` no longer calls `screenshot()` at all --
    it calls a new, separate `zoomScreenshot(tabId, region)` function (its own CDP capture, its own
    canvas downscale, its own `screenshotCtx.set`) that this task's Out of scope section explicitly
    excludes ("The zoom action's region semantics (T11). zoom keeps delegating to the same
    screenshot(); the only change at its call site is the note-plumbing pattern above" -- the second
    half of that sentence is what no longer holds). Separately, T09/T10 (mouse click fidelity,
    scroll verify) had already expanded the single `scroll` call site the prompt describes into SIX
    call sites (blind-claim fallback, re-read-failed fallback, verified-movement success, and three
    outcomes of the direct-scroll fallback), none of which existed when this prompt was authored.
    Reconciled literally per the prompt's own escape hatch ("if other release-1 tasks have added
    more screenshot-returning paths, update those identically"): updated all SEVEN actual call sites
    (the plain `screenshot` action plus all six `scroll`-case sites) to the note-plumbing pattern,
    and left `zoom` alone since it has no `screenshot(tabId)` call site left to update -- the
    Out-of-scope clause's literal premise (zoom delegates to screenshot()) is false in the actual
    tree, so there is nothing there for this task to touch without violating "The zoom action's
    region semantics (T11)" being out of scope.
  - The prompt's Required-behavior text and Task-specific constraints both describe the
    ScreenshotContext shape as the pre-T11 four fields, `{ vpW, vpH, shotW, shotH }` (for example:
    "On success: `screenshotCtx.set(tabId, { vpW, vpH, shotW: w, shotH: h });`" and "The
    ScreenshotContext shape stays `{ vpW, vpH, shotW, shotH }`"). T11 (immediately prior in the fixed
    sequence) had already extended the shape to eight fields, `{ vpW, vpH, shotW, shotH, offX, offY,
    regionW, regionH }`, with both existing writers (`screenshot()`'s own visible-path
    `screenshotCtx.set` and `zoomScreenshot()`) always writing all eight. Reconciled by writing the
    same eight-field shape from the new clipped-path success branch too (`offX: 0, offY: 0, regionW:
    vpW, regionH: vpH`, matching a full, un-zoomed viewport capture) rather than reverting to four
    fields. This is required, not just consistent, by the Project context's own coordinate-model
    contract ("the context recorded for a non-visible tab must hold the same CSS viewport dims and
    the same final pixel dims that the visible path would have recorded for the same viewport") --
    the ACTUAL visible path (unchanged by this task) writes all eight fields, so parity means the
    clipped path must too. `rescaleCoord`'s `|| c.vpW`/`|| c.vpH`/`|| 0` fallbacks would have made a
    four-field write behaviorally equivalent through the fallback chain, but the eight-field write
    is the more literal, structurally-identical parity the contract describes, and was verified not
    to violate "rescaleCoord stays untouched" (it was not touched; only `screenshotCtx.set`'s call
    sites were).
- Decisions made:
  - Factored the two clipped-path capture calls (quality 55, then quality 30 on oversize) through a
    shared `clipParams` object (`{ clip: {...}, fromSurface: true, captureBeyondViewport: false }`,
    spread into each call alongside its own `quality`) rather than writing out two fully separate
    literal CDP-params objects. The prompt's own snippet writes them as two separate literal calls;
    this is a direct, low-risk DRY simplification (same rationale T06 used for its `error_result`
    helper) that cannot let the two calls' clip/fromSurface/captureBeyondViewport drift apart from
    each other, and produces the exact same two CDP calls the prompt describes.
  - Did NOT factor the seven `const shot = await screenshot(tabId); return textImage(shot.note ?
    caption + " " + shot.note : caption, shot.base64);` call sites through a shared helper function
    (for example a hypothetical `shotWithNote(tabId, caption)`), even though they are close to
    verbatim duplicates. The prompt's own wording ("Update EVERY call site... The pattern at each
    site:") frames this as a literal per-site pattern rather than an extractable helper, and every
    other task in this run that faced a similar choice (T11's note on variable naming, T09/T10's
    per-branch verification blocks) chose direct traceability against the prompt's described shape
    over introducing new abstractions the prompt did not ask for. Kept as seven direct, easily
    diffable instances instead.
  - The combined hard-failure error (`new Error(\`screenshot of non-visible tab failed: ...\`)`) is
    deliberately built with the plain `Error` constructor, not `hopError(...)`, exactly as the
    prompt's own snippet shows (`new Error(...)`, no third `hop` argument). This means `dispatch`'s
    catch takes the `else` branch (`fail(id, \`${tool} failed: ${...}\`)` -- prefixing with the tool
    name, "computer failed: ...") rather than the hop-preserving branch, which matches the prompt's
    own stated expectation verbatim ("dispatch will surface this as `computer failed: screenshot of
    non-visible tab failed: ...`"). This is a deliberate exception to T06's hop-attribution
    convention: a background-capture-specific failure genuinely straddles two capture attempts (not
    one CDP call), so there is no single accurate hop to tag it with; leaving it hop-less falls back
    to the tool-name-prefixed generic form, which is the most honest attribution available.
  - Used `w / vpW` (not `vpW / w` or a pre-divided constant) for `scale`, matching the prompt's
    literal formula; added a one-line comment noting it is always <= 1 (since `targetDims` never
    grows), which is the load-bearing invariant that makes the CDP `scale` parameter meaningful here
    (CDP's `clip.scale` is a magnification factor on the captured surface, not a re-encode ratio;
    a `scale <= 1` here means "shrink", which is what every clipped-path caller in this task actually
    wants).
  - Placed the new `return` (clipped-path success) and the two `throw`/`throw` (visible-tab
    passthrough, non-visible combined failure) sites exactly where the prompt's ten-line pseudocode
    implies them, without adding any additional branching, retry, or logging beyond what was
    specified (Out of scope explicitly forbids retry loops, capture timeouts, settle-time tuning,
    and new logging).
  - Left `src/policy/redact.rs` and `tests/tool_schema_fidelity.rs` untouched (same pre-existing
    rustfmt-version drift every prior task in this run has flagged). This task touched no Rust
    files at all, so there was nothing to run `cargo fmt` on and no reformatting side effect to
    revert this time; confirmed via `cargo fmt --check`, whose only reported diffs are in exactly
    those same two files, byte-for-byte the same diffs every prior task's log already described.
- Notes for later tasks:
  - `screenshot(tabId)` now returns `{ base64, note }`, NOT a bare base64 string. Any future task
    that adds a new call site to `screenshot()` (there are none currently planned in T16/T17/T05,
    which touch `javascript_tool`, tabId-resolution, and service-worker startup/recovery
    respectively -- none of the three prompts as authored appear to add a new screenshot call site,
    but re-verify against the actual tree before assuming) MUST use the `const shot = await
    screenshot(tabId); ...shot.base64...shot.note...` pattern established here, not treat the return
    value as a string.
  - `probeViewport(tabId)`'s return shape gained a fifth field, `visible` (boolean); its only other
    caller besides `screenshot()` is none currently (`zoomScreenshot` has its OWN separate
    `Runtime.evaluate` probe for `{w, h, sx, sy}`, unrelated to `probeViewport`, and was not touched
    by this task). A later task that calls `probeViewport` directly should be aware `visible` is now
    part of its contract.
  - The combined hard-failure message template (`` `screenshot of non-visible tab failed: clipped
    capture: ${clipMsg}; fallback capture: ${fbMsg}` ``) and the fallback warning note (`"Warning:
    this tab was not visible and direct background capture failed; the image was taken with the
    standard capture path and may be blank or stale."`) are now byte-exact contracts at the same
    tier as every prior task's contract in this run (T01's marker-line formats, T02's Note line,
    T03's get_page_text contract, T06's `[hop: ...]` contract, T08's type-dispatch contract, T09's
    click-event contract, T10's scroll-verify strings, T11's zoom-result contract, T13's
    exception-text format, T14's network per-line format, T15's zero-result strings) -- do not
    reword either string in a later task without updating this note and the T18 BROWSER-TESTS.md
    entries.
  - `zoomScreenshot()` was NOT given the same visibility-aware clip+scale treatment this task adds
    to `screenshot()` (explicitly out of scope: "The zoom action's region semantics (T11)"). If a
    future task ever wants background-tab support for `zoom` too, it would need its own prompt; do
    not silently extend `zoomScreenshot` to reuse this task's `visible`/`clipMsg` pattern without one.
  - No `src/mcp/schemas/tools.json` edits were made or needed; `tests/tool_schema_fidelity.rs`
    passed unchanged (6/6), confirming the frozen `computer` schema was left untouched, per this
    task's Constraints section.
- Browser checks queued: T18-1, T18-2, T18-3, T18-4, T18-5, T18-6, T18-7 in
  docs/tasks/release-1/BROWSER-TESTS.md (appended after T11-8, preserving task order).

### T16 javascript_tool REPL semantics + 50KB output cap -- done -- 2026-07-02
- Commit: (recorded after commit; see git log for `feat(extension): T16 ...`)
- Files touched: extension/service-worker.js, docs/tasks/release-1/BROWSER-TESTS.md,
  docs/tasks/release-1/LEDGER.md
- Tests added: none in the Rust sense (this task touches only extension JS, which has no test
  harness per project constraints). Verification performed instead:
  - `node --check extension/service-worker.js` (syntax only), clean, both before and after the
    edit.
  - Full diff review confirming: the handler's first `Runtime.evaluate` call now carries
    `replMode: true` in addition to the pre-existing `expression`/`returnByValue`/`awaitPromise`
    fields, matching the prompt's literal parameter object; the `inGroup` guard on the first line
    is byte-identical to before; the probe string is built as
    `(r.exceptionDetails.text || "") + ((ed && ed.description) || "")` where
    `ed = r.exceptionDetails.exception` -- algebraically the same as the prompt's
    "concatenate text and, when present, exception.description, tolerate both missing" contract
    (an absent `exceptionDetails.exception` short-circuits `ed && ed.description` to `undefined`,
    then `|| ""` makes it the empty string; an absent `text` likewise falls back to `""`); the
    retry only fires when the probe contains the exact substring `Illegal return statement`; the
    retry expression is built as `"(async () => {\n" + a.text + "\n})()"`, matching the prompt's
    literal contract with newlines around the user code; the retry's CDP params
    (`{ expression: wrapped, returnByValue: true, awaitPromise: true }`) deliberately omit
    `replMode`, matching constraint 3's instruction; the retry's result unconditionally replaces
    `r` for every later step (no branching kept on the original response); the function never
    loops or retries a second time (the retry's own `exceptionDetails`, if any, is only ever
    consulted once more by the same unconditional `if (r.exceptionDetails) return text(...)` a few
    lines down -- there is no second probe/retry code path at all, structurally guaranteeing
    "never retry more than once"); the exception-result line
    (`` return text(`Error: ${r.exceptionDetails.text || "exception"}`); ``) is byte-identical to
    the pre-task code; the success-result computation
    (`v.value !== undefined ? JSON.stringify(v.value) : (v.description || String(v.type))`) is
    byte-identical to the pre-task code; the 50KB cap
    (`if (out.length > 50 * 1024) out = out.slice(0, 50 * 1024) + "\n[OUTPUT TRUNCATED: Exceeded
    50KB limit]";`) matches the prompt's literal contract exactly, applied via `.length`
    (UTF-16 code units, no byte-level accounting) as required; no note is appended anywhere for a
    successful fallback-retry result (grepped the whole new handler body for the words "retry" or
    "fallback" in any string literal -- zero occurrences), satisfying step 6's truthfulness
    framing (the retry IS the promised contract, not a substitute).
  - Confirmed no `timeout` parameter was added to either `Runtime.evaluate` call, no new timer
    (`setTimeout`/`setInterval`) was introduced in the handler, and `src/browser.rs`'s
    `TOOL_TIMEOUT` constant was never opened for edit (`git status --short -- '*.rs' src/ tests/`
    was empty throughout).
  - Confirmed the diff touches exactly one function (`handlers.javascript_tool`) and adds no new
    top-level helper: `git diff --stat` reports one file changed; the diff hunk boundaries in
    `service-worker.js` fall entirely inside the `javascript_tool` handler body (no other handler,
    `dispatch()`, `text()`, or `cdp()` line was touched).
  - `cargo test` (all 91 tests across the workspace -- 80 unit + 4 mcp_protocol + 1 peer_death + 6
    tool_schema_fidelity -- plus 0 doc-tests) passes unchanged, confirming the frozen
    `javascript_tool` schema and every other Rust surface were left untouched.
  - `git status --short -- '*.rs' src/ tests/` was empty throughout -- this task made zero Rust
    changes, exactly as the prompt's "Build and test" note predicted ("no Rust changes in this
    task").
  - `cargo clippy --all-targets -- -D warnings` clean (nothing to lint; no Rust changed).
  - `cargo fmt --check` reports only the same two pre-existing drifted files every prior task in
    this run has flagged (`src/policy/redact.rs`, `tests/tool_schema_fidelity.rs`); neither was
    touched by this task, and there was nothing to run `cargo fmt` on (zero Rust changes).
  - ASCII scan (the BOOTSTRAP.md python one-liner) on both edited files (`extension/service-
    worker.js`, `docs/tasks/release-1/BROWSER-TESTS.md`) returned empty lists.
- Drift reconciled: only line-number drift, exactly as the prompt itself warned ("The
  `javascript_tool` handler is at lines 505-511"; the actual working tree, after every earlier
  extension-touching task in this run landed first, had it at lines 859-865). The handler's exact
  code shape (the `inGroup` guard, the single `Runtime.evaluate` call with
  `{ expression, returnByValue: true, awaitPromise: true }` and no `replMode`, the bare
  `exceptionDetails`-check-and-return, the value/description/type success computation with no size
  cap) matched the prompt's "Current behavior" section verbatim once located by function name;
  `src/browser.rs`'s `TOOL_TIMEOUT` (cited by the prompt as "line 25"/"line 99") and `dispatch()`'s
  wrap-as-tool_error behavior (cited as "lines 558-565") were both read-only reference points for
  this task and were not re-verified line-for-line since neither was touched or needed editing.
- Decisions made:
  - Built the probe string with an explicit local `const ed = r.exceptionDetails.exception;`
    before the concatenation, rather than inlining `r.exceptionDetails.exception &&
    r.exceptionDetails.exception.description` twice. This is a direct readability simplification
    (avoids repeating the property-access chain) with identical observable behavior; the prompt
    describes the two fields to concatenate but does not mandate a specific expression shape.
  - Did not extract a shared "evaluate and inspect" helper function (the prompt explicitly allowed
    but did not require this: "You may extract the shared evaluate-and-inspect step into one small
    helper function... but the observable behavior must match the steps above exactly"). The
    handler stays at 17 lines with a single nested `if` block for the retry decision, which reads
    linearly top to bottom without an extra indirection; no second call site exists that would
    benefit from a shared helper (the retry's CDP params are deliberately a different literal
    object, missing `replMode`, so there is little to actually share beyond the `cdp(...)` call
    itself).
  - The retry-decision comment ("A bare top-level 'return' is only legal inside a function; retry
    once wrapped in an async IIFE, which also preserves top-level await for the wrapped code.") is
    the one comment this task's constraint 7 budget allows on non-obvious logic (the reason a
    SyntaxError substring match triggers a full re-evaluation is not self-evident from the code
    alone); no other comment was added anywhere else in the handler.
  - Left `src/policy/redact.rs` and `tests/tool_schema_fidelity.rs` untouched (same pre-existing
    rustfmt-version drift every prior task in this run has flagged). This task touched no Rust
    files at all, so there was nothing to run `cargo fmt` on and no reformatting side effect to
    revert this time; confirmed via `cargo fmt --check`, whose only reported diffs are in exactly
    those same two files, byte-for-byte the same diffs every prior task's log already described.
- Notes for later tasks:
  - `javascript_tool`'s success/exception/truncation strings (`Error: <text or "exception">`, and
    the truncation marker `\n[OUTPUT TRUNCATED: Exceeded 50KB limit]`) are now byte-exact contracts
    at the same tier as every prior task's contract in this run (T01's marker-line formats, T02's
    Note line, T03's get_page_text contract, T06's `[hop: ...]` contract, T08's type-dispatch
    contract, T09's click-event contract, T10's scroll-verify strings, T11's zoom-result contract,
    T13's exception-text format, T14's network per-line format, T15's zero-result strings, T18's
    background-capture contract) -- do not reword any of them in a later task without updating this
    note and the T16 BROWSER-TESTS.md entries.
  - T17 (effective-tabId fallback + valid-ID errors) touches tabId resolution generally, not
    `javascript_tool` specifically; per T18's own note, none of T16/T17/T05 as authored appeared to
    add a new `screenshot()` call site, and this task confirms `javascript_tool` adds none either
    (it never calls `screenshot`, `zoomScreenshot`, or `probeViewport`). T17 should re-verify
    against the actual tree before assuming, per that same T18 note.
  - No `src/mcp/schemas/tools.json` edits were made or needed; `tests/tool_schema_fidelity.rs`
    passed unchanged (6/6), confirming the frozen `javascript_tool` schema (including its `text`
    parameter description promising REPL semantics, which this task's handler now actually
    honors) was left untouched, per this task's Constraints section.
- Browser checks queued: T16-1, T16-2, T16-3, T16-4, T16-5, T16-6 in
  docs/tasks/release-1/BROWSER-TESTS.md (appended after T18-7, preserving task order).

### T17 Effective-tabId fallback + valid-ID errors -- done -- 2026-07-02
- Commit: (recorded after commit; see git log for `feat(extension): T17 ...`)
- Files touched: extension/service-worker.js, docs/tasks/release-1/BROWSER-TESTS.md,
  docs/tasks/release-1/LEDGER.md
- Tests added: none in the Rust sense (this task touches only extension JS, which has no test
  harness per project constraints). Verification performed instead:
  - `node --check extension/service-worker.js` (syntax only), clean, both before and after the
    edit.
  - Full diff review confirming every one of the ten tabId-bearing handlers
    (`computer`, `navigate`, `read_page`, `get_page_text`, `find`, `form_input`,
    `javascript_tool`, `read_console_messages`, `read_network_requests`, `resize_window`) now
    opens with `const tabId = await effectiveTabId(a.tabId);` and every later use of `a.tabId` in
    each handler body was rewritten to the local `tabId` (confirmed with
    `grep -n "a\.tabId" extension/service-worker.js`, which now only matches the ten
    `effectiveTabId(a.tabId)` call sites themselves; and `grep -n "inGroup("`, which now only
    matches `inGroup`'s own definition plus the single internal call inside `effectiveTabId`).
  - `TabAccessError extends Error {}` and `async function effectiveTabId(rawTabId)` were inserted
    verbatim (byte-for-byte, including both comments) immediately after `inGroup`'s closing brace,
    per the prompt's exact code block. Confirmed algebraically: a numeric or truthy `rawTabId`
    that passes `inGroup` returns unchanged (one `inGroup` call, same cost as before); a
    stale/foreign `rawTabId` triggers `ensureGroup(false)` (recover-only, never `true`) then either
    the empty-group message or the valid-IDs message, both built from the `GROUP_TITLE` template
    literal (never a hardcoded "Browser MCP" string) with the id list joined by `", "` in
    `chrome.tabs.query` order (`groupTabs()`'s own order, unmodified) with a trailing period;
    `rawTabId === 0` takes the provided-id branch (`0 !== undefined && 0 !== null` is true), not
    the fallback, matching the prompt's explicit "0 counts as provided" rule; a string id is never
    coerced (no `Number(...)`/`parseInt` anywhere in the new code) and simply fails `inGroup`,
    landing on the stale-id path as specified. Omitted/null `rawTabId` with tabs present: filters
    to `t.active`, falls back to the full pool when none active, then linear-scans for the highest
    `(t.lastAccessed || 0)` with strict `>` comparison (so an equal or lower `lastAccessed` never
    replaces `best`, preserving first-in-query-order on ties, exactly as specified) -- tolerates a
    missing `lastAccessed` on older Chrome per the prompt's manifest-version note. Omitted/null
    `rawTabId` with an empty/absent group: throws the exact "No tabs in the Browser MCP group.
    Use tabs_create_mcp to open one, or tabs_context_mcp with createIfEmpty: true." message.
    Grepped the whole new function body for `chrome.tabs.create`, `chrome.tabs.group`,
    `chrome.windows.create`, and `ensureGroup(true)` -- zero occurrences, confirming the helper
    never provisions a tab, group, or window on any failure path, per constraint 4 and the
    Verification section's explicit "confirm no new tab, group, or window appeared" check (left
    as a deferred browser check; see below).
  - `dispatch` now carries three branches in its catch block, in this order:
    `TabAccessError` -> `reply(id, text(e.message))` (added by this task, matching the prompt's
    literal replacement code); then the pre-existing (T06) `e.hop` -> `fail(id, e)` branch; then
    the pre-existing untagged fallback -> `fail(id, ...)`. This is a deliberate reconciliation
    against the prompt: the prompt's "Required behavior" step 2 shows a two-branch `dispatch`
    (`TabAccessError` and the generic `fail`) because it was authored before T06 landed in this
    run's actual sequence; T06 already added hop-tagged passthrough to the working tree by the
    time this task ran. Dropping the `e.hop` branch to match the prompt literally would have
    reintroduced a regression T06 fixed (hop-tagged cdp/page errors losing their `hop`/`detail`
    fields and gaining a redundant `<tool> failed:` prefix), which is exactly the kind of drift
    the prompt itself warns about ("trust function names and prose over line numbers"; the prose
    here is "keeps the exact message and the exact delivery channel" for `TabAccessError`, which
    does not require removing the unrelated `e.hop` branch). Kept both: `TabAccessError` is
    checked first (an `instanceof` check that only matches the new class; `TabAccessError`
    instances never carry `.hop`, so there is no branch-ordering ambiguity), then the T06 hop
    branch, then the original catch-all.
  - Found and fixed one call site the prompt's own line-inventory did not enumerate: inside
    `javascript_tool`'s illegal-top-level-return retry path (added by T16, which landed after this
    prompt was authored), the retry's `cdp(a.tabId, ...)` call still read `a.tabId` directly. The
    prompt's instruction ("every later use of a.tabId in that handler body becomes tabId") covers
    this by prose even though the retry branch did not exist when the prompt's line-numbered
    inventory was written; fixed it to `cdp(tabId, ...)` for consistency with the rest of the
    handler and to avoid resolving the tab twice under two different fallback rules within one
    call (a latent bug this task's helper introduction made newly visible, since `a.tabId` could
    now be null/omitted on a fallback-resolved call and `cdp(null, ...)` would have failed the
    retry outright).
  - `resize_window`'s inner loop variable was renamed from `tabId` (shadowing the new outer const)
    to `attachedId` at its three occurrences (the `for` header over `attached.keys()`,
    `chrome.tabs.get(attachedId)`, and `screenshotCtx.delete(attachedId)`), per the prompt's
    explicit instruction; confirmed no other reference to the old shadowed name remains
    (`grep -n "for (const tabId of attached" extension/service-worker.js` returns nothing).
  - `cargo test` (all 91 tests across the workspace -- 80 unit + 4 mcp_protocol + 1 peer_death + 6
    tool_schema_fidelity -- plus 0 doc-tests) passes unchanged, confirming the frozen tool schemas
    (tabId still `required` everywhere) and every other Rust surface were left untouched.
  - `git status --short -- '*.rs' src/ tests/` was empty throughout -- this task made zero Rust
    changes, exactly as the prompt's "Build and test" note predicted ("no Rust rebuild is
    required").
  - `cargo clippy --all-targets -- -D warnings` clean (nothing to lint; no Rust changed).
  - `cargo fmt --check` reports only the same two pre-existing drifted files every prior task in
    this run has flagged (`src/policy/redact.rs`, `tests/tool_schema_fidelity.rs`); neither was
    touched by this task, and there was nothing to run `cargo fmt` on (zero Rust changes).
  - ASCII scan (the BOOTSTRAP.md python one-liner) on both edited files (`extension/service-
    worker.js`, `docs/tasks/release-1/BROWSER-TESTS.md`) returned empty lists.
- Drift reconciled:
  - Line-number drift throughout, exactly as the prompt warned: the prompt cited `inGroup` at
    lines 178-190 and the tabId-bearing handlers at lines 357-566 (568-line file at authoring
    time); the actual working tree (954 lines, after every earlier task in this run landed first)
    has `inGroup` at lines 269-281 and the handlers spread from line 664 (`computer`) to line 963
    (`resize_window`'s close), with `dispatch` at line 971. Located everything by function/handler
    name instead, per the prompt's own guidance, and the code shape at each site matched the
    prompt's "Current behavior" section verbatim once located (both message variants --
    `computer`/`navigate`'s fuller `GROUP_TITLE` phrasing and the short `is not in the group.`
    phrasing on the other eight handlers -- were exactly as described).
  - `dispatch`'s two-branch prompt code vs. the three-branch actual result: see the T06-interaction
    note above; this is the one substantive (not just line-number) reconciliation this task made,
    and it was resolved in favor of preserving T06's contract (explicitly named as an "Open
    concerns" / "Notes for later tasks" item in this run's ledger for T06 and every subsequent
    task) while adding exactly the new behavior this prompt specifies.
  - The `javascript_tool` retry-path `a.tabId` call site (introduced by T16, not present when this
    prompt was authored) -- see above; fixed for consistency, not left as `a.tabId`.
- Decisions made:
  - Kept `dispatch`'s new `TabAccessError` check as the first branch in the catch block (before
    the T06 `e.hop` check), since `TabAccessError` instances are a disjoint class from hop-tagged
    `Error` instances (the two never overlap) and the prompt's replacement code lists
    `TabAccessError` first; ordering has no observable effect given the disjointness, but matching
    the prompt's stated order keeps the diff closest to its literal instruction.
  - Did not add a `tabId` coercion, range check, or default beyond exactly what the prompt
    specifies (constraint 10 / Out of scope explicitly forbid this); a string tabId still fails
    `inGroup` and takes the stale-id path with the valid-IDs message, as required.
  - Did not touch `tabs_context_mcp`, `tabs_create_mcp`, or `update_plan` (they take no tabId and
    are explicitly Out of scope); confirmed their handler bodies and the
    "No Browser MCP tab group. Call with createIfEmpty: true." message are byte-identical to
    before (`git diff` shows no hunk touching any of the three).
  - Left `src/policy/redact.rs` and `tests/tool_schema_fidelity.rs` untouched (same pre-existing
    rustfmt-version drift every prior task in this run has flagged). This task touched no Rust
    files at all, so there was nothing to run `cargo fmt` on and no reformatting side effect to
    revert this time; confirmed via `cargo fmt --check`, whose only reported diffs are in exactly
    those same two files, byte-for-byte the same diffs every prior task's log already described.
- Notes for later tasks:
  - `effectiveTabId(rawTabId)` and `class TabAccessError extends Error {}` are now permanent
    top-level names on the service worker's global scope, defined immediately after `inGroup`.
    `dispatch` converts `TabAccessError` to a plain `text(...)` tool result (never `tool_error`);
    any later task that adds a new tabId-bearing handler should call
    `const tabId = await effectiveTabId(a.tabId);` rather than reintroducing a direct `inGroup`
    check, to keep the fallback and valid-ID-listing behavior consistent across all handlers.
  - The three refusal/fallback message templates
    (`` `Tab ${rawTabId} is not in the ${GROUP_TITLE} group. Valid tab IDs are: ${...}.` ``,
    `` `Tab ${rawTabId} is not in the ${GROUP_TITLE} group. The group has no tabs; use
    tabs_create_mcp to open one.` ``, and
    `` `No tabs in the ${GROUP_TITLE} group. Use tabs_create_mcp to open one, or
    tabs_context_mcp with createIfEmpty: true.` ``) are now byte-exact contracts at the same tier
    as every prior task's contract in this run (T01's marker-line formats, T02's Note line, T03's
    get_page_text contract, T06's `[hop: ...]` contract, T08's type-dispatch contract, T09's
    click-event contract, T10's scroll-verify strings, T11's zoom-result contract, T13's
    exception-text format, T14's network per-line format, T15's zero-result strings, T18's
    background-capture contract, T16's javascript_tool contract) -- do not reword any of them in a
    later task without updating this note and the T17 BROWSER-TESTS.md entries. The old short
    `` `Tab ${a.tabId} is not in the group.` `` message no longer exists anywhere in the file
    (grepped for the literal substring `is not in the group.` -- zero matches remaining).
  - T05 (service-worker state recovery, the LAST task in this run's sequence) is the only
    remaining pending task. It touches service-worker startup/recovery generally; per this task's
    own scope, `effectiveTabId`/`TabAccessError`/`dispatch`'s new branch, and all ten updated
    handler bodies were left otherwise untouched (only the `a.tabId` -> `tabId` /
    `inGroup` -> `effectiveTabId` substitution was made in each), so T05 should have no reason to
    revisit this task's specific diff hunks unless it specifically needs to reason about
    tab-group/tabId resolution during recovery -- re-verify against the actual tree before
    assuming, per this run's standing convention.
  - No `src/mcp/schemas/tools.json` edits were made or needed; `tests/tool_schema_fidelity.rs`
    passed unchanged (6/6), confirming `tabId` stays `required` in every one of the ten schemas,
    per this task's Constraints section (defense in depth only; the extension now tolerates an
    absent tabId even though the schema still marks it required).
- Browser checks queued: T17-1, T17-2, T17-3, T17-4, T17-5 in
  docs/tasks/release-1/BROWSER-TESTS.md (appended after T16-6, preserving task order).

### T05 Service-worker death recovery (rehydrate tab group, reattach lazily) -- done -- 2026-07-02
- Commit: (recorded after commit; see git log for `feat(extension): T05 ...`)
- Files touched: extension/service-worker.js, docs/tasks/release-1/BROWSER-TESTS.md,
  docs/tasks/release-1/LEDGER.md
- Tests added: none in the Rust sense (this task touches only extension JS, which has no test
  harness per project constraints). Verification performed instead:
  - `node --check extension/service-worker.js` (syntax only), clean.
  - Full diff review confirming every required piece is present and wired: `persistSessionState()`
    (new) writes exactly `{ groupId, tabIds }` under the `"sessionState"` key, deriving `tabIds`
    live from `chrome.tabs.query({ groupId })` (or `[]` when `groupId` is null or the query
    throws), wrapped in its own try/catch that swallows a `chrome.storage.session.set` failure.
    Grepped for `chrome.storage.local` and `chrome.storage.sync` -- zero occurrences; only
    `chrome.storage.session` is used, per constraint 10.
  - `persistSessionState()` is called at exactly the four listed persistence points --
    `ensureGroup`'s every exit (verified all four return paths: existing-valid-group,
    adopt-by-title, stale-id-with-create-false, and the create branch), `inGroup`'s re-adopt
    branch only (not its other paths), `tabs_create_mcp` after the `chrome.tabs.group` call, and
    `chrome.tabs.onRemoved`'s end -- and nowhere else (grepped every call site of
    `persistSessionState(` in the final file: four in `ensureGroup`, one in `inGroup`, one in
    `rehydrate`, one in `tabs_create_mcp`, one in `chrome.tabs.onRemoved`; none in `navigate`,
    `computer`, or any read-tool handler, matching constraint "do not add persistence calls
    anywhere else").
  - `rehydrate()` (new) matches the prompt's four sub-steps in order: reads
    `chrome.storage.session.get("sessionState")` and returns early on no stored value (fresh
    start, no notice flags set); decides `priorSession` from `storedGroupId !== null ||
    tabIds.length > 0` and sets both `consoleResetNotice`/`networkResetNotice` true only then;
    verifies a non-null stored `groupId` with `chrome.tabGroups.get` and adopts it by id with NO
    title comparison and no rename/recolor call (grepped the whole function body for
    `tabGroups.update` -- zero occurrences); calls `persistSessionState()` unconditionally at the
    end (which re-derives `tabIds` from a live query, pruning stale ids for free, as the prompt
    specifies). Grepped the whole function for `chrome.tabs.create`, `chrome.tabs.group`,
    `chrome.windows.create`, `chrome.debugger.attach`, and any `.enable` CDP call -- zero
    occurrences, confirming rehydration never creates a group/tab/window and never eagerly attaches
    or enables a CDP domain, per the prompt's explicit "must NOT" list and Out of scope.
  - `rehydrate()`'s entire body is one `try { ... } catch { /* ... */ }` with an empty-on-error
    fallthrough (implicit `return` via falling off the end of the catch block), so it truly cannot
    reject regardless of which internal `await` throws; the whole promise it returns therefore
    always settles (never rejects), satisfying "must never reject."
  - `const ready = rehydrate();` sits immediately before the pre-existing top-level `connect();`
    call at the very end of the file (the only other module-level statement anywhere near
    `connect()`); `dispatch(id, tool, args)` now begins with `await ready;` as its first statement,
    before the unknown-tool lookup or any handler dispatch -- confirmed by reading the function from
    its `async function dispatch(id, tool, args) {` line downward with no statement above the new
    `await ready;` line.
  - `ensureAttached`'s new resilience branch: on a `chrome.debugger.attach` rejection whose message
    matches `/already attached/i`, calls `chrome.debugger.getTargets()` and looks for a target with
    matching `tabId` and `attached === true`; if found, falls through (without throwing) to the
    pre-existing shared `attached.set(tabId, { domains: new Set() })` line -- adopting the surviving
    attachment with a fresh empty `domains` set exactly as specified; if not found, or if the
    original error did not match `/already attached/i`, throws the same `hopError("cdp", "debugger
    attach failed: ...")` the pre-T05 code always threw, unchanged in shape (no retry, no
    force-detach -- grepped the new branch for `chrome.debugger.detach` -- zero occurrences).
  - Buffer-loss notices: `consoleResetNotice`/`networkResetNotice` both declared `let ... = false`
    at module scope; both handlers now build the final `out` string first (entries-joined or the
    pre-existing fallback text, byte-identical to before), THEN conditionally append exactly
    "\nNote: console event buffer was reset by a browser service-worker restart; tracking resumed
    from that point." (or the network equivalent) and clear the flag, then `return text(out)`. Since
    both handlers call `effectiveTabId(a.tabId)` as their very first statement and that throw
    propagates out of the handler before any of this code runs, the early-rejection path can never
    consume a flag, per the prompt's explicit rule. Confirmed the two flags are fully independent
    (no shared state, no handler touches the other tool's flag).
  - `cargo test` (all 91 tests across the workspace -- 80 unit + 4 mcp_protocol + 1 peer_death + 6
    tool_schema_fidelity -- plus 0 doc-tests) passes unchanged, confirming the frozen tool schemas
    and every other Rust surface were left untouched.
  - `git status --short -- '*.rs' src/ tests/` was empty throughout -- this task made zero Rust
    changes, exactly as the prompt's "Build and test" note predicted ("this task changes no Rust
    code").
  - `cargo clippy --all-targets -- -D warnings` clean (nothing to lint; no Rust changed).
  - `cargo fmt --check` reports only the same two pre-existing drifted files every prior task in
    this run has flagged (`src/policy/redact.rs`, `tests/tool_schema_fidelity.rs`); neither was
    touched by this task, and there was nothing to run `cargo fmt` on (zero Rust changes).
  - ASCII scan (the BOOTSTRAP.md python one-liner) on both edited files (`extension/service-
    worker.js`, `docs/tasks/release-1/BROWSER-TESTS.md`) returned empty lists.
- Drift reconciled: line-number and code-shape drift throughout, exactly as the prompt itself
  predicted ("all line numbers refer to extension/service-worker.js as it stands now (568
  lines)" -- the actual working tree, after every earlier task in this run landed first, is 984
  lines). Every function the prompt's "Current behavior" section named (`connect`, `ensureAttached`,
  `cdp`, `enableDomain`, `chrome.tabs.onRemoved`, the console/network buffering block, `ensureGroup`,
  `inGroup`, `tabContext`, `tabs_context_mcp`, `tabs_create_mcp`, `read_console_messages`,
  `read_network_requests`, `dispatch`) was present with the described shape once located by name;
  only exact line numbers, plus a few structural additions from later-landing tasks in this run's
  fixed sequence, had shifted:
  - T06 added hop-tagging: `ensureAttached`'s attach failure is wrapped in `hopError("cdp", ...)`
    (the prompt's "Current behavior" describes a bare throw of the raw error) -- reconciled by
    keeping T06's `hopError` wrapping in both the pre-existing success/failure paths and the new
    "already attached" adoption's not-found/no-match fallthrough throw, since the prompt's own
    instruction ("rethrow the original attach error unchanged") only makes sense read against the
    actual (T06-wrapped) throw shape, not the prompt's stale pre-T06 snippet.
  - T12 added the `tabHost` map and `bufferFor`/per-domain buffer-reset machinery the prompt's
    "Current behavior" never mentions (it was authored describing plain `tabId -> [...]` buffers).
    This task's new code (`persistSessionState`, `rehydrate`) never touches `consoleBuffer`,
    `networkBuffer`, or `tabHost` at all, per the prompt's own explicit "Do NOT persist
    consoleBuffer, networkBuffer, screenshotCtx, or attached" rule, so T12's buffer-ownership
    machinery needed no reconciliation beyond confirming it was left untouched (grepped every new
    function body for `consoleBuffer`, `networkBuffer`, `tabHost`, `screenshotCtx` -- zero
    occurrences).
  - T13/T14 added `exceptionText`/`Network.loadingFailed` handling inside the
    `chrome.debugger.onEvent` listener, structurally between `ensureAttached` and `ensureGroup` in
    the actual file (the prompt's line numbers place the buffering section immediately after
    `ensureAttached`, which was true at authoring time but is now separated by ~100 lines of T13/T14
    code) -- purely positional drift; this task's edits to `ensureAttached` and to the
    `chrome.tabs.onRemoved` listener are both above this block and untouched by it, and this task's
    new "Tab group" section code is below it, so nothing needed reconciling beyond locating the
    right section by its `// --- Tab group ...` header comment rather than a line number.
  - T17 added `effectiveTabId`/`TabAccessError` immediately after `inGroup` (the prompt's "Current
    behavior" section, describing the pre-T17 file, does not mention them at all and places
    `tabContext` directly after `inGroup`). Reconciled by inserting the new `rehydrate()` function
    between `inGroup` and `TabAccessError`'s class declaration (i.e., still directly after
    `inGroup`, preserving the prompt's stated adjacency intent -- "the branch [in inGroup] where
    groupId transitions..." -- while not disturbing T17's own `effectiveTabId`/`TabAccessError`/
    `tabContext` block, which sits immediately below the newly inserted `rehydrate()`). `dispatch`'s
    catch block (T06's hop branch plus T17's `TabAccessError` branch) was read in full and left
    completely untouched by this task except for the new `await ready;` line prepended before the
    handler lookup -- this task's Required-behavior text ("dispatch must begin with await ready")
    only mandates a new first line, not any change to the existing try/catch shape, and T17's own
    ledger note already flagged that T05 "should have no reason to revisit" its `TabAccessError`
    diff hunks unless reasoning about tab-group resolution during recovery, which this task's
    `rehydrate()`/`persistSessionState()` additions do (indirectly, by keeping `groupId` correct
    before any `effectiveTabId` call runs, thanks to `await ready;` guaranteeing rehydration
    completes first) but without touching `effectiveTabId`/`TabAccessError` themselves.
  - T16 added a `javascript_tool` illegal-return retry path; unrelated to this task's scope and not
    touched (grepped `javascript_tool`'s body -- no `a.tabId`/`tabId` resolution changes needed,
    since T17 already normalized every tabId-bearing handler including this one).
  No prompt fact turned out to be substantively wrong once mapped onto the actual code; every
  discrepancy above was purely "the prompt describes an earlier snapshot of this file" drift, which
  BOOTSTRAP.md and the prompt itself both anticipated.
- Decisions made:
  - In `ensureGroup`, added a `persistSessionState()` call even on the very first early-return
    branch (existing valid `groupId`, `chrome.tabGroups.get` succeeds) even though that specific
    case is not one of the three the prompt calls out by name ("covers the adopt-by-title branch,
    the create branch, and the stale-id-cleared case"). Read "at the end of ensureGroup(create), on
    every call, after groupId has settled" literally: "on every call" is the more specific,
    all-branches instruction, and the three named cases are given as examples of what this achieves
    ("covers ...") rather than an exhaustive branch list. A `persistSessionState()` call here is a
    no-op in effect when nothing changed (it re-derives and re-writes the same `{ groupId, tabIds }`
    it already had), so this reading cannot be observably wrong even if a human intended the
    narrower one; it was chosen because it is the literal, simpler-to-verify rule ("every call, at
    the end") and avoids a special-cased fifth branch that skips persistence for no stated reason.
  - Restructured `ensureGroup` from three early `return` statements (two of which returned with no
    persistence call in the original code) into four explicit `await persistSessionState(); return;`
    exit points (one per branch: existing-valid, adopt-by-title, stale-id-with-create-false, and the
    fall-through create-branch which persists then implicitly returns at function end) rather than a
    single trailing `await persistSessionState();` after a restructured single-exit function body.
    Chose per-branch calls over a single-exit refactor to keep the diff minimal and the branch
    structure (and its comments/behavior) otherwise completely unchanged, matching this run's
    general preference (seen in every prior task's log) for the smallest diff that satisfies the
    contract over a broader refactor.
  - `getTargets` survivor lookup: used `targets.find((t) => t.tabId === tabId && t.attached)`,
    matching the prompt's literal English description exactly ("a target whose tabId equals this
    tab and whose attached flag is true"). Did not additionally filter on `target.type === "page"`
    or similar (the prompt does not mention target type, and `chrome.debugger.getTargets()` in a
    normal single-tab-debugging scenario has at most one target per tabId regardless of type) --
    the prompt's own "Caveat you must preserve" paragraph explicitly accepts that this adoption
    cannot distinguish whose debugger is attached, so a narrower type filter would not change the
    fundamental tradeoff the prompt already accepts, and adding one un-requested would be scope
    creep.
  - Placed the two notice flags (`consoleResetNotice`, `networkResetNotice`) at module scope
    immediately after the `tabHost` declaration (the last of the existing per-tab state maps),
    rather than colocated with `rehydrate()`/`persistSessionState()` in the "Tab group" section
    further down -- the prompt does not mandate a location, and grouping all plain module-level
    `let`/`const` state declarations together (as the file already does for `nativePort`, `groupId`,
    the five Maps) keeps that declaration block a single, complete inventory of the worker's
    in-memory state, which a later task auditing "what state does this worker hold" would want to
    find in one place rather than split across the file.
  - Left `src/policy/redact.rs` and `tests/tool_schema_fidelity.rs` untouched (same pre-existing
    rustfmt-version drift every prior task in this run has flagged). This task touched no Rust files
    at all, so there was nothing to run `cargo fmt` on and no reformatting side effect to revert
    this time; confirmed via `cargo fmt --check`, whose only reported diffs are in exactly those
    same two files, byte-for-byte the same diffs every prior task's log already described.
- Notes for later tasks:
  - T05 is the LAST task in this run's fixed sequence. All 18 tasks (T04, T06, T07, T01, T02, T03,
    T12, T13, T14, T15, T08, T09, T10, T11, T18, T16, T17, T05) are now done. A future call should
    run BOOTSTRAP.md's "Completion" section next (write the RUN SUMMARY, verify a clean tree, commit
    `chore(ledger): run summary`, then stop) -- do not re-execute any of the 18 tasks.
  - `persistSessionState`, `rehydrate`, and the `ready` promise are now permanent top-level names on
    the service worker's global scope. Any future task that adds a new group-membership-changing
    code path (a new way to create, adopt, or empty the tab group) should call
    `persistSessionState()` at that point too, mirroring the four existing call sites, to keep
    `chrome.storage.session`'s `sessionState` record accurate.
  - `dispatch` now begins with `await ready;`. Any future change to `dispatch`'s structure must
    preserve this as the very first statement (before the unknown-tool check, before any handler
    lookup), since the entire point is that no tool handler -- including ones that read `groupId`
    indirectly through `ensureGroup`/`inGroup`/`effectiveTabId` -- can run before rehydration has
    settled `groupId` from a prior session.
  - The two notice-line strings ("Note: console event buffer was reset by a browser service-worker
    restart; tracking resumed from that point." and the network equivalent) are now byte-exact
    contracts at the same tier as every other task's contract in this run (T01's marker-line
    formats, T02's Note line, T03's get_page_text contract, T06's `[hop: ...]` contract, T08's
    type-dispatch contract, T09's click-event contract, T10's scroll-verify strings, T11's
    zoom-result contract, T13's exception-text format, T14's network per-line format, T15's
    zero-result strings, T18's background-capture contract, T16's javascript_tool contract, T17's
    tabId-fallback/valid-IDs messages) -- do not reword either notice line in a later task (there is
    none left in this run's sequence, but a future non-release-1 task touching this file should
    know) without updating this note and the T05 BROWSER-TESTS.md entries.
  - No `src/mcp/schemas/tools.json` edits were made or needed; `tests/tool_schema_fidelity.rs`
    passed unchanged (6/6), confirming every frozen schema was left untouched, per this task's
    Constraints section.
- Browser checks queued: T05-1 through T05-9 in docs/tasks/release-1/BROWSER-TESTS.md (appended
  after T17-5, preserving task order).
