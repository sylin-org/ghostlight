# ADR-0191: Correlated Evaluator Cancellation and Resource Retirement

Date: 2026-10-01. Status: Accepted for the authorized unpublished local improvement trial.

Amends the adapter cleanup lifecycle in ADR-0169 and the resolution/recovery contract in ADR-0190.
Preserves exact document authority, workspace custody, explicit global controls and no replay.

## Context

Independent candidate acceptance proved actual effects, frozen receipts and predecessor history
readers. It withheld full agent recovery UX acceptance. An evaluation that mutated once then awaited
an unresolved promise returned an uncertain deadline. The service sent its existing correlated
Cancel, but the adapter checked a cancellation marker only after Runtime.evaluate returned.
The scoped handler consequently held the tab's document scope indefinitely. Read, Inspect and
dialog requests shared that guard and received generic DocumentUnavailable guidance to inspect
again. Explicit human End session / Start session released debugger custody and restored a read
without replay or changing the independent counter. This was a baseline mechanism gap.

The architect recommended repairing Cancel's actual lifecycle, with exact resource identity and
bounded failure recovery. A wording-only global-reset suggestion would leave normal recovery broken.

## Decision

1. Retain the existing correlated Cancel command. It disposes of an existing operation and bypasses
   the attempt journal, so journal saturation cannot prevent cleanup. Unknown/early cancellation
   markers remain bounded and a service-epoch transition clears them. Ordinary success adds no
   extra cancellation roundtrip, new tool, service, permission or protocol mechanism.
2. One evaluator resource owner binds service epoch, correlation, physical tab, exact active
   document context, navigation watcher and debugger lease. The lease belongs to one attachment
   generation. Check that identity at the actual evaluation boundary and after awaited work.
3. Cancel coordinates one targeted retirement and the same original finally. Detach only the
   attachment supplying that lease. Confirm detachment and wait for the scoped handler to settle.
   Do not use Runtime.terminateExecution, erase a still-active scope, or overlap unresolved handlers.
4. A new scope may wait for its predecessor's bounded cleanup at the existing admission guard.
   Revalidate its current inventory and exact scope afterward. Recheck ownership after awaited
   inventory discovery. Only the original scope's finally can remove that scope.
5. Late completion/finally is fenced by owner identity and attachment generation. Old work cannot
   delete a newer scope or watcher, decrement its debugger lease, enter a handler after an epoch
   transition, or overwrite a new epoch's same-correlation journal record.
6. Cleanup proves local custody release. It cannot prove rollback or termination of arbitrary
   asynchronous page activity. The already frozen unknown effect and unsafe repeat judgment stay
   unchanged. Fresh reads cross normal admission and never replay the uncertain mutation.
7. A mechanically occupied scope or failed cleanup yields a closed OperationCleanupRequired cause,
   with no effect from the refused request. Work and Language use the existing frozen resolution
   and canonical projection. Its authored recovery names the existing human Status End session /
   Start session route, makes the global scope explicit and does not authorize an agent toggle.
   Generic DocumentUnavailable instead points to existing Show tab for manual inspection and the
   document-access prerequisite. No task intent or cause is inferred from historical adjacency.
8. Keep optional retained metadata and tolerant historical readers. No raw script, page value or
   cleanup error text enters retained history. Do not expand the strict nested composition cause
   vocabulary for this mechanism. The current architecture contract records lease-before-snapshot,
   consuming resolution, fresh child evidence and this cleanup lifecycle.

## Validation and limits

Source tests must independently count effects and handlers. Prove bounded ordinary read recovery
without End/Start, journal-full cleanup, failed cleanup with no observation effect, unrelated tabs,
setup/epoch/attachment fences, stale cleanup ownership and delayed page continuation without a
false stopped claim. Preserve existing evaluator, journal, document, debugger, Work/composition,
privacy, controls and predecessor-compatible optional metadata coverage.

