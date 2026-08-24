# BOOTSTRAP -- handle continuity

Purpose: three owner-approved adjustments (2026-08-24) so the tooling recovers instead of
ceremonies -- continuation for tab identity, immediate recovery for transient states, and
one-call recreation for dead tabs.

## Authority order

1. AGENTS.md and the current docs/1.0/ contracts.
2. This file and the design decisions recorded in LEDGER.md (they are normative).
3. LEDGER.md progress.

## Design decisions (owner-approved, do not re-litigate)

- D1. Tab handles are durable correlation slots. A `tab_...` handle resolves to current
  reality on every use: alive -> act; gone -> recreate through the governed OpenPage path
  and REBIND THE SAME HANDLE to the new physical tab. Closing an already-gone tab succeeds
  ("That tab was already closed.", effect none). Recovered navigations report
  `repeat_safe: false`, effect applied, and a summary that says plainly what happened.
- D2. Element targets and view handles stay strictly generation-bound. They are perception,
  not identity: a stale element token must force re-inspection rather than allow acting on
  a page the model has not seen. LANGUAGE.md documents the two-tier distinction.
- D3. Transient no-browser states get one bounded immediate wake-and-retry inside the port
  seam before any refusal. The honest refusal remains the fallback after recovery fails.
- D4. Recovery always routes through existing governed machinery (OpenPage landing rules,
  policy, audit). Recovery never bypasses or duplicates a decision path.

## Ground rules

- One task = one logical commit set, gates green per commit (Rust: fmt/clippy/test;
  extension: npm test + node --check; live proofs stated only after required reloads).
- Extension changes are not live until the unpacked adapter is reloaded.
- BLOCKED protocol: revert, record the reason in LEDGER.md, stop.
- Boundaries: no main merge, tag, publish/store action, or added network behavior.
- Windows live swaps via scripts/dev-loop.ps1 only.

## Task sequence

1. T1 idle-wake-and-retry at the port chokepoint (+ regression test simulating an idle
   relay that returns on the second attempt).
2. T2 tab-handle continuation: dead-binding detection, governed recreation, same-handle
   rebind, per-tool semantics (navigate/focus recover; close succeeds as already-gone),
   `repeat_safe:false` on recovered work (+ tests).
3. T3 documentation: LANGUAGE.md two-tier handle distinction; scripting guide steers
   drivers toward selectors plus durable tab handles instead of stash-and-hope.
4. T4 stale-target refusals arrive pre-recovered: the refusal carries up to three current
   candidate targets matching the dead handle's role/name, labeled as observations.
5. T5 actionability refusals name the exact failing predicate (display:none,
   visibility:hidden, opacity:0, zero-size, disabled, inert).
6. T6 wait conditions accept typed semantic selectors alongside handles
   (`selector_present`) so waiting never forces pre-resolution.
7. T7 every tab-scoped success envelope echoes the workspace `tab` handle under the same
   key.
8. T8 credential-handoff refusals name the exact field (bounded role + name/id).
9. T9 deadline honesty and transparency: deadline variants stop wearing the browser-stopped
   sentence; refusals carry phase (before/after dispatch), elapsed, and budget.

Standing rule for all of them: every new sentence is authored in the language layer with a
pinned oracle; delight never bends truth.

## Environment

Windows development graph deploys via dev-loop; Linux verification can reuse the
coordination lane once the batch is gated.
