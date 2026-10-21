# tests-1: adapter ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). Decision: [ADR-0204](../../adr/0204-meaningful-tests.md).
Machine: test-02 (Bluefin). Paths without a crate are under `crates/orchestrator/src`. Statuses:
`todo`, `landed` (with commits), `blocked` (with the reason).

## RESUME HERE

Opened on 2026-10-21. Every test of `browser/adapter/` was classified first, read-only, against
ADR-0204's four reasons (see Notes); the cut then goes one file per commit. A1 (`wire.rs`) has
landed. The protocol 4 goldens and the pins in `protocol4/golden.rs` are contracts both sides
read; keep them.

## Areas

| Area | Tests at opening | Budget | Now |
| --- | --- | --- | --- |
| `browser/adapter/` | 337 | 55 | 313 |

## Slices

One row per slice: what it cuts, its counts before and after, and what the kept tests protect
(promise, contract, defect, unsafe).

| Slice | Subject | Before | After | Kept for | Status |
| --- | --- | --- | --- | --- | --- |
| A1 | `protocol4/wire.rs`: the golden pins already prove the wire | 25 | 1 | contract 1 | landed (this commit) |

## Notes

- The classification (2026-10-21) read every test with its file, the ACCEPTANCE items and
  `docs/1.0/` contracts it carries and the defects recorded against its file, and gave each a
  verdict and what it protects. Kept tests that restate literals are rewritten to compare against
  the code's constants; groups of tests of one behavior become one table-driven test.
- A1: 24 of `wire.rs`'s 25 tests repeated what `golden.rs` proves against the fixtures both sides
  read (each frame, request, receipt and closed name), or restated a constant. Kept: one test that a
  word a newer extension sends decodes to its type's catch-all, for the four types the golden
  tolerance fixtures do not cover (call failure, recording result, diagnostic entry, export
  delivery). The constants test's two derived checks, that a base64 chunk fits Chromium's 1 MiB
  native message and that a full transfer fits its chunks, are now `const` assertions beside the
  constants, so every build checks them.
