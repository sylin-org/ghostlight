# T2: Extract the navigation family into `work/navigation.rs`

**Goal.** Move `list_tabs`, `activate_tab`, `open_page`, `navigate_page`, `navigate_history`,
`reload_page`, `complete_navigation`, and `close_tab` out of `work/mod.rs` into a new file
`work/navigation.rs`, verbatim. No behavior change. Normative: `DESIGN.md` (method inventory,
visibility rule, import strategy, the `CloseCompensation` coupling fact below); `BOOTSTRAP.md`
(procedure, gates).

## Tree facts (AS OF AUTHORING 2026-08-15 -- RE-READ before editing)

```rust
fn list_tabs(&self, context: &InvocationContext<'_>, lease: &WorkspaceLease) -> Terminal {

fn activate_tab(
    &self,
    context: &InvocationContext<'_>,
    lease: &WorkspaceLease,
    requested_tab: &str,
) -> Terminal {

fn open_page(
    &self,
    context: &InvocationContext<'_>,
    lease: &WorkspaceLease,
    url: &str,
) -> Terminal {

fn navigate_page(
    &self,
    context: &InvocationContext<'_>,
    lease: &WorkspaceLease,
    requested_tab: Option<&str>,
    url: &str,
) -> Terminal {

fn navigate_history(
    &self,
    context: &InvocationContext<'_>,
    lease: &WorkspaceLease,
    requested_tab: Option<&str>,
    direction: &str,
) -> Terminal {

fn reload_page(
    &self,
    context: &InvocationContext<'_>,
    lease: &WorkspaceLease,
    requested_tab: Option<&str>,
    bypass_cache: bool,
) -> Terminal {

fn complete_navigation<F>(
    &self,
    context: &InvocationContext<'_>,
    lease: &WorkspaceLease,
    selected: &SelectedTab,
    decision: Decision,
    outcome: Result<BrowserOutcome, BrowserError>,
    make_outcome: F,
    mut facts: Value,
) -> Terminal
where
    F: FnOnce(Option<String>) -> Outcome,
{

fn close_tab(
    &self,
    context: &InvocationContext<'_>,
    lease: &WorkspaceLease,
    requested: &str,
) -> Terminal {
```

**Cross-family coupling fact (from `DESIGN.md`, restated because it changes what you import):**
`open_page` matches `CloseCompensation`'s three variants directly
(`CloseCompensation::Closed | Retained | Unknown`) when a newly-opened tab's landing is denied.
`complete_navigation` does **not** use `CloseCompensation` -- it takes a different recovery path
(`lease.hold_tab`) since it handles navigation of an *already-existing* tab, not a freshly opened
one. `CloseCompensation` itself, and the `compensate_close` method that produces it, both stay in
`work/mod.rs` (shared infrastructure); only the `open_page` arm here needs to name the enum's
variants.

**STOP preconditions.** If any of these is false when you read the file, STOP and mark BLOCKED:
- Any of the eight signatures above does not match what is currently in `work/mod.rs`.
- `complete_navigation` is called from anywhere other than `navigate_page` and `reload_page`
  (re-grep `self.complete_navigation(` across the whole file before trusting the pin above).
- `open_page` no longer matches on `CloseCompensation` variants, or matches on additional variants
  not listed here.

## Edits

1. Create `crates/orchestrator/src/work/navigation.rs` with a one-line `//!` module doc comment.
2. Move the eight items above, verbatim, into a new `impl ApplicationExecutor { ... }` block, in
   the order listed (`complete_navigation` is a generic method, `fn complete_navigation<F>(...)
   where F: FnOnce(Option<String>) -> Outcome { ... }` -- keep its generic parameter and `where`
   clause exactly as they are).
3. Change visibility: `list_tabs`, `activate_tab`, `open_page`, `navigate_page`,
   `navigate_history`, `reload_page`, `close_tab` become `pub(super) fn` (all called from
   `work/mod.rs`'s `run()`). `complete_navigation` stays plain `fn` (private helper, called only
   from `navigate_page` and `reload_page`, both now in this same file).
4. Add imports. Minimum known needs: `use super::{ApplicationExecutor, InvocationContext, Terminal,
   CloseCompensation};` plus `use crate::workspace::SelectedTab;` plus
   `use crate::browser::BrowserError;` plus `use ghostlight_bridge::browser::BrowserOutcome;` plus
   `use crate::governance::Decision;` plus `use crate::language::outcome::Outcome;` plus
   `use serde_json::Value;`. Then build and add whatever else the compiler reports missing.
5. Add `mod navigation;` to `work/mod.rs`'s module declarations near the top.
6. Delete the eight items' original text from `work/mod.rs`. `run()` keeps calling
   `self.list_tabs(...)`, `self.activate_tab(...)`, etc. exactly as before.

## Verify (literal commands)

```
CARGO_TARGET_DIR=.target-executor-split cargo fmt --check
CARGO_TARGET_DIR=.target-executor-split cargo clippy -p ghostlight --all-targets -- -D warnings
CARGO_TARGET_DIR=.target-executor-split cargo test -p ghostlight --lib
```

The third command's final summary line must read `test result: ok. 226 passed; 0 failed; ...` for
the `ghostlight` lib target. Any other count is a failed gate.

`git diff --stat` must show `work/mod.rs` shrinking and `work/navigation.rs` as a new file of
comparable size, with no `tests/` file or `#[cfg(test)]` block anywhere in the diff.

## Out of scope (do NOT do these in T2)

- No other family -- T1, T3-T6 are independent of this one.
- No change to any of the eight methods' logic, only file location and, for seven of them,
  visibility.
- No test relocation or test edits of any kind.

## Commit

`refactor(work): extract navigation family into work/navigation.rs` -- `work/mod.rs` and the new
`work/navigation.rs` only. Then update the LEDGER.
