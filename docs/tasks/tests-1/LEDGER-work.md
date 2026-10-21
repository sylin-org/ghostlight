# tests-1: work ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). Decision: [ADR-0204](../../adr/0204-meaningful-tests.md).
Machine: test-03 (Alpine). Paths without a crate are under `crates/orchestrator/src`. Statuses:
`todo`, `landed` (with commits), `blocked` (with the reason).

## RESUME HERE

arch-1's W-D4 (eac543ea) and W-D5 (39b7fe8f) landed first. Every test of `work/` and `workspace/`
is classified (the plan below, read-only, 2026-10-21): 373 tests to 55, plus the P3 reserve test.
Cut order: the inline modules, then `work/tests/` file by file (golden, channels, provenance,
audit_health, credentials, controls, control, configured_authority, catalog_authority, documents,
presentation, composition, findings, replacement, restarts, availability, protocol4, rebound,
tabs), then `workspace/tests.rs`, then `execution.rs`. A test that folds into another file's test is
added there in the commit that deletes it. `governance/` moved to the language lane on 2026-10-21.

## Areas

| Area | Tests at opening | Budget | Now |
| --- | --- | --- | --- |
| `work/` and `workspace/` | 373 | 55 | 291 |
| `governance/` | 136 | 40 | moved to the language lane (2026-10-21) |

## Slices

One row per slice: what it cuts, its counts before and after, and what the kept tests protect
(promise, contract, defect, unsafe).

