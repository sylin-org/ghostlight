# Second sweep, 2026-10-20: workbench, window and desktop

This sweep read the three layers of the Ghostlight window: `crates/orchestrator/src/workbench/`
(the pages the service serves and their state, about 10k lines with tests),
`crates/orchestrator/ui/` (the page itself: `app.js`, `lib/`, `kit/`, the HTML and the CSS, about
8.9k lines) and `crates/desktop/` (the `ghostlight-desktop` shell, tray, webview and session-end
handling, about 6.9k lines). It was read-only, on `dev` at 8f6a43ec: nothing was built, run, edited
or committed. It enforces the Code style rules of AGENTS.md: a repeated literal becomes a named
constant beside its use, a vocabulary that leaves its module lives in one domain module, every
module opens with a doc comment stating its role, and the root fix is preferred with the design
kept simple. The window's words and look are product identity (`docs/MEMORY.md`, "One visual
language: sylin.org's"), so each finding says whether it changes what a person reads or sees. Only
SURF-11 touches appearance: it edits CSS but keeps every colour value, so the look should not
change, and the headless history journey confirms it. SURF-3 moves visible sentences word for
word. No other finding changes anything a person sees. Six findings touch arch-1 slices that are
still `todo` in `LEDGER-surface.md` and `LEDGER-core.md` (S3, S4b, S5, S7 and C18 of ADR-0203);
their sections say what the slice already decides and what this sweep adds. Line numbers are as of
8f6a43ec; where the ADR cites older ones, the sections give today's.

## SURF-1: Three names for the same pages

- Kind: naming.
- Ratings: payoff high, risk low, effort S.
- Visible text or look: none. The wire value of a search hit's `view` changes; no person reads it.
- Where:
  - `crates/orchestrator/src/workbench/mod.rs:2337`: `SearchDestination` (home, activity, history,
    checkup, configuration, settings, browser_work, install). `SearchHit.view` (`mod.rs:2291`)
    carries it, and `search()` pairs it with each page (`mod.rs:993-999`).
  - `crates/orchestrator/src/language/workbench.rs:579-611`: `SearchPage.id` writes the page
    addresses again as raw strings ("monitor", "integrations", "status", "policy", "settings").
  - `crates/orchestrator/src/workbench/address.rs`: `PageAddress`, the service's copy of the page's
    address list, already pinned to `ADDRESSES` in `words.js`.
  - `crates/orchestrator/ui/lib/words.js:390`: `SEARCH_ADDRESSES` translates the search words back
    into addresses, read at `crates/orchestrator/ui/app.js:924`.
- Problem: the five pages have three names: `PageAddress`, the raw ids in `SearchPage`, and
  `SearchDestination`'s activity, checkup, configuration and install, which the page then
  translates back. Nothing ever constructs `SearchDestination::Home`. `SearchKind::Installation`
  (`mod.rs:2333`) and `SearchDestination::Install` add two more names for the agent roster, which
  the tab calls Integrations and the hit's chip calls Agent.
- Proposed change: `SearchHit.view` and `SearchPage` carry `PageAddress`. Delete
  `SearchDestination` and `SEARCH_ADDRESSES`. The tests that read them change with it:
  `the_search_names_each_page_as_its_tab_does` (`crates/orchestrator/src/workbench/tests.rs:1877`),
  `the_search_offers_the_browser_work_section_for_browser_work` (`workbench/tests.rs:2831`, which
  scrapes `words.js` for the `browser_work` entry) and the address test at
  `tests/workbench-settings.test.mjs:472-476`.
- Verdict: confirmed. Nothing builds `SearchDestination::Home` (its definition is its only use),
  and every destination maps onto one `PageAddress`, so each hit opens where it opens today. More
  churn than listed: the fake hits at `tests/workbench-settings.test.mjs:2990-2991` and core's
  `tests/workbench-surface.mjs:1501` carry the old words. `language/` imports nothing from
  `workbench/` today, so keep the pairing in `search()` (`mod.rs:993-999`) instead of putting
  `PageAddress` into `SearchPage`.
- Class: refactor
- arch-1: none.

## SURF-2: The snapshot is reshaped four times, with dead fields

- Kind: structure and dead code.
- Ratings: payoff high, risk medium (test churn, mostly in `crates/orchestrator/src/service/mod.rs`
  and `crates/orchestrator/src/workbench/tests.rs`), effort M.