All source gates precede the local commit. Architect and independent review plus actual Chromium
and native acceptance precede deployment. Synthetic debugger callbacks validate resource ordering;
they cannot establish Chrome's installed detach behavior, draft preservation or keyboard activation.
No deployment, public version, publication, new dashboard, notification queue or automatic hold is
authorized by this decision.

## Amendment: shared attachment ownership (2026-10-01)

Architect review withheld source candidate `d936b708`. Its generation check did not establish
exclusive attachment ownership: recording frame capture and beforeunload navigation independently
acquire the same generation outside the evaluator's document guard. Chrome detach would release
their shared attachment too. Ordinary callbacks also released by tab id alone, so an old finally
could decrement a new generation's lease. This was a source counterexample, not a native incident.

Targeted retirement now requires exactly one live lease, checked before beginning retirement and
again at the serialized detach boundary. Another live owner causes cleanup to fail conservatively
and leaves all leases and attachment state untouched. The existing bounded typed prerequisite
applies. No automatic drain, replay, control toggle or new synchronization framework is added.

Acquire registers its token and count only after serialized setup, in the actual attachment
generation. Acquisition begun during retirement refuses before setup. Setup already running can
finish with its own counted lease; retirement must then refuse at its exclusive-owner recheck.
Every ordinary release requires and consumes its original token, and a stale or duplicate token
cannot change current ownership. The beforeunload watcher and acceptor also use exact owner identity
in finally. Recording and screenshot cleanup release their tokens even if presentation cleanup
fails. Ordinary global End session retains its explicit all-owner release semantics.

Discriminating source tests cover shared evaluator/ordinary leases, setup versus retirement,
recording capture and beforeunload navigation during Cancel, old ordinary finally after reattachment,
presentation failures and the count of a lease admitted behind an in-flight detach. Frozen candidate
review and actual Chromium/native acceptance remain prerequisites to deployment.

## Amendment: cooperative observation disposal (2026-10-02)

Independent native review rejected quiet-success candidate `66213c61`. A cancelled read-only
visual observation returned cancelled/none, but its original Chrome scripting callback retained
the document scope for about 19.2 seconds. Active requests, evaluations and navigation watchers
were already zero. The first immediate read still refused with OperationCleanupRequired.
The earlier short-budget component probe waited for scope release before reading and did not
test this recovery promise. Its eventual read was insufficient acceptance evidence.

The existing correlated operation owner now includes Observe alongside Evaluate. Register epoch,
correlation and exact document scope before setup awaits. Bind observation disposal to the captured
page-runtime object and exact original document IDs. Each frame receives an opaque cancellation
token. Page-local AbortControllers are transient disposal handles, not workspace authority.
Bounded early-token markers prevent an already queued observation from starting after Cancel.
They cannot cancel another token or another runtime's observation.

Before disposal awaits, register bounded cleanup through the existing documents.cleaning seam.
Cancel wakes polling sleep and disposes visual ceiling/poll timers, RAF callbacks and abort
listeners. Page animation and arbitrary background code remain outside that disposal. Cancellation
rejects the observation; it cannot become fulfilled, timeout, text-absence success or readiness.
Multi-frame observation waits for every original callback to settle before reporting cancellation.

The existing 1000 ms cleanup bound covers cooperative disposal, the original operation handler
and its scoped finally. Only documents.run's original finally removes its scope. Failed disposal,
runtime replacement or an unsettled Chrome callback leaves the admission guard conservative.
No Promise.race abandons a handler, no scope is erased early, and no debugger detach is used for
Chrome scripting observations. Evaluator exclusive-owner retirement and generation fences remain.

Validation uses a 20000 ms budget and a finite 60000 ms page animation. Record the first next read
before any recovery polling, retry or reset. Count page sensor resources and prove the animation
continues. Source counterexamples cover unsettled original callbacks, failed disposal, shared
debugger custody, multiple frames, runtime/navigation races, old epochs and old finally versus
new same-correlation work. Independent native re-review remains required before deployment.
