# T3: Extract the recording family into `work/recording.rs`

**Goal.** Move `perform_record`, `start_recording`, `stop_recording`, `ensure_recording_stopped`,
`save_recording`, `recording_destination`, `recording_delivered`, `discard_recording`,
`recording_observed`, `recording_selection_failure`, and `recording_export_failure` out of
`work/mod.rs` into a new file `work/recording.rs`, verbatim. No behavior change. Normative:
`DESIGN.md` (method inventory, visibility rule, import strategy); `BOOTSTRAP.md` (procedure,
gates).

## Tree facts (AS OF AUTHORING 2026-08-15 -- RE-READ before editing)

```rust
fn perform_record(
    &self,
    context: &InvocationContext<'_>,
    lease: Option<&WorkspaceLease>,
    value: &Record,
) -> Terminal {

fn start_recording(
    &self,
    context: &InvocationContext<'_>,
    lease: &WorkspaceLease,
    value: &Record,
) -> Terminal {

fn stop_recording(&self, context: &InvocationContext<'_>, requested: Option<&str>) -> Terminal {

fn ensure_recording_stopped(
    &self,
    context: &InvocationContext<'_>,
    requested: Option<&str>,
) -> Result<PhysicalRecordingSummary, Box<Terminal>> {

fn save_recording(
    &self,
    context: &InvocationContext<'_>,
    lease: Option<&WorkspaceLease>,
    value: &Record,
) -> Terminal {

fn recording_destination(
    &self,
    context: &InvocationContext<'_>,
    lease: Option<&WorkspaceLease>,
    value: &Record,
    stopped: &PhysicalRecordingSummary,
) -> Result<(RecordingDestination, Decision, Option<u64>, usize), Box<Terminal>> {

fn recording_delivered(
    &self,
    context: &InvocationContext<'_>,
    decision: Decision,
    summary: &PhysicalRecordingSummary,
    encoded: EncodedRecording,
    delivery: RecordingDelivery,
) -> Terminal {

fn discard_recording(
    &self,
    context: &InvocationContext<'_>,
    requested: Option<&str>,
) -> Terminal {

fn recording_observed(
    &self,
    context: &InvocationContext<'_>,
    decision: Decision,
    summary: &PhysicalRecordingSummary,
) -> Terminal {

fn recording_selection_failure(
    &self,
    context: &InvocationContext<'_>,
    outcome: BrowserOutcome,
) -> Terminal {

fn recording_export_failure(&self, context: &InvocationContext<'_>, reason: &str) -> Terminal {
```

**Cross-family coupling fact (from `DESIGN.md`, restated because it changes what you can assume):**
`save_recording` calls `self.credential_handoff(...)`, a method that **stays in `work/mod.rs`**
(also called from the forms family's `perform_fill`/`perform_type_text`/`upload_files`). This needs
no import here -- `credential_handoff` is a method call through `self`, reachable from
`work::recording` as a descendant of `work` with zero visibility change, exactly like every other
still-in-`mod.rs` shared helper this family calls (`authorize`, `succeeded`, `blocked`, `failed`,
`browser_failure`, `workspace_failure`, `emit`, the free-function helpers, etc.). Do not add
`credential_handoff` to `work/recording.rs`'s own `impl` block or move it -- it belongs to neither
family alone.

**STOP preconditions.** If any of these is false when you read the file, STOP and mark BLOCKED:
- Any of the eleven signatures above does not match what is currently in `work/mod.rs`.
- Any of `start_recording`, `stop_recording`, `ensure_recording_stopped`, `save_recording`,
  `recording_destination`, `recording_delivered`, `discard_recording`, `recording_observed`,
  `recording_selection_failure`, `recording_export_failure` is called from anywhere **outside**
  this same list plus `perform_record` (re-grep `self.<name>(` for each across the whole file
  before trusting the "recording-internal only" pin from `DESIGN.md`).

## Edits

1. Create `crates/orchestrator/src/work/recording.rs` with a one-line `//!` module doc comment.
2. Move the eleven methods above, verbatim, into a new `impl ApplicationExecutor { ... }` block, in
   the order listed.
3. Change visibility: `perform_record` becomes `pub(super) fn` (called from `work/mod.rs`'s `run()`
   and `run_without_workspace_lease()`). Every other method in this list stays plain `fn` (called
   only from within this same file, per the STOP precondition above).
4. Add imports. Minimum known needs: `use super::{ApplicationExecutor, InvocationContext,
   Terminal};` plus, from `ghostlight_bridge::browser`:
   `BrowserOutcome, EncodedRecording, PhysicalRecordingSummary, RecordingDelivery,
   RecordingDestination` plus `use crate::governance::Decision;` plus `use crate::language::Record;`.
   Then build and add whatever else the compiler reports missing.
5. Add `mod recording;` to `work/mod.rs`'s module declarations near the top.
6. Delete the eleven methods' original text from `work/mod.rs`. `run()` and
   `run_without_workspace_lease()` keep calling `self.perform_record(...)` exactly as before.

## Verify (literal commands)

```
CARGO_TARGET_DIR=.target-executor-split cargo fmt --check
CARGO_TARGET_DIR=.target-executor-split cargo clippy -p ghostlight --all-targets -- -D warnings
CARGO_TARGET_DIR=.target-executor-split cargo test -p ghostlight --lib
```

The third command's final summary line must read `test result: ok. 226 passed; 0 failed; ...` for
the `ghostlight` lib target. Any other count is a failed gate.

`git diff --stat` must show `work/mod.rs` shrinking and `work/recording.rs` as a new file of
comparable size, with no `tests/` file or `#[cfg(test)]` block anywhere in the diff.

## Out of scope (do NOT do these in T3)

- No other family -- T1, T2, T4-T6 are independent of this one.
- No change to any of the eleven methods' logic, only file location and, for one of them,
  visibility.
- No test relocation or test edits of any kind.
- Do not move `credential_handoff` -- it stays in `work/mod.rs` (see the coupling fact above).

## Commit

`refactor(work): extract recording family into work/recording.rs` -- `work/mod.rs` and the new
`work/recording.rs` only. Then update the LEDGER.
