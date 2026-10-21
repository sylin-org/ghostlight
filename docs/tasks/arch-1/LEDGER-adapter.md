# arch-1: adapter ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). The specification of each slice is its entry in
[ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`, `landed`
(with commits), `blocked` (with the reason).

## RESUME HERE

A1 to A4 have landed, and so has the package-lane fix A2's top-frame ring reset waited on (see
Notes).

The second sweep (2026-10-20) opens this lane again: see its section below. BROW-10, BROW-4, BROW-7,
BROW-6, BROW-14, BROW-8a, BROW-11a, BROW-15, D52, D52(c) and BROW-12a have landed; BROW-3a is
next.

## Slices

In the ADR's order. "After" names what a slice waits for, including the ux-2-first rule.

| Slice | Subject | Gate | After | Note | Status |
| --- | --- | --- | --- | --- | --- |
| A1 | fix(adapter): an embedded document's change is stale, and a targeted read reports the tab | candidate | none | see Notes | landed (fa3bcb02 and a9d40c83) |
| A2 | fix(adapter): a Downloads save carries effect, and diagnostics bind only when asked | candidate | none | see Notes | landed (a0dfc56b) |
| A3 | refactor(adapter): no content command runs without a scope | none | none | see Notes | landed (8f5722e8) |
| A4 | refactor(adapter): the authority's budgets reach the adapter as values | none | ux-2 closes (`assets.sha256`) | cross-lane work's `DOCUMENT_TREE_NODE_LIMIT`; see Notes | landed (f6ff657d and its review fix) |

## Notes

- A1, cross-lane test: core's `work/tests/protocol4.rs` pinned a click whose input met
  `document_changed` as "Document access could not be verified.". A click names one locator, so
  Decision 5.4 makes it a stale target that navigated. The test is renamed
  `a_document_that_changed_before_input_is_a_stale_target_not_unknown` and asserts that, in this
  commit, so the tree stays green.
- A1, a malformed locator: with the locator branch of `unresolved` gone, a locator that does not
  decode fails the description as `StaleLocator { Unknown }`, the error `Locator::decode` already
  gives, instead of marking the inventory unresolved. Nothing is read in either case.
- A1, scope of the receipt rule: the `document_changed` receipt failure is restated in the Link
  (`link.rs`), as the ADR names it. A world call's own `document_changed` answer
  (`translate::value`) still reads as an unavailable document; the guard on the receipt fires
  first in the common case.
- A1, proofs: `a_target_in_a_reloaded_iframe_is_refused_as_navigated`,
  `an_incomplete_inventory_keeps_document_unavailable`,
  `a_targeted_read_in_an_iframe_reports_the_tabs_url_and_host`, the rewritten
  `describing_documents_binds_locators_points_and_focus_to_current_documents`, and
  `a_document_change_on_a_one_locator_call_is_a_navigated_target`, all in
  `protocol4/translate/tests.rs`. The work-level proof is C9's. No journey is needed.
- A1, review fix: a recording exported into a page names one locator, the file input, and runs
  under the scope guard, so `BrowserCommand::sole_locator` names it. A `document_changed` on that
  export is a navigated target, pinned by
  `a_document_change_during_a_page_export_is_a_navigated_target` in
  `protocol4/translate/tests/points.rs`.
- A2, effect: a Downloads save now carries effect, so runtime control admits it like any other
  effect, and a pause refuses it before anything is sent. Work already reported a delivered
  Downloads save as applied and not repeat-safe, so the two now agree.
- A2, binding: `Scope::watches` names the authority's `watch_changes`, and the doc of
  `DocumentScope.watch_changes` says it also binds a diagnostics read. Work sets it when its
  policy does not admit every document, so no work file changes.
- A2, proofs: `a_downloads_save_whose_receipt_is_lost_reads_as_after_dispatch` in
  `protocol4/link/tests.rs`; `an_unrestricted_diagnostics_read_sends_no_binding` and
  `a_restricted_read_still_binds_its_documents`, which replaces
  `the_diagnostics_binding_follows_the_admitted_set`, in `protocol4/translate/tests.rs`. The
  work-level proof is C9's. No journey is needed.
- A2, review fix, open dependency on the package lane: with no binding sent when the scope does
  not watch, the default all-open mode binds no tab, and the extension's `beforeNavigate`
  (`extension/lib/buffers.js`) resets a ring only when it deletes a binding. So a top-frame
  navigation no longer starts a fresh ring there, and a diagnose after it returns the prior page's
  console and network entries with the new page's. Decision 6.3 keeps the top-frame reset for every
  tab, so this is a defect, not a choice. The fix is the extension's: a top-frame `beforeNavigate`
  starts a fresh ring for any tab with diagnostics capture, bound or not, and deletes the binding
  if there is one. Its proof is an unbound read in `worker.test.js` or `buffers.test.js`: a
  subframe navigation keeps the entries and a top-frame navigation clears them. It changes a
  packaged file, so that commit regenerates the package record (ADR-0200 Decision 16). Recorded in
  LEDGER-package.md, where it has landed, so A2's candidate gate no longer waits.
- A3, `Reach` goes whole, not only `Reach::Open`: with one variant left it only forwarded to the
  scope. `reach()` in `translate/mod.rs` turns the dispatch's optional scope into the scope a
  content command needs, or `BrowserError::Protocol` before anything is sent, and every content
  translator takes `&mut Scope` (the diagnostics and recording translators `&Scope`), not only
  `script::evaluate` and the recordings. `Reach::target` becomes `Scope::target`, which replaces
  the test-only `doc_of` and `doc_or_top`.
- A3, `PlannedCall::unscoped` stays for the calls no scope covers: metadata (a route's hit point,
  a description's focus probe) and a screenshot's target measurement. Work scopes no screenshot,
  so a screenshot is not a content command, and its locator is the agent's own handle. The one
  content-command use, a page export sent without a scope, is gone.
- A3, `EffectClass::Prepare` goes: no rule reads it, because the link counts only `Effect`. Its
  messages are `Read`, whose doc now says it covers positioning a target for an action.
- A3, `Listing.forms` goes, so core's `work/tests/controls.rs` drops its `..Listing::default()`,
  which clippy reads as a needless update. That is the only cross-lane edit.
- A3, tests: eight tests sent content work bare, not six. The fill tests
  `a_submit_outside_the_fields_documents_is_refused_before_anything_is_sent` and
  `malformed_locators_are_plain_failures_before_anything_is_sent`, the three recording-start tests
  in `protocol4/link/tests.rs` (through a `start_recording` helper) and three page-export tests in
  `translate/tests/points.rs` now run inside an all-open scope. Removed as pinning unguarded
  behavior: `a_point_without_a_scope_is_clicked_unguarded`,
  `a_recording_exported_into_a_page_without_a_scope_carries_no_guard`, and the unscoped tails of
  the key-press and script tests in `translate/tests/interaction.rs`. The tests of the deleted
  `FrameTree::ancestors` and `geometry::output_scale` go with them. The proof is
  `a_content_command_without_a_scope_is_a_protocol_error` in `protocol4/translate/tests.rs`. No
  journey is needed.
- A4, who owns each number: work names every semantic query's maximum, as it does for find:
  `SELECTOR_MATCH_MAX` (8) for a selector, a stale handle's candidates and a `selector_present`
  wait. The page reads `max_results` and keeps no number of its own.
- A4, review fix: the first cut asked a presence wait for 1 match and the stale candidates for 3.
  The page marks its answer truncated when more matched than it was asked for, and the scope turns
  that into the coverage size limit, so a satisfied wait over two "Save" buttons said "The result
  reached its size limit." and the stale refusal said it past 3 fuzzy matches. The size limit
  should mean content the agent was owed was left out. Both queries now ask with
  `SELECTOR_MATCH_MAX`, so the threshold is the 8 the page held before A4, and work keeps the
  stale refusal's `STALE_CANDIDATES_MAX` (3) with its own `take`. The scope still marks any
  truncated answer, the one rule for every content call. Proof:
  `a_selector_present_wait_over_two_matches_reports_no_size_limit` in `work/tests/protocol4.rs`,
  which runs through the protocol-4 scope with a page that keeps the maximum it is sent.
- A4, the cut flag: `Listing.truncated` carries the merge's `over`. `Outcome::SelectorUnresolved`
  gains `more`, and with it the sentence starts "At least". Zero matches keep their sentence. The
  outcome change is the one edit in `language/` (core's `outcome.rs` and `self_check/hints.rs`).
- A4, the screenshot budget: `Screenshot` and `ScreenshotRegion` carry `settle_ms` in place of
  `visual_settle`. The authority's one rule, half of what the deadline leaves and at most
  `MAX_WAIT_MS`, moves into `preparation_budget_ms` in `work/documents.rs`, which document
  preparation and capture both call. Zero sends no settle, as `visual_settle: false` did. The
  adapter's settle call and its 500 ms answer margin are one helper, `observe::settle_top`, shared
  by describing documents and capturing. The adapter no longer caps a budget the authority sent;
  the scope test that pinned the cap now pins that the budget is the authority's.
- A4, the tree: the typed contract is `TreeNode` and `TreeNodeKind` beside `BrowserOutcome`. The
  kind's words are spelled once, in `TreeNodeKind::as_str`, and the page's word is decoded once, in
  `world.rs`, where a word outside the set is a container. `TreeValue.nodes`, which nothing read,
  goes, and `TreeNode::nodes` replaces the merge's `count_nodes` and work's count. The adapter's
  `TREE_NODE_LIMIT` is the one node bound. Work no longer bounds labels again: the page bounds them
  to 120 characters in the isolated world, so the label bound went with `bounded_tree`.
- A4, cross-lane edits: core's `work/mod.rs`, `work/forms.rs`, `work/documents.rs`,
  `work/reading.rs`, `language/outcome.rs`, `language/self_check/hints.rs` and the tests
  `work/tests/configured_authority.rs`, `controls.rs`, `documents.rs` and `execution.rs`.
- A4, proofs: `a_selector_matching_thirty_controls_says_at_least_eight` in
  `work/tests/controls.rs`, with the page-asset test "a semantic query keeps the service's maximum
  and says when more matched" for the page's half;
  `a_screenshot_under_a_short_deadline_keeps_half_for_the_capture` in `work/tests/documents.rs`;
  `a_target_screenshot_under_one_second_still_measures` in `protocol4/translate/tests/interaction.rs`;
  `a_tree_over_the_node_budget_is_cut_once_and_says_so` in `protocol4/translate/tests.rs`. The
  page-asset tests pass with `assets.sha256` regenerated. No journey is needed.

## The second sweep (2026-10-20)

Rules: [BOOTSTRAP.md](BOOTSTRAP.md), "The second sweep (2026-10-20)". Machine: test-02 (Bluefin). A
slice's specification is its finding in [sweep-2026-10-20/browser.md](sweep-2026-10-20/browser.md), read
with its skeptic's corrections. BROW-10, BROW-4, BROW-7, BROW-6, BROW-14, BROW-8a, BROW-11a, BROW-15, D52, D52(c) and BROW-12a have landed; BROW-3a is next.

| Slice | Subject | After | Source | Status |
| --- | --- | --- | --- | --- |
| BROW-10 | refactor(assets): one table of the page's error codes, tested against what `content.js` throws, and `visualIdentity` derived from its beats | - | BROW-10 as corrected; `assets.sha256` regenerates | landed (this commit) |
| BROW-4 | refactor(adapter): one activate variant each, with a modifier list | - | BROW-4 | landed (this commit) |
| BROW-7 | refactor(adapter): one call request in place of seven arguments | - | BROW-7 | landed (this commit) |
| BROW-6 | refactor(adapter): the test-only reverse conversions go | - | BROW-6 as corrected; no macro | landed (this commit) |
| BROW-14 | refactor(adapter): world kinds and primitive names are one enum each | - | BROW-14 | landed (this commit) |
| BROW-8a | refactor(adapter): the adapter's literals are named | - | BROW-8's adapter half | landed (this commit) |
| BROW-12a | refactor(adapter): the adapter's copies of the hash and failure-name helpers go | SERV-7 | BROW-12's adapter half, with the crate `lock` helper | landed (this commit) |
| BROW-15 | refactor(adapter): the command variants wrap their argument structs | BROW-4 | BROW-15 | landed (this commit) |
| BROW-3a | refactor(adapter): the closed vocabularies reach the adapter as types | LANG-3 | BROW-3's adapter half | todo |
| BROW-11a | docs(adapter): the adapter's docs and constant names match the code | - | BROW-11's adapter half | landed (this commit) |
| D52 | fix(adapter): Decision 5.2's receipt codes reach the refusal as a typed cause | SERV-7 | the coordinator's placement: `BrowserError::Primitive { detail, cause }` with `language::outcome::PrimitiveCause`; core-work's routing_refusal half is D52(c) in its ledger | landed (696fa557) |
| D52(c) | fix(work): a refusal names its typed cause, and a capture over its budget says so | D52 | test-03 handed core-work's D52(c) to this lane on 2026-10-20; cross-lane in `work/mod.rs` | landed (this commit) |

### Notes (second sweep)

- BROW-10, the table: `world.js`'s `ERROR_CODES` is a frozen table in the shape of `content.js`'s
  `FIELD_PROBLEMS`, a camelCase key per wire word, and every `AssetError` in the bundle names its
  code by key: 35 in `content.js` (the `option_missing` or `not_fillable` choice is a condition
  between two keys), one in `sensor.js` and one in `world.js`. `failure` checks a code against the
  table's values, so the separate `CODES` set goes. The wire words and their order are unchanged,
  and `golden.rs` reads the table with the closing marker it already used for the other two, now
  one `TABLE_CLOSE`.
- BROW-10, the guard: "every page failure names an ERROR_CODES key" in `assets/tests/world.test.js`
  scans the expanded bundle, so a literal code, a mistyped key or a computed code fails it. Its
  negative control, "the unlisted-code scan catches a literal, a mistyped key and a computed code",
  runs the scan over a sample that holds each. Every thrown code was already listed, so this is a
  guard, not a fix.
- BROW-10, the beats: `visualIdentity` reads its six beats (ripple, field splash, read scan,
  navigation, screenshot, zoom) from `EFFECT_BEATS`. `cursor_ms` (150) and `border_breathe_ms`
  (4000) stay literal: they are the stylesheet's cursor transition and breathe, not rows of
  `TRANSIENT_EFFECTS`. The signature's 900, 180 and 460 ms and the caption's 1500 ms are named
  constants; C18 rewrites `showCaption` and carries `CAPTION_MS`. Every value is unchanged, and
  only `tests/golden/protocol4/assets.sha256` regenerates.
- BROW-10, owed to docs: `docs/design/visual-language.md:87` credits `visualIdentity` with the
  workwheel (2400 ms), waiting-light (1050 ms) and keyboard (1150 ms) beats. It holds none of them;
  they are looping CSS animations. The file is the docs lane's.
- BROW-4: `Activate` and `ActivatePoint` carry `modifiers`, empty for none, and
  `ActivateModified` and `ActivatePointModified` go. The executor builds one variant whatever the
  modifiers, and the translator passes the list through. The input frames are unchanged:
  `keys::modifier_mask` of an empty list is 0, the mask the plain variants sent. The mask 0 of a
  click without modifiers is pinned by `a_click_in_the_top_document_needs_no_owner_box` and
  `an_open_scope_sends_a_point_click_without_routing_it`, and a held mask by
  `a_modified_click_holds_its_modifiers` and `a_modified_point_click_holds_its_modifiers`, all in
  `protocol4/translate/tests/`. Cross-lane edits, one arm or one construction each: core-work's
  `work/pointer.rs` and `work/documents.rs`, and core-service's `browser/mod.rs`
  (`addressed_tab`). Neither lane had landed a slice in those files.
- BROW-7, the request: `link::CallRequest` holds the tab, the plans, the mode, the guard, the
  settle and an optional budget, built with `sequence`, `parallel` or `new` (for a mode chosen at
  run time, as `observe_text` chooses race or parallel) and set with `guard`, `settle` and
  `budget`. `Link::call(request, scope)` takes it, and the lint expectation goes. A request
  without a budget of its own is bounded by what the deadline leaves, which is what the 12 sites
  that passed `l.budget_ms()` sent, since `call` already took the smaller of the two. The four
  budgets of their own (the fill's, the settle, and two waits) stay through `budget`. The same
  `wire::Call` goes out.
- BROW-7, `forms::Batch` goes: it restated the request. `forms::send` keeps the fill's rule that
  no calls sends nothing, and its budget, an `Instant`, becomes the request's.
- BROW-7, not taken: `read::one` and `read::all` stay, rewritten over the request. They are not
  only the seven-argument gap: `one` carries the first primitive's guard, and `all` the rule that
  no calls is no readable document and the choice of a guard. Removing them would spread those
  rules over their 15 call sites in `read.rs`, `keyboard.rs` and `points.rs`.
- BROW-6: the ten reverse conversions go, 147 lines: wire to physical for the runtime control
  state, the diagnostic source and detail, the click shape and the presentation kind and
  activity, which only the service sends, and physical to wire for the readiness, the diagnostic
  entry, the encoded recording and the delivery, which only the extension sends. The library
  compiled with all ten removed before any test changed. The two mirror tests now check what each
  direction promises against the physical type's own serde names:
  `the_outbound_mirrors_write_the_physical_names` (a converted value encodes as the physical
  value does) and `the_inbound_mirrors_read_what_the_physical_names_write` (the physical encoding
  reads through the mirror back to the same value). `the_runtime_control_state_round_trips_the_wire`
  becomes `the_runtime_control_state_reaches_the_wire_by_its_name`. The module doc now says each
  mirror converts in the one direction it crosses; completing its list of mirrors is BROW-11a's.
  No macro, as the skeptic corrected, and the protocol 4 goldens pass unchanged.
- BROW-14: `world::WorldKind` and `wire::PrimitiveName` replace the `kind` and `primitive_name`
  string modules, each with `ALL` and a `const fn as_str`, in the shape of `TreeNodeKind`.
  `Message` carries its `WorldKind`, and `Spec::NAME` is a `PrimitiveName`, so a world message or a
  primitive request cannot name a word outside its set. `golden.rs` loses its two hand lists
  (`WORLD_KINDS`, `PRIMITIVES`) and reads `ALL`, and its two string matches (the world builder and
  the receipt outcomes) decode the fixture's word into the enum first and match it exhaustively,
  so a new kind or primitive without a builder or an outcome fails to compile. The words are
  unchanged: the request, receipt and world fixtures and the vocabulary golden pass as they were.
  The slice was parked uncommitted while C17 was in flight (the coordinator's direction), saved
  as a patch outside the tree, and re-applied cleanly on top of it.
- BROW-8a: the adapter's half of BROW-8 is the world-call limit. `MAX_WORLD_CALLS` stays the
  frozen 256 the wire, the vocabulary golden and the 2.0 extension pin, and a compile-time
  assertion beside it keeps `adapter::documents::DOCUMENT_LIMIT` from passing it, as the skeptic
  corrected (a check, not a tie). Raising the document limit to 257 fails the build at the
  assertion; it was restored. The adapter's timings are named constants already (`link.rs`), and
  BROW-8's other literals and the three poll loops sit in `browser/` outside the adapter, in
  core-service's BROW-8, whose row also names this check: it landed here because both constants
  are adapter files.
- BROW-11a: `world.rs`'s `FILL_STABLE_MS` and `FILL_POLL_MS` (250 and 25) become
  `FILL_RETAINED_STABLE_MS` and `FILL_RETAINED_POLL_MS`, the names `content.js` gives the same two
  values; `content.js`'s own `FILL_STABLE_MS` is the 750 ms settle, a different wait. The world
  message keys `stable_ms` and `poll_ms` stay. The `wire.rs` module doc now lists every mirror,
  `DiagnosticSource`, `DiagnosticDetail` and `Readiness` included. BROW-11's other two items are
  not adapter files: the `docs/1.0/ARCHITECTURE.md` line that still names the removed closed
  `Family` is the docs lane's, and the one-line doc of `browser/mod.rs` is core-service's, after
  BROW-1 settles what that file holds.
- BROW-15: 15 field-heavy `BrowserCommand` variants are newtypes over owned argument structs in
  the port vocabulary: `Activate(PhysicalClick)`, `ActivatePoint(PhysicalPointClick)`,
  `WheelAt(PhysicalPointWheel)`, `HoverPoint(PhysicalViewPoint)`, `DropImageAt(PhysicalPointDrop)`,
  `DragPoints(PhysicalPointDrag)`, `Drag(PhysicalDrag)`, `Scroll(PhysicalScroll)`,
  `TypeText(PhysicalTypeText)`, `TypeFocused(PhysicalTypeFocused)`, `PressKey(PhysicalPressKey)`,
  `Fill(PhysicalFill)`, `Observe(PhysicalWait)`, `QuerySemantic(PhysicalSemantic)` and
  `ReadDiagnostics(PhysicalDiagnosticsRead)`. The three point variants that shared a tab, a point
  and a viewport carry it as `at: PhysicalViewPoint`, as the skeptic corrected, and `HoverPoint`
  is that view point alone. The executors build the structs, each translator borrows its own, and
  every `translate` arm of them is one line; the translator's 15 borrowed copies go, and their
  struct and field docs move onto the port structs. The `Physical` prefix is the layer's own
  (`PhysicalPoint`, `PhysicalField`); the translator's names would have met the language layer's
  `Click`, `Drag`, `TypeText`, `PressKey` and `Wait` in the executors that import both. Field
  names and types are unchanged, so test-01's LANG-3 retypes each field in one place.
  `observe::observe` and `read::query_semantic` read the tab from their struct. Not taken:
  `DescribeDocuments`, whose `Subjects` held three of its five fields, and
  `RecordingDestination::Target`, a serde-tagged type the language layer decodes. 27 files, 226
  lines fewer. Cross-lane, one construction or pattern each, the `work/` executors and tests
  (core-work, which test-03 cleared) and `browser/mod.rs`'s `addressed_tab` and fake (core-service),
  landed after SERV-7. No wire byte changes.
- D52, (a) and (b) of the coordinator's placement, landed as 696fa557 (this row came in the next
  commit): `BrowserError::Primitive` is `{ detail, cause: language::outcome::PrimitiveCause }`,
  the vocabulary C12b-1 defined, so it has one home, as C17 did with `PageCondition`;
  `BrowserError::primitive(detail)` builds the common `Refused` case beside `BrowserError::stale`.
  `link.rs` stops folding the receipt's closed codes into one string: `debugger_unavailable` and
  `session_lost` keep their cause, `capture_too_large` is the new
  `BrowserError::CaptureTooLarge { detail }`, and `browser_refused` and `other` stay `Refused`.
  The browser's words stay in `detail`. `work/mod.rs` got only the arms that keep today's refusal,
  facts and audit tag (`routing_refusal`, `unchecked`, `browser_reason`), so no result golden
  moved. Proof: the rows for all five codes in
  `receipt_failures_map_to_plain_or_local_failures_before_any_effect` (`protocol4/link/tests.rs`),
  whose `debugger_unavailable` row read the folded string before. Cross-lane: `browser/mod.rs`
  (handed over by the coordinator after SERV-7), three `work/mod.rs` arms and four
  `work/tests/execution.rs` constructions.
- D52(c): test-03 asked this lane to write the routing too, so it is a slice here; core-work's
  ledger marks its row as taken by D52.
- D52(c): `routing_refusal` passes the receipt's cause through, so a refusal the browser names
  `debugger_unavailable` or `session_lost` reads that cause's own sentence and model code from
  C12b-1's `PrimitiveCause`, with the browser's words only in `facts.detail`, and a capture the
  browser could not fit in its budget reads `Refusal::CaptureTooLarge` (`screenshot_too_large`),
  the refusal the service's own bound already gives. The facts' `reason` is the refusal's code,
  read from the cause or `Refusal::reason`, not a literal. The audit keeps
  `browser_primitive_failed` (`browser_reason`), as Decision 5.2 says. A deliberate
  model-visible change, for these three codes only; no result golden pinned them. Proofs, the
  ADR's names, in `work/tests/execution.rs`:
  `a_debugger_refusal_reads_its_own_sentence_and_keeps_the_browser_text_in_detail` (both causes)
  and `a_capture_over_the_budget_says_capture_too_large_with_its_step`.
- BROW-12a: `protocol4/fake.rs` drops its copy of the poison-tolerant lock for SERV-7's
  `crate::sync::lock`, which `link.rs` and its tests now import directly instead of through
  `crate::browser`. The four SHA-256 hex loops (the asset id in `assets.rs`, the transfer hash in
  `link.rs`, the diagnostics binding in `translate/records.rs` and the group key in
  `translate/tabs.rs`) call the crate's one `install::icons::sha256_hex`, which `agent/image.rs`
  already uses; the output is byte-identical, as the pinned asset id, the group-key and binding
  tests show. `failure_code` in `link.rs` and `wire_name` in `translate/mod.rs` were one function
  and are `wire::name_of`, beside the vocabulary whose words it reads. The helper's home in
  `install/icons.rs` is a reuse, not a move: the other lanes' hex copies (governance,
  `workbench/parts.rs`, `workspace/mod.rs`) are theirs, and a crate-level digest module would be
  core-service's `lib.rs`.
