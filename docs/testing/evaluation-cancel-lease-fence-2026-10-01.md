# Shared debugger ownership correction: source evidence

Date: 2026-10-01. Local source only. Branch: `codex/evaluation-cancel-lease-fence`.
Baseline: `d936b708645c3a1b84da1e8c52e1264eff31bb95`.
Exact signed-off candidate, patch and artifact hashes are frozen in
`.tmp/evaluation-cancel-lease-fence-2026-10-01/checkpoint.md` and its evidence manifest.

Architect review withheld the baseline for a demonstrated source ownership gap. A valid evaluator
lease identifies a generation, but Chrome detach releases the entire shared attachment. Recording
capture and beforeunload navigation can independently hold that same generation outside the
evaluator's document guard. Ordinary finally callbacks also released by tab id alone. These are
source counterexamples, not observed native incidents. The marked
[ADR-0191 amendment](../adr/0191-correlated-evaluator-cancellation-and-resource-retirement.md)
records the correction without rewriting the earlier decision.

## Before and after

| Trigger | Baseline | Corrected candidate |
| --- | --- | --- |
| Evaluator Cancel shares an attachment with an ordinary owner | Generation check permits whole-attachment detach and clears both owners | Exclusive ownership is required; shared retirement refuses without detach or count changes |
| Another acquisition is already in setup when retirement begins | No final exclusive-owner check | Serialized setup registers its real lease before retirement rechecks; a new shared owner prevents detach |
| Recording capture or beforeunload navigation is in flight during Cancel | Other owner can lose its attachment | Other owner completes normally; observation refuses with the existing typed cleanup prerequisite |
| Old ordinary finally runs after external/global release and reattachment | Release by tab id can decrement the new generation | Every release consumes its original generation token; stale and duplicate releases cannot change a new owner |
| New lease waits behind an in-flight detach | Its early count can be reset before it is admitted | Its token and count register after serialized setup in the actual attachment generation |
| Capture presentation preparation or restoration fails | An earlier throw can skip lease release | Protected preparation and nested finally still release the exact token |

Retirement checks exactly one live lease before beginning and again at the serialized detach
boundary. Acquisition begun during retirement refuses before setup. Already-running setup can
finish with its own counted lease, which makes retirement refuse. This uses the existing per-tab
queue and bounded cleanup-required fallback; no drain manager or synchronization framework is added.

All ordinary worker callers capture the lease returned by ensureDebugger and pass it to release.
Release validates tab, lifecycle-state identity and generation, consumes the token once and never
falls back to a current tab count. Beforeunload finally also checks its exact watcher and acceptor
owner. Explicit human global release retains its existing all-owner meaning.

## Retired and preserved

Retired: generation-only targeted retirement, ordinary release by tab id, lease counts registered
before setup, unqualified beforeunload watcher/acceptor deletion and capture cleanup paths that could
skip token release. Existing tests retain their assertions and now use the actual returned tokens.
One typing fixture now returns an opaque token instead of void; no behavior coverage was removed.

Preserved: original frozen unknown/repeat-unsafe resolution, exact document/epoch/correlation
identity, original handler/scope settlement requirement, journal-full cleanup, normal fresh-read
admission, independent child receipts and storage health, privacy and no automatic replay. Shared
retirement failure preserves the other live owner and leaves the occupied evaluator scope visible.
Its existing canonical recovery names the human global End/Start prerequisite. No agent toggle,
Runtime.terminateExecution, new protocol/tool/permission, runtime startup or installation change.

No human renderer, shared style, About card, guardian art, navigation or control wording changed.
No new screenshots were captured. Native acceptance at an earlier candidate does not establish
this follow-on's ownership or Chrome-detach behavior.

## Validation actually run

Isolated target: `.target-quiet-trial`. Logs:
`.tmp/evaluation-cancel-lease-fence-2026-10-01/`.

| Gate | Result | Evidence |
| --- | --- | --- |
| Focused debugger/evaluator/focus/typing suite | Pass: 56 tests | Full cases included in extension-tests.log |
| cargo fmt --check | Pass | fmt.log |
| cargo clippy --workspace --all-targets --target-dir .target-quiet-trial -- -D warnings | Pass | clippy.log |
| cargo test --workspace --target-dir .target-quiet-trial | Pass: 615 tests, zero failures or ignored tests | workspace-tests.log |
| npm test in extension | Pass: 393 tests, zero failures or skips | extension-tests.log |
| node tests/workbench-surface.mjs | Pass: 110 assertions | workbench-tests.log |
| cargo build --workspace --target-dir .target-quiet-trial | Pass | build.log |
| node --check on six changed JS files, native probe and counterproof | Pass | final-checks.log |
| Changed-file ASCII and git diff --check | Pass | final-checks.log |

Seven new cases independently count detach calls, page effects, handlers, capture frames and
navigation calls. They cover shared evaluator/ordinary leases, acquisition during retirement,
already-running setup, real worker recording capture during Cancel, real worker beforeunload
navigation during Cancel, old ordinary navigation finally after reattachment, and capture hide/show
failures. The existing in-flight-detach test now requires the newly admitted lease's final release
to detach, proving the actual count rather than only its initial attachment.

The actual-worker fixture runs production functions with the real debugger, document, journal and
evaluator modules. Shared cancellation leaves the independent owner intact, permits its normal
completion and refuses the blocked read without effects. Explicit synthetic global release then
admits a read without replay, with the page counter and draft preserved. Late ordinary navigation
cannot erase a new watcher/acceptor or release its new lease. Failed cleanup, delayed page effects,
new epochs, unrelated workspaces, privacy, optional history and controls remain covered.

An independent counterproof loads the exact baseline lifecycle and the corrected module. Baseline
shared retirement detaches once and invalidates the ordinary owner; corrected retirement detaches
zero times and preserves it. Baseline old ordinary finally detaches a fresh generation; the token
callback preserves it. Baseline queued acquisition remains attached after its final release because
its count was erased; corrected acquisition detaches on final release. The script and output are
`ownership-counterproof.cjs` and `ownership-counterproof.json` in the evidence directory.

The first focused run found one fixture still releasing without a token after an intervening
assertion. Its token was preserved and passed to release. The next focused suite and aggregate
gates passed. No test was deleted or relaxed.

## Remaining acceptance

NOT RUN here: process journeys, real Chromium/native recovery, fresh screenshots, keyboard
acceptance and real predecessor binaries at this corrected commit. Architect and independent
review remain pending. Deployment remains held.

The original d936b708 patch, manifest and native probe are untouched for the parent's independent
review. A separate probe, `.tmp/evaluation-cancel-lease-native.mjs`, is prepared and syntax checked
but not executed. Its default binary directory is the freshly built `.target-quiet-trial/debug`.
Parent owns runtime coordination and exact-candidate acceptance. Actual Chrome behavior is still
a limit: detach acknowledgement must be followed by original handler/scope settlement, and arbitrary
page continuations can still have effects after custody release. No stopped or rollback claim is made.

The working installation, selection, service, data, registrations, policy, adapter grants and
public versions are unchanged. No deployment, push, release or outreach was performed.
