# S5: human runtime control

## Objective

Give pause, resume, and stop one domain meaning across the workbench, the tray, the extension, the
CLI, and every supported MCP era. The orchestrator owns the state; surfaces request it and render it.

This is the largest semantic change in the epic. Read the verified facts before planning it.

## Read first

- [BOOTSTRAP.md](BOOTSTRAP.md) and [PINS.md](PINS.md), including the pinned stop directive.
- The ADR S1 wrote, which decides the questions this stage would otherwise have to guess at.
- ADR-0103 (language-owned outcome voice), ADR-0105 (scripted intake channels), ADR-0106
  (caller-owned sessions), ADR-0113 (end-to-end browser adapter liveness), ADR-0114 (plural browser
  adapters), ADR-0121 and ADR-0122 (policy, including the denial-attention path).
- `crates/orchestrator/src/governance/mod.rs`, `crates/orchestrator/src/work/`,
  `crates/orchestrator/src/workbench/mod.rs`, `crates/orchestrator/src/language/outcome.rs`,
  `extension/popup.js`.

## Verified facts as of authoring

Confirmed at `2f24943f`. Re-read before relying on any of them. **This is not a rename. Today's hold
refuses; the contract may require it to wait.**

- `RuntimeControls` in `crates/orchestrator/src/governance/mod.rs` is one process-global `AtomicU8`.
- `decision()` returns `Decision::deny(ReasonCode::RuntimeHold)` while held. A held operation is
  refused at the final boundary, not suspended.
- The state vocabulary is `Active`, `Held`, `Attention`, `Ended`. `Attention` is reached by the
  repeated-denial path, not only by a human.
- The intent vocabulary is `ToggleHold`, `Hold`, `Resume`, `EndSession`, `StartSession`.
- The workbench renders `Held` as `paused`; the extension popup renders held and attention alike as
  `Agent browsing is PAUSED.`
- The pinned stop directive does not exist anywhere in the tree today.
- ADR-0113 quarantines an operation whose post-dispatch probe goes unanswered at its deadline. A
  waiting operation and a deadline are therefore two mechanisms that must be reconciled, not one.

## Required behavior

1. **One owner, one state machine.** Running, paused, resumed, and stopped transitions live in one
   tested place. No surface computes whether Ghostlight is paused, and no surface authors a sentence
   about it.
2. **Scope is explicit.** Operation, session, and global scopes are defined for plural work. A
   single-session presentation may collapse controls visually and may not create singleton behavior.
3. **Pause prevents the next effect.** An already-dispatched effect settles truthfully as complete,
   partial, or uncertain. Whether the caller is held pending or refused is the decision S1 recorded;
   implement exactly that, including the caller-timeout and disconnect behavior it names, and its
   answer for the ADR-0113 deadline interaction.
4. **Resume revalidates.** Leases, browser generation, transient handles, and authority gates are
   rechecked before the next effect. Resuming never assumes the world stood still.
5. **Stop is terminal and idempotent.** Every affected invocation completes through the typed outcome
   path beginning with the pinned directive, verbatim, followed where necessary by the completed,
   partial, or uncertain effect facts. No automatic retry is recommended after a stop.
6. **`Attention` keeps its meaning.** The repeated-denial path is not collapsed into the human pause
   unless S1's ADR says so explicitly.
7. **Every era agrees.** Older and current MCP clients receive the same semantic result even where
   transport mechanics differ.

## Tests to add

Rust, by name:

- `pause_prevents_the_next_browser_effect`
- `a_dispatched_effect_settles_truthfully_after_pause`
- `resume_revalidates_before_the_next_effect`
- `stop_is_terminal_and_idempotent`
- `stop_outcome_begins_with_the_pinned_directive`
- `stop_recommends_no_automatic_retry`
- `caller_loss_cannot_leave_work_to_continue_later`
- `plural_sessions_scope_controls_unambiguously`
- `held_operation_and_liveness_deadline_agree`

Extension, in `extension/tests/`, by name:

- `"the popup requests control and computes no control state of its own"`

## Verification

    cargo fmt --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    npm test --prefix extension
    cargo build --workspace --target-dir .target-ghostlight-1.0
    node tests/process-journey.mjs
    node tests/workbench-surface.mjs

## Out of scope

Workbench layout and hierarchy, which is S6. Recovery, which is S7. Any new control surface. Any
change to what the extension may decide.

## STOP preconditions

- S1's ADR does not answer the hold-versus-refuse question, the caller-timeout question, or the
  deadline interaction.
- A surface would have to compute whether Ghostlight is paused.
- A connector or the extension would have to author the interruption sentence.
- Supporting one protocol era would weaken another.
- The implementation would need a second execution queue, scheduler, or workflow engine.
- Reconciling a held operation with the ADR-0113 deadline would leave an effect's truth unknown and
  then retry it.
