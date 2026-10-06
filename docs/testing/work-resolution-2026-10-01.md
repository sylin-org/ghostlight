# Work resolution cleanup: source evidence

Date: 2026-10-01. Unpublished local source candidate. No installed acceptance claimed.

Leo approved the bounded cleanup after the architecture investigation. The decision is
[ADR-0190](../adr/0190-work-owned-resolution-and-real-attempt-journal.md). Branch:
`codex/in-service-outcome-ux`. Baseline: `0f98cc068f495149a075295e687208d91f601189`.
The exact committed candidate and patch digest are in the ignored parent handoff checkpoint:
`.tmp/architecture-investigation-2026-10-01/implementation-checkpoint.md`.

## Product before and after

| Trigger | Before | Candidate |
| --- | --- | --- |
| Script changes state, then throws | A received exception becomes a generic missing-confirmation account | Closed ScriptException cause distinguishes the received error; effects remain unknown and unsafe to repeat |
| Confirmed change, then declared check fails or loses access | Check failure can replace the action account | Primary action and separate NotMet or Unavailable check survive in one resolution |
| Document preparation times out before mutation | Preparation transmission can imply requested effects | Requested handler/effect counts stay zero; the sentence says the action was not sent |
| Repeated keys make progress, then a hold or malformed reply stops work | An acknowledged prefix can disappear | Confirmed counts survive; malformed remainder stays unknown; no further stroke is sent after a denied landing |
| Fresh operation starts its journal save | First saved snapshot can omit the fresh ID | ID is registered before the sole awaited pre-handler save; queued snapshots and acknowledgments stay ordered |
| Full journal rejects a fresh command | Known unsent work reports uncertain effects | Distinct operation_ledger_full code establishes no requested effect; restored uncertainty stays uncertain |
| Human reads a new receipt or uses CLI plain text | Human history infers another account; CLI omits next steps | Stored authored presentation and service-authored CLI text share the same frozen truth and safe recovery |

Work now owns mutable attempt evidence and consumes it into a non-Clone resolution before
completion reactions and audit. Language derives caller, retained metadata and human presentation
in one projection. Exact request identity and admitted document scope validate before confirming
physical effects. Each keyboard landing is governed and committed before the next stroke.
Composition reads child frozen truth directly and preserves ordinary child receipts and storage
health. No client payload supplies retained language.

## Retired and preserved

Retired: Terminal result/audit assembly, CompletionGate result buffer, verification result
overwrites, with_final_effect, with_unmet_expectation, audit-based child-cause inference, duplicate
dispatch error conversion, unused with_authorized_target, duplicate default recovery markup and
summary-only CLI plain rendering. No inactive alternative builder remains.

Preserved: shared executor, original workspace lease and authority snapshot, final live admission,
exact browser/tab/document custody, deadlines and cancellation, composition accumulator and
per-step receipts, privacy, independent history storage and no automatic replay. New optional
resolution data falls back to a named legacy reader when absent or unsupported. Unknown optional
nested data never hides the whole valid record or changes existing JSONL bytes.

Exact tool names remain primary in the existing activity list. One recovery paragraph replaces
duplicate visible guidance; expandable technical depth includes the closed cause, phase,
verification and progress. Pause during reconnect, deliberate human control, exact Show tab,
global control scope and no-replay Resume/Start behavior remain covered. History search targeting
stays deferred. No destination, panel system, notification engine or acknowledgment queue is added.

The About card, lantern art, guardian typography, frame, cyan ornament, version medallion, ability
icons, flavor line, real local facts and help links are unchanged. No shared CSS, About renderer,
image or asset changed. Actual native visual parity is still a parent acceptance task. This source
work captured no screenshots. Earlier accepted native captures document the installed baseline
only; see [activity-list acceptance](activity-tool-column-2026-10-01.md).

## Tests run on this candidate

Isolated target: `.target-quiet-trial`. Executables under installation directories were not replaced
or launched. Evidence logs are in `.tmp/architecture-investigation-2026-10-01/`.

| Gate | Final result | Evidence |
| --- | --- | --- |
| cargo fmt --check | Pass | Final formatting check |
| cargo clippy --workspace --all-targets --target-dir .target-quiet-trial -- -D warnings | Pass | clippy.log |
| cargo test --workspace --target-dir .target-quiet-trial | Pass: 613 tests, zero failures or ignored tests | workspace-tests.log |
| npm test in extension | Pass: 372 tests, zero failures or skips | extension-tests.log |
| node tests/workbench-surface.mjs | Pass: 109 assertions | workbench-tests.log |
| node --check on all eight changed JS/MJS files | Pass | Final syntax check |
| git diff --check; changed-source ASCII check | Pass | Final patch checks |

Eight new executor tests independently count actual synthetic handlers and effects. They cover:
preparation loss; two confirmed keys then an unsent hold; governed per-key landings and generation;
same-variant wrong tab/key/document scope; valid effect followed by workspace update failure;
confirmed prefix followed by wrong-tab or wrong-scope receipt; script false/throw/lost reply; and
acknowledged action followed by unmet or unavailable verification. A false script returns Boolean
false without inventing a business goal. Exception and lost reply each cause one synthetic effect,
remain unsafe to repeat and invoke no second handler.

The 12 focused journal tests include crash restoration, serialized overlapping saves, rejection
recovery, capacity and cached failures. Five relevant new assertions fail against the unchanged
baseline engine. Existing direct/child/Continue, held cleanup, controls, ownership, privacy,
CLI/MCP text, history storage and legacy compatibility coverage remains.

The first aggregate Rust run passed 525 tests and failed two new cases. The script relay fixture
advertised revision 1 instead of required revision 2. The metadata test exposed Serde's acceptance
of unknown fields on an internally tagged unit cause despite deny_unknown_fields. The fixture was
corrected, and the optional reader now requires the closed cause to consume its entire value.
Unsupported metadata is discarded while the record survives. Both cases pass in the final aggregate.
The first Clippy run found a needless borrow; it is removed. No behavior test was deleted or weakened.

## Not run and handoff

Real process journeys, live Chromium, installed native GUI, narrow/large-text/keyboard journeys,
fresh screenshots and real predecessor executable history compatibility are not run. Parent
instructions reserve runtime access pending independent structural/redteam review and whole-journey
human/agent acceptance. Source tests and the earlier installed captures cannot substitute for them.

`tests/history-compatibility.mjs` is extended and syntax checked. It now asks the verified predecessor
to read current receipts and tests unsupported optional cause, verification, progress, composition
issue and presentation beside readable neighbors, preserving original bytes. Run it later with
`GHOSTLIGHT_BIN_DIR` bound to the freshly built candidate and
`GHOSTLIGHT_PREVIOUS_EXECUTABLE` bound to the already verified predecessor; it does not download one.

No install, service restart, adapter reload, registration, live data, public version, deployment,
push, release, outreach, Tangent, private material or machine-local notes were changed. The saved
paused patch remains investigation evidence and is reconciled into this candidate. Parent should
review the exact local commit before authorizing any runtime change.
