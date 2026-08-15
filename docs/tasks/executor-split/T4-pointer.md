# T4: Extract the pointer family into `work/pointer.rs`

**Goal.** Move `perform_click`, `perform_scroll`, `set_zoom`, `resize_window`, `perform_hover`, and
`perform_drag` out of `work/mod.rs` into a new file `work/pointer.rs`, verbatim. No behavior
change. Normative: `DESIGN.md` (method inventory, visibility rule, import strategy, the
`ResolvedLocation` and `sequence()` coupling facts below); `BOOTSTRAP.md` (procedure, gates).

## Tree facts (AS OF AUTHORING 2026-08-15 -- RE-READ before editing)

```rust
fn perform_click(
    &self,
    context: &InvocationContext<'_>,
    lease: &WorkspaceLease,
    value: &Click,
) -> Terminal {

fn perform_scroll(
    &self,
    context: &InvocationContext<'_>,
    lease: &WorkspaceLease,
    value: &ScrollPage,
) -> Terminal {

fn set_zoom(
    &self,
    context: &InvocationContext<'_>,
    lease: &WorkspaceLease,
    requested_tab: Option<&str>,
    percent: u16,
) -> Terminal {

fn resize_window(
    &self,
    context: &InvocationContext<'_>,
    lease: &WorkspaceLease,
    requested_tab: Option<&str>,
    width: u32,
    height: u32,
) -> Terminal {

fn perform_hover(
    &self,
    context: &InvocationContext<'_>,
    lease: &WorkspaceLease,
    value: &Hover,
) -> Terminal {

fn perform_drag(
    &self,
    context: &InvocationContext<'_>,
    lease: &WorkspaceLease,
    value: &Drag,
) -> Terminal {
```

**Cross-family coupling facts (from `DESIGN.md`, restated because they change what you import and
why the visibility below is `pub(super)` for all six, not just the ones `run()` calls directly):**
- `perform_click`, `perform_hover`, and `perform_drag` match on `ResolvedLocation`'s two variants
  directly (`ResolvedLocation::Target { .. } | Point { .. }`), so `work/pointer.rs` needs
  `use super::ResolvedLocation;` by name. `ResolvedLocation` itself, and the `resolve_location`
  method that produces it, both stay in `work/mod.rs`.
- All six methods are called from `work/mod.rs`'s `run()`. In addition, `perform_click`,
  `perform_scroll`, and `perform_hover` are called a **second** way, directly from `sequence()`
  (moving to `work/sequence.rs` in T6, independent of this task), which constructs a `Click`/
  `ScrollPage`/`Hover` value from a `SequenceStep` and calls the matching method. This is why all
  six need `pub(super)`, not only the three `sequence()` happens to reuse -- and why this task has
  no ordering dependency on T6: `pub(super)` visibility reaches `work::sequence` regardless of
  whether T6 has run yet.

**STOP preconditions.** If any of these is false when you read the file, STOP and mark BLOCKED:
- Any of the six signatures above does not match what is currently in `work/mod.rs`.
- `perform_click`, `perform_hover`, or `perform_drag` no longer matches on `ResolvedLocation`
  variants, or matches on additional variants not named above (`Target`, `Point`).

## Edits

1. Create `crates/orchestrator/src/work/pointer.rs` with a one-line `//!` module doc comment.
2. Move the six methods above, verbatim, into a new `impl ApplicationExecutor { ... }` block, in
   the order listed.
3. Change visibility: all six become `pub(super) fn` (see the coupling fact above -- every one of
   them is called from outside this file).
4. Add imports. Minimum known needs: `use super::{ApplicationExecutor, InvocationContext, Terminal,
   ResolvedLocation};` plus `use crate::language::{Click, Drag, Hover, ScrollPage};`. Then build and
   add whatever else the compiler reports missing.
5. Add `mod pointer;` to `work/mod.rs`'s module declarations near the top.
6. Delete the six methods' original text from `work/mod.rs`. `run()` keeps calling
   `self.perform_click(...)`, `self.perform_scroll(...)`, etc. exactly as before; if T6 has already
   landed, `work/sequence.rs`'s `sequence()` keeps calling the same methods unchanged too.

## Verify (literal commands)

```
CARGO_TARGET_DIR=.target-executor-split cargo fmt --check
CARGO_TARGET_DIR=.target-executor-split cargo clippy -p ghostlight --all-targets -- -D warnings
CARGO_TARGET_DIR=.target-executor-split cargo test -p ghostlight --lib
```

The third command's final summary line must read `test result: ok. 226 passed; 0 failed; ...` for
the `ghostlight` lib target. Any other count is a failed gate.

`git diff --stat` must show `work/mod.rs` shrinking and `work/pointer.rs` as a new file of
comparable size, with no `tests/` file or `#[cfg(test)]` block anywhere in the diff.

## Out of scope (do NOT do these in T4)

- No other family -- T1-T3, T5, T6 are independent of this one.
- No change to any of the six methods' logic, only file location and visibility.
- No test relocation or test edits of any kind.

## Commit

`refactor(work): extract pointer family into work/pointer.rs` -- `work/mod.rs` and the new
`work/pointer.rs` only. Then update the LEDGER.
