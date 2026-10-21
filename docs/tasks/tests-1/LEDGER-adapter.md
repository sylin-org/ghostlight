# tests-1: adapter ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). Decision: [ADR-0204](../../adr/0204-meaningful-tests.md).
Machine: test-02 (Bluefin). Paths without a crate are under `crates/orchestrator/src`. Statuses:
`todo`, `landed` (with commits), `blocked` (with the reason).

## RESUME HERE

The `browser/adapter/` area is cut: 337 Rust tests at opening, 55 now (24 promise, 16 contract, 15
defect), in slices A1 to A19, all on dev. Every test was classified first, read-only, against
ADR-0204's four reasons (see Notes). Next for this lane: the page assets' node tests, once the owner
sets the JavaScript ceiling. The protocol 4 goldens and the pins in `protocol4/golden.rs` are
contracts both sides read; keep them.

## Areas

| Area | Tests at opening | Budget | Now |
| --- | --- | --- | --- |
| `browser/adapter/` | 337 | 55 | 55 |
| `crates/desktop` | 161 | 20 | 94 |

## Slices

One row per slice: what it cuts, its counts before and after, and what the kept tests protect
(promise, contract, defect, unsafe).

| Slice | Subject | Before | After | Kept for | Status |
| --- | --- | --- | --- | --- | --- |
| A1 | `protocol4/wire.rs`: the golden pins already prove the wire | 25 | 1 | contract 1 | landed (6728d73e) |
| A2 | `protocol4/keys/tests.rs`: what a press, a text fill and a double click send | 30 | 3 | promise 2, contract 1 | landed (82e4df75) |
| A3 | `protocol4/world/tests.rs`: what the service reads from a page's answer | 21 | 4 | promise 2, contract 1, defect 1 | landed (a50e4956) |
| A4 | `protocol4/assets.rs`: the shipped page assets and their frame | 3 | 2 | contract 2 (one also defect 95faf5ee) | landed (008afe1e) |
| A5 | `work.rs`: the command vocabulary's checks become build-time assertions | 3 | 0 | - | landed (355d492c) |
| A6 | `protocol4/geometry/tests.rs`: a view is current only within its tolerances | 17 | 1 | promise 1 | landed (6f414572) |
| A7 | `protocol4/locator.rs`: the handle form the page mints | 6 | 1 | contract 1 | landed (08f65ec6) |
| A8 | `protocol4/scope/tests.rs`: admission, the guard, the settle budget and coverage | 26 | 4 | promise 3, defect 1 | landed (6dcc12bf) |
| A9 | `protocol4/tree/tests.rs`: the tree the service builds from the extension's inventory | 15 | 2 | contract 2 | landed (91439a0f) |
| A10 | `protocol4/merge/tests.rs`: one answer across documents | 37 | 5 | promise 2, defect 3 | landed (72bdfc86) |
| A11 | `protocol4/link/tests.rs`: what the Link promises the executor | 20 | 8 | promise 4, contract 2, defect 2 | landed (e7ab9fd6) |
| A12 | `protocol4/connection/tests.rs`: one browser's connection, from hello to its end | 16 | 7 | promise 4, contract 1, defect 2 | landed (dd3eded6) |
| A13 | `protocol4/golden.rs`: the goldens both sides read, one test per contract | 19 | 4 | contract 4 | landed (2a3a574e) |
| A14 | `protocol4/translate/script.rs`: which scripts `browser_execute` awaits | 2 | 1 | contract 1 | landed (4de312d7) |
| A15 | `protocol4/translate/tests/forms.rs`: a fill's order, its refusals and its partial outcomes | 20 | 3 | promise 2, defect 1 | landed (1a4ccfed) |
| A16 | `protocol4/translate/tests/points.rs`: where a point or a target may land | 22 | 3 | promise 2, defect 1 | landed (f64971b8) |
| A17 | `protocol4/translate/tests/interaction.rs` (and `world`'s removed lookup): input refusals, unknown effects, capture and scripts | 37 + 1 | 4 | promise 3, defect 1 | landed (fa3d1e60) |
| A18 | `protocol4/translate/tests.rs`: no unguarded content, the diagnostics binding and a changed document | 18 | 4 | contract 1, defect 3 | landed (8be0e02c) |
| A19 | `protocol4/tree/tests.rs`: an inventory decodes to a tree or to a refusal | 2 | 1 | contract 1 | landed (fb1b7b29) |
| D1 | `crates/desktop/src/machine.rs`: what the shell starts and when | 37 | 5 | promise 4, defect 1 | landed (60dbba71; its ledger row came in the next commit) |
| D2 | `crates/desktop/src/link.rs`: the frames the service writes, the bearer and the link's life | 26 | 3 | contract 2, promise 1 | landed (e3d4bb73) |
| D3 | `crates/desktop/src/states.rs`: a state screen navigates nowhere else | 9 | 1 | promise 1 | landed (7ce3ebf5) |
| D4 | `crates/desktop/src/lib.rs`: the launch parser's restating tests | 4 | 0 | - | landed (this commit) |

## Notes

- From the coordinator (2026-10-21): `crates/desktop` moves here from the install lane, with its
  budget of 20 (161 tests at the batch's opening, 158 now). Some of its code and tests compile only on
  Windows: send any change there to the coordinator to check on Windows before it lands. Its
  `link.rs` golden test checks the service's longest tray, its bearer and its address with the
  shell's own `validate()`, `well_formed_bearer` and `validate_workbench_url`, against the desktop
  goldens the service writes: keep it. `tray.rs`'s `assert_eq!(MAX_TOOLTIP_LEN, 127)` only restates
  the number and can go.

- `crates/desktop` (from 2026-10-21): classified first, read-only, like the adapter. Only
  `tests/loads.rs` is Windows-only (`#![cfg(windows)]`); its verdicts go to the coordinator, who
  cuts it on Windows.
- D1: kept five: before the shell attaches only a person's launch or the Start button starts the
  service, so no shell undoes a Quit (ACCEPTANCE 13 and gate 1); each goodbye reason decides what
  the shell does (13); a loss without goodbye recovers at most `RECOVER_TRIES` times; a service
  that is shutting down spends no recover try (4e1a8e2a); and nothing starts once the session is
  ending (gate 1). The 32 others were their cases, now named functions, or pinned the patience
  durations, chose failure screens, restated the relay's backoff, a derived `PartialEq` or
  `std::fs::metadata`. The fake world's two failure switches, which only cut tests set, go, and
  the restated recover count and patience name `RECOVER_TRIES` and `RECOVER_PATIENCE`. Its ledger
  row missed 60dbba71 (an edit script failed on a stale anchor while the commit still ran) and
  came in the next commit.
- D2: kept three: every frame the service writes to `tests/golden/desktop` decodes to a frame the
  shell knows, re-encodes to the same JSON and passes the shell's arrival checks (every tray
  validates, the longest included, every item id parses, the workbench's address and bearer pass
  `validate_workbench_url` and `well_formed_bearer`; the coordinator's keep), with the loopback
  address rows and a refused other address as its cases (gate 11); the bearer is never printed,
  in any frame, workbench or error; and the link opens as the desktop shell and ends once, at a
  goodbye or the end of the stream (17ec7571). The golden test no longer restates the service's
  tooltip or item lists, which earlier service changes had to edit. Cut: restated wire words, a
  private bearer shape helper, reader internals, socket writes the goldens prove, and error
  passthroughs. The link test's 200 ms wait for a second end goes: `on_end` is `FnOnce`.
- D3: kept one: a state screen loads its own document and takes its buttons as presses, and
  refuses every other navigation (gate 11). Cut: tests that restated the screens' words, buttons,
  classes and the const document's contents, and an escape test of text that is only ever
  constant. The `SCREENS` list goes with them.
- D4: the four parser tests restated `TRAY_ARGUMENT`, `UPDATING_ARGUMENT` and the parser's own
  match, and one round-tripped through `Launch::argument`, which only that test called; both go.
  A contract gap stays open: the service writes the shell's `--tray` separately
  (`service/presence.rs`), and no test sees both words. The root fix is one constant both crates
  read; a test would only restate it.
- The classification (2026-10-21) read every test with its file, the ACCEPTANCE items and
  `docs/1.0/` contracts it carries and the defects recorded against its file, and gave each a
  verdict and what it protects. Kept tests that restate literals are rewritten to compare against
  the code's constants; groups of tests of one behavior become one table-driven test.
- The plan, per file (before, then kept), with the cross-file folds that bring it to the budget:
  `wire.rs` 25 to 1; `keys` 30 to 3; `world` 21 to 3 (the partly readable lookup becomes a row
  of interaction's refusal table); `assets.rs` 3 to 2; `work.rs` 3 to 0 (translate's reloaded-
  iframe table proves the single-locator rule end to end); `merge` 37 to 5; `scope` 26 to 4
  (the point-routing test is points' all-open row); `tree` 15 to 2 (the inventory's order and
  addressable documents, and a malformed inventory; tree's addressable rows decode the inventory,
  a layer below scope's admission, so they stay in tree);
  `geometry` 17 to 1 (planned as rows of points' refused-point table, kept instead as one table
  of the tolerances, because the points rig would have to script drifted viewports); `locator.rs`
  6 to 1 (the stale-lookup binding is interaction's removed-control case); `link` 20 to 8 (it keeps the notice and the chunked transfer, which interaction's upload test
  only repeated);
  `connection` 16 to 7 (accepted and refused hellos, one table); `golden.rs` 19 to 4, one contract each: every fixture reads and writes as the extension's does
  (requests, receipts, frames, events, world messages), unknown words map safely (tolerance),
  `vocabulary.json` says what the service says (names, scalars, limits), and every closed word a
  page asset or the extension spells is one the service knows; no golden file changes; translate 99 to 14 (`tests.rs` 4,
  `interaction.rs` 4, `points.rs` 3, `forms.rs` 3, `script.rs` 1, less the composed ceiling, which
  `merge` proves, and the fill's followed handle, which `world` proves). Total 55 after the recount at A16: `world`'s removed-lookup test becomes a case of
  interaction's refusal test, interaction's click and hover geometry go (the placement is the one
  points' nested-frames test proves), and `tree`'s two tests become one.
- A2: the 27 cut tests restated the key tables, the modifier masks, the wheel tick, the drag
  stride count and the button map, or were cases of the three kept rules: a press types its text
  unless a shortcut modifier is held (Enter as the carriage return a form submits on), a text fill
  selects with the physical Control-A chord, types each character and commits with Tab, and a
  double click's second press says `clickCount` 2. Expectations now name the module's constants.
- A3: kept, an unreadable page failure is a page exception that may follow an effect; a removed
  handle offers the page's lookup only when the service reads it whole (the partly readable
  lookup, the other stale word and the bare stale locator are its rows, and the test moves into
  interaction's refusal table in that slice); a followed handle is a fact and never crosses the
  seam (3466718c); and the page's `interactive` readiness survives. The 17 cut tests restated the
  constructors' table rows, duplicated the world golden, the vocabulary's pre-effect codes or the
  forms translator, or tested decoder branches. `Message::payload`, which only a cut test read,
  goes.
- A4: kept, the shipped source hashes to the id both suites pin (95faf5ee, LF line endings), and
  `hello_accepted`, which carries the assets and is never chunked, fits one native message. Cut:
  the private include expander's test, which the pinned hash covers, and a source budget no
  component reads (`MAX_SOURCE_BYTES`).
- A5: the single-locator test reproduced finding G1 (a9d40c83), which points' page-export test
  and translate's document-change tests prove end to end. The recording-bytes test asserted what
  the type's shape enforces. The transfer-ceiling check, which lived in a `const` block inside a
  test, is now two module `const` assertions, so every build checks it: the local ceiling stays
  above the transfer one (`work.rs`), and a returned GIF fits one transfer base64-encoded
  (`wire.rs`, beside `TRANSFER_MAX_BYTES`).
- A6: kept, one table derived from `POSITION_TOLERANCE`, `DEVICE_SCALE_TOLERANCE` and
  `ZOOM_TOLERANCE`: a view is current within half its tolerance, stale beyond one and a half, and
  never current when a value is not a number. Cut: the budget and region tests (restated
  literals; the language bounds region input), the struct copies and point arithmetic, and the
  point-routing cases, which points' tests prove through the translator.
- A7: kept, a handle the page mints round-trips through the frame-qualified form, and any other
  text is a stale `unknown` handle before anything is sent. The minted handle now comes from the
  world golden `activate.json`, so a change in the page asset's form fails here. Cut: the
  rescoping tests (every translate test that returns handles covers them; one restated struct
  update) and the stale-lookup binding, which interaction's removed-control test proves through
  the translator.
- A8: kept four, each a table of one behavior: a content call or input reaches only a real
  document the scope allows, a metadata call only one in the tree, and a reused frame id revives
  nothing (document boundary evidence 1); the guard names every document whose change stops the
  work (browser job journeys 15); the job's one settle budget is shared and the agent's choice not
  to settle is kept (3e80fb91); and what a job reports it reached stays within what was admitted
  (document boundary evidence 3). The cut tests were their cases, private helpers, a restated
  constructor, and the point-routing test, which points' all-open row proves through the
  translator; that slice keeps an open page's and a hidden frame's rows. The coverage rows name
  `AssetErrorCode`, not restated words.
- A9: kept, an inventory becomes one tree in depth-first order (out-of-process subtrees under
  their parent, so `nth` and form numbers count in one order), frames the extension cannot see
  are placeholders that are never addressable, and only a real web page is addressable; and an
  inventory that is not one tree, or is over `DOCUMENT_LIMIT`, is refused. Cut: private helpers
  (children, the owner chain, which points' nested-frame test proves), a restated `From`, the
  re-parse of the service's own document list, and the ancestor closure, which scope's guard
  test proves.
- A10: kept five tables, one behavior each: a capped list keeps page content first and counts
  what it left out (a19-inspect-tiers); a form keeps one page-wide number however much fits
  (f7d430f4); a repeated control is numbered once across the page (3fbe9b18); a read across
  documents is one text under one ceiling, in characters (executor and truth gates 7); and the
  page tree stays under one node bound (the same gate). Each of the 32 cut tests is a row of one
  of them, except two trivial empty cases. Area words come from `PageArea`, separators from
  `SECTION_SEPARATOR`.
- A11: kept eight: each native command's primitive and outcome (with the two export-failed rows,
  6734f99b); each receipt code's typed error before and after dispatch (D52); a receipt the
  service cannot trust; an interrupted call forwards its cancel and says before or after dispatch
  (with the deadline, the cancel grace and the Downloads save, a0dfc56b); a written command
  reached the browser and a held effect is never written (b0436754); a notice is a frame never
  awaited (presentation 3 and 6); an effect counts only when an answer shows it ran, after which a
  later failure is unknown (executor 5); and a large request travels as chunks. Cut: a restated
  group-key digest, a spec-name check the golden pins, the recording-id test the request fixtures
  pin, and the tab lookup's private listing; the rest are rows of the kept tests. The deadline
  test's 200 ms lateness bound goes: a deadline only bounds a failure.
- A12: kept seven: the service's answer to a first frame (accepted with the assets, refused with
  why, or closed silently; contracts 5); the extension life each hello names and each departure
  carries (ADR-0203 Decision 1); events through the real reader, attention, and unknown frames
  ignored (executor H8); heartbeats keep liveness and silence fails nothing (contracts 17); an
  undecodable frame ends nothing (P-D5, 583b637a); the end of the stream fails waiting calls and
  detaches; and a publish during attach reaches the connection (4f7b3b69). The heartbeat test's
  two fixed sleeps are now waits on heartbeat counts, with a 500 ms timeout in place of 60 ms, and
  a 10 ms receive on a closed channel is a `try_recv`; ten runs in a row passed. Cut: a writer
  test the link's cancel and notice tests prove through the real path, and a publish test the race
  test repeats.
- A13: no assertion goes. The 19 pins are now named check functions, and four tests, one per
  contract, run them: every fixture reads and writes as the extension's tests read it (requests,
  receipts, frames, events, world messages); the tolerance fixtures decode to their catch-alls;
  `vocabulary.json` says what the service says (closed names, scalars and limits, the longest
  group title, the chunk limits, the unknown dialog type); and every closed word a page asset or
  the extension spells is one the service knows. A failure still names the check that broke. No
  golden file changes, and no active document cited the old names.
- A14: the scan's two tests were one behavior, the wraps and the declines of one rule: a script
  is awaited only when the scan reads it with certainty as one expression, so a wrap never changes
  what a script means. Every case stays, and the wrap is checked against `AWAIT_OPEN` and
  `AWAIT_CLOSE`.
- A15: kept three, each running its cases as named functions so every assertion stays: a fill
  across documents prepares together inside one settle budget, types behind focus and submits last
  (2160ca74); a fill refused before any edit names every field by its place and edits nothing
  (e5641da3); and once a field is edited, anything short of every field retained is partial or
  unknown, never no effect. Cut: a batching detail, defensive decoding of handles work never
  sends, and two followed-handle tests that `world`'s test and the work lane's rebound tests prove.
  The failure sentences are compared against the translator's constants, now `pub(super)`. The
  spent-budget case slept 400 ms against a 300 ms budget; the page now holds the first edit for a
  whole budget from its arrival, after the budget began, so the case no longer depends on how fast
  the call starts. A first version measured from before the call and failed one run in five;
  fifteen runs in a row passed after the fix.
- A16: kept three: a point or target reaches only the admitted document it lands in, checked
  before any input, with ten cases as named functions (document boundary evidence 4, executor 8);
  a target in nested frames is placed through its owners' boxes (journeys 10); and a drag reports
  a handle its preparing hover followed (259241ad). The page-export document change (a9d40c83)
  is now a case of `translate/tests.rs`'s reloaded-iframe test, which calls it. Cut: hover, wheel,
  stride and modifier tests `keys` proves, a viewport-edge failure passed through as is, the
  export's attach call the golden pins, and the mid-drag dialog, which interaction's mid-input
  dialog case proves. Scope's admission test regains the rows A8 left to this slice: a point needs
  routing on a page with a frame the scope cannot read, and an open page routes none.
- A17: kept four, running their cases as named functions: a refusal before any input names its
  cause and sends nothing (executor 6), with the removed control's lookup offered only when the
  service reads it whole (a new case through the translator that replaces `world`'s decoder test,
  as A3 planned); a failure after dispatched input leaves the effect unknown, and a dialog the
  click opened is success (executor 4); a capture refuses a hidden tab first (efe54caf) and
  settles within the job's budget; and a script runs at most once, with L-D6's thrown script as a
  case (28ea74b3). Cut: click, hover and scroll geometry (points' nested-frames test proves the
  placement), guarded-call shapes `tests.rs` and the goldens prove, a modified key `keys` proves,
  region captures (a clip that restates its input, and staleness of a read-only image), the upload
  tests (the link's chunked request proves the chunking), and two script cases of the scanner
  `script.rs` proves.
- A18: kept four: no content reaches a page outside an admitted scope (ADR-0203 Decision 6.6);
  a diagnostics read binds its records only when the scope watches documents (a0dfc56b); a handle
  whose document changed is stale as navigated, with the strict-guard, incomplete-inventory,
  one-locator and page-export cases (fa3bcb02, a9d40c83); and an iframe read reports the tab's url
  and host (fa3bcb02). Cut: the text conditions' race internals, the strict guard's unseen count
  (scope's guard test), admission and coverage (scope's tests), the read and tree merges (merge's
  tables), the recording-start shape (the request golden), and call-position wire fields (points'
  routed-point case).
- A19: `tree`'s two tests become one, as the recount at A16 planned: the inventory decodes to one
  tree or to a refusal, and the refusal cases run as a named function of that test.
- The close: 337 to 55. By kind: 24 promise, 16 contract, 15 defect, 0 unsafe. Kept tests that
  waited on fixed sleeps or tight real-time bounds were rewritten to wait on conditions (the
  heartbeat test in A12, the fill's spent budget in A15, the deadline lateness bound in A11).
  Product code only deleted tests used went (`Message::payload`), test-only budgets became
  build-time assertions beside their constants (A1, A5), and three translator failure sentences
  became `pub(super)` so tests compare against them (A15).
- A1: 24 of `wire.rs`'s 25 tests repeated what `golden.rs` proves against the fixtures both sides
  read (each frame, request, receipt and closed name), or restated a constant. Kept: one test that a
  word a newer extension sends decodes to its type's catch-all, for the four types the golden
  tolerance fixtures do not cover (call failure, recording result, diagnostic entry, export
  delivery). The constants test's two derived checks, that a base64 chunk fits Chromium's 1 MiB
  native message and that a full transfer fits its chunks, are now `const` assertions beside the
  constants, so every build checks them.
- From the language lane's close (2026-10-21; `LEDGER-language.md` Notes): the tier cut order's only
  proof stays in this area. A10 folded it into `protocol4/merge/tests.rs`'s
  `a_capped_list_keeps_page_content_first_and_counts_what_it_left_out`, as an unchanged row of its
  table; keep that test.
