# BOOTSTRAP: splitting `work/mod.rs` by operation family

`DESIGN.md` in this directory is normative for facts, method groupings, the visibility rule, and
the import strategy. This file is ground rules and sequencing only; do not restate `DESIGN.md`'s
content here or in a task file, cite it.

All six tasks are authored and READY. Each is a self-contained, independently landable code-motion:
move a named set of methods, verbatim, from `work/mod.rs` into one new sibling file, with the exact
visibility change `DESIGN.md` pins. **The six tasks have no ordering dependency on each other** --
see `DESIGN.md`'s privacy-rule explanation for why a family module can call back into
still-in-`mod.rs` shared infrastructure, and forward into a not-yet-extracted sibling family,
without either side needing to move first. Execute them in the order below for a steadily shrinking
`work/mod.rs` and the lowest-risk-first ordering, but reordering, interleaving, or dropping one task
does not break any other task in this batch.

## Authority order (on conflict, higher wins; an unanticipated conflict = STOP)

1. The live tree (facts). Every task states tree facts AS OF AUTHORING (2026-08-15, branch `dev`,
   including that day's uncommitted code-quality-pass fixes to `work/mod.rs`). **Always re-read the
   named method by name and signature before editing** -- an earlier task in this batch, or
   unrelated concurrent work, may have already changed line numbers.
2. `DESIGN.md` in this directory (method groupings, the visibility rule, the import strategy, the
   verified cross-family coupling facts). Transcribe its pins; do not re-derive them.
3. The task file being executed.

Do not re-litigate the family boundaries or the `pub(super)` visibility rule `DESIGN.md` already
worked out from real grep/read evidence. Do not resolve an unexpected compile error by judgment
beyond the compiler-driven import step `DESIGN.md` describes: if the compiler asks for something
that isn't a plain missing `use`, STOP.

## Environment facts

- Windows 11; repo root `f:\Replica\NAS\Files\repo\github\sylin-org\browser-mcp`; branch `dev`.
- The file being split: `crates/orchestrator/src/work/mod.rs`, in the `ghostlight` package
  (`crates/orchestrator`, binary/lib crate name `ghostlight`).
- **Build/test in an isolated target dir**: prefix every cargo command with
  `CARGO_TARGET_DIR=.target-executor-split` (a live deployment or another workstream's build may
  hold `target/*.exe` open; a plain build can fail to relink and leave a stale binary).
- Gates (ALL must pass before every task's commit):
  1. `CARGO_TARGET_DIR=.target-executor-split cargo fmt --check`
  2. `CARGO_TARGET_DIR=.target-executor-split cargo clippy -p ghostlight --all-targets -- -D warnings`
  3. `CARGO_TARGET_DIR=.target-executor-split cargo test -p ghostlight --lib` -- must report exactly
     `226 passed; 0 failed` (the pinned baseline; see `DESIGN.md`). A different count of any kind is
     a failed gate, not a pass to investigate later.
- ASCII only in code and docs: no em-dashes, arrows, or curly quotes.
- No new module doc comment content is required beyond a one-line `//!` naming the file's role
  (match the existing style, e.g. `crates/orchestrator/src/governance/managed/crypto.rs`'s `//!
  Customer-owned composite signatures for managed policy bundles.`).
- No SPDX header convention is in force in this crate today (`work/mod.rs` itself has none); do not
  add one to the new files either -- match the file you are extracting from, not an unrelated crate.

## Task sequence (recommended order; no hard dependency between tasks -- see above)

| # | File | One-line goal | New file | Depends on | On block |
|---|---|---|---|---|---|
| T1 | T1-reading.md | Extract the reading family | `work/reading.rs` | -- | HALT |
| T2 | T2-navigation.md | Extract the navigation family | `work/navigation.rs` | -- | HALT |
| T3 | T3-recording.md | Extract the recording family | `work/recording.rs` | -- | HALT |
| T4 | T4-pointer.md | Extract the pointer family | `work/pointer.rs` | -- | HALT |
| T5 | T5-forms.md | Extract the forms family | `work/forms.rs` | -- | HALT |
| T6 | T6-sequence.md | Extract sequence/dialog/diagnose | `work/sequence.rs` | -- | HALT |

Every prefix of any ordering of `T1..T6` leaves a coherent, green tree (fmt clean, clippy clean,
226/226 tests passing). Doing only some of these tasks is a valid, complete, shippable partial
result, not a half-finished batch.

## Per-task procedure

1. Re-read `work/mod.rs`. Locate each method the task names by its **name and full signature**
   (given in the task file), not by the line number alone -- line numbers shift as earlier tasks in
   this batch move code out of the file. If a named method's signature does not match what the task
   pins, STOP (the file changed under you in a way this batch did not anticipate).
2. Create the new file named in the task, with a one-line `//!` module doc comment describing its
   role (write your own, matching this crate's existing style; it is not pinned verbatim).
3. Cut the named methods out of `work/mod.rs`, in the order listed, and paste them into a fresh
   `impl ApplicationExecutor { ... }` block in the new file. Apply the exact visibility pinned in
   the task (`pub(super) fn` for entry points; plain `fn` for private-to-this-family helpers) --
   nothing else about each method's signature or body changes.
4. Add the new file's `use` lines per `DESIGN.md`'s import strategy: start with the types
   `DESIGN.md` and the task already name, then run
   `CARGO_TARGET_DIR=.target-executor-split cargo build -p ghostlight` and add every
   compiler-reported missing name to the same `use super::{...}` list (or a top-level crate `use`
   line, matching how `work/mod.rs` already imports that same crate item) until it builds clean.
5. Add `mod <name>;` to `work/mod.rs`'s existing module-declaration block near the top of the file
   (next to `pub mod result;`), keeping the new `mod` line private (no `pub`) unless the task says
   otherwise -- none of these do.
6. Run all three gates from Environment facts. All green, test count exactly `226 passed; 0
   failed`.
7. Confirm the diff is a pure move: `git diff --stat` should show `work/mod.rs` shrinking and the
   new file growing by a comparable amount, and no line inside any `#[cfg(test)]` block anywhere in
   the repository should appear in the diff. If a test file shows up in the diff, STOP -- this batch
   never touches tests.
8. One task = one commit: `refactor(work): extract <family> into work/<file>.rs`. Update the
   LEDGER's RESUME HERE and add a task-log row (numbered deviations, if any).

## Completion criteria

- Per task: all three gates green, the pure-move check in step 7 holds, one commit.
- Batch done (however many of T1-T6 have landed): `work/mod.rs` contains only the "stays" list from
  `DESIGN.md`, each landed family lives in its own `work/<name>.rs` file, `cargo test -p ghostlight
  --lib` still reports `226 passed; 0 failed`, and `cargo clippy -p ghostlight --all-targets -- -D
  warnings` is clean.

## Failure protocol

If a task cannot complete as written: revert its edits (`git checkout -- crates/orchestrator/src/
work/` and delete the new file if created, leaving the tree at the prior commit), mark it BLOCKED in
the LEDGER with the specific reason and the exact tree fact that did not hold, and HALT that task.
Because tasks are independent, a BLOCKED task does not block the others -- continue with the
remaining tasks in the sequence unless the owner says otherwise. Do not improvise around a broken
assumption (an unexpected type error that isn't a plain missing `use`, a method whose signature
doesn't match the pin, a test count that doesn't come back to 226).

## NEVER touch (each NEVER names its one sanctioned exception, if any)

- Any `#[cfg(test)]` block anywhere in the repository. (No exception. Test relocation is explicitly
  out of scope -- see `DESIGN.md` Scope.)
- Any method's logic, control flow, error handling, or field access. This batch changes **only**
  which file a method's text lives in and, for the named entry points, its visibility keyword. (No
  exception.)
- `crates/orchestrator/src/governance/**`, `crates/orchestrator/src/browser/**`,
  `crates/orchestrator/src/workspace/**`, `crates/orchestrator/src/language/**`, `extension/**`, or
  any file this batch does not name. (No exception. `DESIGN.md`'s Scope section lists other
  oversized files on purpose, as a deferred backlog, not as work this batch touches.)
- `work/result.rs` (the sibling `pub mod result;` already declared at the top of `work/mod.rs`).
  (No exception -- it is unrelated to this split.)
- The sacred MCP tool schemas / any tool surface. (No exception -- this batch is entirely internal
  code organization with zero observable behavior change.)
