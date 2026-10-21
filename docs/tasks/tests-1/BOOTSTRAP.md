# tests-1: meaningful tests

This batch carries out [ADR-0204](../../adr/0204-meaningful-tests.md). The owner set the mandate on
2026-10-21: "meaningful tests, not massive number of tests", with at most 500 Rust tests, a
JavaScript ceiling to follow, and the cut before 2.0.1 in lanes on the test machines.

Ledgers: [language](LEDGER-language.md), [adapter](LEDGER-adapter.md), [work](LEDGER-work.md),
[install](LEDGER-install.md) and [service](LEDGER-service.md).

## What a lane does with an area

1. Read the area's tests, the ACCEPTANCE items and `docs/1.0/` contracts it carries, the goldens it
   reads, and the defects the ledgers and ADRs record against its files (grep `docs/tasks/` and
   `docs/adr/` for the file names).
2. Decide each test under ADR-0204 Decision 1: keep, rewrite or delete. A kept test that is badly
   written (a fixed sleep, a restated literal, unrelated assertions) is rewritten under Decision 2.
3. Cut one module or file per commit. Delete the helpers, fixtures and `#[cfg(test)]` code that only
   deleted tests used. The cut changes no product behavior. A test that exposes a product defect
   stops the slice, and the lane tells the coordinator.
4. When a promise or contract has no meaningful test left, write one from the reserve below and
   say so in the ledger.
5. Record each slice's counts before and after in the lane's ledger, and what the kept tests protect
   as counts per kind (promise, contract, defect, unsafe). No essay per test.

Count an area's Rust tests with:

```bash
grep -r --include=*.rs -c "#\[test\]" <paths> | awk -F: '{s+=$2} END {print s+0}'
```

## Lanes, areas and budgets

Paths without a crate are under `crates/orchestrator/src`. Counts were taken at `e2e3a313`, with
2,851 Rust tests in the workspace.

| Lane | Areas, in order | Tests now | Budget | Machine | Ledger |
| --- | --- | --- | --- | --- | --- |
| language | `language/`; then `workbench/` | 349; 252 | 55; 35 | test-01 (CachyOS) | [LEDGER-language.md](LEDGER-language.md) |
| adapter | `browser/adapter/` | 337 | 55 | test-02 (Bluefin) | [LEDGER-adapter.md](LEDGER-adapter.md) |
| work | `work/` and `workspace/`; then `governance/` | 371; 136 | 55; 40 | test-03 (Alpine) | [LEDGER-work.md](LEDGER-work.md) |
| install | `install/`, `cli/`, `agent/`, `crates/bridge` and `crates/win-peer`; then `crates/desktop` | 626; 161 | 75; 20 | leo-desktop-02 | [LEDGER-install.md](LEDGER-install.md) |
| service | `service/`, `intake/`, `browser/` outside `browser/adapter/`, `presentation/`, `audit/`, the crate's top-level files and `crates/orchestrator/tests`; then `crates/relay` and `crates/edge`; then the guard | 396; 157; 66 | 60; 35; 20 | the coordinator, on the owner's machine | [LEDGER-service.md](LEDGER-service.md) |

The budgets add up to 450. The other 50 are a reserve the coordinator grants for tests the cut finds
missing. On 2026-10-21 the owner gave the coordinator standing leave to grant it for boundary and
contract tests the lanes find missing, telling the owner each grant. Granted so far: 1 for
ACCEPTANCE P3's first real test (work lane), 9 for the service area, whose last 52 tests in
`service/` hold eight rules' last proofs (the owner's own grant), and 3 for the install area (the
install command grammar the scripts call, the install.json the launchers read, and the dev extension
id the native host must match); 37 left. A budget is a ceiling, not a target: an area whose meaningful tests number fewer ends
under it.

The JavaScript ceiling for the extension (553 tests) and the page assets (275) is set after the
Rust cut. The adapter lane then takes the page assets, and the package files go to whoever the
coordinator names.

## Rules

- AGENTS.md and docs/MEMORY.md bind, and ADR-0204 decides. A test's meaning decides what stays.
- Each lane works in its own checkout of `dev`, as arch-1's second sweep does. After a slice, run
  fmt, workspace clippy, the touched crates' tests (`cargo test --workspace` when relay, bridge or
  win-peer changed) and the integrity script. Commit by path with `-s` and the ledger row, run
  `git pull --rebase origin dev`, test again when the rebase brought code, and push. Build in
  `.target-lane-tests-<lane>`. Never stash, amend or `git add -A`.
- A test helper another lane's tests use (the fakes in `browser/adapter/chromium/protocol4/fake.rs`,
  `service/test_home.rs`, the scratch helpers) changes only after a grep across `crates/`, with
  the other lane's uses edited in the same commit while that lane has no open slice in those files.
- A golden both sides read stays, and so do its pin tests. A golden is regenerated only when a kept
  test's fixture set changes on purpose, and its diff is read.
- Journeys and guards under `tests/` are not cut in this batch.
- arch-1: a lane finishes its open arch-1 defect slices first (core-work's W-D4 and W-D5,
  install's I-D4). arch-1's refactor
  slices (package EXT-6, EXT-12 and B-final, install INST-5, core-service C19c-s and the cli
  journey's checks) wait until this batch closes.
- No lane deploys or releases. The cut changes no product behavior; the coordinator deploys `dev`
  at the close and checks it live.

## Close

The batch closes when the workspace holds at most 500 Rust tests, each lane's ledger says what its
kept tests protect, the guard has landed (ADR-0204 Decision 4), CI is green on both systems, and
the coordinator has proposed the JavaScript ceiling to the owner.
