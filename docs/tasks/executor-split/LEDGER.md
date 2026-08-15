# LEDGER: splitting `work/mod.rs` by operation family

Durable progress. One task = one commit. Update RESUME HERE and add a log entry after each task.

## RESUME HERE

- **Batch authored, READY. Nothing executed yet.** All six tasks (T1-T6) are independent -- see
  `BOOTSTRAP.md` -- and may be run in any order or subset.
- Baseline to reconfirm before starting: `cargo test -p ghostlight --lib` reports
  `226 passed; 0 failed` on the current tree (pinned 2026-08-15 in `DESIGN.md`, includes that day's
  uncommitted code-quality-pass fixes to `work/mod.rs`). If this baseline does not hold when you
  start, STOP and record why before touching anything.

## Task sequence

`T1, T2, T3, T4, T5, T6` -- no ordering dependency; every prefix, subset, or reordering leaves a
green tree. Recommended order (smallest/most self-contained first) is T1, T2, T3, T4, T5, T6.

## Task log

| Task | Commit | Status | Notes |
|------|--------|--------|-------|
| T1 reading | -- | READY | -- |
| T2 navigation | -- | READY | -- |
| T3 recording | -- | READY | -- |
| T4 pointer | -- | READY | -- |
| T5 forms | -- | READY | -- |
| T6 sequence | -- | READY | -- |

## Deviations

None yet.

## Research resolution

Not applicable -- this batch is pure internal code motion (no external client/vendor behavior to
verify), authored entirely from the live tree per `DESIGN.md`'s Provenance section.
