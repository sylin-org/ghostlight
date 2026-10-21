# arch-1: core-language ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md), "The second sweep (2026-10-20)". Files:
`language/` apart from `policy.rs`, `deployment.rs`, `workbench.rs`, `setup.rs` and
`readiness.rs`, plus the catalog and result goldens. Machine: test-01 (CachyOS). A slice's
specification is its finding in [sweep-2026-10-20/language.md](sweep-2026-10-20/language.md),
read with its skeptic's corrections, or its entry in
[ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`,
`landed` (with commits), `blocked` (with the reason).

## RESUME HERE

L-D1 to L-D5 and C17 (C17a and C17b) have landed; the coordinator's hold on C17's files is
released. Next is C12b-1. A slice's hash is filled in by the next slice's commit, since a rebase
before the push changes it.

## Slices

In order. "After" names what a slice waits for.

| Slice | Subject | After | Source | Status |
| --- | --- | --- | --- | --- |
| L-D1 | fix(language): a wheel scroll may leave out its direction, and its amount is checked | - | LANG-3's skeptic note | landed (59c67621) |
| L-D2 | fix(language): a summary with its coverage note stays within its bound, to the character | - | LANG-6 and its note | landed (69e58321) |
| L-D3 | fix(language): one skipped history entry is counted in the singular | - | LANG-9's note | landed (09d78bd9) |
| L-D4 | fix(language): the handle-like text check matches real `diag_` and `invocation_` ids | - | LANG-2's note | landed (eff79366) |
| L-D5 | fix(language): an inline upload of exactly 5,000,000 bytes is accepted | L-D4 | WORK-1's note; one pub exact counter in `language/`, which core-work's WORK-1 then reads | landed (b5135a14) |
| C17 | refactor(language): one wait vocabulary and one coverage predicate | - | ADR-0203 C17, moved from core; LANG-3, WORK-5 and BROW-3's wait parts | landed (C17a df5b5923, C17b) |
| C12b-1 | refactor(language): each refusal is typed where it is learned, and its code is written once (the language half) | C17 | ADR-0203 C12b, moved from core | todo |
| LANG-4 | refactor(language): RAWX requirements are read from the directory, failing closed | - | LANG-4 | todo |
| LANG-1 | refactor(language): every bound and default has one home, read by the catalog and the decoder | C12b-1 | LANG-1; a test of every advertised default comes first | todo |
| LANG-2 | refactor(language): one `HandleKind` names every handle prefix | L-D4 | LANG-2; core-work adopts it at the mint sites | todo |
| LANG-3 | refactor(language): the closed fields are enums decoded once | C17 | LANG-3, reopened by Decision 14; its uses in `work/` and `browser/` edit in the same commit | todo |
| LANG-10 | refactor(language): `mod.rs` splits into its module root, operations and decoding, and the catalog mounts plainly | LANG-3 | LANG-10; the move lands alone | todo |
| LANG-5 | refactor(language): one `Tool` table derives the tool lists | LANG-10 | LANG-5; the catalog golden's order stays | todo |
| LANG-7 | refactor(language): repeated sentences and mappings are written once | - | LANG-7 | todo |
| LANG-9 | refactor(language): one phrase module counts, lists and bounds | L-D3 | LANG-9; no word changes | todo |
| LANG-8 | refactor(language): the composition sentences leave `Outcome` | LANG-9 | LANG-8; `StepNotStarted` stays, its sentence is a persisted audit summary | todo |
| LANG-11 | docs(language): the item docs match the code | - | LANG-11 | todo |
| LANG-12 | refactor(language): the small cleanups | - | LANG-12; `invites_control` stays while the window reads it | todo |
| C20 | test(language): the plugin skill names only catalog tools and fields | I9b | ADR-0203 C20, moved from core | todo |

## Notes

- L-D1: the decoder fills a wheel scroll's missing direction with `SCROLL_DIRECTION_DEFAULT`, so
  `work/pointer.rs`'s `expect("language validated direction")` still holds and work/ is untouched.
  A wheel scroll accepts the advertised amount default, which a defaults-filling client sends, and
  refuses any other amount: ticks set how far a wheel scrolls. The wheel branch's required list
  drops `direction`; the union requires only what every branch requires, so the catalog golden
  is unchanged.
- L-D2: `outcome::with_note` joins a summary and one closed note within `SUMMARY_MAX_CHARS`,
  cutting the summary and never the note. The coverage qualifier and both storage notes use it,
  so a qualified summary is at most 500 characters and a storage note reaches 500, not 499 (one
  more character at the bound, model-visible). The storage notes are named constants
  (`CHILDREN_UNSAVED`, `HISTORY_UNSAVED`). When storage fails on a summary already at the bound,
  the storage note wins and can cut the coverage note's words; the result's `coverage` fact still
  carries them. `workbench/mod.rs` still spells the children note (surface, LANG-6's refactor).
- L-D3: one skipped entry reads "1 unreadable history entry was skipped." in the history and
  status detail, as the unconfirmed-receipts sentence already did. Person-facing, on purpose.
  LANG-9 later folds both hand-written plurals into the shared phrase helpers.
- L-D4: each handle prefix in `field_problem.rs` carries its own shape: eight lowercase base32
  characters after `target_`, `tab_`, `view_` and `image_`, 32 lowercase hex digits after
  `invocation_`, and `<counter>_<8 hex>` after `diag_` (`extension/lib/diagnostics.js`). The set
  of prefixes is unchanged; `snapshot_` and `browser_` stay out of page-text replacement until
  LANG-2's `HandleKind` decides each kind's shape.
- L-D5: `language::base64_decoded_len` counts the bytes standard base64 decodes to exactly
  (padding dropped), and the upload decoder sums it, so 5,000,000 bytes pass and 5,000,001 are
  refused. The executor's `decode_inline_files` decodes for real and already accepted the
  boundary. For core-work: `work/forms.rs` keeps its own `base64_decoded_len` until WORK-1 reads
  this one, and the screenshot path estimates the same way the decoder did
  (`work/reading.rs`, `data.len() / 4 * 3`, against `workspace/mod.rs`'s image ceiling).
- C17a: `language/wait.rs` holds `WaitCondition` (all nine, in catalog order, with
  `settles_by_default`, `settles` and `page`) and `PageCondition`, the seven the page evaluates,
  which `BrowserCommand::Observe` now carries (with `tests_absence` and `reads_whole_page`).
  `Wait.condition` and `Postcondition.condition` decode through serde hooks that keep the curated
  refusal words exactly, pinned through `decode`. The catalog builds the wait branches, the
  `visual_settle` description and the expect enum from the types; the golden is byte-identical,
  and `the_wait_catalog_is_unchanged` compares the wait schema without the update variable. A
  duration wait now reads "Waited 1 second on example.com." instead of "The condition was met
  ..." (model-visible on purpose); no result golden holds a wait. Gone: the literals in
  `work/forms.rs` (its three settle rules are `WaitCondition::settles`), `work/documents.rs` and
  the test fake, and the translator's `condition` module with its unknown-condition failure.
  Cross-lane edits, under the coordinator's hold: `work/forms.rs`, `work/documents.rs`,
  `work/mod.rs`, `work/tests/execution.rs` (two test matches), `browser/adapter/work.rs`,
  `protocol4/mod.rs`, `translate/observe.rs`, `translate/tests.rs` and `browser/mod.rs`. The
  duration re-parse in `work/forms.rs` stays for WORK-13's `finish_wait`.
- C17b: `Coverage::withholds` (a limit on the requested scope, or an exclusion elsewhere on the
  page) and `COVERAGE_FACT` live in `language/coverage.rs`. Composition reads a step's coverage
  from `terminal.audit.coverage()`, which the completion path sets with the fact, so its JSON
  parse is gone; the lean view deserializes the block and asks `withholds`, so its field-name
  list is gone (`a_new_limitation_field_reaches_the_lean_view`). One difference: a partial
  coverage object, which no result carries, now stays in the lean view instead of being dropped.
  Cross-lane: `work/composition.rs`, `work/receipt.rs`, `intake/mcp/lean.rs`.
