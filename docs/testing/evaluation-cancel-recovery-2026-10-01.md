# Correlated evaluation cleanup: source evidence

Date: 2026-10-01. Unpublished local source candidate. No installed acceptance claimed.

The architect recommended repairing the existing Cancel lifecycle after independent acceptance
found a pending evaluation held its document scope indefinitely. Leo's approved bounded cleanup
authorizes this follow-on. The decision is
[ADR-0191](../adr/0191-correlated-evaluator-cancellation-and-resource-retirement.md).
Branch: `codex/evaluation-cancel-recovery`. Baseline:
`fcb4bce6489b2e8a538ec32fb294d53e11abefd6`. The exact committed candidate, patch digest,
executable hashes and logs are frozen in
`.tmp/evaluation-cancel-recovery-2026-10-01/checkpoint.md` and its evidence manifest.

## Product before and after

| Trigger | Before | Candidate |
| --- | --- | --- |
| Script changes a counter, then awaits forever | Service freezes an uncertain deadline and sends Cancel; the adapter waits for evaluation before checking its marker, so Read and Inspect keep refusing | Cancel retires the exact evaluator attachment; a fresh read uses normal admission only after its original handler and scope settle |
| Cancel arrives when the attempt journal is full | Cleanup can be rejected as fresh work | Existing Cancel bypasses journal admission and consumes no journal slot |
| Detach fails, times out, or leaves its handler pending | Generic document guidance repeats an unavailable observation | The scope remains occupied; a typed cause names the existing human Status End/Start prerequisite and its global scope |
| Document access is unverified for another reason | Guidance can send the agent straight back into an unavailable read | Guidance points to existing Show tab for manual inspection and requires document access to be resolved before automated observation |
| Old work completes after a new epoch or attachment | Unqualified finally, lease release or persistence can touch a newer owner | Exact record, scope, watcher and attachment-generation ownership fences late cleanup and journal completion |
| Page continuation runs after debugger custody is released | Cleanup could be mistaken for stopping or rolling back the script | Original unknown effects and unsafe repeat remain frozen; delayed page effects never cause replay or a stopped claim |

The adapter keeps one concrete evaluator resource owner. It binds the captured service epoch,
correlation, physical tab, exact document context, watcher and debugger lease. One teardown helper
coordinates deduplicated targeted retirement and the original handler's finally. It directly
detaches that lease's attachment, then waits at most one second for retirement, handler completion
and document-scope settlement. It never erases an occupied context or uses Runtime.terminateExecution.
Fresh scope admission revalidates inventory and checks again after awaited discovery.

Cleanup failure is a closed browser cause, not a raw exception interpreted by presentation.
Work freezes it with no requested effects. Language owns the sentence and recovery used by both
caller and history. The human fallback says End session blocks new requests across all sessions,
Start session permits new work without replay, and neither control proves earlier page activity
stopped. There is no agent control toggle, automatic retry or new permission.

## Retired and preserved

Retired: cancellation checked only after evaluation returns, unqualified script watcher deletion,
unqualified script lease release, Cancel journal admission, ambiguous occupied-scope refusal and
the direct unavailable-observation loop. No second evaluator completion path remains.

Preserved: existing wire Cancel, shared executor, normal document admission, exact workspace and
physical identity, original frozen unknown/repeat-unsafe receipt, direct composition receipts,
privacy, optional history metadata, audit bytes, healthy-call browser round trips and independent
storage health. No new journal record is created for cleanup. Existing ordinary debugger users
retain their lifecycle behavior; evaluator leases add exact attachment-generation ownership.

Existing controls, reconnect readiness, exact Show tab, Pause, Resume, Stop, global scope and
no-replay behavior retain their executable UI coverage. No destination, panel, notification,
queue or automatic hold is added. History search targeting stays deferred.

The complete Sylin About card, lantern art, guardian typography, frame, cyan ornament, version
medallion, ability icons, flavor line, local facts and help links are untouched. No shared style,
About renderer or asset changed in this follow-on. No new screenshots were captured. Native visual
parity at the baseline was independently accepted; new candidate runtime acceptance is pending.

## Tests run on this candidate

Isolated target: `.target-quiet-trial`. Installed executables were not replaced or launched.
Logs are in `.tmp/evaluation-cancel-recovery-2026-10-01/`.

| Gate | Result | Evidence |
| --- | --- | --- |
| cargo fmt --check | Pass | Final formatting check |
| cargo clippy --workspace --all-targets --target-dir .target-quiet-trial -- -D warnings | Pass | clippy.log |
| cargo test --workspace --target-dir .target-quiet-trial | Pass: 615 tests, zero failures or ignored tests | workspace-tests.log |
| npm test in extension | Pass: 386 tests, zero failures or skips | extension-tests.log |
| node tests/workbench-surface.mjs | Pass: 110 assertions | workbench-tests.log |
| cargo build --workspace --target-dir .target-quiet-trial | Pass | build.log |
| node --check on ten changed JS/MJS files and the prepared native probe | Pass | final-checks.log |
| git diff --check and changed-file ASCII check | Pass | final-checks.log |

Nine new worker tests run the production dispatcher, cancellation helpers and browser boundary
with the real evaluator, document, debugger and attempt-journal modules. Independent page-side
counters and handler counts verify read recovery without End/Start, retained draft, one execution,
full-journal cleanup, failed detach, confirmed detach without handler settlement, setup cancellation,
old Cancel and completion across epochs, healthy calls and delayed page continuation after custody
release. The fixture uses a shorter cleanup timer; the production bound remains one second. Fake
debugger callbacks establish ordering and conservative failure behavior, not Chrome performance.

Additional tests cover stale lease release after reattachment, concurrent document admission,
stale cleanup promises, old journal saves/completions, typed browser decoding, no-effect Work
refusal and canonical optional-history round trips. The human recovery appears once with its
global prerequisite and exact technical cause. Existing behavior coverage was kept. The old
generic dialog guidance expectations were updated to require available document access; no
assertions or test cases were removed to obtain passing gates.

## Remaining acceptance and handoff

NOT RUN here: process journeys, actual Chromium/native recovery, installed visual/keyboard
acceptance, real predecessor binaries against this follow-on and exact-candidate independent
architect/reviewer acceptance. Parent acceptance of `fcb4bce` covered 11 native checks and seven
real predecessor-reader phases; it does not establish this candidate's cancellation behavior.

An isolated native regression probe is prepared at `.tmp/evaluation-cancel-native.mjs`, syntax
checked and not executed. It requires fresh `.target-quiet-trial/debug` siblings, checks their
hashes, retains the ordinary controls/About/history/privacy journeys, and now requires the pending
script's read, Inspect and dialog recovery before any explicit End/Start control. Parent owns
runtime coordination and exact-candidate review.

The remaining architectural risk is actual Chrome debugger-detach behavior. A detach acknowledgement
does not admit overlap: the original handler and scope must settle. If cleanup is unconfirmed,
fresh observation refuses without effects and gives the explicit human prerequisite. Arbitrary
page asynchronous work may continue after custody release. This implementation makes no rollback
or stopped-effects claim and does not revise the original uncertain resolution.

Installation, running service, selection, policy, registrations, adapter grants and public versions
are unchanged. No local deployment, release, push or outreach was performed.
