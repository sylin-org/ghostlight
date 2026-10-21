# Second sweep, 2026-10-20: browser

This sweep read `crates/orchestrator/src/browser/` (about 23,400 lines with its tests) and the
protocol 4 page assets in `crates/orchestrator/src/browser/adapter/chromium/protocol4/assets/`. It
was read-only, on `dev` at `8f6a43ec`; nothing was edited, built, run or committed. Adapter
protocol 4 is a published wire contract with the store extension 2.0.0 and 2.0.1, and
`tests/golden/protocol4/` pins it, so no finding changes a wire name or shape. Only the Rust
representation, the names inside the service and the page-asset source may change, and a change to
the page-asset source regenerates the pinned asset id. Where a finding meets a slice of ADR-0203
that is still open, the section says so under "Related". Findings are ranked by payoff over risk.
Paths are relative to `crates/orchestrator/src/browser/`, and `p4/` stands for
`adapter/chromium/protocol4/` below it. `work/`, `language/`, `install/`, `governance/`,
`workbench/` and `workspace/` are its siblings under `crates/orchestrator/src/`. Paths that begin
with `crates/`, `docs/` or `tests/` are from the repository root. Line numbers are as of
`8f6a43ec`.

## BROW-1: The browser port and protocol 4 use each other's private items

- Kind: structure.
- Locations:
  - `p4/connection.rs:25-29` (the import) and `p4/connection.rs:125-197` (the field and method
    use).
  - `mod.rs:656` (`Liveness::probe`), `mod.rs:1062` (`refuse_incompatible`) and `mod.rs:1163`
    (`publish_control_state`), which build protocol 4 frames.
  - `mod.rs:886` (`attach_reader` calls `protocol4::connection::attach`) and `mod.rs:1030`
    (`call_inner` calls `protocol4::run`).
  - `mod.rs:1274-1297` (`validated_browser_name`, `valid_opaque_id`) and `mod.rs:39-41`
    (`BROWSER_ID_PREFIX`, `LIFE_PREFIX`).
  - `mod.rs:422` (`Reachability`), `mod.rs:79` (`Availability`) and `mod.rs:1043-1047` (the
    mapping between them in `await_registered`).
  - `mod.rs:1471-1786` (`testing::FakeBrowser`).
  - `p4/link.rs:28`.
- Problem: `connection.rs` imports 14 private items from `crate::browser` (six functions, five
  types, three constants) and drives `RelayBrowserPort`'s private fields and methods: `lifecycle`,
  `control_state`, `service_epoch`, `assets`, `adapters`, `notifications`, `register`,
  `detach_registered` and `spawn_heartbeat`. In the other direction, `mod.rs` builds protocol 4
  frames itself (`wire::Outbound::Heartbeat`, `Error` and `ControlState`), and it holds the hello
  validators and their prefixes, which only `connection.rs` calls. `link.rs:28` also imports the
  private `lock` and `CONTROL_DELIVERY_TIMEOUT`. The generic port module and the protocol module are
  one component split across the tree in both directions.
- Proposed change: move `RelayBrowserPort`, `AdapterRegistry`, `Connection`, `Liveness`,
  `ConnectionLiveness`, `HeartbeatSettings`, the heartbeat thread, `ConnectionTag`,
  `AdapterNotifications`, `end_connection`, `retire`, `is_current`, the hello validators and
  `refuse_incompatible` into a new `p4/port.rs` beside `connection.rs`, and re-export the port from
  `browser`. `mod.rs` keeps the contract: the `BrowserPort` trait, `BrowserDispatch`,
  `BrowserSummary`, `Availability`, `choose_browser`, `addressed_tab`, `BrowserEventSink`,
  `AdapterLifecycleObserver`, `RETURN_GRACE`, `RELAY_RETURN_WINDOW` and `BrowserError`. It drops
  from 1,786 lines to about 650 (an estimate). `testing::FakeBrowser` moves to its own file, and
  the private `Reachability` folds into the public `Availability` it is mapped to today.
- Ratings: payoff high; risk low to medium (a move, but `contract_tests/relay.rs` and
  `p4/connection/tests.rs` reach private items); effort M. Wire: no. Golden: none.
- Related: ADR-0203 slices C3b and C3c (todo in `docs/tasks/arch-1/LEDGER-core.md`) change the
  same registry: departures kept per browser identity, and pending launches moved into the port's
  registry beside `arrivals_of`. Land them first, or carry them in the move.
- Verdict: partial. The coupling is as counted: 14 private items at `connection.rs:25-29`
  (`BrowserError` and `RelayBrowserPort` are public), the nine fields and methods, the three frames
  `mod.rs` builds, and tests that reach in too (`contract_tests/relay.rs:7`,
  `p4/connection/tests.rs:59`, `:500`). The proposed home is wrong. `AdapterRegistry`
  (`mod.rs:465-490`) is the one registry of every browser connection, keyed by browser id
  (ADR-0114 D1), and its attention, departures, installed browsers and arrivals belong to no wire
  level. AGENTS.md has `chromium/` hold the wire levels and treats families as plural, and ADR-0196
  Decision 3 picks the family and level from the hello. In `p4/port.rs` the next level or family
  would need a second port or the move undone. What fits: the registry, liveness and heartbeat
  stay in `browser/`, and p4 takes the frames, the hello validators and their prefixes behind a
  narrow registration call. Not in Related: C16 (todo) also changes `BrowserPort::call`
  (`mod.rs:173-180`) and `Line.recordings` (`connection.rs:163`).
