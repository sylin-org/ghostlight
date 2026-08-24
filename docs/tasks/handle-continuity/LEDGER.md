# LEDGER -- handle continuity

One task = one logical commit set. This ledger is the authority on progress.

## RESUME HERE

T5 (actionability predicate detail) is in progress. T1-T4 not started; T6-T9 designed and
queued. Order chosen small-to-large: T5, T8, T9, T6, T4, T7, then T1-T3.

## Tasks

### T1 -- idle wake-and-retry

Status: COMPLETE (windows-codex, 2026-08-24). Implemented as a DELETION-plus-window: the
list guard that refused instantly was removed and replaced with `wait_for_any_browser` --
one bounded (2 s, cancellation-aware, deadline-capped) local wait at the chokepoint for a
waking adapter to reattach, then the honest startup-manual refusal. Deviation from the
original sketch: routing through full recovery was rejected because it consults
installed-browser discovery (host-dependent in tests) and can launch applications during a
mere read; the local window achieves the owner's "attempt immediate recovery" without
either. Regression test covers both phases: relay attaches mid-wait -> one succeeded call;
nothing attaches -> honest refusal. Live analog already observed on the dev graph.

Deviations: recovery-by-launch rejected; recovery-by-bounded-wait chosen.

### T2 -- tab-handle continuation

Status: NOT STARTED.

Depends on T1 only in spirit; can run in parallel. Key seams: binding resolution (where
TabUnavailable-class refusals originate), the OpenPage path for governed recreation,
same-handle rebind on recovery, per-tool semantics per BOOTSTRAP D1. New regression tests:
navigate-to-dead-tab recreates and rebinds with repeat_safe false; close-of-dead-tab
succeeds as already-gone; focus-of-dead-tab recreates and brings it forward.

Deviations: none yet.

### T3 -- language and guidance

Status: NOT STARTED.

LANGUAGE.md gains the two-tier handle distinction (identity slots vs perception tokens).
The scripting guide's handle guidance flips from stash-and-hope to selectors plus durable
tab handles.

Deviations: none yet.

### T4 -- stale-target refusals arrive pre-recovered

Status: NOT STARTED. Recon done 2026-08-24, recorded so the next session starts warm:

- The single chokepoint is the executor's `resolve_target` wrapper (work/mod.rs) -- every
  handle-based family passes through it.
- `workspace::TargetState` retains tab handle, generation, locator, credential_class, and
  role -- but NOT the accessible name. T4 therefore requires adding a bounded `name`
  at registration time (register_targets) plus a `stale_target_context(handle)` accessor;
  without the name, role-only re-query matches too much.
- Staleness has three distinct exits inside lease.resolve_target: unknown handle (nothing
  to recover from), tab entry gone, and generation mismatch. Only the last two carry a
  recoverable context; keep unknown-handle refusals undecorated.

Deviations: none yet.

### T5 -- actionability refusals name the failing predicate

Status: COMPLETE (windows-codex, 2026-08-24). Extension-side only. `requireActionable`
names the exact predicate -- display:none, aria-hidden, visibility:hidden, opacity:0,
zero-size -- in the refusal message. Tests pin both a display:none and an aria-hidden case.

`requireActionable` gains predicate-specific reasons: display:none, visibility:hidden,
opacity:0, zero-size, plus the existing disabled/inert wording. Messages remain authored
in the content layer and travel through the typed primitive refusal path.

Deviations: none yet.

### T6 -- wait conditions accept typed semantic selectors

Status: NOT STARTED.

`selector_present` joins text/url conditions for `browser_wait`; evaluation polls through
the existing semantic-target query executor-side (duration waits already run there). No
handle pre-resolution required to wait on something being there.

Deviations: none yet.

### T7 -- uniform workspace tab echo

Status: NOT STARTED.

Every tab-scoped success envelope echoes `"tab": <workspace handle>` under that key.
Additive; existing keys stay so no consumer breaks.

Deviations: none yet.

### T8 -- credential-handoff refusals name the field

Status: COMPLETE (windows-codex, 2026-08-24). All three throw sites route through one
helper whose message names the bounded role plus accessible name/id ("the textbox \"Master
password\""). Travels through BrowserPrimitive detail, which now renders faithfully. The
content-test harness stopped stubbing isCredentialMetadata to false, so the real classifier
is exercised from tests.

The refusal message carries a bounded role + name/id description of the exact control, so
the model can ask the human for one precise thing. Travels through BrowserPrimitive detail
(now rendered faithfully).

Deviations: none yet.

### T9 -- deadline honesty and transparency

Status: COMPLETE (windows-codex, 2026-08-24). Writing this task surfaced a third member of
the dishonest-sentence family: DeadlineBeforeDispatch fell through to the browser-stopped
sentence, and after-dispatch deadlines wore the generic unknown sentence. Now:
DeadlineBeforeDispatch gets "The job ran out of time before reaching the browser." with
facts reason=deadline/phase=before_dispatch; every effect-unknown terminal carries its
phase (after_dispatch vs adapter_reported), and DeadlineAfterDispatch gets its own truthful
sentence while staying Status::Unknown with repeat_safe false. Elapsed/budget numbers are
deliberately deferred -- they need start-time plumbing through InvocationContext and are
recorded here as the follow-up rather than faked.

Deadline variants stop rendering as disconnections (same fallthrough disease class as the
primitive fix). New refusal carries phase (before/after dispatch) plus elapsed and budget
in facts. Oracle pins updated.

Deviations: none yet.

## Evidence

- (appended per task)