- Visible text or look: none. The wire is unchanged: the route already serves parts.
- Where:
  - `crates/orchestrator/src/workbench/live.rs:93` (`Gathered::connections`) and `live.rs:384`
    (`Live::status`): the live parts are built from one `Gathered`.
  - `crates/orchestrator/src/workbench/mod.rs:821-951` (`snapshot()`): those parts are copied field
    by field into the flat `WorkbenchSnapshot` (`mod.rs:1665`).
  - `crates/orchestrator/src/workbench/parts.rs:214` (`StatusPart::of`) and `parts.rs:252`
    (`ConnectionsPart::of`): the snapshot route rebuilds the same parts from the flat snapshot, so
    `ConnectionsPart` has two constructors over two sources.
  - `crates/orchestrator/ui/lib/transport.js:126-166` (`flatten`): the page flattens the parts back
    into the old shape.
  - Dead: `OverviewSummary` (`mod.rs:1757`, built at `mod.rs:923`), `generated_at_ms`
    (`mod.rs:1677`) and the six booleans of `ConfigurationSummary` (`mod.rs:2075`, the
    `local_policy_*` and `managed_authority_*` fields). Only its `runtime_state` is read, by search
    (`mod.rs:1067`).
  - `crates/orchestrator/ui/lib/transport.js:146` and `crates/orchestrator/ui/lib/store.js:182`: the
    page recomputes `overview` and keeps it current.
- Problem: one read model takes four shapes. The dead fields are computed on every snapshot and
  never served: `crates/orchestrator/src/workbench/parts/tests.rs:937` and
  `crates/orchestrator/src/workbench/web/tests.rs:1123` assert they are absent. The page rebuilds
  `overview` and nothing reads it; only `tests/workbench-transport.test.mjs:73-86` checks its shape.
- Proposed change: the snapshot holds the parts, built once from `Gathered`. Delete
  `OverviewSummary`, the six booleans, `generated_at_ms`, the second `ConnectionsPart` constructor
  and the page's `overview`.
- arch-1: finding A-08, decided as S3 ("every part comes from one gather"), still `todo` in
  `LEDGER-surface.md`. S3 deletes `WorkbenchSnapshot`, `OverviewSummary`, `ConfigurationSummary`,
  `generated_at_ms` and the page's `overview`. This sweep confirms it still holds at 8f6a43ec and
  adds nothing beyond it.
- Verdict: partial. The dead fields, the four shapes and the two `ConnectionsPart` constructors
  hold. Correction: more of `ConfigurationSummary` is read than `runtime_state`. `StatusPart::of`
  reads its `policy`, `managed_policy` and `organization` (`parts.rs:221-223`) for every status
  part the snapshot route serves, and tests read `runtime_state` at `service/mod.rs:6221`, `6284`
  and `7052`. Only the six booleans are unread, so the struct goes only once the route builds the
  status part from the gather, which is S3's order.
- Class: refactor
- arch-1: S3 (`todo`; its After list, P5, I6 and C2, has landed). Like this finding, S3 keeps the
  page's `flatten` (`transport.js:126-166`) and drops only its `overview`.

## SURF-3: Sentences written at call sites

