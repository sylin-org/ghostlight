# M01: stage-4 ledger post-run correction

## Goal

Correct the historical record (ADR-0026 Decision 7): the stage-4 ledger's
closing statement says stage 4 was never verified against a live browser, but
the live pass HAS since run and passed (commit 44db1f3). Ledgers are
append-only, so the fix is an appended note, not an edit.

## Authority

ADR-0026 Decision 7; 00-design.md Provenance.

## Depends on

Nothing. STOP precondition: `rg -n "PLAIN STATEMENT" docs/tasks/stage-4/LEDGER.md`
matches exactly once, and `rg -n "t-live-1 stage-4 regression pass -- PASS"
docs/tasks/stage-2/BROWSER-TESTS.md` matches. If either fails, STOP.

## Current behavior (verified 2026-07-03; re-read before editing)

- docs/tasks/stage-4/LEDGER.md is 936 lines; its final paragraph (lines
  930-936) begins "PLAIN STATEMENT (per BOOTSTRAP Completion):" and states that
  hot-reload and the org-policy fix are "neither ... verified end-to-end
  against a live browser and a live MCP client."
- That statement is stale: docs/tasks/stage-2/BROWSER-TESTS.md line 100 records
  "### 2026-07-03: t-live-1 stage-4 regression pass -- PASS (live Chrome +
  Claude Code, stage-4 tree)", covering s-live-1, s-live-2, s-live-4, t01-1,
  t05-1, t06-1, t06-2 (re-run) plus s-live-3 (first run), all PASS. The pass
  landed in commit 44db1f3.
- Still not covered live, per BROWSER-TESTS.md lines 167-169: g13-1 steps 4-5,
  g13-3's governed half, and g15-1/g15-2; Linux live checks are also
  owed.

## Required behavior

Append EXACTLY this block to the end of docs/tasks/stage-4/LEDGER.md (after the
final line, preceded by one blank line):

    ## POST-RUN CORRECTION -- 2026-07-03

    The closing statement above is superseded on one point: the consolidated
    live pass it called for HAS since been run and passed. Commit 44db1f3
    ("docs(architecture): stage-4 live verification pass (t-live-1)") ran
    t-live-1 against live Chrome plus Claude Code on the stage-4 tree,
    covering s-live-1, s-live-2, s-live-4, t01-1, t05-1, t06-1, and t06-2
    (re-run) plus s-live-3 (first run), all PASS; the record lives in the
    2026-07-03 entry of docs/tasks/stage-2/BROWSER-TESTS.md. One
    observability gap stands from that pass (the expected ERROR-level server
    log line for the invalid mid-edit could not be confirmed; the behavioral
    guarantee was confirmed via identical denial ids). Still owed to a human:
    g13-1 steps 4-5, g13-3's governed half, g15-1 and g15-2, and Linux live
    checks. This note is appended per ADR-0026 Decision 7; the
    statement above is preserved unedited as the record of what was known at
    stage close.

No other file changes.

## Constraints

Append-only: not one existing byte of LEDGER.md changes. ASCII only.

## Tests (all rg from repo root)

- `rg -c "POST-RUN CORRECTION" docs/tasks/stage-4/LEDGER.md` prints `1`.
- `rg -c "44db1f3" docs/tasks/stage-4/LEDGER.md` prints `1`.
- `rg -c "PLAIN STATEMENT" docs/tasks/stage-4/LEDGER.md` still prints `1` (the
  appended block deliberately says "closing statement", not that phrase, so the
  original single occurrence is preserved unchanged).
- `git diff docs/tasks/stage-4/LEDGER.md` shows only appended lines (no existing
  line changed). This task's commit also contains the maturity-1 LEDGER.md
  entry and RESUME HERE update (per BOOTSTRAP), so `git status` will show those
  two ledger files, which is expected.

## Verification

The rg assertions above; ASCII diff scan; `cargo test` untouched (nothing
compiled changed; a spot-run of `cargo test --test hot_reload` suffices).
Ledger entry in docs/tasks/maturity-1/LEDGER.md; commit.

Commit subject: `docs(tasks): stage-4 ledger post-run correction (t-live-1 ran, ADR-0026 D7)`

## Out of scope

Any edit to existing LEDGER.md lines; BROWSER-TESTS.md; ADR text; the stage-2
or stage-3 ledgers.
