# T5: Extract the forms family into `work/forms.rs`

**Goal.** Move `perform_fill`, `perform_type_text`, `upload_files`, `run_script`, `perform_key`,
and `perform_wait` out of `work/mod.rs` into a new file `work/forms.rs`, verbatim. No behavior
change. Normative: `DESIGN.md` (method inventory, visibility rule, import strategy, the
`credential_handoff` and `sequence()` coupling facts below); `BOOTSTRAP.md` (procedure, gates).

## Tree facts (AS OF AUTHORING 2026-08-15 -- RE-READ before editing)

```rust
fn perform_fill(
    &self,
    context: &InvocationContext<'_>,
    lease: &WorkspaceLease,
    value: &FillForm,
) -> Terminal {

fn perform_type_text(
    &self,
    context: &InvocationContext<'_>,
    lease: &WorkspaceLease,
    value: &TypeText,
) -> Terminal {

fn upload_files(
    &self,
    context: &InvocationContext<'_>,
    lease: &WorkspaceLease,
    value: &UploadFiles,
) -> Terminal {

fn run_script(
    &self,
    context: &InvocationContext<'_>,
    lease: &WorkspaceLease,
    value: &RunScript,
) -> Terminal {

fn perform_key(
    &self,
    context: &InvocationContext<'_>,
    lease: &WorkspaceLease,
    value: &PressKey,
) -> Terminal {

fn perform_wait(
    &self,
    context: &InvocationContext<'_>,
    lease: &WorkspaceLease,
    value: &Wait,
) -> Terminal {
```

**Cross-family coupling facts (from `DESIGN.md`, restated because they change what you can assume
and why the visibility below is `pub(super)` for all six):**
- `perform_fill`, `perform_type_text`, and `upload_files` call `self.credential_handoff(...)`, a
  method that **stays in `work/mod.rs`** (also called from the recording family's
  `save_recording`). This needs no import -- it is a method call through `self`, reachable from
  `work::forms` as a descendant of `work` with zero visibility change. Do not move
  `credential_handoff` or duplicate it into this file.
- All six methods are called from `work/mod.rs`'s `run()`. In addition, `perform_fill`,
  `perform_type_text`, `perform_key`, and `perform_wait` are called a **second** way, directly from
  `sequence()` (moving to `work/sequence.rs` in T6, independent of this task). This is why all six
  need `pub(super)`, and why this task has no ordering dependency on T6.

**STOP preconditions.** If any of these is false when you read the file, STOP and mark BLOCKED:
- Any of the six signatures above does not match what is currently in `work/mod.rs`.
- `perform_fill`, `perform_type_text`, or `upload_files` no longer calls
  `self.credential_handoff(...)`, or a method in this list now calls it that isn't listed here
  (re-grep `self.credential_handoff(` across the whole file before trusting the pin).

## Edits

1. Create `crates/orchestrator/src/work/forms.rs` with a one-line `//!` module doc comment.
2. Move the six methods above, verbatim, into a new `impl ApplicationExecutor { ... }` block, in
   the order listed.
3. Change visibility: all six become `pub(super) fn` (see the coupling fact above -- every one of
   them is called from outside this file).
4. Add imports. Minimum known needs: `use super::{ApplicationExecutor, InvocationContext,
   Terminal};` plus `use crate::language::{FillForm, PressKey, RunScript, TypeText, UploadFiles,
   Wait};`. Then build and add whatever else the compiler reports missing.
5. Add `mod forms;` to `work/mod.rs`'s module declarations near the top.
6. Delete the six methods' original text from `work/mod.rs`. `run()` keeps calling
   `self.perform_fill(...)`, `self.upload_files(...)`, etc. exactly as before; if T6 has already
   landed, `work/sequence.rs`'s `sequence()` keeps calling the same methods unchanged too.

## Verify (literal commands)

```
CARGO_TARGET_DIR=.target-executor-split cargo fmt --check
CARGO_TARGET_DIR=.target-executor-split cargo clippy -p ghostlight --all-targets -- -D warnings
CARGO_TARGET_DIR=.target-executor-split cargo test -p ghostlight --lib
```

The third command's final summary line must read `test result: ok. 226 passed; 0 failed; ...` for
the `ghostlight` lib target. Any other count is a failed gate.

`git diff --stat` must show `work/mod.rs` shrinking and `work/forms.rs` as a new file of comparable
size, with no `tests/` file or `#[cfg(test)]` block anywhere in the diff.

## Out of scope (do NOT do these in T5)

- No other family -- T1-T4, T6 are independent of this one.
- No change to any of the six methods' logic, only file location and visibility.
- No test relocation or test edits of any kind.
- Do not move `credential_handoff` -- it stays in `work/mod.rs`.

## Commit

`refactor(work): extract forms family into work/forms.rs` -- `work/mod.rs` and the new
`work/forms.rs` only. Then update the LEDGER.