- Kind: magic strings (visible words outside the word modules).
- Ratings: payoff medium-high, risk low, effort S.
- Visible text or look: none, if the sentences move word for word.
- Where:
  - `crates/orchestrator/src/workbench/mod.rs:1205-1221`: the four policy-preview summaries.
    `mod.rs:1256`: "Your rules are applied: {} {}." `mod.rs:1274`: "Your rules are removed."
    `mod.rs:1987`: "Some step history could not be saved.", which
    `crates/orchestrator/src/language/audit_health.rs:23` also writes.
  - `crates/orchestrator/ui/app.js:540` ("Process diagnostics are ..."), `app.js:615` ("Opened the
    official ... download page."), `app.js:982` ("Policy re-read."), `app.js:986` ("Status
    refreshed."), `app.js:992` ("Cleared N entries from this view. Audit history is unchanged.").
  - `crates/orchestrator/ui/lib/view.js`: 209 and 551 (the "completed" fallback), 447 (the step
    labels Completed, Could not start, Not run, Pending, Receipt unavailable), 468 ("Coverage
    details"), 524 ("Refused by ..."), 833 ("No other activity in this session."), 902 and 987 (the
    "Open Policy." label), 1647 ("Not allowed", which repeats `CAPABILITY_BADGE.unavailable`), 1926
    and 2216 ("Your rules", "Ghostlight default"), 1951-1952 (the never-touched sentences), 2116
    (the placeholder "What is this rule for? (optional)" and its `maxlength="200"`), 2265 ("Matches
    ..." and "Add a site for this rule to do anything."), 2328 ("Back to the rules that are
    applied.").
- Problem: every other sentence lives in `language::workbench` or `ui/lib/words.js`, so a wording
  pass over those misses these. Acts also answer in mixed shapes, so who words a confirmation
  varies by act. Runtime and policy acts answer `WorkbenchIntentResult` (`accepted`,
  `runtime_state`, `message`); harness acts answer `HarnessActionResult` with a message;
  `set_watching` answers the controls part; `set_diagnostics` answers a `DiagnosticsReport`; the
  notification test, reveal and open answer `{}`; quit answers `{message}` with 202. The page
  toasts `result.message` for some and its own words for others (`WATCHING_TURNED_ON` and
  `WATCHING_TURNED_OFF`, `TEST_NOTIFICATION_SENT`, and the diagnostics and download sentences
  above).
- Proposed change: move the Rust sentences into `language::workbench` and the page's into
  `words.js`. Optionally, give every act result a service-authored `message`, so the page always
  toasts it.
- arch-1: A-09 (words outside `language/`) is decided as S5, I10 and S8, and S4b deletes the
  preview sentences (the ADR cites `workbench/mod.rs:1203-1272`; they are now at 1205-1274).
  Decision 8's list of words moved into `language/` names install, search and policy words only.
  Neither names `mod.rs:1987` or the page's inline sentences in `app.js` and `view.js`; those are
  new here.
- Verdict: partial. Every cited line holds, and so do the act shapes (`routes.rs:997-1217`). The
  list is short: `view.js` also writes sentences at 450, 457, 532, 646-647, 1612-1614 (About's
  line), 1700-1705 (the passport's provenance), 1727, 2034-2041 (each rule's sentence, though
  `view.js:1622-1624` says the page composes none), 2047-2049, 2103, 2117-2118 and 2277, and 2110
  repeats 2265's sentence. `index.html` holds about 50 static phrases, a third home. "native
  presentation is unavailable" (`mod.rs:1624`, `1632`) reaches a watcher's toast when no shell
  can show the window (`service/shell.rs:396-400`, `routes.rs:993`). Moved word for word, the
  apply toast keeps reading "Your rules are applied: Your rules 2026-10-20.", because the page
  names its document "Your rules" (`view.js:2216`).
- Class: refactor
- arch-1: S4b (`todo`; P7 and P8 have landed) deletes `mod.rs:1205-1274`. The rest: none. S5's
  toast is Decision 5.6's desktop notice, not these page toasts.

## SURF-4: Activity labels written twice, glyphs keyed by English

- Kind: magic strings (a vocabulary that crosses Rust and JavaScript).
- Ratings: payoff medium-high, risk low, effort S.
- Visible text or look: none.
- Where:
  - `crates/orchestrator/src/workbench/mod.rs:2434`: `activity_label`, a 17-arm match identical to
    `crates/orchestrator/src/presentation/mod.rs:360`. The same words are also in
    `extension/lib/shared.js:32-33` and
    `crates/orchestrator/src/browser/adapter/chromium/protocol4/assets/page-shared.js:64-65`,
    outside this area.
  - `crates/orchestrator/ui/lib/words.js:752`: `ACTIVITY_GLYPH` keys the glyphs on the displayed
    words ("Reading page" to scan), read by `glyphFor` (`words.js:1103`).
- Problem: rewording a label silently changes its icon. The waiting label, `WAITING_FOR_WORK`
  ("Waiting for earlier browser work", `crates/orchestrator/src/language/outcome.rs:1421`), which
  `OperationSummary` sends for waiting work (`mod.rs:1822-1823`), has no entry, so a waiting row
  already gets its tool's glyph.
- Proposed change: one label function in the language layer. `OperationSummary` also sends the
  `PresentationActivity` token, and the glyph map keys on it.
- arch-1: V-06 and W-04, decided as C18 and S5, both `todo`. S5 carries the activity code on
  `OperationSummary`, keys the glyphs on it, and deletes "the second `activity_label`
  (`workbench/mod.rs:2339-2359`)", now at 2434-2454. New here: the waiting label already falls
  through to the tool glyph.
- Verdict: partial. The two 17-arm tables are identical (`mod.rs:2434-2454`,
  `presentation/mod.rs:360-380`), the glyphs key on English (`words.js:752-770`), and the waiting
  label falls through to `TOOL_GLYPH` (`words.js:1102-1103`). The JavaScript tables span
  `extension/lib/shared.js:24-40` and `page-shared.js:56-72`. "Visible: none" does not hold:
  keyed on the activity token, a waiting row's medallion changes wherever its tool glyph and its
  activity glyph differ, for `browser_tabs` (navigate today, Quiet's scan), `browser_flow` (gear,
  scan) and a `browser_record` other than start (camera, scan) (`work/mod.rs:2594-2624`,
  `words.js:1076-1100`). Today that medallion changes when the work starts; S5 has to pick one
  and pin it.
- Class: refactor, visible
- arch-1: S5 (`todo`, after C18 and C12b) keys the glyphs and deletes this `activity_label`; C18
  (`todo`) deletes `presentation/mod.rs`'s. No slice deletes the extension's copy, which is
  packaged and frozen while 2.0.1 is in store review.

## SURF-5: Duplicated limits that already disagree

- Kind: magic numbers.
- Ratings: payoff medium, risk low, effort S.
- Visible text or look: none.
- Where:
  - `crates/orchestrator/src/workbench/web/routes.rs:50`: `POLICY_BODY` (256 KiB), and
    `crates/orchestrator/src/workbench/input.rs:13`: `POLICY_DOCUMENT_LIMIT` (256 KiB).
  - `crates/orchestrator/src/workbench/input.rs:9`: `SEARCH_QUERY_LIMIT` (120), and
    `crates/orchestrator/ui/index.html:467`: `maxlength="120"`, with no test between them.
  - `crates/orchestrator/src/workbench/parts.rs:45`: `SACRED_DOMAINS_KEY` repeats
    `PolicySetting::SacredDomains.key()` (`crates/orchestrator/src/governance/manifest.rs:216`).
  - Document coverage is capped at 200 by the page (`crates/orchestrator/ui/lib/store.js:285`,
    `FEED_LIMIT`) and at 500 by the service (`crates/orchestrator/src/workbench/mod.rs:170`,
    `HISTORY_LIMIT`). `HISTORY_HELD` is pinned to `HISTORY_LIMIT` by a test
    (`workbench/tests.rs:2855`); the coverage cap is not.
- Problem: the body cap counts the JSON-escaped document inside `{"document": ...}`, so a document
  the validator accepts near its limit is refused first with `words::TOO_LARGE` ("That request is
  too large.") instead of `policy_too_large` ("A policy document is limited to 262144 bytes."), and
  the stated limit cannot be reached.
- Proposed change: derive the body cap from the document limit with headroom for escaping; use the
  registry key; add a test that reads `index.html` for the search `maxlength`, as `address.rs`
  reads `words.js`.
- arch-1: S4b (`todo`) deletes `SACRED_DOMAINS_KEY` (the ADR cites `parts.rs:42`; it is now at 45)
  and pins the page's setting keys to `PolicySetting`. The body cap, the search length and the
  coverage cap are new here.
- Verdict: partial. Both pairs differ, but "Visible: none" does not hold. Today an oversized
  policy draft reads "That request is too large." (`routes.rs:743-748`) in the editor's status
  line (`app.js:459`, `489`). "A policy document is limited to 262144 bytes."
  (`input.rs:34-35`) can never show: the body is the document plus 15 bytes plus one byte for
  each quote and line break, which the pretty-printed draft has on every line
  (`view.js:2214-2220`, `transport.js:240`), so the body cap always answers first, as
  `web/tests.rs:1351-1352` pins. Reaching it takes thousands of host patterns. A fix lets drafts
  just under the limit through and changes that sentence, so it needs a test. Coverage: the page
  keeps the service's whole list, up to 500, after every snapshot (`transport.js:158`) and cuts
  it to 200 only on a live change (`store.js:285`). It draws coverage only for its at most 200
  feed rows (`store.js:53-58`, `app.js:102`), so the ordinary feed shows no difference. After
  Clear, a rebuild seeds older rows (`store.js:91-97`), and the next live change can drop their
  Coverage details. The search pair agrees in effect: `maxlength` counts UTF-16 units, so it
  never allows more than 120 characters.
- Class: refactor, defect, visible
- arch-1: S4b (`todo`) deletes `SACRED_DOMAINS_KEY` and pins the page's third copy, `SACRED_KEY`
  (`words.js:938`). The body cap, the search length and the coverage cap: none.

## SURF-6: Incomplete JavaScript copies of Rust vocabularies

- Kind: magic strings.
- Ratings: payoff medium, risk low, effort S.
- Visible text or look: none.
- Where:
  - `crates/orchestrator/ui/lib/words.js:102`: `PHASE` has 3 of the 5 `OperationPhase` values;
    `crates/orchestrator/ui/lib/entries.js:42` and `entries.js:87` write "blocked" and "completed"
    as literals. `OUTCOME` is pinned to `OutcomeClass` by `surface_tests.rs` (arch-1 S2); `PHASE`
    is not pinned.
  - No runtime-state constant: "active" is a literal at `crates/orchestrator/ui/lib/store.js:42`
    and `crates/orchestrator/ui/lib/view.js:884`, `995` and `1002`.
  - `crates/orchestrator/ui/app.js:748` writes "retired" and "done" though
    `crates/orchestrator/ui/lib/setup-view.js:19` exports `STAGE`.
  - `crates/orchestrator/ui/lib/store.js:276-304` (`applyChange`) returns bare strings ("ignored",
    "gap", "refresh", "hinted", "applied") that `app.js:286-288` compares as literals.
  - "motion-reduced" is written in both `app.js:723` and
    `crates/orchestrator/ui/kit/settings.js:30`.
  - The context tokens are split: `CONTEXT_CLOSED` in `crates/orchestrator/ui/lib/transport.js:55`,
    `CONTEXT_CONTROL` in `kit/settings.js:42`. `app.js:701` `GET_CHROME = "get_chrome"` repeats
    `WorkbenchDestination::GetChrome.key()` (`crates/orchestrator/src/workbench/mod.rs:2231`).
- Proposed change: complete `PHASE` and pin it to `OperationPhase`; add a `RUNTIME` constant;
  export the store's results; use `STAGE`; keep each context token and class name in one module.
- arch-1: S5's pins of `words.js` to `language::control` are the natural home for the runtime
  tokens. The rest is new here.
- Verdict: confirmed. Every copy is where the finding says, and only `OUTCOME` is pinned
  (`surface_tests.rs:186-247`). Two more of the same: `SETUP_PART` (`app.js:698`) repeats the
  setup part's token and `CONNECT_ACT` (`app.js:704`) the install act's.
- Class: refactor
- arch-1: S5 (`todo`) pins `words.js` to `language::control` and could carry the runtime tokens.
  The rest: none.

## SURF-7: view.js is one 2,590-line closure

- Kind: structure.
- Ratings: payoff high, risk medium, effort L.
- Visible text or look: none.
- Where: `crates/orchestrator/ui/lib/view.js:119-2705`, one `create()` that holds the monitor feed,
  the band, At a glance, the agents pane, Status, the policy editor, the palette, the dialogs and
  the clocks. It imports about 120 names from `GhostlightWords` (`view.js:14-27`).
- Problem: `app.js` (1.1k lines) is a sound composition root; the oversized file is `view.js`. Its
  parts barely share state. The policy editor (`view.js:1617-2356`, 740 lines) keeps its own
  `draft` and `applied` (`view.js:1627-1628`) and touches only `el` (38 uses) and `can` (11) from
  the closure. The agents pane (`view.js:1183-1616`) touches only `can`, `continueMarkup`,
  `ifChanged`, `opensLinks`, `now`, `access` and `current`.
- Proposed change: extract `ui/lib/policy-view.js` and `ui/lib/agents-view.js` on the pattern of
  `setup-view.js` and `kit/settings.js` (injected `el`, `can` and `onFailure`). Update the script
  list in `crates/orchestrator/ui/index.html:494-502`, `ASSETS` in
  `crates/orchestrator/src/workbench/web/assets.rs:71-91`, `surface_source` in
  `crates/orchestrator/src/workbench/surface_tests.rs:39`, and the tests that name `view.js`
  (`tests/workbench-settings.test.mjs`, `tests/workbench-setup-surface.mjs`,
  `crates/orchestrator/src/workbench/web/tests.rs`).
- Verdict: partial. The split is sound, but its inventory is off. `view.js` takes 95 names from
  `GhostlightWords` (`view.js:14-30`), not about 120. Beyond `el` (38) and `can` (11), the
  editor (`view.js:1617-2355`; 2356 is `collections`) uses `drawnOnce` (1638, declared at 2453),
  and the band's `policyAccess` (`view.js:967-975`) reads its `applied` and `draft` and calls
  `renderRules`, so that function moves with it. The agents pane is `view.js:1176-1598`
  (1599-1615 is `status`); it uses `el` 28 times, `replaceHistoryMarkup` (1509, 1516) and
  `drawnOnce` (1490), and never `now`. `setup-view.js` and `kit/settings.js` are handed
  `document`, `words` and callbacks (`app.js:126-140`, `156-170`), not `el` and `can`.
- Class: refactor
- arch-1: none.

## SURF-8: What a watcher may do is stated three times

- Kind: structure.
- Ratings: payoff medium, risk low to medium (it is a trust gate, but behavioral tests cover it),
  effort M.
- Visible text or look: none.
- Where:
  - `crates/orchestrator/src/workbench/context.rs:169`: `WebAct::open_to_watchers`, what the page
    is told it may offer (Search, CheckPolicy, Rescan, ContinueInWindow, Hold).
  - `crates/orchestrator/src/workbench/web/routes.rs:139-287`: `WatchAccess` on each route, what
    the server answers (Context, Snapshot, Events, Policy, Search, PolicyPreview, Rescan, Window and
    Runtime are open to a watcher).
  - `crates/orchestrator/src/workbench/web/routes.rs:1002-1004`: `runtime()` refuses a watcher
    anything but Hold.
- Problem: the page's offers and the server's gate agree today only because three lists are kept
  in step by hand. A new act needs all three, and only behavioral tests would notice a mismatch:
  `a_watcher_reads_projected_state_and_may_only_hold` and
  `every_control_only_route_refuses_a_watcher_and_changes_nothing`
  (`crates/orchestrator/src/workbench/web/tests.rs:543` and `580`).
- Proposed change: each route names its `WebAct` (none for a read); the gate calls
  `availability(act, context)`; `runtime()` maps the intent to its act.
- Verdict: confirmed. The lists agree today: the five open acts are the Search, PolicyPreview,
  Rescan, Window and Runtime routes, and `runtime()` lets a watcher Hold only
  (`routes.rs:1002-1009`). The second test keeps its own route list (`web/tests.rs:588-609`), a
  fourth copy. The change can keep every answer: 401 `needs_window` (`routes.rs:352-359`) for a
  watcher, `watching_off` for Closed.
- Class: refactor
- arch-1: none.

## SURF-9: workbench/mod.rs is four modules in one, and some docs are stale

- Kind: structure and docs.
- Ratings: payoff medium, risk low, effort M.
- Visible text or look: none.
- Where:
  - `crates/orchestrator/src/workbench/mod.rs` is 2,472 lines. Its doc (`mod.rs:1`) says
    "Orchestrator-owned desktop read model, user intents, and operating-system presentation port."
    and predates most of what the file holds: the projection, the facade, search (`mod.rs:977-1115`
    and `2272-2417`), the destination vocabulary with its `documentation_branch!` and
    `repository_document!` macros (`mod.rs:2098-2234`), and five ports with their errors
    (`mod.rs:1390-1661`).
  - `crates/orchestrator/src/workbench/tests.rs:1` has no module doc.
  - `crates/orchestrator/ui/lib/words.js:323` says `VIEWS` is keyed by the orchestrator's search
    vocabulary; it is keyed by page addresses. The header of `words.js` calls it the fixed
    vocabulary, but it also exports glyph markup, CSS class maps, the host grammar, address parsing
    and timings.
  - `crates/orchestrator/ui/kit/kit.css` holds only the Settings layout and is easy to confuse with
    `extension/kit.css`, whose header has to say it is not this file.
- Proposed change: split `search.rs`, `destination.rs` and `ports.rs` out of `mod.rs`, keep the
  projection and the facade, and rewrite its doc. Give `tests.rs` a doc, correct the `VIEWS` doc,
  and rename `kit/kit.css` to `kit/settings.css` beside `kit/settings.js`.
- Verdict: partial. The stale doc, the line ranges and the missing doc hold; `tests.rs` is the
  only workbench file without one. Two corrections. `VIEWS` is keyed by the ids `SearchPage.id`
  repeats (`language/workbench.rs:579-611`), which `workbench/tests.rs:1898-1902` pins, so its
  doc misleads rather than errs. Renaming `kit/kit.css` leaves the packaged `extension/kit.css`
  (`tests/golden/extension-2.0.0.files.sha256:6`) naming a file that is gone (its header, line
  8); correcting it means resubmitting the staged 2.0.1 extension (ADR-0200 Decision 16). The
  rename also edits `index.html:8`, `assets.rs:77` and `tests/workbench-settings.test.mjs:27`,
  `2364` and `2432`.
- Class: refactor, frozen
- arch-1: none.

## SURF-10: Small Rust duplicates

- Kind: duplication.
- Ratings: payoff low-medium, risk low, effort S.
- Visible text or look: none.
- Where:
  - The runtime-state word match is written twice: `runtime_state_word`
    (`crates/orchestrator/src/workbench/mod.rs:2457`) and inline in `ControlsPart::of`
    (`crates/orchestrator/src/workbench/parts.rs:515-519`).
  - The diagnostic ids "service", "browser", "authority" and "audit" are repeated literals in
    `snapshot()` (`mod.rs:842-907`), each two or three times.
  - One error message, "native presentation is unavailable", is written twice (`mod.rs:1624` and
    `1632`).
  - `push_bounded` (`mod.rs:2419`) serves only `history.rs`.
  - `crates/orchestrator/src/workbench/history.rs:74` and `76` write "unknown" twice for a flow's
    placeholder parent.
  - The poison-tolerant `lock` helper is defined five times in the workbench (`mod.rs:2465`,
    `live.rs:229`, `setup.rs:491`, `web/events.rs:89`, `web/limits.rs:118`) and 17 times in the
    crate.
- Proposed change: one runtime-word function in `language::workbench`; an enum or constants for the
  diagnostic ids; one constant for the error message; move `push_bounded` into `history.rs`; one
  crate-level `lock` helper.
- arch-1: S5 deletes `runtime_state_label`, which commit 2301b163 renamed to `runtime_state_word`,
  so the runtime-word half is already decided (V-12). The rest is new here.
- Verdict: partial. Every item holds but one count: "service" is written once (`mod.rs:843`);
  "browser" and "authority" repeat three times and "audit" twice. The lock helper is 5 copies
  here and 17 in the crate, as counted, but 12 sit in core, adapter, policy and install files,
  so one crate-level helper is cross-lane. "native presentation is unavailable" is a sentence a
  person can read (SURF-3), so it belongs in `language::workbench`, not in a constant here.
- Class: refactor
- arch-1: S5 (`todo`) deletes the runtime word (V-12; 2301b163 renamed it after the ADR's
  5464865e). The rest: none.

## SURF-11: The palette is written outside its tokens

- Kind: magic colours.
- Ratings: payoff low-medium, risk low, effort S.
- Visible text or look: touches the look. Every value stays the same, so nothing should change on
  screen; run the headless history journey (`GHOSTLIGHT_TEST_HEADLESS=1 node
  tests/workbench-history-browser.mjs`, as `docs/MEMORY.md` asks of a window layout change) before
  it lands.
- Where:
  - `crates/orchestrator/ui/styles.css` names the accent once as `--argb` and uses
    `rgba(var(--argb), ...)` 28 times, but writes the held, danger and ok colours raw: `#fbbf24`
    and `rgba(251, 191, 36, ...)`, `#f87171` and `rgba(248, 113, 113, ...)`, and
    `rgba(74, 222, 128, ...)` at lines 146-149, 205-206, 366, 467, 622, 638, 674, 749-750 and
    955-960. The raised surface `#17161b` is written three times (724, 748, 755), `#fecaca` three
    times (205, 749, 750), and `#fca5a5` at 674 repeats `--danger-soft`. `--amber`, `--held` and
    `--cap-write` are all `#fbbf24` (`styles.css:50`, `57`, `58`).
  - `extension/kit.css` already names `--warn-rgb`, `--danger-rgb` and `--ok-rgb`, under
    `.gl-page`.
  - The desktop crate spells one palette three ways: `GROUND` as an RGBA tuple
    (`crates/desktop/src/webview.rs:31`); `WORKING_COLOUR`, `PAUSED_COLOUR`, `ATTENTION_COLOUR` and
    `NIGHT_COLOUR` as RGB arrays (`crates/desktop/src/tray.rs:55-68`), where `NIGHT_COLOUR` is
    `GROUND` again; and the state screens' inline CSS (`crates/desktop/src/states.rs:59`), which
    fills only `{ground}` from the constant and writes ink, accent and danger as hex.
  - `crates/orchestrator/src/workbench/surface_tests.rs:19` pins only the accent and the ground of
    `styles.css`.
- Proposed change: add `--held-rgb`, `--danger-rgb`, `--ok-rgb` and a raised-surface token to
  `styles.css` and use them as `--argb` is used. Give the desktop crate one palette module that the
  tray and the state screens both read.
- Verdict: partial. Every raw value and the 28 uses of `rgba(var(--argb), ...)` count right.
  Correction: the tray's colours are already pinned to the stylesheet:
  `the_dot_colours_are_the_tokens_of_the_windows_stylesheet` (`tray.rs:1045-1063`) requires
  `--a`, `--held`, `--danger` and `--bg` in `styles.css`. So `surface_tests.rs:19` is not the
  only pin; only the state screens' ink, ink-2, accent and danger (`states.rs:59`) are unpinned.
  The values stay equal, so the look stays. One more raw colour: `#241419` (`styles.css:749`).
- Class: refactor
- arch-1: none.

## SURF-12: "Viewing" and "watching" name one setting

- Kind: naming.
- Ratings: payoff medium, risk medium (a wire field and a route path change), effort M.
- Visible text or look: none.
- Where:
  - Viewing: `ViewingFacts`, `ViewingControls`, `ViewingActs` and the `viewing` fields of
    `ControlsFacts` and `ControlsPart` (`crates/orchestrator/src/workbench/parts.rs:301-409`, the
    fields at 321 and 341); `Api::SetViewing` and `/api/controls/viewing`
    (`crates/orchestrator/src/workbench/web/routes.rs:94` and `224-229`); `set_viewing` and
    `ViewingArguments` (`routes.rs:1093` and `811`); the `CONTROLS_VIEWING_*` word constants; the
    `controls-viewing-*` element ids, and `VIEWING_TOGGLE` and `VIEWING_COPY`, in
    `crates/orchestrator/ui/kit/settings.js:54-55` and `170-180`.
  - Watching: `Watching` (`crates/orchestrator/src/workbench/watching.rs`), `WatchingError`,
    `set_watching`, `WebAct::SetWatching`, `Audience::Watch`, `WebContext::Watch`, the request
    body's `watch` field, the transport command `set_watching`
    (`crates/orchestrator/ui/lib/transport.js:98`), and the setting's own name, "Others can watch".
- Problem: a reader has to learn that viewing and watching are the same switch; the transport
  command `set_watching` posts to `/api/controls/viewing`.
- Proposed change: rename the viewing family to watching end to end: the Rust types and fields, the
  route path, the element ids, and the fakes and tests that name the path
  (`crates/orchestrator/src/workbench/web/tests.rs:457`, `594`, `2396` and `2523`;
  `tests/workbench-preview-server.mjs:984`; `tests/workbench-surface.mjs:1193` and `1475`).
- Verdict: confirmed. Both families are as listed, and the page ships inside the service, so the
  route path and the field are no seam. The churn is larger: the ids and `data-viewing` at
  `index.html:272-280`, `tests/workbench-settings.test.mjs` (51, 324-325, 421, 608-643), and
  about twenty lines of core's `tests/workbench-surface.mjs` and
  `tests/workbench-preview-server.mjs` (209, 797-802, 914), so it is cross-lane.
- Class: refactor
- arch-1: none.

## SURF-13: app.js keeps loose state and an order-dependent click handler

- Kind: structure and magic numbers.
- Ratings: payoff medium, risk medium (click dispatch is behavior), effort M.
- Visible text or look: none.
- Where:
  - `crates/orchestrator/ui/app.js:712-718`: seven module-level `let`s for the setup flow
    (`setupPart`, `setupShown`, `setupSkipped`, `setupRequested`, `setupRescanned`,
    `setupComplete`, `setupTurn`).
  - `app.js:886-931`: one click listener runs 15 `closest()` lookups in a row. Only two branches
    return, so the rest fall through and their order matters; the comment at `app.js:902-903`
    exists to explain one such ordering.
  - Unnamed timings: the 140 ms search debounce (`app.js:937`), the 100 ms and 15 s clock ticks
    (`app.js:1014-1019`), and `WORKING_LATCH_MS + 60` (`crates/orchestrator/ui/lib/store.js:80`).
  - `app.js:281` bounds the buffer of changes that arrive before the first snapshot with
    `FEED_LIMIT`, an unrelated limit.
- Proposed change: one small setup-flow object; a table of selector and handler pairs where the
  first match wins; named timings and a named bound for the early buffer.
- Verdict: partial. The seven `let`s, the two returns, the comment and the timings hold. Count:
  the listener runs 14 `closest()` lookups (`app.js:891-930`), not 15. A first-match table has to
  keep today's order and the confirmation's conditional return (`app.js:891-892`).
- Class: refactor
- arch-1: none.

## SURF-14: The desktop contract's names drifted between its two copies

- Kind: naming.
- Ratings: payoff low, risk low, effort S.
- Visible text or look: none.
- Where: the shell's copy of the desktop frames uses `View` (`crates/desktop/src/link.rs:369`,
  whose doc says "The words are the service's `ShellView`") and `MAX_TOOLTIP_LEN`
  (`crates/desktop/src/tray.rs:52`) for the service's `ShellView`
  (`crates/orchestrator/src/intake/desktop.rs:136`) and `MAX_TOOLTIP_LENGTH`
  (`intake/desktop.rs:41`).
- Problem: the copy is deliberate and held together by `tests/golden/desktop/`, but a search for one
  side's name does not find the other's.
- Proposed change: use the same names on both sides.
- arch-1: V-10 kept the two copies ("Moving the desktop frames into `relay`" is under Considered and
  not taken), and S7 (`todo`) makes the shell decode and re-encode every desktop golden. The name
  drift is new here.
- Verdict: confirmed. These two are the only drifted names: the frame variants,
  `UnavailableReason`, `NativeAct`, `ActOutcome`, `TrayItemId`, `TrayItem`, `TrayState` and
  `TrayModel` match on both sides. No wire word changes, so no desktop golden moves.
- Class: refactor
- arch-1: none. S7 (`todo`) sits beside it, and the rename keeps V-10's two copies, so it reopens
  nothing.

## Overall

The web layer (`routes.rs`, `http.rs`, `events.rs`, `limits.rs`, `assets.rs`) and the desktop
shell are disciplined: closed tables, named constants and module docs. The debt sits where a
vocabulary crosses Rust and JavaScript: pages, activities, phases, limits and colours are written
on both sides and kept in step by hand or by tests that scan the JavaScript source. The structural
change that helps most is SURF-2, which ADR-0203 already decides as S3: make the parts the single
read model, built once and carrying typed tokens (`PageAddress`, the activity code) that the page
keys on. SURF-1, SURF-4 and SURF-6 then become mostly deletions, and SURF-7, the split of
`view.js`, is the matching change on the page's side and the largest finding no slice covers yet.

## Skeptic summary

- Confirmed: 5 (SURF-1, SURF-6, SURF-8, SURF-12, SURF-14). Partial: 9 (SURF-2, SURF-3, SURF-4,
  SURF-5, SURF-7, SURF-9, SURF-10, SURF-11, SURF-13). Refuted: 0.
- Most corrections are counts and dependency lists. Two go against the opening's claim that only
  SURF-11 touches appearance: SURF-4's keying changes some waiting rows' medallion, and SURF-5's
  body-cap fix changes which drafts are accepted and which sentence a person reads. SURF-9's
  rename meets the extension freeze. Nothing here touches `crates/relay` or `crates/edge`.
- First:
  1. SURF-5's body cap. It is the one defect here a person can meet: the stated limit cannot be
     reached, and an oversized draft gets a vaguer sentence. It is small, with one web test at
     the limit and one byte over; pin the search `maxlength` and the coverage cap in the same
     pass.
  2. SURF-2, as S3. It is decided, unblocked since P5, I6 and C2 landed, and it deletes the most
     dead work: an overview that scans history, six booleans and a timestamp on every snapshot,
     and the second `ConnectionsPart` constructor.
  3. SURF-8. A trust gate is kept in step by hand in three places, four with the test. Deriving
     the route gate from `availability` stops the drift a new act would bring and changes no
     answer.
