# H4: Grouped operation history and permission explanations

Status: implemented and verified locally; deployed locally on 2026-09-07, not published.
Owner decision: 2026-09-06. See the [deployment record](../../STATUS.md#local-deployment-2026-09-07).

## Accepted experience

One collapsed history entry represents each flow or sequence. Expand it to see child receipts,
with the first relevant problem brought into view. An already-open entry updates as children
finish. Routine child completion produces no extra notification. Successful policy decisions
explain applicable grants from each evaluated authority layer, alongside refusals and observe-mode
admission. Existing all-open behavior and H1 retention boundaries remain intact.

Each attempted operation records its safe receipt as it finishes. Invalid inputs and never-run
steps remain distinct from execution. Missing final completion or child evidence never proves
that work did not run. Parent summaries do not double-count child actions or denials. Caller step
labels, arguments, results, and arbitrary errors remain outside retained child metadata.

## Implementation boundary

One logical commit adds correlated child receipts to the existing completion seam, retains
permission evidence from the actual immutable-snapshot evaluation, and projects grouped history
through the existing workbench feed. Parent invocation plus step position identifies a child;
children do not reacquire the parent's lease or take new authority snapshots. History remains
bounded by whole groups and the existing maximum composition size. Policy previews evaluate
recorded child operations and exclude aggregate wrappers.

Automatic resumption, new retention profiles, H5 runtime scope/timing, and H7 audit availability
policy remain separate decisions. No new connector protocol, extension logic, or network behavior.

## Acceptance

Prove direct/composed receipt equivalence; exactly one record per attempted child; retained progress
before parent completion; safe invalid-input and missing-record states; and no double-counting.
Prove positive and negative attribution under layered grants, request restrictions, observe mode,
resource-less operations, and all-open. Keep synthetic content sentinels out of actual JSONL.
Exercise live updates, collapsed defaults, expansion, escaping, restart reconstruction, bounded
group retention, and policy previews. Run all repository gates and fresh-build process tests.
## Delivered

`work/receipt.rs` is the common completion seam. Direct operations and ordinary flow/sequence
children write the same bounded audit projection; each child completes under its parent's lease
and immutable snapshot. Parent invocation plus one-based position correlates records. Preparation
failure records have no admitted capabilities or invented dispatch. The parent retains H2b's
summary/progress; children keep their own status, effect, measurements, and safe permission trace.
Only the parent settles the lifecycle. Actual child denials count once.

`governance/evidence.rs` receives grant witnesses and whether request checks were reached from
the admission evaluation itself. Layer results, full capabilities, normalized hosts, request
restriction presence/evaluation, and observe admission stay distinct. At most 64 distinct checks
are retained per operation; omission is explicit. No fresh evaluation of today's policy supplies
an old receipt's explanation. Layer and grant identities remain bounded by manifest admission.

`workbench/history.rs` restores and updates at most 500 whole groups, with at most 20 steps each.
Absent parent completion states that completion was not recorded. A missing child receipt is
unconfirmed unless the final parent proves the step was not run. Policy previews use attempted
child records and exclude wrappers/preparation facts. The existing workbench feed keeps live
parents active. One collapsed entry expands to the first relevant problem; existing expansion,
scroll, and keyboard focus survive incremental updates. Routine children add no notifications.

## Verification

- All 463 Rust tests, including 386 orchestrator library tests; formatting and workspace Clippy.
- All 183 extension tests. No extension or connector implementation changed.
- Unit regressions cover direct/composed receipt equivalence, common authority, exactly-once child
  records, denial counting, positive layered grant witnesses, all-open, restrictions, observe mode,
  protected refusal, evidence bounds, restart gaps, JSONL reconstruction, and whole-group retention.
- Fresh workspace build and `GHOSTLIGHT_BIN_DIR=.target-ghostlight-1.0/debug node tests/process-journey.mjs`:
  real MCP/relay/orchestrator processes with a synthetic browser adapter. Actual JSONL retains
  child progress before the parent finishes, ordered correlation, requirements, and permission
  evidence. Parent records match H2b outcomes and synthetic content sentinels stay out of audit.
- `node tests/workbench-surface.mjs`: grouped live/snapshot projection, collapsed defaults,
  escaping, incremental updates, and truthful missing completion, alongside the existing surface.
- `node tests/workbench-history-browser.mjs`: bundled UI in isolated Chromium, using synthetic
  projection events. Checks expansion, first-problem visibility, permission detail, preservation
  of scroll/focus/open state, and the supported 720-pixel minimum width. Screenshots were inspected
  at 1280 and 720 pixels. No existing browser profile or registration was used by this lane.
- Changed JavaScript syntax, ASCII/whitespace checks, and repository integrity pass.

## Limits and continuation

This cycle does not add automatic resumption, new content retention, audit-health policy, or
workspace-local attention repair. Request evidence records the evaluator's existing behavior,
including an earlier policy return that does not reach request checks; it does not change admission
semantics. The evaluator test reproduces observed Write allowance despite a Read-only request
restriction and records `request_evaluated: false`. The admission-order gap is in the ledger for
a separate contract-preserving repair; no browser dispatch proof was run for it. H5 timing/scope and H7 recording failures remain separate ideation work. These checks do
not establish installed Tauri/MV3 deployment, cross-platform UI behavior, or durable-storage health.

Local commit: `feat(history): retain grouped child receipts and permission evidence` (use Git for
the hash). Next: H5 ideation I4. No deployment, push, or publication.