| Slice | Subject | Before | After | Kept for | Status |
| --- | --- | --- | --- | --- | --- |
| W1 | the inline modules: `invocation_id.rs`, `receipt.rs`, `pending.rs` and `documents.rs` keep no test, `item.rs` keeps only its three credential-masking tests until they fold into the credentials test | 373 | 357 | none kept; the 3 left fold later (promise E7) | landed (0d55d314) |
| W2 | `tests/channels.rs` and `tests/audit_health.rs`: one test per promise, with the channel refusal's words read from the typed refusal and the storage note from its constant | 357 | 354 | promise 2 (Gov 4, ADR-0159 1-2), contract 1 (`channel_refusal.json`) | landed (71fee018) |
| R1 | reserve: `a_stopped_recordings_status_says_it_stopped_and_a_running_one_that_it_runs` in `tests/execution.rs`, the coverage be54ad44 lost with the language outcome table | 354 | 355 | defect 1 (be54ad44), from the reserve | landed (d6a305f9) |
| W3 | credentials: `tests/credentials.rs`'s four tests are one, with a new grouped-fill row; `item.rs`'s three masking tests fold into `tests/controls.rs`'s executor-level masking test; the golden and provenance tests say what they protect | 355 | 349 | promise 2 (ADR-0185 3; Executor 7 masking), contract 5 goldens and 1 promise (Connection reporting 3, 5) kept as they were | landed (d40f1247) |
| W4 | `tests/controls.rs`: five tests that restated pass-through keys, a key list, a literal next step and a summary go, with their helpers; the masking test stays and the repeated-row test waits for the RAWX test | 349 | 344 | promise 1 (Executor 7); 1 left to fold | landed (c2f4b188) |
| W5 | `tests/control.rs`: five control tests are one, over the moment Pause or End lands; a cancellation unit, a workbench start-time check, a retired-argument decode test (the language lane's) and an explain deadline test (Executor 3, kept in execution.rs) go; two tests wait for the RAWX test | 344 | 336 | promise 1 (Gov 12, ADR-0185 4-5); 2 left to fold | landed (7cddce52) |
| W6 | `tests/configured_authority.rs`: the selector-fill credential row and the credential-like upload row join the credentials test; the typing-authority, unsent-form and retired-host tests go (the RAWX test, Exec 1 decode); the b9c2da60 landing test stays until `tests/tabs.rs`'s held-tab test takes it | 336 | 331 | contract C10c; 1 left to fold | landed (cf0913cb) |
| W7 | `tests/catalog_authority.rs`: four variant tests are one RAWX test, now also refusing then admitting each call in one session (control.rs's refusal test) with every listed control in a repeated row that never reaches audit or history (controls.rs's row test); two recording-save tests are one; control.rs's publication test is a row of the person's-control test | 331 | 324 | promise 2 (Gov 3, 10, 13; ADR-0158 4) | landed (5df9f077) |
| W8 | `tests/documents.rs`: nine document tests are one over ADR-0158; the two settle-budget tests are one, for ACCEPTANCE Bounded preparation (ADR-0195, 2513f862), which no other test held, until the execution.rs waits test takes it; an effect-unknown script row goes (execution.rs's typed-refusal test) | 324 | 315 | promise 1 (ADR-0158 1-3, 5); 1 left to fold | landed (37f5225f) |
| W9 | `tests/presentation.rs`: four feedback and notice tests are one, where feedback lands and what a refusal draws; the per-step receipt count goes (execution.rs keeps one receipt per step) | 315 | 312 | promise 3 (Presentation 1, 2, 3, 5) | landed (cc8f6580) |
| W10 | `tests/findings.rs`: five record tests are one, each record holds only what its frame reached; three flow-step tests are one, each step now compared with the same call made alone | 312 | 306 | 2 (ADR-0202 D9-13; 77952a37, b0436754, b411e6ad, 0c9bb755) | landed (6926865b) |
| W11 | `tests/replacement.rs`: sixteen tests are one, every sentence and step now read from `WorkspaceReason::StaleTarget` instead of literals | 306 | 291 | promise 1 (Exec 6, LANGUAGE stale handles; 43eff032) | landed |

## Notes

- From the coordinator (2026-10-21), out of the batch's reserve: `work/tests/presentation.rs` gains
  `a_presentation_failure_changes_no_result_browser_command_or_audit_line`, the first real test of
  ACCEPTANCE Presentation 3 (the old one in `presentation/mod.rs` only asserted that nothing
  panics). The same read, drawn once and with every signal refused once, gives the same result,
  browser commands and audit line. Keep it in this lane's cut; it counts against the reserve, not
  this lane's budget.

## The plan for `work/` and `workspace/` (2026-10-21)

Four read-only passes read every test against ADR-0204 Decision 1. Kept tests, by file, with
what each protects; a fold names the tests a kept one absorbs in its own doc comment when it lands.

- `tests/golden.rs` (5) and `tests/channels.rs`'s golden test: the five model goldens and the audit
  success golden. Contract, unchanged.
- `tests/configured_authority.rs`: `policy_explain_and_its_audit_line_name_one_authority` (C10c).
- `tests/provenance.rs`: queued refusals keep their original evidence (Connection reporting 3, 5).
- `tests/presentation.rs`: a refusal draws its notice on the tab it kept or held (W-D2, C2, P2);
  `fill_upload_and_scroll_name_their_targets_once_the_browser_has_acted` (f544f45c); the P3 test
  (reserve).
- `tests/catalog_authority.rs`: every catalog variant is admitted by exactly its RAWX set (Gov 3,
  10, 13; ADR-0185 1-2; ADR-0156 1-2; C10b); a recording save needs Read on every source and Write
  on its destination (ADR-0158 4; C10b).
- `tests/control.rs`: a person's control stops the next effect and never rewrites a sent one
  (ADR-0185 4-5, Gov 12).
- `tests/credentials.rs`: credential input needs the user's word on each call (ADR-0185 3), with a
  new grouped-fill row.
- `tests/channels.rs`: a channel switch refuses only its own channel (Gov 4).
- `tests/audit_health.rs`: an audit failure keeps the outcome and Require audit refuses later work
  (ADR-0159 1-2).
- `tests/documents.rs`: content and effects come only from admitted documents (ADR-0158 1-3, 5).
- `tests/availability.rs`: a pinned browser that left is waited for or started again; a session
  with no browser waits only while one is returning; a pinned browser that stays gone is started
  only when Ghostlight knows it; the no-browser answers name the store only while setup is
  incomplete; a session leaves its browser only when it is gone and the agent names another
  (W-D5). (Contract 17, 19, 20; ADR-0196 D12; b88f3d3c.)
- `tests/findings.rs`: each record holds only what its own frame reached (77952a37, b0436754,
  ADR-0202 D9-13); a flow step meets what the same call meets outside a flow (b411e6ad, 0c9bb755).
- `tests/restarts.rs`: a tab binding across a gap is confirmed before any command reaches it
  (Contract 19, ADR-0203 D1; 1d10bf74, 8a16343f, c4e382aa), with the inert-binding rows from
  `workspace/tests.rs`.
- `tests/replacement.rs`: a removed handle names a replacement only when the page proved one, the
  tool takes one and governance reveals it (Executor 6, LANGUAGE stale handles; 43eff032).
- `tests/protocol4.rs`: a protocol-4 failure reads to the model with its true effect (contract);
  a dialog is answered from the browser's record and authorized by the document that raised it
  (c2df20e2, 925d573c, ADR-0158 5), with the executor's dialog rows; a fill's answer says what the
  page did and names only what policy keeps; what the service asks the page (3e80fb91, 8256f32d).
- `tests/rebound.rs`: every action that followed its handle says rebound (ADR-0198 D17; 3466718c,
  259241ad).
- `tests/tabs.rs`: default reuse takes only a kept tab of the same agent still at its address (BJ
  25, ADR-0196 D9); a group is titled for the opener as the window names it (Ext 8); a same-document
  route keeps targets and a commit or new origin expires them (BJ 2); a call naming no tab (Exec 6,
  ea58ecb4); a held tab refuses with its landing's decision (ADR-0203 D2.4, D5.5); a close is
  authorized on its own (Gov 9, 14).
- `workspace/tests.rs`: a busy workspace waits within the deadline; a session is its opener and
  owner and outlives its connections (ADR-0106; c78d4bd3, 10fc554a); a control keeps one handle and
  one record per document, the book stays bounded and every handle has the decoded shape
  (d80b64c3); a tab forgotten by any close leaves nothing its page minted (7bd0bf20).
- `tests/execution.rs`: a browser failure renders its typed refusal with the true effect (Exec
  3-4, L-D6, ADR-0203 D5); deadline advice; unconfirmed expectations; one executor and one receipt
  per step; a flow under `on_error` and human directives (Exec 5); stale handles (Exec 6, d0cea694);
  W-D4; open, read and close (BJ 1); view handles (Exec 8, 8f2241c0); waits (BJ 5, 2513f862);
  recording save and one recording per session (BJ 17-18, 79d460c9, C16); session isolation and
  the ownership refusal (BJ 24); the audit holds no page, caller or browser text (Gov 10, 15-17).
- The inline modules (`item.rs`, `pending.rs`, `composition.rs`, `documents.rs`, `invocation_id.rs`,
  `receipt.rs`, `reading.rs`) keep no test: their behaviors fold into the executor tests above.

Gaps found, for the reserve or other lanes: uploads refused before any effect (Executor 9, BJ 13);
a late receipt after a cancellation commits nothing (Executor 2); back, forward and reload under
landing governance (BJ 9); diagnostics exclude the client's claim (Connection reporting 5, once
`receipt.rs`'s test goes); ACCEPTANCE BJ 25 still names a removed `reuse: never` field, and
ADR-0185 item 4's "names that action before saying why" disagrees with b97335a4.
- From the language lane's close (2026-10-21; `LEDGER-language.md` Notes): keep
  `work/tests/execution.rs`'s `every_tool_gets_deadline_advice_it_can_follow` and
  `calls_at_the_longest_timeout_are_not_told_to_raise_it` (ACCEPTANCE Executor 3),
  `work/tests/protocol4.rs`'s `a_refused_fill_lists_what_would_succeed_and_names_each_handle` and
  `a_fill_the_page_partly_undid_is_a_partial_failure_with_no_dialog_advice`, and one end-to-end
  group-title assertion in `work/tests/tabs.rs`. Out of the reserve, 1 more: an executor-level test
  of be54ad44 (a stopped recording's status says it stopped and is ready to save; a running one
  keeps its sentence), whose only test went with the language outcome table.