- Class: refactor.
- arch-1: none. C3b, C3c and C16 (all todo) edit the same registry, trait and `Line`.

## BROW-2: Recovery writes "launch" three ways

- Kind: dead code.
- Locations:
  - `recovery.rs:128` (`RecoveryDecision::Launch`).
  - `recovery.rs:455` (`RecoveryPlan::Prepare`) and `recovery.rs:1079-1107` (`phase_from_plan`).
  - `work/mod.rs:1613` and `work/mod.rs:1659` (the `unreachable!` arms).
  - `work/mod.rs:1616-1617` and `work/mod.rs:1662-1663` (the `RecoveryWaitError` mapping).
- Problem: nothing constructs the public `RecoveryDecision::Launch`, yet both of its consumers in
  `work/mod.rs` must match it with `unreachable!("the recovery service consumes its internal
  launch plan")`. Inside recovery, `RecoveryPlan::Prepare` carries two bools
  (`repair_owned_registration`, `launch`), and `phase_from_plan` re-maps their four combinations
  onto `FlightPhase::Repairing`, `FlightPhase::Launching` and `FlightPhase::Complete(Manual)`.
- Proposed change: delete `RecoveryDecision::Launch`. Have `decide`, `decide_browser` and
  `start_plan` return the `FlightPhase` directly (or a three-variant plan), and drop
  `phase_from_plan`; `starts_on_request` then matches the launching phases. Add
  `From<RecoveryWaitError> for BrowserError` for the mapping `work/mod.rs` writes twice.
- Ratings: payoff medium to high; risk low; effort S. Wire: no. Golden: none.
- Related: ADR-0203 slice C3c (todo) moves recovery's `PendingLaunch` map into the port. This
  change is independent of it but touches the same file.
- Verdict: confirmed. Nothing constructs `RecoveryDecision::Launch` (`recovery.rs:128`); its only
  matches are the two `unreachable!` arms. More is dead than stated: a `Prepare` with both bools
  false is never built, because `start_plan` answers `Complete(Manual)` itself under manual
  (`recovery.rs:1048-1052`) and every other `Prepare` without a repair launches (`:939-943`,
  `:1053-1057`), so the third arm of `phase_from_plan` (`:1099-1105`) is unreachable. The
  `decide` tests assert on `RecoveryPlan::Prepare` (`:1560`, `:2001`, `:2009`, `:2022`, `:2163`,
  `:2614`) and change with it. WORK-6 in `work.md` notes the same doubled error mapping.
- Class: refactor.
- arch-1: none. C3c (todo) edits the launch guard in the same file.

## BROW-3: Closed vocabularies cross the in-process port as String

- Kind: magic strings; a vocabulary without one home.
- Locations:
  - `adapter/work.rs`, `BrowserCommand`: `TraverseHistory.direction` (`:689`), `Inspect.kind`
    (`:709`), `ReadDocument.mode` (`:715`), `Find.kind` (`:728`), `button` and `modifiers` on the
    activate variants (`:766-796`), the wheel and scroll directions (`:802`, `:809`) and
    `Observe.condition` (`:899`).
  - `work/reading.rs:37` and `p4/translate/read.rs:24-29` (`mode::ARTICLE`, `mode::VISIBLE`).
  - `work/forms.rs:1144` and `work/documents.rs:396`.
  - `mod.rs:1672` (the test fake).
  - `language/mod.rs:1830-1836`, `language/mod.rs:1866` and `language/mod.rs:2510`.
  - `language/catalog.rs:578`, `language/catalog.rs:855-863`, `language/catalog.rs:1490` and
    `language/catalog.rs:1560`.
  - `p4/translate/observe.rs:29` (`UNKNOWN_CONDITION`) and `p4/translate/observe.rs:32-47` (the
    `condition` module).
  - `p4/translate/tabs.rs:19` (`UNKNOWN_DIRECTION`) and `p4/translate/tabs.rs:22-27` (the
    `direction` module).
  - `p4/keys.rs:24-36` (modifier, button and direction names) and `p4/keys.rs:304-310`
    (`mouse_button`).
- Problem: `BrowserCommand` carries `Observe.condition`, `TraverseHistory.direction`,
  `ReadDocument.mode`, the `Inspect` and `Find` kinds, `button`, `modifiers` and the scroll and
  wheel directions as strings. The language layer already decodes a typed `ReadMode`, yet
  `work/reading.rs:37` turns it back into a string
  (`mode.unwrap_or(ReadMode::Visible).as_str().to_string()`) and `translate/read.rs` re-declares
  the words. The same words are spelled by hand elsewhere: `work/forms.rs:1144` writes
  `"visual_settle".into()`, `work/documents.rs:396` infers an absence test from
  `condition.ends_with("_absent")`, the test fake compares `condition == "visual_settle"`, and the
  button and modifier names repeat in `language/mod.rs`, `language/catalog.rs` and `p4/keys.rs`.
  `keys::mouse_button` maps any unknown name to the primary button, and `UNKNOWN_DIRECTION` and
  `UNKNOWN_CONDITION` are runtime failures that a type would rule out.
- Proposed change: closed enums with `as_str()` in the port vocabulary (a wait condition, a history
  direction, the read mode, the inspect and find kinds, a pointer button, a modifier, a scroll
  direction), decoded once by the language layer. The translator writes `as_str()` into world
  messages, so the bytes on the wire stay the same.
