# LEDGER: blocked-target evidence re-land

Durable progress for the evidence-1 batch. Update before work, after findings, and at each task
boundary. `RESUME HERE` names the only next task.

## RESUME HERE

- State: BATCH COMPLETE. E1 through E3 are done and deployed; nothing is next in this batch.
- Baseline: `dev` at `4d5ddb95` (docs(tasks): record D4 and close the delight ledger).

## Task table

| Task | State | Commit subject | Evidence |
| --- | --- | --- | --- |
| E1 projection evidence | COMPLETE | `feat(install): carry blocked-target evidence` | `5e5725fa`; Foreign carries the bounded found command, three cause-specific details, optional evidence field, 4 new pins, gate green. |
| E2 card rendering | COMPLETE | `feat(workbench): show blocked-target evidence` | `203ea6b4`; verbatim evidence paragraph on blocked rows, tone-bordered style, journey assertion, preview fixtures modernized, gate green. |
| E3 live proof + close | COMPLETE | `test(integrations): prove blocked-target evidence live` | `a3fab76d`; swap pid 33280 -> 31488; deployed-binary doctor --json read the exact evidence sentence from a seeded foreign file; STATUS Owed item closed. |

## Deviations

- E3 (resolved same day): the evidence paragraph was not seen with human eyes during the overnight run. The owner then verified it live in the real workbench: a foreign command was swapped into the real Claude Desktop configuration under a hash-verified backup, the deployed authority's card flipped to Needs Attention showing the cause sentence and the tone-bordered evidence paragraph naming `npx intruder-agent --mode quiet` and the owned connector, and the original bytes were restored and hash-confirmed. Nothing remains owed.

## Task log

| Task | Date | Status | Findings |
| --- | --- | --- | --- |
| Planning | 2026-08-24 | COMPLETE | Batch scaffolded from ADR-0129 Decision 4 and the STATUS owed item. Prior art (`5403b339`, reverted by `eb7cf4ed`) supplies the found-command technique; ADR-0135 re-decides the substance after ADR-0130 Decision 4 was superseded. Research confirmed no remnants of the reverted implementation remain in code. |
| E1 | 2026-08-24 | COMPLETE | `RegistrationState::Foreign` now carries the bounded found command line; `command_registration_state` composes it at the one seam all three dialects cross, so JSON, TOML, and YAML foreign entries all disclose without per-dialect work. `inspect()` names the actual cause (foreign / malformed / unreadable) instead of one conflated sentence and sets the optional `evidence` field only when blocked; `bounded_disclosure` strips control and bidi characters, keeps whitespace through collapsing, and caps at 200 visible characters. Four new pins: foreign disclosure (normalization plus cap math), malformed parser reason, commandless-foreign and unblocked states, and dialect coverage with direct helper pins. Two test-expectation corrections during authoring: tab is whitespace and survives disclosure, and detection counts a remaining parent directory after file removal. Full gate green (63 install tests, workspace clean). |
| E2 | 2026-08-24 | COMPLETE | The integrations renderer emits one `.integration-evidence` paragraph per blocked row carrying evidence, gated on both state and presence, rendered verbatim from the projection with no surface-authored words; styled by a left border in the card's category tone. Surface fixtures gained a foreign-entry evidence string plus one deliberately commandless blocked row (exercising absence); new journey assertion pins verbatim rendering on the blocked card and its absence elsewhere. The preview server's stale conflated needs-attention sentence was replaced with the cause-specific production sentences, all four preview blocked rows gained realistic evidence (foreign, malformed, commandless), and the fixture builder omits the field for unblocked rows exactly like `skip_serializing_if`. Preview boots clean against the roster drift guard. Full gate green (surface journey, workspace, 127 extension tests, integrity). |
| E3 | 2026-08-24 | COMPLETE | Release orchestrator exact-path swapped (live pid 33280 replaced by 31488; relays reconnected on their own). Live projection proven against the deployed binary: a foreign Claude Desktop configuration seeded under `.tmp/evidence-live/` with redirected APPDATA/USERPROFILE roots drove one-shot `ghostlight doctor --json`, which returned needs_attention with the cause-specific detail and the exact evidence sentence naming the intruding command line and the owned connector path, while an unblocked row omitted the evidence key entirely (skip_serializing_if verified on the wire). No live harness configuration was touched; the fixture forest was removed after the run. STATUS Owed item closed and a batch section added; tasks index updated. Full gate green. |
