# arch-1: surface ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). The specification of each slice is its entry in
[ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`, `landed`
(with commits), `blocked` (with the reason).

## RESUME HERE

S1, S2, S4a and S9 landed. Next: S8 and S7, which can start now.

The second sweep (2026-10-20) adds its slices below, on leo-desktop-02 after install; the order there includes S3 to S8.

## Slices

In the ADR's order. "After" names what a slice waits for, including the ux-2-first rule.

| Slice | Subject | Gate | After | Note | Status |
| --- | --- | --- | --- | --- | --- |
| S1 | fix(tray): every surface offers the acts the runtime state accepts | candidate | P10 | - | landed (37554eab) |
| S2 | fix(workbench): history rows and counts say what the service decided | candidate | C12a | see Notes | landed (this commit) |
| S4a | fix(workbench): the Policy page previews and names what governance decides | candidate | P3, P4 | see Notes | landed (this commit) |
| S8 | fix(window): a window that cannot be built says so | none | ux-2 closes | - | todo |
| S6 | fix(setup): setup starts browsers the way recovery does, and marks the right host in use | none | C3c, C13 | - | todo |
| S3 | refactor(workbench): every part comes from one gather | none | P5, I6, C2 | - | todo |
| S4b | refactor(workbench): the Policy page's words come from governance | none | P7, P8 | - | todo |
| S5 | refactor(workbench): the window's words come from language/ and are pinned | none | C18, C12b | - | todo |
| S7 | test(desktop): the shell decodes and re-encodes every desktop golden | none | ux-2 closes | - | todo |
| S9 | fix(workbench): the window offers Remove whenever the service does | none | I4 | the window half of G3-02; see Notes | landed (this commit) |

## Notes

- S1: ux-2's 77840ccc had already relabeled the tray's Resume entry "Start a new session" for an
  ended session, with the StartSession intent, and corrected the doc the ADR names at
  `intake/desktop.rs:383-384`. So the entry stays enabled once the session ended: what it asks for
  is offered. `TrayModel::of` now enables a runtime entry exactly when `governance::offers` names
  its act, so Resume is enabled only while paused. The test the ADR names replaces
  `an_ended_tray_offers_a_new_session_but_not_pause`.
- S1: the controls part carries `runtime.offers`. Settings shows a runtime button when its act is
  offered and the context may do it; the band's wheel does the first offered act. Before any
  controls part has arrived, the band shows neither the wheel nor the Continue line.
- S1, cross-lane: the fake controls parts in core's `tests/workbench-surface.mjs` and
  `tests/workbench-preview-server.mjs` gain `offers`, and the status-push check in
  `workbench-surface.mjs` also pushes the controls part, as the service does. Nothing else in
  those files changes.
- S2, the rows: `HistoryItem.outcome` is `OutcomeClass::recorded` of the record, and a flow's
  placeholder parent is `unknown`. The page keeps the class words as `OUTCOME` in `words.js` and
  draws the row, the running card, the screen reader's word and a flow's step labels from it.
  `isBlocked` reads the `refused` class and `isFailed` the `failed` and `unknown` ones; the entry
  no longer carries `status` or `allowed`, which nothing drew from once the class arrived. The
  refused class keeps the word "Blocked"; a credential refusal reads "Needs you" in the quiet
  look a kept tab has. A flow held back by a refused step, which the page used to call Kept, is
  now Blocked.
- S2, search (beyond the ADR's list, same finding V-05): `HitOutcome` re-derived the outcome a
  third time and also called a credential refusal Kept. It is deleted, and `SearchHit.outcome` is
  the row's class. `language::workbench::search_outcome` gives the word each class is found by,
  the row's word in lower case ("blocked", "kept", "needs you", "failed", "unknown"), and
  `surface_tests.rs` pins `OUTCOME` and `OUTCOME_WORD` to `OutcomeClass` and to it.
- S2, the counts: the connections part carries `counts`, the `ConnectionCounts` the tray and
  `status` already say, and the band and About read them. The ADR's "count connections, not
  sessions" is read as these service counts: agents stay counted one per agent id, as At a glance
  groups them (ux-2's 7a5065d3, ADR-0199 amendment of 2026-10-18). At a glance keeps grouping the
  sessions it draws. `OverviewSummary` and the page's `overview` stay for S3.
- S2, proofs: `a_credential_refusal_row_says_needs_you`, `a_kept_tab_row_says_kept`,
  `the_connections_part_carries_the_counts_the_tray_says` and the reworked search test in
  `workbench/tests.rs`; `the_page_draws_every_outcome_class_and_is_found_by_its_word` in
  `surface_tests.rs`; the page tests "a credential refusal row says needs you, in ink, and is never
  drawn as kept or blocked" and "the band and About count what the tray counts", which replaces
  "the band, the pill and About count one thing".
- S2, cross-lane: core's `tests/workbench-surface.mjs` and `tests/workbench-preview-server.mjs`
  fakes now carry `counts` on the connections part and `outcome` on their records, as the service
  does. Nothing else in those files changes. No golden changed, and no journey needs a test
  machine.
- S4a, the preview: `preview_user_policy` replays each recorded action through
  `AuthoritySnapshot::replay` with the action's own channel, as simulate does, so a line that kept
  no channel is MCP work. No departure.
- S4a, the name: the card and the refusal row draw the publication's one identity, the snapshot's
  `organization()`, which is the compiled view's `organization`. The window gets it as
  `OrganizationView`: the identity plus the two ways the window says it, `title` ("Your
  organization" when nothing names it) and `in_sentence` ("your organization"), both from
  `language::policy`. The policy route sends it in place of the view's `organization`, and the
  status part carries the same value, because the refusal row is drawn on the activity page and
  the policy is read only when its page opens. The six literals were four "Your organization" and
  two "your organization"; the settings, the browser-startup row and the capability boxes read
  `title` too.
- S4a, beyond the ADR's Deletes (same finding G-07): the passport the window is sent no longer
  carries `organization`, `rationale` or `contacts`, which only repeated the identity once P4 named
  the passport from it. The status file keeps them. The card reads the passport for provenance
  only.
- S4a, proofs: `a_draft_that_turns_mcp_off_previews_every_mcp_action_refused` in
  `workbench/tests.rs`; `the_refusal_row_and_the_card_name_one_organization` in
  `workbench/parts/tests.rs`; the page test "the refusal row and the card name one organization,
  in the service's words", which also holds that `view.js` names no organization of its own. The
  part key lists in `parts/tests.rs` follow the new shape.
- S4a, cross-lane: core's `tests/workbench-surface.mjs` and `tests/workbench-preview-server.mjs`
  fakes carry the organization as the service names it, in the status part and the policy, and
  passports without the naming. Nothing else in those files changes. No golden changed, and no
  journey needs a test machine.
- S9, added by the coordinator at integration: I4 made the service offer Uninstall for every
  entry Ghostlight owns, updatable ones included (`can_uninstall`), but `actsOf` in
  `ui/lib/view.js` still showed Remove only for an installed agent, so the window hid a Remove the
  service offered (the second half of G3-02, Decision 10.2). Remove now reads the service's answer
  alone (`manage && agent.can_uninstall`). The page test "Remove follows the service's answer
  alone, so an updatable entry it can remove shows Remove" pins it both ways. No golden changed,
  and no journey needs a test machine.

## The second sweep (2026-10-20)

Rules: [BOOTSTRAP.md](BOOTSTRAP.md), "The second sweep (2026-10-20)". Machine: leo-desktop-02 (Windows), after install. A
slice's specification is its finding in [sweep-2026-10-20/surface.md](sweep-2026-10-20/surface.md), read
with its skeptic's corrections. Nothing has landed yet; start with U-D1.

Order: U-D1 to U-D3, then the first sweep's open slices in their rows above (S8 and S7, then S3,
which also covers SURF-2 and wires INST-15's `resymbol`, then S4b, S5 after C18, and S6 after
C3c), then the rows below.

| Slice | Subject | After | Source | Status |
| --- | --- | --- | --- | --- |
| U-D1 | fix(workbench): a policy draft near the limit gets the policy-size sentence | - | SURF-5's size defect; one web test | todo |
| U-D2 | fix(window): coverage details survive Clear | - | SURF-5's coverage defect | todo |
| U-D3 | fix(window): the applied-rules toast says its words once | - | SURF-3's note | todo |
| U-D4 | fix(workbench): a runtime control that changes nothing a screen shows publishes nothing | - | found by tests-1's service pilot, 2026-10-21 (Notes, second sweep); check first | todo |
| U-D5 | fix(workbench): a window with the bearer always finds room among the workbench's connections | - | found by test-01's tests-1 classification, 2026-10-21 (Notes, second sweep); test-01 took it first in `workbench/` | landed (9db6e896) |
| U-D6 | fix(workbench): a refused HEAD request carries no body | - | the same classification; test-01 | landed (dc1c4fbf) |
| SURF-1 | refactor(workbench): search names pages by `PageAddress` | - | SURF-1 | todo |
| SURF-4 | refactor(window): the glyph map is keyed by the activity token | S5 | SURF-4; a deliberate visible change on some waiting rows | todo |
| SURF-6 | refactor(window): the JS copies of the service's vocabularies are complete | - | SURF-6 | todo |
| SURF-8 | refactor(workbench): what a watcher may do is said once | - | SURF-8 | todo |
| SURF-10 | refactor(workbench): the small repeats go | - | SURF-10 | todo |
| SURF-12 | refactor(workbench): watching, everywhere | - | SURF-12; the route path changes with the preview server | todo |
| SURF-13 | refactor(window): the setup flow is one object and the click handler a table | - | SURF-13 | todo |
| SURF-14 | refactor(desktop): the contract's names match the service's | S7 | SURF-14 | todo |
| SURF-7 | refactor(window): the policy editor and the agents pane leave `view.js` | SURF-6 | SURF-7 as corrected | todo |
| SURF-9 | refactor(workbench): `workbench/mod.rs` splits on its seams | S3 | SURF-9; no `kit.css` rename | todo |
| SURF-11 | refactor(window): the palette has tokens, and the desktop one module | - | SURF-11; colour values stay | todo |
- U-D4, found by tests-1's pilot cut of `service/mod.rs` on 2026-10-21: the service test
  `the_browser_toolbar_pauses_through_the_workbench_and_says_so` required a second
  `RuntimeChanged` event for a repeated Hold, which its own comment calls a change of nothing a
  screen shows. `WorkbenchFacade::apply_runtime_control` publishes on every call. It also records
  which source paused, so check whether a repeated control from another source changes what a
  screen shows before deciding; publish only when the state or the pausing source changed. The
  pilot deletes the test.
- U-D5 and U-D6, found by test-01 classifying `workbench/`'s tests on 2026-10-21 and decided by the
  coordinator for what delights the person. U-D5: the loopback workbench's 32-connection cap is
  shared with callers that carry no bearer, so any local account could hold all 32 open and lock the
  person's own window out; connections without a bearer get their own smaller share. U-D6: a HEAD
  request that is refused (401, 403, 404, 421) is answered with a JSON body, which an HTTP HEAD
  answer never carries (RFC 9110); only page files and agent images stripped it.
- U-D5 as landed: 32 places in all, of which at most 12 are without the bearer
  (`MAX_WATCH_STREAMS` 8 plus `SHORT_REQUESTS_WITHOUT_BEARER` 4, so a ninth watcher still gets its
  503 too_many_screens), and 20 stay for the window. A peer starts in the share without a bearer and
  leaves it when its request shows the bearer. A peer let in past the share has `BEARER_DEADLINE`,
  1 s, to send a request that carries the bearer, or it is closed unanswered. A screen that holds
  the bearer must send it on the request that opens the connection.
- U-D6 as landed: the connection answers a HEAD the way every route answers a read and sends only
  the headers, in one place (`workbench/web/mod.rs`), so the per-route copies went.
