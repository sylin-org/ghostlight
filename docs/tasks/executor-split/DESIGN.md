# DESIGN: splitting `work/mod.rs` by operation family

Normative for this batch. `BOOTSTRAP.md` cites this document instead of restating it; the task
files (`T1`-`T6`) cite the specific sections they need. Where this document and a task file
disagree, re-verify against the live tree before trusting either -- see Provenance.

## Why

`crates/orchestrator/src/work/mod.rs` is the single largest and most over-limit file in the
Ghostlight codebase: 5824 total lines, of which roughly 4200 are production code and the remaining
~1621 are the `#[cfg(test)] mod tests` block. `AGENTS.md`'s coding-style section says "200-400
lines typical, 800 max" per file, extracting utilities from large modules, organized by
feature/domain. At ~4200 production lines, `work/mod.rs` is more than 5x the stated maximum and by
a wide margin the worst violation in the tree (the next-largest production body, in
`governance/mod.rs`, is roughly 2080 lines -- also over limit, but explicitly out of scope for this
batch; see Scope below).

This was flagged, and explicitly deferred, during a 2026-08-15 whole-codebase code-quality pass
("address all findings" -- see that session's fixes to `work/mod.rs` for C4/H3/M2, which land in
the areas this design's method inventory already accounts for). That pass fixed correctness and
test-coverage findings; this batch is the follow-up for the file-size finding it declined to fold
in, on the grounds that a structural refactor deserves its own reviewed, sequenced, oracle-pinned
plan rather than a same-session scope expansion.

## What this is, precisely

A pure code-motion refactor. No behavior changes. No test file is edited. Every method moves
verbatim (same body, same logic) from `work/mod.rs` into a new sibling file under `work/`, as an
additional `impl ApplicationExecutor { ... }` block in that file. The only textual change to a
moved method is its visibility keyword, where pinned below. `work/mod.rs` gains one `mod <name>;`
declaration per new file and loses the lines that moved.

The oracle for correctness is not a hand-written expected string: it is the compiler (this must
still compile, `cargo clippy --all-targets -- -D warnings` must still be clean) and the existing
test suite (every test in `ghostlight`'s lib target must still pass, in the same count, with zero
test-file edits). This is a stronger oracle than a transcribed string would be for a pure move: if
the tree still compiles, still lints clean, and the same 226 tests still pass unmodified, the move
did not change behavior.

## Verified current-tree facts (AS OF AUTHORING, 2026-08-15, branch `dev`)

Verified against the live working tree, **including today's uncommitted code-quality-pass fixes**
(the session that produced C4/H3/M2 and others). Those fixes touch `execute`/`perform_record`/
`stop_recording`/`discard_recording`/the `ActiveAuthorityRegistry` type inside the "stays in
mod.rs" set below, not any method this batch moves or any signature this batch pins. If HEAD has
since moved and `work/mod.rs` no longer matches the signatures pinned in `T1`-`T6`, STOP per each
task's own precondition rather than trusting this document.

- `crates/orchestrator/src/work/mod.rs` is 5824 lines total.
- `impl ApplicationExecutor { ... }` is one single inherent-impl block spanning nearly the whole
  file (opens at line 134), containing every method below plus the ones staying put.
- `ApplicationExecutor`'s fields (`governance`, `workspaces`, `browser`, `presentation`,
  `workbench`, `audit`, `active_authority`, `observations`) are private (no `pub`/`pub(crate)`
  qualifier). This is load-bearing for the plan below: Rust's default privacy rule makes a private
  item visible to its defining module **and that module's descendants**, but not to ancestors or
  siblings without explicit widening. Every method that stays in `work/mod.rs` remains reachable
  from a new `work::<family>` submodule with **zero** visibility change, because `work::<family>`
  is a descendant of `work`. The only methods that need a visibility change are the ones that
  *leave* `work/mod.rs` and are then called *back into* from `work/mod.rs`'s own `run()`/
  `run_without_workspace_lease()`, or from a sibling family module (`sequence()` calls into the
  pointer and forms families directly -- see below). `pub(super)` (equivalent to
  `pub(in crate::work)`) is the correct, minimal widening for both cases: it reaches the parent
  module and the parent's entire subtree, which covers both "called from mod.rs" and "called from a
  sibling".
