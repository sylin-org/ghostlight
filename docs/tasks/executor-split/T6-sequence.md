# T6: Extract sequence/dialog/diagnose into `work/sequence.rs`

**Goal.** Move `sequence`, `handle_dialog`, and `diagnose` out of `work/mod.rs` into a new file
`work/sequence.rs`, verbatim. No behavior change. Normative: `DESIGN.md` (method inventory,
visibility rule, import strategy, the pointer/forms reuse fact below); `BOOTSTRAP.md` (procedure,
gates).

## Tree facts (AS OF AUTHORING 2026-08-15 -- RE-READ before editing)

```rust
fn sequence(
    &self,
    context: &InvocationContext<'_>,
    lease: &WorkspaceLease,
    value: &RunSequence,
) -> Terminal {

fn handle_dialog(
    &self,
    context: &InvocationContext<'_>,
    lease: &WorkspaceLease,
    value: &HandleDialog,
) -> Terminal {

fn diagnose(
    &self,
    context: &InvocationContext<'_>,
    lease: &WorkspaceLease,
    value: &Diagnose,
) -> Terminal {
```

**Cross-family coupling fact (from `DESIGN.md`, restated because it drives most of this file's
imports):** `sequence()` iterates `value.steps: &[SequenceStep]` and, for each step, constructs the
matching per-operation value type and calls that family's entry-point method directly:
`SequenceStep::Click` builds a `Click` and calls `self.perform_click(...)` (pointer, T4);
`SequenceStep::TypeText` builds a `TypeText` and calls `self.perform_type_text(...)` (forms, T5);
`SequenceStep::Fill` builds a `FillForm` (with one `FormField`) and calls `self.perform_fill(...)`
(forms, T5); `SequenceStep::PressKey` builds a `PressKey` and calls `self.perform_key(...)` (forms,
T5); `SequenceStep::Scroll` builds a `ScrollPage` and calls `self.perform_scroll(...)` (pointer,
T4); `SequenceStep::Hover` builds a `Hover` and calls `self.perform_hover(...)` (pointer, T4);
`SequenceStep::Wait` builds a `Wait` and calls `self.perform_wait(...)` (forms, T5). None of this
requires T4 or T5 to have already run: those methods are `pub(super)` (or, if T4/T5 have not yet
landed when you do this task, still plain private methods in `work/mod.rs`, which is equally
visible to this new descendant file) either way, so the calls compile regardless of ordering.

**STOP preconditions.** If any of these is false when you read the file, STOP and mark BLOCKED:
- Any of the three signatures above does not match what is currently in `work/mod.rs`.
- `sequence()` no longer matches all seven `SequenceStep` variants listed above (`Click`,
  `TypeText`, `Fill`, `PressKey`, `Scroll`, `Hover`, `Wait`), or constructs a different value type
  for one of them than named here.

## Edits

1. Create `crates/orchestrator/src/work/sequence.rs` with a one-line `//!` module doc comment.
2. Move the three methods above, verbatim, into a new `impl ApplicationExecutor { ... }` block, in
   the order listed.
3. Change visibility: all three become `pub(super) fn` (all called from `work/mod.rs`'s `run()`).
4. Add imports. Minimum known needs: `use super::{ApplicationExecutor, InvocationContext,
   Terminal};` plus `use crate::language::{
   Click, Diagnose, FillForm, FormField, HandleDialog, Hover, PressKey, RunSequence, ScrollPage,
   SequenceStep, TypeText, Wait};` plus `use crate::language::outcome::Outcome;` plus
   `use result::{Effect, InvocationResult, Status};` (or `use super::result::{...};` -- match
   however `work/mod.rs` itself currently spells this import) plus `use serde_json::json;`. Then
   build and add whatever else the compiler reports missing.
5. Add `mod sequence;` to `work/mod.rs`'s module declarations near the top.
6. Delete the three methods' original text from `work/mod.rs`. `run()` keeps calling
   `self.sequence(...)`, `self.handle_dialog(...)`, `self.diagnose(...)` exactly as before.

## Verify (literal commands)

```
CARGO_TARGET_DIR=.target-executor-split cargo fmt --check
CARGO_TARGET_DIR=.target-executor-split cargo clippy -p ghostlight --all-targets -- -D warnings
CARGO_TARGET_DIR=.target-executor-split cargo test -p ghostlight --lib
```

The third command's final summary line must read `test result: ok. 226 passed; 0 failed; ...` for
the `ghostlight` lib target. Any other count is a failed gate.

`git diff --stat` must show `work/mod.rs` shrinking and `work/sequence.rs` as a new file of
comparable size, with no `tests/` file or `#[cfg(test)]` block anywhere in the diff.

## Out of scope (do NOT do these in T6)

- No other family -- T1-T5 are independent of this one, and this task does not require any of them
  to have run first (see the coupling fact above).
- No change to any of the three methods' logic, only file location and visibility.
- No test relocation or test edits of any kind.
- Do not move `perform_click`/`perform_fill`/etc. themselves here -- they belong to T4/T5, this
  task only moves the method that *calls* them.

## Commit

`refactor(work): extract sequence/dialog/diagnose into work/sequence.rs` -- `work/mod.rs` and the
new `work/sequence.rs` only. Then update the LEDGER.
