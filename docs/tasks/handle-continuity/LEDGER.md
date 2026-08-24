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

Status: COMPLETE for navigate and close (windows-codex, 2026-08-24). `navigate` by a
handle whose binding is gone recreates the tab through the governed OpenTab path under the
SAME handle (`WorkspaceLease::restore_tab`, preserving the governance hold), reports
" That tab was gone; opened <host> in a new tab." with facts recovered=new_tab and
repeat_safe=false. `close` of an already-gone handle succeeds plainly ("That tab was
already closed.", effect none) without touching the browser. Landing governance, hold-on-
denied-landing, compensation, and events ride the shared settle_opened_tab path verbatim.
Deviation recorded: focus/activate recovery is deferred -- recreating a dead binding for
focus needs a tombstoned last-known URL (bindings are deleted on TabClosed today); add a
bounded tombstone when that follow-up lands. Second deviation: an unknown handle is
indistinguishable from a closed one in v1, so a typo'd handle creates a tab whose summary
makes the recovery visible; tombstones refine this too.

Deviations: two, both recorded above.

### T3 -- language and guidance

Status: NOT STARTED.

LANGUAGE.md gains the two-tier handle distinction (identity slots vs perception tokens).
The scripting guide's handle guidance flips from stash-and-hope to selectors plus durable
tab handles.

Deviations: none yet.

### T4 -- stale-target refusals arrive pre-recovered

Status: COMPLETE (windows-codex, 2026-08-24). `TargetState` retains the bounded accessible
name plus the page-authored role string at registration; `stale_target_context` exposes
them for handles that no longer resolve; `record_stale_candidates` re-queries the live page
by exactly that name+role through the governed read path (authorized, same dispatch seam)
and holds up to three {role,name} candidates for the invocation. `workspace_failure` then
attaches them as `recovery_candidates`. Silence on empty name, governance denial, or query
failure is deliberate. The old "fails before browser dispatch" test was upgraded to the new
contract: exactly one extra crossing (the candidate probe), the click itself never
dispatches, and candidates ride in facts.

Deviations: none.

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

Status: NOT STARTED. Worklist enumerated 2026-08-24 by scanning every `succeeded` call for
a `"tab"` key in its facts window. Sites WITHOUT a tab echo, to be fixed additively (add
`"tab": <workspace handle>`; keep existing keys):

- forms.rs:447
- pointer.rs:211, 307, 362, 423, 506
- reading.rs:172, 287, 426
- navigation.rs:134, 524, 605
- recording.rs:106, 151, 386, 435, 460
- sequence.rs:264, 359

Deliberate exemptions: navigation.rs:84 (list_tabs -- no single tab), recording.rs:435/460
(recording status/discard -- not tab-scoped). Each fix should reuse the already-selected
binding's handle; where only a physical id exists, resolve through the lease first.

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
