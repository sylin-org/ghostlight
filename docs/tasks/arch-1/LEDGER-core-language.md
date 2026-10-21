# arch-1: core-language ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md), "The second sweep (2026-10-20)". Files:
`language/` apart from `policy.rs`, `deployment.rs`, `workbench.rs`, `setup.rs` and
`readiness.rs`, plus the catalog and result goldens. Machine: test-01 (CachyOS). A slice's
specification is its finding in [sweep-2026-10-20/language.md](sweep-2026-10-20/language.md),
read with its skeptic's corrections, or its entry in
[ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`,
`landed` (with commits), `blocked` (with the reason).

## RESUME HERE

L-D1, L-D2 and L-D3 have landed. Next is L-D4. A slice's hash is filled in by the next slice's commit,
since a rebase before the push changes it.

## Slices

In order. "After" names what a slice waits for.

| Slice | Subject | After | Source | Status |
| --- | --- | --- | --- | --- |
| L-D1 | fix(language): a wheel scroll may leave out its direction, and its amount is checked | - | LANG-3's skeptic note | landed (59c67621) |
| L-D2 | fix(language): a summary with its coverage note stays within its bound, to the character | - | LANG-6 and its note | landed (69e58321) |
| L-D3 | fix(language): one skipped history entry is counted in the singular | - | LANG-9's note | landed |
| L-D4 | fix(language): the handle-like text check matches real `diag_` and `invocation_` ids | - | LANG-2's note | todo |
| L-D5 | fix(language): an inline upload of exactly 5,000,000 bytes is accepted | L-D4 | WORK-1's note; one pub exact counter in `language/`, which core-work's WORK-1 then reads | todo |
| C17 | refactor(language): one wait vocabulary and one coverage predicate | - | ADR-0203 C17, moved from core; LANG-3, WORK-5 and BROW-3's wait parts | todo |
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
