# tests-1: adapter ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). Decision: [ADR-0204](../../adr/0204-meaningful-tests.md).
Machine: test-02 (Bluefin). Paths without a crate are under `crates/orchestrator/src`. Statuses:
`todo`, `landed` (with commits), `blocked` (with the reason).

## RESUME HERE

Opened on 2026-10-21. Every test of `browser/adapter/` was classified first, read-only, against
ADR-0204's four reasons (see Notes); the cut then goes one file per commit. A1 (`wire.rs`), A2
(`keys/tests.rs`) A3 (`world/tests.rs`), A4
(`assets.rs`), A5 (`work.rs`), A6
(`geometry/tests.rs`), A7 (`locator.rs`), A8
(`scope/tests.rs`) and A9 (`tree/tests.rs`) have landed; the plan below names the rest. The protocol 4 goldens and the pins in `protocol4/golden.rs` are contracts both sides
read; keep them.

## Areas

| Area | Tests at opening | Budget | Now |
| --- | --- | --- | --- |
| `browser/adapter/` | 337 | 55 | 209 |

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
| A9 | `protocol4/tree/tests.rs`: the tree the service builds from the extension's inventory | 15 | 2 | contract 2 | landed (this commit) |

## Notes

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
  6 to 1 (the stale-lookup binding is interaction's removed-control case); `link` 20 to 5 (the
  unreadable receipt joins the receipt-mapping table, the notice joins the native rows);
  `connection` 16 to 7 (accepted and refused hellos, one table); `golden.rs` 19 to 10 (one table
  for every closed word a page asset or the extension spells, one for the vocabulary's scalars and
  limits, and the vocabulary halves of the stop-reason, runtime-state and asset-error tests as rows
  of the closed-names test; no golden file changes); translate 99 to 14 (`tests.rs` 4,
  `interaction.rs` 4, `points.rs` 3, `forms.rs` 3, `script.rs` 1, less the composed ceiling, which
  `merge` proves, and the fill's followed handle, which `world` proves). Total 57 with geometry's and tree's extra test: two more overlaps go in later slices.
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
- A1: 24 of `wire.rs`'s 25 tests repeated what `golden.rs` proves against the fixtures both sides
  read (each frame, request, receipt and closed name), or restated a constant. Kept: one test that a
  word a newer extension sends decodes to its type's catch-all, for the four types the golden
  tolerance fixtures do not cover (call failure, recording result, diagnostic entry, export
  delivery). The constants test's two derived checks, that a base64 chunk fits Chromium's 1 MiB
  native message and that a full transfer fits its chunks, are now `const` assertions beside the
  constants, so every build checks them.
