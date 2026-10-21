# tests-1: language ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). Decision: [ADR-0204](../../adr/0204-meaningful-tests.md).
Machine: test-01 (CachyOS). Paths without a crate are under `crates/orchestrator/src`. Statuses:
`todo`, `landed` (with commits), `blocked` (with the reason).

## RESUME HERE

Every area of this lane is cut, on 2026-10-21, and the lane has stopped, as the coordinator asked
when the owner cut token spend. `language/` closed at 55 of 349 (TL1-TL23), `workbench/` at 37 of 252
(TW1-TW6; 35 plus 2 from the reserve for the U-D5 and U-D6 reproductions), and `governance/` at 40
of 136 (TG1-TG6). The cut found product defects, and each landed first with its proof as a row or
one test: arch-1 L-D7, U-D5, U-D6, G-D1 (a trailing-dot host stepped around deny lists), G-D2,
G-D3 and G-D4. S-D7 also came to this lane and landed, with its live check on test-01.

## Areas

| Area | Tests at opening | Budget | Now |
| --- | --- | --- | --- |
| `language/` | 349 | 55 | 55 |
| `workbench/` | 252 | 35 + 2 reserve | 37 |
| `governance/` | 136 | 40 | 40 |

## Slices

One row per slice: what it cuts, its counts before and after, and what the kept tests protect
(promise, contract, defect, unsafe).

| Slice | Subject | Before | After | Kept for | Status |
| --- | --- | --- | --- | --- | --- |
| TL1 | test(language): `self_check/` keeps the 7 tests of the model surface contract | 21 | 7 | 3 promise, 3 contract, 1 defect | landed (52dd543a) |
| TL2 | test(language): provenance.rs keeps the 3 tests of connection reporting | 18 | 3 | 2 promise, 1 defect | landed (d31b030e) |
| TL3 | test(language): workbench.rs keeps the words two surfaces share and the agent count | 26 | 2 | 1 contract, 1 defect | landed (29f57646) |
| TL4 | test(language): area.rs keeps the one test of what left_out may carry | 8 | 1 | 1 contract | landed (1978ecaa) |
| TL5 | test(language): setup.rs keeps the 9 tests of the install journey | 44 | 9 | 4 promise, 3 contract, 2 defect | landed (b8b08fb2) |
| TL6 | test(language): deployment.rs keeps the way back and the dry run | 23 | 2 | 1 contract, 1 defect | landed (dea97d01) |
| TL7 | test(language): readiness.rs keeps the precedence and what the answer means | 15 | 2 | 2 promise | landed (4d332192) |
| TL8 | test(language): environment.rs keeps one table of the machine row | 7 | 1 | 1 promise | landed (b0c24284) |
| TL9 | test(language): channel_switch.rs keeps the policy sentence | 4 | 1 | 1 promise | landed (00a6928c) |
| TL10 | test(language): agent_icon.rs keeps the doctor line | 2 | 1 | 1 promise | landed (fc88b901) |
| TL11 | test(language): intake.rs, migration.rs and adapter.rs drop tests of their literals | 7 | 0 | - | landed (9820d0c8) |
| TL12 | test(language): the decoder and catalog tests become the model surface contract | 53 | 14 | 5 promise, 6 contract, 3 defect | landed (55efa603) |
| TL13 | test(language): field_problem.rs and field_echo.rs keep the fill sentences LANGUAGE.md tables | 45 | 9 | 6 promise, 1 contract, 2 defect | landed (c22367a2) |
| TL14 | test(language): capability_map.rs keeps the rawx table LANGUAGE.md documents | 5 | 1 | 1 promise | landed (7b09ce75) |
| TL15 | test(language): handle.rs keeps the prefix contract with the extension | 2 | 1 | 1 contract | landed (af86d9d7) |
| TL16 | test(language): provenance.rs folds its live and record tables into one | 3 | 2 | 2 promise | landed (48d07ae0) |
| TL17 | test(language): outcome.rs and audit.rs keep the 8 tests of what happened | 57 | 8 | 4 promise, 2 contract, 2 defect | landed (90fee6d0) |
| TL18 | test(language): history.rs keeps the window words and the outcome class | 8 | 2 | 2 promise | landed (6d8d51e2) |
| TL19 | test(language): page_refusal.rs and composition.rs drop tests the executor proves | 7 | 0 | - | landed (5418db01) |
| TL20 | test(language): audit_health.rs keeps L-D2 and L-D3 | 3 | 2 | 2 defect | landed (c8f0f788) |
| TL21 | test(language): settlement.rs keeps one table of what settles | 1 | 1 | 1 promise | landed (e9d4889c) |
| TL22 | test(language): setup.rs words join the shared words test, and the listing URL is built from the id | 11 | 8 | 4 promise, 3 contract, 1 defect | landed (4d3eae13) |
| TL23 | test(language): readiness.rs folds the sentence into the precedence table | 2 | 1 | 1 promise | landed (7fea1702) |
| TW1 | test(workbench): the loopback server keeps the 14 tests of its promises and gates | 105 | 14 | 5 promise, 5 contract, 4 defect | landed (c0be969a) |
| TW2 | test(workbench): the facade keeps the 14 tests of what the window shows | 83 | 14 | 10 promise, 3 contract, 1 defect | landed (9737dc14) |
| TW3 | test(workbench): history.rs keeps the two grouped-history promises | 6 | 2 | 2 promise | landed (d0265970) |
| TW4 | test(workbench): setup.rs keeps the 5 tests of the setup journey, and parts/ its none | 54 | 5 | 4 promise, 1 defect | landed (15cf6d1a) |
| TW5 | test(workbench): watching.rs keeps the marker rule | 3 | 1 | 1 promise | landed (3e477cde) |
| TW6 | test(workbench): input.rs keeps the search bound, checked against the page | 2 | 1 | 1 promise | landed (f7b86902) |
| TG1 | test(cli, governance): the policy subcommands parse in the completions test, and governance drops its copy | 1 | 0 | - (moved to cli/parse.rs) | landed (0cee90cc) |
| TG2 | test(governance): managed policy keeps the 13 tests of its published claims | 36 | 13 | 10 promise, 1 contract, 2 defect | landed (370ea14d) |
| TG3 | test(governance): the authority keeps the 18 tests of what policy decides | 64 | 18 | 10 promise, 2 contract, 5 defect, 1 unsafe | landed (570b13ba) |
| TG4 | test(governance): the manifest and the host grammar keep the 4 tests of the file admins write | 16 | 4 | 1 promise, 2 contract, 1 defect | landed (c3f7f4e6) |
| TG5 | test(governance): migrate.rs keeps the schema-3 move and the file move | 11 | 2 | 2 promise | landed (73454678) |
| TG6 | test(governance): simulate and the decision evidence keep their 3 tests | 8 | 3 | 2 promise, 1 defect | landed (8cef6994) |