- The full `cargo test -p ghostlight --lib` run passes at **226 tests, 0 failed** on the current
  tree (includes today's uncommitted fixes). This exact count is the per-task regression gate: a
  count that drops means a test silently stopped compiling/running (a moved `#[test]` fn losing its
  attribute, e.g.), and a count that changes at all outside of an explicit, pinned exception means
  something other than a pure move happened.

### Method inventory: what moves, what stays, and why

Every method signature below was read directly from the live file at the cited line (2026-08-15).
STOP preconditions in each task file restate the exact signature to re-check before editing.

**Stays in `work/mod.rs`** (shared infrastructure, called from more than one family, or the
dispatch spine itself): `CancellationToken`, `ApplicationExecutor` (struct + all 8 fields),
`ActiveAuthorityRegistry` + `register_active_authority` + `deregister_active_authority`,
`ObservationRegistry`, `new`, `active_authority`, `execute`, `finish`, `run`,
`run_without_workspace_lease`, `authorize`, `authorize_commits`, `authorize_tab_close`, `dispatch`,
`target_browser`, `observe`, `take_observation`, `observations`, `compensate_close`,
`credential_handoff` (called from `perform_fill`, `perform_type_text`, `upload_files` -- forms
family -- **and** `save_recording` -- recording family; a genuine cross-family shared call, verified
by grep, not assumed), `resolve_optional_target`, `resolve_target`, `resolve_location`,
`action_success`, `succeeded`, `blocked_at`, `blocked`, `failed`, `unknown`, `protocol_failure`,
`browser_failure`, `workspace_failure`, `emit`, plus the private types `InvocationContext`,
`Terminal`, `ResolvedLocation`, `CloseCompensation`, `Completion`, plus every free (non-method)
helper function at the bottom of the file (`denial_presentation`, `elapsed_ms`,
`operation_requires_workspace_lease`, `permitted`, `recording_facts`, `operation_activity`,
`step_activity`, `routing_refusal`, `operation_browser`, `operation_timeout`, `action_subject`,
`named_key`, `observation_budget_ms`, `load_physical_files`, `media_type`, `readiness`,
`readiness_name`, `observed_from`, `landed`, `observed_host`, `word_count`, `bounded`,
`status_name`, `browser_reason`), plus the entire `#[cfg(test)] mod tests` block (test relocation
is explicitly out of scope for this batch -- see Scope).

**Moves, grouped by family, each group becoming one new file:**

| Family | New file | Methods (in current file order) | Entry points needing `pub(super)` |
|---|---|---|---|
| Reading | `work/reading.rs` | `read_page`(952), `inspect_page`(1006), `find`(1032), `targets_operation`(1060, private helper), `screenshot`(1143) | `read_page`, `inspect_page`, `find`, `screenshot` |
| Navigation | `work/navigation.rs` | `list_tabs`(493), `activate_tab`(535), `open_page`(590), `navigate_page`(680), `navigate_history`(747), `reload_page`(790), `complete_navigation`(831, private helper), `close_tab`(903) | `list_tabs`, `activate_tab`, `open_page`, `navigate_page`, `navigate_history`, `reload_page`, `close_tab` |
| Recording | `work/recording.rs` | `perform_record`(2616), `start_recording`(2664), `stop_recording`(2722), `ensure_recording_stopped`(2764), `save_recording`(2791), `recording_destination`(2839), `recording_delivered`(2943), `discard_recording`(2999), `recording_observed`(3045), `recording_selection_failure`(3066), `recording_export_failure`(3087) | `perform_record` only (called from `run` and `run_without_workspace_lease`; every other recording method is called only from within this same family, verified by grep) |
| Pointer | `work/pointer.rs` | `perform_click`(1214), `perform_scroll`(1318), `set_zoom`(1403), `resize_window`(1458), `perform_hover`(1519), `perform_drag`(1804) | all six (`perform_click`, `perform_scroll`, `perform_hover` are also called directly from `sequence()`, which will live in a sibling file -- `pub(super)` covers that too) |
| Forms | `work/forms.rs` | `perform_fill`(1601), `perform_type_text`(1715), `upload_files`(1924), `run_script`(2016), `perform_key`(2108), `perform_wait`(2172) | all six (`perform_fill`, `perform_type_text`, `perform_key`, `perform_wait` are also called directly from `sequence()`) |
| Sequence/dialog/diagnose | `work/sequence.rs` | `sequence`(2256), `handle_dialog`(2437), `diagnose`(2513) | all three |

Line numbers are current-file positions **as of authoring**, used to locate and verify each method
before moving it, not a byte-offset the executor should trust blindly (methods above it may have
already moved in an earlier task; re-locate by name and signature, not by line number, in a tree
already partway through this batch).

### Cross-family coupling verified (not assumed)

- `credential_handoff` (stays in mod.rs) is called from `perform_fill`, `perform_type_text`,
  `upload_files` (forms) and `save_recording` (recording) -- confirmed by grepping every
  `self.credential_handoff(` call site. This is the one piece of shared logic reached from two
  different families; it stays put rather than being duplicated or arbitrarily assigned to one
  family.
- `open_page` (navigation) matches on `CloseCompensation`'s three variants directly
  (`CloseCompensation::Closed | Retained | Unknown`), so `work/navigation.rs` needs
  `use super::CloseCompensation;` by name, not just a call through `self.compensate_close(...)`.
  `complete_navigation` (also navigation, shared by `navigate_page`/`reload_page`) does **not** use
  `CloseCompensation` -- it takes a different recovery path (`lease.hold_tab`) for an
  already-existing tab. Verified by reading both bodies, not inferred from name similarity.
- `perform_click`, `perform_hover`, `perform_drag` (pointer) match on `ResolvedLocation`'s two
  variants (`ResolvedLocation::Target { .. } | Point { .. }`) directly, so `work/pointer.rs` needs
  `use super::ResolvedLocation;` by name.
- `targets_operation` (reading, private helper) is called only from `inspect_page` and `find`, both
  moving into the same `work/reading.rs` -- confirmed by grep, no cross-file split needed for it.
- `complete_navigation` (navigation, private helper) is called only from `navigate_page` and
  `reload_page`, both moving into the same `work/navigation.rs`.
- `ensure_recording_stopped`, `recording_destination`, `recording_delivered`,
  `recording_observed`, `recording_selection_failure`, `recording_export_failure` (recording,
  private helpers) are called only from other recording methods, all moving into the same
  `work/recording.rs` -- confirmed by grep across the whole file, not just within the family's own
  line range.
- `sequence()` constructs a `Click`/`ScrollPage`/`Hover`/`FillForm`/`TypeText`/`PressKey`/`Wait`
  value from each `SequenceStep` variant and calls the matching family entry point directly (e.g.
  `self.perform_click(context, lease, &Click { ... })`), verified by reading the method body. This
  is why `perform_click`, `perform_scroll`, `perform_hover` (pointer) and `perform_fill`,
  `perform_type_text`, `perform_key`, `perform_wait` (forms) all need `pub(super)`: `work::sequence`
  is a sibling of `work::pointer`/`work::forms`, and `pub(super)` (reaching the whole `work`
  subtree) is what makes a sibling-to-sibling call compile.

### Import strategy

Do not use a glob (`use super::*;`). The rest of this codebase's sibling-module split
(`governance/managed/{crypto,bundle,cli,http}.rs`) uses explicit named imports
(`use super::{A, B, C};`), and this batch matches that convention rather than introducing a new
style.

Each new file's import line is **compiler-driven, not hand-enumerated**: start with
`use super::{ApplicationExecutor};` (every moved method needs the type its `impl` block is for)
plus whatever named types this document already pins for that family (`CloseCompensation` for
navigation, `ResolvedLocation` for pointer -- see above), then run `cargo build -p ghostlight` and
add every name the compiler reports as unresolved (`cannot find type/value/function X in this
scope`) to the same `use super::{...}` list, sorted, letting `cargo fmt` normalize the final order.
Do not import from anywhere except `super::` (this file's parent, `work`) and whatever top-level
crates the moved methods reference directly (`serde_json::json`, `ghostlight_bridge::browser::*`,
etc. -- these are already `use`d at the top of `work/mod.rs` today and, being crate-external, need
their own `use` line in the new file exactly as `work/mod.rs` already has them). This is
deterministic, not a judgment call: every name added is a name the compiler said is missing, so
there is no unused-import risk, and no name is added that the compiler didn't ask for.

### `InvocationContext` and `Terminal`

Every moved method takes `&InvocationContext<'_>` and/or returns `Terminal`. Both are private
structs defined in `work/mod.rs` (`InvocationContext` at line 3738, `Terminal` at line 3751 as of
authoring). Every new file needs `use super::{InvocationContext, Terminal};` at minimum, in
addition to `ApplicationExecutor` and whatever else the compiler-driven step above adds.

## Scope

**In scope for this batch:** `crates/orchestrator/src/work/mod.rs` only, per the table above.

**Explicitly out of scope, deferred to a future batch (not authored here):**

- Relocating `work/mod.rs`'s `#[cfg(test)] mod tests` (~1621 lines) alongside the families it
  tests. The tests exercise `execute()` end-to-end, mostly across more than one family in a single
  test (opening a page, then interacting with it, then reading it back), so there is no clean
  per-family boundary to assign them to without either duplicating fixtures or introducing a shared
  test-support module -- a real design question, not a mechanical move, and doing it in the same
  batch as the production-code split would blur which risk caused which failure if something broke.
- Every other file over (or near) the 800-line production-code guideline, reviewed 2026-08-15 at
  the same time as this design and left for a separate batch: `governance/mod.rs` (~2080
  production lines -- the second-worst violation, GovernanceFacade + policy authoring + protected
  destinations), `language/mod.rs` (~1630), `workbench/mod.rs` (~1407), `language/catalog.rs`
  (~1311, mostly static per-tool descriptor data rather than logic), `language/outcome.rs`
  (~1098), `bridge/browser.rs` (~1094, in the `ghostlight-bridge` crate, not `orchestrator`),
  `browser/mod.rs` (~1059), `workspace/mod.rs` (~1077), `install/mod.rs` (~999),
  `install/native_host.rs` (~832, borderline). None of these were structurally investigated deeply
  enough to pin a safe decomposition the way `work/mod.rs` is pinned above; doing so for one file at
  a time, at the same rigor as this document, is the recommended shape for that follow-up work,
  not one combined mega-batch.
- Long individual functions within a file that otherwise stays under the file-size guideline (a
  different code-quality axis from file size) were not inventoried in this pass.

## Provenance

- Line numbers, signatures, and call-graph facts above were read directly from the live working
  tree on branch `dev`, 2026-08-15, via `Read`/`Grep`, not recalled from memory or inferred from
  names. If the tree has since diverged (a task's own STOP precondition fails), re-verify against
  the live file rather than trusting this document -- it is a snapshot, not a live index.
- This batch was authored in response to the 2026-08-15 whole-codebase review pass explicitly
  deferring "large-file/long-function refactors" as out of scope for that pass, at the requesting
  owner's follow-up instruction to turn that deferred item into an executable batch for a smaller
  model.
