# T1: Extract the reading family into `work/reading.rs`

**Goal.** Move `read_page`, `inspect_page`, `find`, `targets_operation`, and `screenshot` out of
`work/mod.rs` into a new file `work/reading.rs`, verbatim, as an additional
`impl ApplicationExecutor { ... }` block. No behavior change. Normative: `DESIGN.md` in this
directory (method inventory, visibility rule, import strategy); `BOOTSTRAP.md` (procedure, gates).

## Tree facts (AS OF AUTHORING 2026-08-15 -- RE-READ before editing)

All five methods are within the single `impl ApplicationExecutor { ... }` block in
`crates/orchestrator/src/work/mod.rs`. Their signatures, as of authoring:

```rust
fn read_page(
    &self,
    context: &InvocationContext<'_>,
    lease: &WorkspaceLease,
    requested_tab: Option<&str>,
    target: Option<&str>,
    max_chars: usize,
) -> Terminal {

fn inspect_page(
    &self,
    context: &InvocationContext<'_>,
    lease: &WorkspaceLease,
    requested_tab: Option<&str>,
    kind: &str,
    max_items: usize,
) -> Terminal {

fn find(
    &self,
    context: &InvocationContext<'_>,
    lease: &WorkspaceLease,
    requested_tab: Option<&str>,
    text: &str,
    kind: &str,
    max_results: usize,
) -> Terminal {

fn targets_operation(
    &self,
    context: &InvocationContext<'_>,
    lease: &WorkspaceLease,
    requested_tab: Option<&str>,
    capability: Capability,
    command: BrowserCommand,
    noun: TargetNoun,
) -> Terminal {

fn screenshot(
    &self,
    context: &InvocationContext<'_>,
    lease: &WorkspaceLease,
    requested_tab: Option<&str>,
    target: Option<&str>,
    full_page: bool,
) -> Terminal {
```

**STOP preconditions.** If any of these is false when you read the file, STOP and mark BLOCKED:
- Any of the five signatures above does not match what is currently in `work/mod.rs` (name,
  parameter list, or return type differs).
- `targets_operation` is called from anywhere other than `inspect_page` and `find` (re-grep
  `self.targets_operation(` across the whole file; `DESIGN.md` pins it as reading-only, verified,
  but re-verify before trusting a stale pin).

## Edits

1. Create `crates/orchestrator/src/work/reading.rs` with a one-line `//!` module doc comment (your
   own wording, e.g. describing this as the reading/inspection family of the executor).
2. Move the five methods above, verbatim, into a new `impl ApplicationExecutor { ... }` block in
   `work/reading.rs`, in the order listed.
3. Change visibility: `read_page`, `inspect_page`, `find`, `screenshot` become
   `pub(super) fn` (called from `work/mod.rs`'s `run()`). `targets_operation` stays plain `fn`
   (private helper, called only from `inspect_page` and `find`, both now in this same file).
4. Add imports per `DESIGN.md`'s import strategy. Minimum known needs (paths verified against
   `work/mod.rs`'s own top-of-file `use` block as of authoring):
   `use super::{ApplicationExecutor, InvocationContext, Terminal};` plus
   `use crate::governance::Capability;` plus `use crate::language::outcome::TargetNoun;` plus
   `use ghostlight_bridge::browser::BrowserCommand;`. Then build and add whatever else the compiler
   reports missing.
5. Add `mod reading;` to `work/mod.rs`'s module declarations near the top (next to
   `pub mod result;`).
6. Delete the five methods' original text from `work/mod.rs` (they now live only in
   `work/reading.rs`). `work/mod.rs`'s `run()` keeps calling `self.read_page(...)`,
   `self.inspect_page(...)`, `self.find(...)`, `self.screenshot(...)` exactly as before -- no
   change to any call site.

## Verify (literal commands)

```
CARGO_TARGET_DIR=.target-executor-split cargo fmt --check
CARGO_TARGET_DIR=.target-executor-split cargo clippy -p ghostlight --all-targets -- -D warnings
CARGO_TARGET_DIR=.target-executor-split cargo test -p ghostlight --lib
```

The third command's final summary line must read `test result: ok. 226 passed; 0 failed; ...` for
the `ghostlight` lib target specifically (the workspace has other, smaller test binaries too --
`cargo test -p ghostlight --lib` scopes to the one that matters here). Any other count is a failed
gate.

`git diff --stat` must show `work/mod.rs` shrinking by roughly the size of the five moved methods
and `work/reading.rs` as a new file of comparable size, and must not show any file under `tests/`
or any `#[cfg(test)]` block changing anywhere.

## Out of scope (do NOT do these in T1)

- No other family (navigation, recording, pointer, forms, sequence) -- those are T2-T6,
  independent of this one.
- No change to `read_page`/`inspect_page`/`find`/`targets_operation`/`screenshot`'s logic, only
  their file location and, for four of them, their visibility keyword.
- No test relocation or test edits of any kind (see `BOOTSTRAP.md`'s NEVER list).

## Commit

`refactor(work): extract reading family into work/reading.rs` -- `work/mod.rs` and the new
`work/reading.rs` only. Then update the LEDGER.