## Notes

- `language/` kept for, by main kind: 32 promise, 11 contract, 12 defect, 0 unsafe. Folded defects
  are counted once, under the test that keeps them.
- The method: four read-only plans (one per group of files) proposed 70; one review read them
  side by side, removed promises proven twice across plans (the decoder and catalog refusals now
  live once, in the gate-2 tests), and found 55 without merging unrelated behaviors.
- Product code that only deleted tests used went: `COPIED_COMMAND`, `COPIED_SETUP`,
  `Environment::ALL`, `CATALOG_TOOL_COUNT`, `EXPECTED_TOOL_NAMES`, `FieldProblemKind::as_str`,
  two `WaitCondition` conversions and, under the service lane's hold, `RecoveryFailure::as_str`.
  The store listing URL is now built from the store extension id at compile time, so the two
  cannot drift.
- Tests in other lanes that a `language/` promise now rests on, which those lanes must keep:
  `work/tests/execution.rs` `every_tool_gets_deadline_advice_it_can_follow` and
  `calls_at_the_longest_timeout_are_not_told_to_raise_it` (Executor 3); `work/tests/protocol4.rs`
  `a_refused_fill_lists_what_would_succeed_and_names_each_handle` and
  `a_fill_the_page_partly_undid_is_a_partial_failure_with_no_dialog_advice`;
  `protocol4/merge/tests.rs` `a_cut_keeps_page_content_over_chrome_over_hidden_in_document_order`
  (the tier cut order); one end-to-end group-title assertion in `work/tests/tabs.rs`.
- Gaps found while cutting `language/`. The claim sentinel was already covered:
  `tests/provenance-journey.mjs` plants claim canaries and asserts they are absent from the audit
  and from diagnostics. be54ad44 (a stopped recording read as running) lost its only test with the
  outcome table, and the coordinator granted the work lane 1 from the reserve for an executor-level
  test of it.
- `workbench/` kept for, by main kind: 23 promise, 8 contract, 6 defect, 0 unsafe. The two
  `workbench/tests.rs` tests the `language/` cut relies on were renamed in TW2:
  `a_control_confirmation_states_the_authority_and_never_browser_delivery` is now
  `a_runtime_control_is_offered_and_applied_by_the_authority_with_or_without_a_browser` (still
  asserting no `browser_notified` on the wire), and
  `agents_are_counted_as_at_a_glance_groups_them_and_connections_by_intake` is now
  `the_window_the_tray_and_status_count_agents_one_way` (still asserting the ADR's sentence).
- `workbench/`'s page and server contracts now read the page's own files: the route table equals
  `ui/lib/transport.js` `ROUTES` (both ways, methods included), the sequence header, the stale code
  and the closed-context word come from `transport.js`, the address list from `words.js`, and the
  search bound from the page's `maxlength`.
- Accepted losses in `workbench/`, none of them a docs/1.0 promise, a docs/trust claim or a named
  defect's only test: the connection cap's 33rd connection (U-D5's test proves the window's share),
  the watcher rate limits, the stream keep-alive against the page's dead timer, the stream places
  per audience, and the facade with no server attached. `WorkbenchServer::start` is now called
  only by the test fixture.
- `governance/` kept for, by main kind: 25 promise, 5 contract, 9 defect, 1 unsafe (the workspace
  unsafe-confinement guard AGENTS.md names, which lives in `governance/tests.rs`). One review fitted
  the two plans (46) into 40 by folding the same rule seen from two files. The policy subcommand
  check moved into `cli/parse.rs`'s completions test (TG1), with test-04's hold and the coordinator's
  approval. Gaps left open: no test drives an HTTPS managed source through the authority or shows a
  pinned CA honored, since both need a TLS test server as a new dev-dependency, the owner's call.
