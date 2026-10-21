# tests-1: service ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). Decision: [ADR-0204](../../adr/0204-meaningful-tests.md).
Machine: the coordinator, on the owner's machine. Paths without a crate are under `crates/orchestrator/src`. Statuses:
`todo`, `landed` (with commits), `blocked` (with the reason).

## RESUME HERE

T1 and T2 landed on 2026-10-21: `service/mod.rs` keeps 29 of 133, and intake, browser outside the
adapter, presentation, audit and the top-level files keep 32 of 211. The area holds 113 against a
budget of 60, because the plan left out `service/`'s other modules (52 tests: presence, idle,
lifetime, the lock, the shell and the rest), which the pilot leans on for the rules it no longer
proves end to end. Next: those modules (T3), cut against what the pilot kept, then `crates/relay`
and `crates/edge`, then the guard. The coordinator moves budget from the reserve or other lanes
only with the owner's leave.

## Areas

| Area | Tests at opening | Budget | Now |
| --- | --- | --- | --- |
| `service/`, `intake/`, `browser/` outside `browser/adapter/`, `presentation/`, `audit/`, top-level files, `crates/orchestrator/tests` | 396 | 60 | 113 |
| `crates/relay` | 157 | 35 | 157 |
| `crates/edge` | 66 | 20 | 66 |

## Slices

One row per slice: what it cuts, its counts before and after, and what the kept tests protect
(promise, contract, defect, unsafe).

| Slice | Subject | Before | After | Kept for | Status |
| --- | --- | --- | --- | --- | --- |
| T1 | test(service): `service/mod.rs` keeps the 29 tests that protect something | 133 | 29 | 15 promise, 11 defect, 3 contract | landed (5e9ec53d) |
| T2 | test(intake, browser, audit): the rest of the area outside `service/` keeps 32 | 211 | 32 | 17 promise, 7 contract, 8 defect | landed (this commit) |
| G | test(guards): the Rust test ceiling, `tests/guards/test-budget.test.mjs` (ADR-0204 Decision 4) | - | - | - | todo, after every area is under its budget |

## Notes

- T1, the pilot. An analysis read all 133 tests against ADR-0204 and found the overlap the owner
  suspected: the quit latch was proven 4 times, the goodbye 8, the idle limit 7 and the resume
  confirmation 6. 41 tests were deleted outright (they restated construction or literals, tested a
  one-line routing arm, or repeated a sibling unit test or a CI journey), and 77 were folded into 25
  tests of one behavior each. The owner approved 28: those 25 with the goodbye and the quit drain
  split in two, the native acts (file pick, open link and folder, and their release) as one test,
  and Gov21's one catalog notice. The wake and the ready limit's cancel then split too, because they
  fail independently: 29. Test code went from about 5,100 lines to 3,540, so the count fell 78% and
  the code 30%, because each merged test keeps every meaningful assertion it replaced. Assertions
  dropped: a timing bound on a test-only zero lock wait, the token's sort order, an exact count of
  listings (now "no request names a tab"), a restated `Result:` prefix, a tray rule
  `intake/desktop.rs` owns, and an `is_quitting` the latch implies. Every fixed sleep became a
  condition, a channel that disconnects, or the fake clock. `presence.rs` loses `Fake::into_world`,
  which only deleted tests used.
- T2: the store-listing checks are now `tests/guards/store-listing.test.mjs`, which CI runs in the
  extension job (CI names each guard file, so a new guard runs only once it is listed). Product
  code that only deleted tests used goes: `RecoveryFailure::ALL`, `TrayItemId::parse` and
  `TrayItemId::as_str`. The desktop goodbye's stale doc comment is fixed. The tooltip test stays
  as it was until the shared golden replaces it.
- The service area's plan after the pilot (an analysis of 2026-10-21, read-only, of every test in
  `intake/`, `browser/` outside `browser/adapter/`, `presentation/`, `audit/`, the crate's
  top-level files and `crates/orchestrator/tests`): 211 tests to 32 (17 promise, 7 contract, 8
  defect). The two store-listing checks become one node guard, `tests/guards/store-listing.test.mjs`:
  they read Markdown and `manifest.json`, not Rust. It waits for the owner to see the pilot land.
- Gaps the plan found, to close inside the budget:
  - ACCEPTANCE P3 (a presentation failure changes nothing) never had a real test: the old one only
    asserted that nothing panics. Write it in the executor's fixture: the same call with a failing
    and with a recording presentation port gives the same status, effect, receipts and audit line.
  - Limits one side checks alone, the P-D4 pattern: the tray tooltip (127 characters, checked only
    by `crates/desktop/src/tray.rs`), the window bearer's shape (the workbench and the shell each test
    their own), and the workbench address (the shell's golden test compares the URL for equality
    instead of parsing it). Each wants a golden both sides read; the desktop half is the install
    lane's file. Done by the coordinator on 2026-10-21: the service writes its longest tray (the
    longest tooltip at full capacity, with an ended session's labels) to
    `tests/golden/desktop/tray-model-longest.json`, and the workbench golden carries a real-shaped
    bearer at the address the service formats; the shell's golden test runs `validate()`,
    `validate_workbench_url` and `well_formed_bearer` on them, and the workbench's bearer test
    checks a minted bearer against the golden's shape. The service's own copy of the 127 limit and
    its one-sided tooltip test go. The clear-diagnostics limit (256 on both sides, in no shared vocabulary) is now
    the vocabulary's `clear_diagnostics_tabs`, which `golden.rs` and the extension's protocol test
    pin, and the extension test that restated 256 goes.
  - A client before MCP revision 2025-06-18 gets a cut-off or expired answer without the lean JSON
    C21 promises (`intake/mcp.rs` builds the templates at the latest revision). A defect to fix,
    not a test to keep.
  - `intake/desktop.rs`'s doc comment says the desktop goodbye shares the MCP one's code path; the
    service writes them separately. Fix the comment in this area's cut.
  - A batch's tool results go out outside the batch's array (`intake/mcp.rs`), against JSON-RPC
    batches. MCP dropped batching in 2025-06-18; whether to keep that is the owner's call.