- Ratings: payoff high; risk medium (it touches the language decoder, the executor and the
  translator); effort M. Wire: no change. Golden: the wire and world fixtures stay unchanged.
- Related: the wait-condition part is ADR-0203 Decision 8 and slice C17 (todo in
  `docs/tasks/arch-1/LEDGER-core.md`), which deletes the literals in `work/forms.rs` and
  `work/documents.rs` and the second `condition` module in `translate/observe.rs`. This finding
  adds the other vocabularies. ADR-0203 declined closed enums for `on_error`, `Record.action`,
  `HandleDialog.action` and Diagnose's fields because they fix no defect a person meets. The ones
  here cross into the adapter, but the language layer validates their names first, so no agent
  meets the primary-button fallback today: once C17 lands, the rest pays in one spelling per word
  and no re-parse at the adapter, which is medium payoff.
- Verdict: confirmed. Every cited field, the typed-then-stringed `ReadMode` and each fallback is
  at its line. More copies: eight test commands use `button: "left"` (for example
  `p4/translate/tests.rs:831`, `adapter/work.rs:1328`), a word the language never decodes
  (`language/mod.rs:1866`) that reaches the primary button only through `mouse_button`'s
  fallback; `presentation.js:285` compares the click shape's button to `"secondary"`;
  `keys::modifier_mask` drops unknown names (`p4/keys.rs:157-166`); `WheelDirection::from_name`
  reads any word but up as down (`p4/keys.rs:373-381`). `language/mod.rs:1830-1836` is the
  expect condition list, not buttons. Ratings say payoff high, Related says medium; medium holds.
  LANG-3 and WORK-5 of this sweep cover the language half.
- Class: refactor.
- arch-1: C17 (todo) for the wait conditions. Decoding the other words in `language/`, as
  proposed, reopens "Closed enums for `on_error`, `Record.action`, `HandleDialog.action` and
  Diagnose's fields" for more fields, on the same no-defect ground.

## BROW-4: ActivateModified and ActivatePointModified duplicate Activate and ActivatePoint

