# S6: At a glance

## Objective

Make the workbench answer one question well: is this working, and if not, what is the next step. The
window is the confidence surface for a person who does not yet live in the terminal. It is never the
only way to learn anything.

## Read first

- [BOOTSTRAP.md](BOOTSTRAP.md) and [PINS.md](PINS.md).
- The ADR S1 wrote, which decided whether this replaces Monitor or joins it as a destination.
- ADR-0102 (integrated desktop workbench) and its amendment, ADR-0119 (durable authority, disposable
  workbench), ADR-0122 (readable policy destination), ADR-0083 (action signature medallions),
  ADR-0089 (destination-aware spatial cues).
- `crates/orchestrator/src/workbench/mod.rs`, `crates/orchestrator/ui/`, `tests/workbench-surface.mjs`.

## Verified facts as of authoring

Confirmed at `2f24943f`. Re-read before relying on any of them.

- The window has four tabs: Monitor, Status, Policy, About. Monitor is the landing surface and
  carries the current action plus a newest-first queue.
- Pause and resume live in the persistent header and match the tray.
- The orchestrator publishes a closed sequenced change vocabulary through a best-effort sink, and
  snapshots carry the sequence they reflect. A surface that receives a gap resynchronizes from a
  fresh snapshot.
- `tests/workbench-surface.mjs` is the surface journey and will need extending, not replacing.

## Required behavior

1. **One orchestrator-owned projection.** The view renders the projection. It does not infer product
   state from process fragments, unrelated fields, or presentation-local booleans.
2. **Lead with the answer.** State readiness first, then only the browser, harness, authority,
   current operation, recent recovery, or next action needed to explain it.
3. **The words match `doctor`.** Every state shown here has the same wording as its `doctor` line
   from S4. Add a guard test that compares the two sources rather than two copies of the text.
4. **Controls are present and delegated.** Pause, resume, and stop appear where they apply and act
   through the S5 owner. Per-session behavior stays unambiguous when more than one session is
   active.
5. **Depth stays deliberate.** Logs, raw audit, paths, protocol revisions, and diagnostics remain
   behind navigation. The surface links to them when a fact needs explaining.
6. **Healthy idle is useful and quiet.** No wall of empty cards, no notification for routine
   success, no ceremony for ordinary recovery.
7. **Remove what this replaces.** Do not leave two status summaries or two control locations. What
   cannot be removed safely in this stage gets one named follow-up in the ledger.
8. **Accessibility is structural.** Keyboard-only operation, accessible names, large text, high
   contrast, and reduced motion all work, and none of them depend on color, animation, pointer use,
   tray presence, or notifications.

## Tests to add

Extend `tests/workbench-surface.mjs` with assertions named for what they prove:

- `"an explicit workbench open lands on the front door"`
- `"the front door states readiness before detail"`
- `"empty, active, paused, recovering, and degraded states each render"`
- `"plural sessions render unambiguous per-session controls"`
- `"the front door renders no page content and no inferred task intent"`
- `"every front-door state uses the same words as its doctor line"`
- `"controls delegate to the runtime owner and compute no state"`

## Verification

    cargo fmt --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    npm test --prefix extension
    node --check crates/orchestrator/ui/app.js
    cargo build --workspace --target-dir .target-ghostlight-1.0
    node tests/workbench-surface.mjs
    node tests/process-journey.mjs

Then open the built workbench on this host and confirm by eye: the landing surface, one keyboard-only
pass, and one reduced-motion pass. Record the host and results in the ledger.

## Out of scope

Recovery behavior, which is S7. Runtime-control semantics, which S5 owns. Any in-page presence. Any
new preference. Any change to the policy destination beyond linking to it.

## STOP preconditions

- JavaScript would have to reconstruct domain state from unrelated fields.
- Technical health would become the primary hierarchy in a healthy state.
- The page would need its own pause or stop semantics.
- Accessibility would depend on color, animation, pointer use, tray presence, or notifications.
- The same words cannot be shared with `doctor` without duplicating them.