- Kind: duplication.
- Locations:
  - `work/pointer.rs:68-108` (the executor's choice).
  - `p4/mod.rs:298-330` and `p4/mod.rs:423-463` (the translator's re-merge).
  - `adapter/work.rs:781-796` (the two variants), `adapter/work.rs:966` (`sole_locator`),
    `mod.rs:131-132` (`addressed_tab`), `work/documents.rs:59` and `work/documents.rs:73`.
- Problem: the executor picks `ActivateModified` or `ActivatePointModified` when the call has
  modifiers, and the plain variant when it has none. The translator then merges them back into the
  same `pointer::activate` and `points::activate`, passing `modifiers: &[]` for the plain variants.
- Proposed change: one variant each, with a modifiers list that may be empty. 13 references in 7
  files change.
- Ratings: payoff medium; risk low; effort S. Wire: unchanged, because `keys::modifier_mask` of an
  empty list is 0, the mask the plain variants send today. Golden: none; the translate tests that
  build the modified variants change.
- Verdict: confirmed. 13 references in 7 files; the translator passes `modifiers: &[]` for the
  plain variants (`p4/mod.rs:311`, `:440`) and an empty mask is 0 (`p4/keys.rs:157-166`), so the
  input frames match. The merge also adds an empty `modifiers` to six test literals of the plain
  variants (`p4/translate/tests.rs:828`, `p4/translate/tests/interaction.rs:156`,
  `p4/translate/tests/points.rs:255`, `:530`, `:557`, `adapter/work.rs:1325`).
- Class: refactor.
- arch-1: none.

## BROW-5: One type with three names

- Kind: naming.
- Locations:
  - `adapter/work.rs:15` (`pub type Work = BrowserCommand`).
  - `mod.rs:16`, `mod.rs:109`, `mod.rs:177`, `mod.rs:191`, `mod.rs:1011`, `mod.rs:1081`,
    `mod.rs:1102` and the `testing` module (the only users).
  - `work/tests/findings.rs:16` (`use crate::browser::adapter::BrowserCommand as Work;`).
  - `p4/wire.rs:17` (`use crate::browser::adapter as physical;`).
- Problem: the alias, documented as "the service's internal browser work vocabulary, by its
  executor name", is used only in `mod.rs` and as a rename in one test, while the executor and
  everything else say `BrowserCommand` (405 uses). `Work` also shadows the `crate::work` module.
  `wire.rs` imports the same layer `as physical`, and its types are named `Physical*`, while the
  module doc calls it the work vocabulary.
- Proposed change: delete the alias and use `BrowserCommand` everywhere. Import the layer in
  `wire.rs` by its module name, so one layer has one name.
- Ratings: payoff medium; risk low; effort S. Wire: no. Golden: none.
- Verdict: partial. The alias is used only in `browser/mod.rs` and its `testing` module, and
  `work/tests/findings.rs:16` is the one rename. But `Work` does not shadow `crate::work`: a type
  `Work` and a module `work` are different names in Rust, and nothing in `browser/` imports
  `crate::work`, so the cost is a near-homonym. Renaming the import in `wire.rs` (135
  `physical::` lines) leaves the layer's `Physical*` types (`adapter/work.rs:34`, `:206`, `:242`,
  `:252`, `:310`, `:428`, `adapter/documents.rs:8`), so the layer still has two names.
- Class: refactor.
- arch-1: none. C16 (todo) edits the `BrowserPort::call` signatures that name `Work`
  (`mod.rs:173-203`).

## BROW-6: The wire mirrors are written by hand, in both directions

- Kind: duplication.
- Locations:
  - `p4/wire.rs:1437-2047` (the mirrors) and `p4/wire.rs:2685-2876` (the round-trip tests).
  - The production conversions: `p4/translate/records.rs:66-67`, `:79`, `:127` and `:280-300`;
    `p4/translate/present.rs:31-37`; `p4/connection.rs:131`, `:198` and `:258`;
    `p4/translate/observe.rs:253` and `:330`.
- Problem: the section holds 12 mirror types (`RuntimeControlState`, `DiagnosticSource`,
  `DiagnosticDetail`, `DiagnosticEntry`, `RecordingState`, `RecordingStopReason`,
  `RecordingSummary`, `Encoded`, `Delivery`, `ClickShape`, `PresentationKind`,
  `PresentationActivity`) and 23 hand-written `From`/`TryFrom` impls (324 lines), two of them for
  `Readiness`, which is defined at `:995`. A 24th impl, `From<Tab>` at `:979`, sits outside the
  section. Production converts each type in one direction only; ten reverse impls exist only for
  the round-trip tests. The outbound-only mirrors (the presentation kinds and activities,
  `ClickShape`, the diagnostic source and detail, the control state) need no tolerance arm.
- Proposed change: a small `mirror!` macro that declares the wire enum, adds
  `#[serde(other)] Unknown` only on inbound types, and generates the one conversion production
  uses. Delete the reverse impls and keep one test per type that the serialized names match. About
  600 lines become under 200.
- Ratings: payoff medium to high; risk low (the goldens pin every name); effort M. Wire: the Rust
  representation only. Golden: `tests/golden/protocol4/` must pass unchanged.
- Verdict: partial. The counts hold: 12 mirrors and 23 impls in 324 lines, two for `Readiness`,
  `From<Tab>` outside, and ten reverse impls (137 lines) that only tests use, among them
  `the_runtime_control_state_round_trips_the_wire` (`p4/wire.rs:2649-2666`), outside the cited
  range. A `mirror!` macro fits only the seven unit enums. `Readiness` is no name mirror: the
  physical `Interactive` has no wire word (`:1487`) and an inbound `Unknown` is a value, not an
  error (`:1499`). `DiagnosticEntry` and `Delivery` are tagged enums with data,
  `RecordingSummary` drops `recording_id` (`:1757`, `:1788-1799`), and `Encoded` and
  `ClickShape` are structs. "Under 200" is unlikely; the sure saving is the reverse impls.
- Class: refactor, frozen.
- arch-1: none. C18 (todo) pins against the JavaScript the presentation and stop-reason names
  these mirrors carry.

## BROW-7: Link::call takes seven positional arguments

- Kind: structure.
- Locations:
  - `p4/link.rs:524-537` (the `#[expect(clippy::too_many_arguments)]` and the signature) and
    `p4/link.rs:545` (the budget clamp).
  - The call sites that compute `let budget = l.budget_ms()` first: `p4/translate/capture.rs:69`,
    `p4/translate/keyboard.rs:129`, `p4/translate/mod.rs:298` and `:309`,
    `p4/translate/observe.rs:173` and `:326`, `p4/translate/pointer.rs:83`, `:229` and `:297`,
    `p4/translate/points.rs:281`, `p4/translate/read.rs:418` and `:443`.
  - The wrappers: `p4/translate/read.rs:411` (`one`), `p4/translate/read.rs:432` (`all`) and
    `p4/translate/forms.rs:358-386` (`Batch` and `send`).
- Problem: `Link::call(tab_id, plans, mode, guard, settle_ms, budget_ms, scope)` takes seven
  positional arguments, with a lint expectation to allow it. 12 of its 16 call sites compute
  `l.budget_ms()` and pass it, though `call` already clamps the budget to `self.budget_ms()`, and
  almost all of them pass a bare settle of `0`. `read::one`, `read::all` and `forms::Batch` are
  three separate wrappers around the same gap.
- Proposed change: one call-request value (tab, plans, mode, guard, settle, budget) with
  `sequence`, `parallel` and `race` constructors and a budget that defaults to what the deadline
  leaves. `forms::Batch`, `read::one`, `read::all` and the lint expectation go.
- Ratings: payoff medium; risk low; effort S to M. Wire: the same `wire::Call` goes out. Golden:
  none.
- Verdict: confirmed. Seven positional arguments under the lint expectation, the clamp at
  `p4/link.rs:545`, 16 production call sites, 12 of them passing `l.budget_ms()` at exactly the
  cited lines, and a settle of 0 at 15 (only `forms::send` passes its own,
  `p4/translate/forms.rs:383`). The four calls with a budget of their own (`forms.rs:378`,
  `observe.rs:77`, `:239`, `:286`) keep it through the optional budget, and `CallMode::Race`
  exists (`p4/wire.rs:518`).
- Class: refactor.
- arch-1: none. C16 (todo) edits `link.rs`, but not `call`.

## BROW-8: Unnamed literals and a poll loop written three times

- Kind: magic literals.
- Locations:
  - `recovery.rs:738`, `recovery.rs:763` and `recovery.rs:836`.
  - `transport.rs:80-81` and `transport.rs:202`.
  - `mod.rs:1293`.
  - `mod.rs:36` and `mod.rs:449`; `mod.rs:1277-1288` (`validated_browser_name`).
  - `mod.rs:37` and `mod.rs:42`.
  - `p4/wire.rs:34-36` and `adapter/documents.rs:4`.
  - `mod.rs:246-269`, `mod.rs:301-317` and `mod.rs:1034-1057`.
- Problem:
  - `Duration::from_millis(10)`, the recovery flight's poll interval, appears three times.
  - `transport.rs` writes 250 ms twice in one expression
    (`.unwrap_or(Duration::from_millis(250)).min(Duration::from_millis(250))`), and it spins on an
    unnamed 2 ms sleep while it waits for the writer.
  - The opaque-id length bound `80` is bare.
  - `"Chromium"` is defined twice in one file (`UNBRANDED_BROWSER_NAME`, `UNNAMED_BROWSER`).
    `validated_browser_name` never returns `None`, so for a real connection `BrowserSummary.name`
    is always set and `display_name`'s fallback serves only the test fakes.
  - `MAX_BROWSER_CONNECTIONS` and `CONTROL_DELIVERY_TIMEOUT` have no doc comment.
  - `MAX_WORLD_CALLS` is documented as equal to the document limit so a route never splits, but
    nothing ties it to `DOCUMENT_LIMIT`; both are a literal 256.
  - The same poll-until loop is written three times: the default `await_available`, `await_any`
    and `await_registered`.
- Proposed change: named constants (a flight poll, a stop poll, a writer retry, an opaque-id bound,
  one Chromium name), `BrowserSummary.name` as a `String`, `MAX_WORLD_CALLS = DOCUMENT_LIMIT`, and
  one `poll_until` helper.
- Ratings: payoff medium; risk low; effort S. Wire: no. Golden: none; every value stays the same.
- Verdict: partial. The literals and the three loops are where cited. Two claims fail. "Golden:
  none" is wrong for `BrowserSummary.name` as a `String`: the fake's `"name": null` is pinned at
  `tests/golden/model/success.json:40` and `work/tests/availability.rs:1002`, so that part
  regenerates a result golden, though production output is unchanged because
  `validated_browser_name` always names the browser. And `MAX_WORLD_CALLS = DOCUMENT_LIMIT` makes
  a frozen limit, pinned at 256 by `p4/wire.rs:2072`, the vocabulary golden (`p4/golden.rs:595`)
  and the 2.0 extension (`extension/lib/primitives.js:138`), follow an internal constant. A route
  needs only `MAX_WORLD_CALLS >= DOCUMENT_LIMIT`, which a `const` assertion states without that tie.
- Class: refactor.
- arch-1: none. C3b (todo) changes `await_returning`, beside the loops.

## BROW-9: The return windows are derived by hand from relay constants

- Kind: magic literals (derived timing) and a stale comment.
- Locations:
  - `mod.rs:44-48` (`RETURN_GRACE`) and `mod.rs:50-56` (`RELAY_RETURN_WINDOW`, with the "every
    500 ms" sentence at `mod.rs:53`).
  - `transport.rs:17` (`EXCHANGE_TIMEOUT`).
  - `crates/relay/src/lib.rs:45` (`BACKOFF_FIRST`) and `crates/relay/src/spawn.rs:50-52`
    (`Backoff::next_wait`, which doubles).
  - `crates/edge/src/port.rs:324-331` (the browser relay's wait for a returning service).
  - `docs/1.0/ARCHITECTURE.md:482` and `docs/1.0/ARCHITECTURE.md:488-489`, which restate both
    values.
- Problem: `RETURN_GRACE` (7 s: one relay reconnect of 500 ms, the 5 s hello exchange and a
  margin) and `RELAY_RETURN_WINDOW` (1.5 s: two relay retries and the hello) are computed by hand
  from `ghostlight_relay::BACKOFF_FIRST` and `EXCHANGE_TIMEOUT`. The comment at `mod.rs:53` says a
  waiting relay "retries every 500 ms", but the browser relay waits on the doubling `Backoff`
  schedule (500 ms, 1 s, 2 s, ...).
- Proposed change: express both constants from `BACKOFF_FIRST` and `EXCHANGE_TIMEOUT`, add a
  compile-time assertion that pins today's values, and correct the comment.
- Ratings: payoff medium; risk low; effort S. Wire: no. Golden: none.
- Verdict: partial. The constants, the stale "every 500 ms" and the doubling wait
  (`crates/edge/src/port.rs:324-331`, `crates/relay/src/spawn.rs:50-54`) are as stated. Only
  `RETURN_GRACE` derives from `BACKOFF_FIRST` and `EXCHANGE_TIMEOUT`. `RELAY_RETURN_WINDOW` (1.5 s)
  is shorter than `EXCHANGE_TIMEOUT` (5 s), and on the doubling schedule two retries take all of
  it (0.5 s, then 1 s), leaving nothing for the hello. The retries also count from the relay's own
  loss of the old service, while the window counts from `endpoint_published` (`mod.rs:784-786`).
  So the corrected comment shows a window that may be shorter than a relay's return after an
  update, against the promise at `work/mod.rs:1636`; a test should settle that before any
  constant is derived. Reading the relay constants changes no `crates/relay` source.
- Class: refactor.
- arch-1: none. C3b (todo) rewrites `returning` (`mod.rs:513-523`), which reads both constants.

## BROW-10: Page assets spell error codes inline, and keep a timing table nothing reads

- Kind: magic strings in the page assets.
- Locations:
  - `p4/assets/content.js`: 34 `AssetError("...")` literals over 20 codes, for example
    `"invalid_request"` five times (`:2378`, `:2407`, `:2463`, `:2613`, `:2763`), `"focus_lost"`
    four times (`:1837`, `:1847`, `:1860`, `:2747`) and `"not_editable"` three times (`:1819`,
    `:1822`, `:2627`).
  - `p4/assets/world.js:26` (`CODES`) and `p4/assets/world.js:52` (the `page_exception`
    fallback).
  - The only checks: `p4/assets/tests/world.test.js:60` and the
    `asset_error_codes_agree_across_rust_the_vocabulary_and_world_js` test in `p4/golden.rs`.
  - `p4/assets/presentation.js:29-31` (the file's rule), `:38-50` (the `TRANSIENT_EFFECTS`
    beats), `:84-98` (`visualIdentity`), `:481` (900), `:497-498` (460 and 180) and `:506`
    (1500).
  - `crates/orchestrator/ui/styles.css:71` and `docs/design/visual-language.md:87`, which cite
    `visualIdentity`.
- Problem: `content.js` throws its asset error codes as string literals. `world.js`'s `CODES` set
  only filters them: an unknown code becomes `page_exception`, so a mistyped code degrades
  silently. The tests check `world.js`'s set against Rust and the vocabulary, but nothing checks
  that `content.js` throws only members of it. In `presentation.js`, `visualIdentity` restates six
  beats of `TRANSIENT_EFFECTS` (ripple 620, field splash 700, read scan 1450, navigation 1600,
  screenshot 1500, zoom 1150), against the file's own rule that nothing about an effect is
  hand-maintained anywhere else. No code reads it; the window's stylesheet and the visual-language
  design cite it as the frozen source of the beats. The signature and caption timers are bare
  numbers.
- Proposed change: export a frozen code table from `world.js`, use it in `content.js`, and add a
  test that every thrown code is in `CODES`. Keep `visualIdentity`, but derive its beats from
  `EFFECT_BEATS`, and name the signature and caption timers. Every value stays the same.
- Ratings: payoff medium; risk low; effort S. Wire: no. Golden: only
  `tests/golden/protocol4/assets.sha256` regenerates. No extension resubmission: the assets ship
  from the service.
- Related: ADR-0203 slice C18 (todo) edits `presentation.js`, `page-shared.js` and `p4/golden.rs`,
  so one asset id regeneration can cover both.
- Verdict: confirmed. 34 literal `AssetError("...")` calls over 20 codes in `content.js` at the
  cited lines, `CODES` (`world.js:26`) with its fallback (`:52`), only the two checks named, and
  `visualIdentity` (`presentation.js:84-98`) repeats six beats and is exported (`:637`) but read
  by nothing. Every thrown code is in `CODES` today, so this is a guard, not a fix. The new test
  must also read `content.js:2183` (`option_missing` or `not_fillable` by a ternary),
  `sensor.js:18` and `world.js:59`. And `docs/design/visual-language.md:87` credits
  `visualIdentity` with workwheel (2400 ms), waiting-light (1050 ms) and keyboard (1150 ms) beats
  it does not hold.
- Class: refactor.
- arch-1: none. C18 (todo) rewrites `showCaption` (`presentation.js:501-507`), where the 1500 ms
  timer sits.

## BROW-11: Stale contract text and a misleading constant name

- Kind: docs.
- Locations:
  - `docs/1.0/ARCHITECTURE.md:345-347` and `adapter/mod.rs:7`.
  - `mod.rs:1`.
  - `p4/wire.rs:9-12`.
  - `p4/world.rs:71` and `p4/world.rs:73`; `p4/assets/content.js:205` and
    `p4/assets/content.js:207-208`.
- Problem: ARCHITECTURE.md still says `browser/adapter/mod.rs` names a closed `Family`
  (Chromium). Commit `c859e5ee` removed the enum on 2026-10-16, and `adapter/mod.rs:7` now says no
  family enum names the families. The doc of `mod.rs` is one line ("The physical browser port and
  authenticated relay-backed adapter implementation.") for a file that holds the port contract,
  routing, the registry, liveness, the error vocabulary and the test fake. The `wire.rs` module doc
  lists the mirrors incompletely: it leaves out the diagnostic source and detail and the readiness.
  `world.rs:71`'s `FILL_STABLE_MS` (250) is the value `content.js:207` calls
  `FILL_RETAINED_STABLE_MS`, while `content.js:205`'s own `FILL_STABLE_MS` is a different 750 ms
  wait.
- Proposed change: correct ARCHITECTURE.md, which is a governing contract; rewrite the `mod.rs`
  module doc once BROW-1 settles what the file holds; complete the `wire.rs` list; and rename the
  Rust constants `FILL_RETAINED_STABLE_MS` and `FILL_RETAINED_POLL_MS`, as `content.js` names them.
- Ratings: payoff medium; risk low; effort S. Wire: no; the world message keys `stable_ms` and
  `poll_ms` stay. Golden: none.
- Verdict: confirmed. `docs/1.0/ARCHITECTURE.md:345-347` still names the closed `Family` that
  `c859e5ee` (2026-10-16) removed. `mod.rs:1` is one line, `p4/wire.rs:9-12` omits
  `DiagnosticSource`, `DiagnosticDetail` and `Readiness`, and `world.rs:71` and `:73` (250, 25)
  are what `content.js:207-208` call `FILL_RETAINED_*`, while `content.js:205` is the 750 ms
  settle. D2's landed commit left the `Family` line in place.
- Class: refactor.
- arch-1: none. The ARCHITECTURE line is D2's kind of edit, but no slice names it.

## BROW-12: The same small helpers are copied per module

- Kind: duplication.
- Locations:
  - The poison-tolerant `lock`: `mod.rs:1299`, `mod.rs:1781`, `recovery.rs:1109` and
    `p4/fake.rs:279`, with the same unwrap inlined three times in `transport.rs:147-165`. It is
    defined 17 times across the crate.
  - Lowercase SHA-256 hex: `p4/assets.rs:78` (`sha256_hex`), `p4/link.rs:896-899`,
    `p4/translate/tabs.rs:251-257` (`group_key`) and `p4/translate/records.rs:94-100` (`binding`).
    `install/icons.rs:1046` already exports `sha256_hex`. Outside this area,
    `governance/managed/mod.rs` (`encode_hex`), `governance/manifest.rs` (`canonical_hash`),
    `governance/mod.rs` (`hex_prefix`), `workbench/parts.rs` (`hex_digest`) and
    `workspace/mod.rs:82` hex-encode by hand too.
  - Wire names: `p4/translate/mod.rs:241` (`wire_name`) and `p4/link.rs:861` (`failure_code`) are
    the same function.
- Problem: one poison-tolerant lock, one lowercase-hex digest and one closed-code-to-wire-name
  helper are each written several times.
- Proposed change: one crate-level helper each.
- Ratings: payoff low to medium; risk low; effort S. Wire: no; the output is byte-identical (the
  group keys, the transfer hashes, the diagnostics binding and the asset id). Golden: none.
- Verdict: confirmed. 17 free `fn lock<T>` copies in the crate, all poison-tolerant
  (`workbench/mod.rs:420` and `workspace/mod.rs:1901` are methods, not copies), the inline
  unwraps at `transport.rs:150`, `:160`, `:164`, four hex loops whose output matches the public
  `install::icons::sha256_hex` (already used by `agent/image.rs`), and `failure_code` is
  `wire_name` for one type. The copies outside `browser/` sit in other lanes' files, so the
  cross-lane rule applies.
- Class: refactor.
- arch-1: none.

## BROW-13: The port vocabulary lives inside the adapter namespace

- Kind: naming (namespaces).
- Locations:
  - `adapter/mod.rs:16` (`pub use work::*;`), with `adapter/work.rs` and `adapter/documents.rs`.
  - 153 `browser::adapter::` import lines that name the port vocabulary, 43 of the files outside
    `browser/`.
  - `session.rs`, used by `recovery.rs:374` and `workbench/setup.rs:28`.
  - `p4/assets.rs:95` (the `include_str!` path) and `p4/golden.rs:26` and `p4/golden.rs:124-126`
    (the fixture path).
  - `docs/adr/0196-simple-pipeline-and-contained-boundaries.md:54-58` (Decision 3) and
    `docs/1.0/ARCHITECTURE.md:345-347`.
- Problem: the depth of `browser/adapter/chromium/protocol4` is earned. ADR-0196 Decision 3 puts
  one adapter per browser family under `browser/adapter/`, and `protocol4` is the wire level, so
  flattening it would need an ADR amendment. The confusion is elsewhere. `adapter/mod.rs`
  re-exports the port vocabulary with a glob, so the rest of the service writes
  `browser::adapter::BrowserCommand` for the port contract, not the adapter. `browser::session`
  holds graphical-session discovery, and its name collides with MCP sessions and with the relay's
  `ghostlight_relay::os::session`, the session-end listener. `p4/assets.rs:95` reaches its golden
  through seven `../` segments.
- Proposed change: move `work.rs` and `documents.rs` up to `browser/` and re-export them from
  `crate::browser`. Rename `session.rs` to `graphical_session.rs`. Build the golden path from
  `env!("CARGO_MANIFEST_DIR")` (with `concat!` inside `include_str!`), as `golden.rs` builds its
  fixture path. Update ARCHITECTURE.md:345-347.
- Ratings: payoff medium; risk low; effort M (a mechanical rename across 43 files). Wire: no.
  Golden: none.
- Verdict: partial. The re-export, the session clash and the seven `../` hold, and the clash is
  concrete: `recovery.rs:38` imports `ghostlight_relay::os::session`, so `recovery.rs:374` must
  spell `crate::browser::session` in full. The counts are off: of the 153 lines, 107 are `use`
  lines and the rest inline paths and doc links, and they sit in 74 files, 39 outside `browser/`.
  The other four outside files (`audit.rs`, `audit/record_tests.rs`, `work/tests/availability.rs`,
  `work/tests/protocol4.rs`) name only `chromium` paths and need no change. A `browser/work.rs`
  would sit beside `crate::work`, the near-homonym BROW-5 objects to, so another leaf name is
  better.
- Class: refactor.
- arch-1: none. C16 and C17 (todo) edit `adapter/work.rs`, so the move goes after them.

## BROW-14: World kinds and primitive names are string modules listed again by hand

- Kind: vocabulary.
- Locations:
  - `p4/world.rs:78-143` (the `kind` module) and `p4/world.rs:167` (`Message.kind`, a
    `&'static str`).
  - `p4/wire.rs:49-84` (the `primitive_name` module).
  - `p4/golden.rs:54-87` (`WORLD_KINDS`, 32 entries) and `p4/golden.rs:90-108` (`PRIMITIVES`, 17
    entries).
  - `adapter/work.rs:1007` (`TreeNodeKind::ALL`).
- Problem: the 32 world message kinds and the 17 primitive names are modules of `&str` constants,
  and `golden.rs` re-lists both by hand. `Message.kind` is a plain `&'static str`. The
  `primitive_name` constants also restate the names serde derives from the `Primitive` variants.
- Proposed change: a `WorldKind` enum with `ALL` and `as_str()`, the way `TreeNodeKind::ALL`
  already works, used by `Message` and by `golden.rs`; do the same for the primitive names.
- Ratings: payoff low to medium; risk low; effort S. Wire: no. Golden: none change; the goldens
  already pin the names.
- Related: ADR-0203 slice C18 (todo) also edits `p4/golden.rs`.
- Verdict: confirmed. 32 `kind` constants (`p4/world.rs:77-143`) behind a `&'static str`
  (`:167`), 17 `primitive_name` constants (`p4/wire.rs:49-84`) that restate the serde tags of
  `Primitive` (`:317-352`), and both lists again at `p4/golden.rs:54-87` and `:90-108`.
  `Spec::NAME` (`p4/link.rs:83-253`) also reads `primitive_name`, so that enum needs a
  `const fn as_str`. The primitive names are tags the 2.0 extension dispatches on; the world
  kinds go only to the service's own page assets.
- Class: refactor, frozen.
- arch-1: none. C18 (todo) also edits `p4/golden.rs`.

## BROW-15: Argument structs restate the command variants

- Kind: structure.
- Locations:
  - `p4/mod.rs:95-542` (`translate`, 448 lines).
  - The argument structs: `p4/translate/pointer.rs:44` (`Click`), `:194` (`Scroll`) and `:256`
    (`Drag`); `p4/translate/keyboard.rs:26` (`TypeText`), `:73` (`TypeFocused`) and `:168`
    (`PressKey`); `p4/translate/points.rs:30` (`ViewPoint`), `:41` (`PointClick`), `:54`
    (`PointWheel`), `:65` (`PointDrag`) and `:78` (`PointDrop`); `p4/translate/forms.rs:52`
    (`Fill`); `p4/translate/observe.rs:51` (`Subjects`) and `:184` (`Wait`);
    `p4/translate/read.rs:111` (`Semantic`); `p4/translate/records.rs:37` (`DiagnosticsRead`) and
    `:203` (`PageExport`).
- Problem: the 448-line `translate` match destructures every variant, and 19 of its arms rebuild
  the fields into one of 17 argument structs (16 borrowed, plus the owned `DiagnosticsRead`) that
  restate the variant's fields.
- Proposed change: make the field-heavy variants newtypes over shared argument structs that the
  executor builds and the translator borrows, so each arm is one line.
- Ratings: payoff medium; risk medium; effort L (405 `BrowserCommand::` uses). Wire: no. Golden:
  none.
- Verdict: confirmed. `translate` is 448 lines (`p4/mod.rs:95-542`), and 19 arms build one of
  the 17 structs, `read::Semantic` at `p4/mod.rs:191` included. Not all restate one variant:
  `ViewPoint` nests the tab, point and viewport that `PointClick`, `PointWheel` and `PointDrop`
  share and `HoverPoint` sends alone, and `Drag` renames its locators, so the variants would take
  that shape. It rewrites every construction and match of these variants, and BROW-3 and BROW-4
  change the same fields, so it comes after them.
- Class: refactor.
- arch-1: none. C16 and C17 (todo) change `BrowserCommand` variants too.

## Overall

The protocol 4 internals (link, scope, tree, merge, geometry, keys, world) are in good shape: every
production module opens with a doc comment, constants are named, the helpers are pure, and the
goldens back the names. The debt is at the seams. The port and the adapter reach into each other,
the in-process command vocabulary is stringly typed, and the wire mirrors are copied by hand. The
structural change that would help most is BROW-1: once `browser` holds only the port contract and
protocol 4 owns its connection registry, dependencies run one way, and BROW-3, BROW-5 and BROW-13
have a clear home. Sequence it with ADR-0203 slices C3b and C3c, which change the same registry.

## Skeptic summary

- Verdicts: 9 confirmed (BROW-2, 3, 4, 7, 10, 11, 12, 14 and 15), 6 partial (BROW-1, 5, 6, 8, 9
  and 13), none refuted. No proposal moves a protocol 4 wire byte. BROW-10 regenerates the asset
  id in `tests/golden/protocol4/`, and BROW-8's `String` name regenerates one result golden.
  `frozen` marks BROW-6 and BROW-14, which rewrite names the 2.0 extension exchanges, so those
  goldens must pass unchanged.
- First, BROW-9: a test that a browser relay returning on its doubling backoff after an update
  still lands inside `RELAY_RETURN_WINDOW`, then the comment fixed. It is the one finding that may
  hide a broken promise: a held call that stops waiting starts recovery for a browser that is
  already open (`work/mod.rs:1636`).
- Second, BROW-11: a governing contract names an enum that is gone, and every agent reads it. It
  is one line in D2's next commit, plus two renamed constants.
- Third, BROW-2: a dead public variant, two `unreachable!` arms in the executor and a dead plan
  arm go in one small commit, before C3c reopens `recovery.rs`.
- Later: BROW-1 after C3b, C3c and C16, with the registry kept in `browser/`; BROW-10 with C18's
  asset-id regeneration; BROW-15 after BROW-3 and BROW-4.
