# Browser operation kernel: LEDGER

Durable progress for the canonical browser-operation, native surface, and compatibility-adapter
batch. Update this file before starting a stage, after every material finding, and when closing or
blocking a stage.

## RESUME HERE

- State: ACTIVE; ADR-0101 authorizes the staged implementation.
- Current stage: R0 -- authority and inventory.
- Next action: freeze the current catalog, result, bridge, extension-wire, readiness, and
  external-capture oracles listed in `BOOTSTRAP.md`.
- Blocking condition: none. R1 starts only after the R0 oracle and baseline gates are recorded.
- Shipping default: current 25-tool surface. `ghostlight-native/v1`, Claude, and Codex profiles are
  candidates only.
- Last green gate: not yet run for this batch.

## Stage table

| Stage | Status | Closing commit(s) | Release checkpoint | Notes |
| --- | --- | --- | --- | --- |
| R0 authority and inventory | IN PROGRESS | -- | Current product unchanged | ADR-0101 accepted; immutable-oracle gate in progress |
| R1 canonical operation bridge | NOT STARTED | -- | Current 25 tools through temporary decoder | Bridge-major cutover must fail loudly |
| R2 legacy surface profile | NOT STARTED | -- | `ghostlight-legacy/v1` remains default | Exact declarations and rendering move to edge |
| R3 typed mechanism isolation | NOT STARTED | -- | Old extension wire only | Core no longer dispatches surface names |
| R4 extension mechanism skew | NOT STARTED | -- | Negotiated new wire plus legacy fallback | Prove old/new matrix |
| R5 native surface and readiness | NOT STARTED | -- | Native explicit opt-in; legacy default | No planned pack is advertised |
| R6 client/profile selection | NOT STARTED | -- | Exact selection; legacy fallback retained | No identity signal affects authority |
| R7 Claude flat adapter | NOT STARTED | -- | Versioned opt-in supported surface | All 22 captured tools dispositioned |
| R8 Codex runtime adapter | NOT STARTED | -- | Versioned opt-in trusted module | All 136 members dispositioned |
| R9 governance and audit equivalence | NOT STARTED | -- | All candidates remain opt-in | Cross-profile adversarial proof |
| R10 e2e, evaluation, and cutover | NOT STARTED | -- | Accepted shipping default | Owner decision follows evidence |

Allowed status values are `NOT STARTED`, `IN PROGRESS`, `BLOCKED`, and `COMPLETE`. At most one
stage may be `IN PROGRESS`.

## Completion evidence matrix

An area is complete only when the evidence is linked to a commit, test output, fixture, capture,
or dated live record. A prose assertion is not evidence.

| Area | Owning stage(s) | Required completion evidence | Status | Evidence |
| --- | --- | --- | --- | --- |
| Inventory and authority | R0 | Accepted ADR ids; hashes and provenance for all captures; exact current catalog/result/RAWX/scheduling/workspace map; bridge and extension-wire baselines | NOT STARTED | -- |
| Canonical operation kernel | R1 | Typed operation/result/handle/default round trips; exhaustive concrete variant descriptors; no vendor or model-facing name as an execution key | NOT STARTED | -- |
| Operation bridge | R1 | Coordinated major cutover; old/new mismatch fails loudly; Start/catalog/result/cancel transcripts; recursive flow carries canonical operations only | NOT STARTED | -- |
| Legacy profile | R2 | Byte-exact 25-tool order, identity, schemas, annotations, examples, results, and errors on both MCP revisions; legacy remains releasable | NOT STARTED | -- |
| Mechanisms and extension skew | R3-R4 | Exhaustive operation-to-mechanism and legacy alias maps; negotiated new wire; new/new, new/old, and old/new process evidence | NOT STARTED | -- |
| Native surface | R5 | Strict twelve-tool core and honest supported packs; output schemas; workspace/addressing; bounded observations; opt-in journey evidence | NOT STARTED | -- |
| Readiness | R5, R9 | Committed-document watcher; initial and landing authorization ordering; one dispatch-to-readiness deadline; condition/settlement result axes; timeout, unavailable, cancel, redirect, PDF/protected-page, and outcome-unknown tests | NOT STARTED | -- |
| Client/profile selection | R6 | Exact precedence and version/fingerprint tests; sessionful and request-stateless state isolation; concurrent-profile and restriction tests; no authority/routing effect | NOT STARTED | -- |
| Claude adapter | R7 | Exact capture provenance; all 22 tools mapped, omitted, or rejected honestly; declaration fidelity; journey, denial, result, and audit equivalence | NOT STARTED | -- |
| Codex adapter | R8 | Frozen module and documentation fingerprint; complete 136-member ledger; proxy generations; locator and terminal-call tests; prohibited paths absent; live read-only bootstrap and journey | NOT STARTED | -- |
| Governance and audit | R1-R9 | Same RAWX/resource/scheduler/authority outcomes for every external variant; all-open parity; payload-free audit; surface provenance; flow source correlation; denial/hold/cancel/unknown twins | NOT STARTED | -- |
| End-to-end and release | R10 | Common gates, Lightbox, extension skew, package gates, real-extension smoke, visible Windows/Linux journeys, multi-model evaluation, and accepted default/fallback disposition | NOT STARTED | -- |

## Gate log

Append one row whenever a stage closes or a blocking rerun changes the evidence.

| Date | Stage | Commit/tree | Common gates | Focused gates | Live/e2e evidence | Result and notes |
| --- | --- | --- | --- | --- | --- | --- |
| -- | -- | -- | -- | -- | -- | No batch gates recorded yet |

## Stage records

### R0 -- authority and inventory

- Status: IN PROGRESS.
- Accepted ADR authority: ADR-0101.
- Capture hashes and evidence classes: `docs/research/21-client-tool-surface-discovery-2026-08.md`
  and its tracked `docs/research/tool-surfaces/` artifacts.
- Current 25-tool inventory fixture: --
- Bridge-major-1 transcript fixture: --
- Extension legacy-wire fixture: --
- Readiness and recovery baseline: --
- Baseline gate output: --
- Deviations/blockers: none.

### R1 -- canonical operation bridge

- Status: NOT STARTED.
- Operation/result DTO evidence: --
- Descriptor and variant coverage: --
- Bridge-major and mismatch evidence: --
- Recursive composition evidence: --
- Current-surface compatibility evidence: --
- Gate output: --
- Deviations/blockers: --

### R2 -- legacy surface profile

- Status: NOT STARTED.
- Edge-owned declaration evidence: --
- Both-revision catalog evidence: --
- Result/error rendering evidence: --
- Core dependency-boundary evidence: --
- Gate output: --
- Deviations/blockers: --

### R3 -- typed mechanism isolation

- Status: NOT STARTED.
- Mechanism directory and exhaustive map: --
- Legacy serializer/alias evidence: --
- Browser behavior parity: --
- Gate output: --
- Deviations/blockers: --

### R4 -- extension mechanism skew

- Status: NOT STARTED.
- Negotiated feature and compatibility range: --
- New service/new extension: --
- New service/old extension: --
- Old service/new extension: --
- Unknown-feature and reconnect evidence: --
- Gate output: --
- Deviations/blockers: --

### R5 -- native surface and readiness

- Status: NOT STARTED.
- Native declaration and pack evidence: --
- Workspace/addressing evidence: --
- Navigation/readiness state-machine evidence: --
- Bounded result/provenance evidence: --
- Native opt-in journeys: --
- Legacy-default regression: --
- Gate output: --
- Deviations/blockers: --

### R6 -- client/profile selection

- Status: NOT STARTED.
- Selection precedence and exact matcher evidence: --
- Sessionful state evidence: --
- Request-stateless isolation evidence: --
- Concurrent profile/restriction evidence: --
- Authority/routing non-interference evidence: --
- Gate output: --
- Deviations/blockers: --

### R7 -- Claude flat adapter

- Status: NOT STARTED.
- Profile id and supported declaration set: --
- 22-tool disposition ledger: --
- Canonical mapping and schema normalization: --
- Journey/result/audit evidence: --
- Unsupported capability evidence: --
- Gate output: --
- Deviations/blockers: --

### R8 -- Codex runtime adapter

- Status: NOT STARTED.
- Plugin/module identity and trust boundary: --
- 136-member disposition ledger: --
- Proxy, locator, generation, reset, and cancellation evidence: --
- Dynamic documentation and capability-filter evidence: --
- Prohibited capability negative tests: --
- Live read-only bootstrap and journey: --
- Gate output: --
- Deviations/blockers: --

### R9 -- governance and audit equivalence

- Status: NOT STARTED.
- RAWX/resource/scheduling equivalence matrix: --
- Restriction, sacred, hold, panic, attention, and denial evidence: --
- Cancellation and outcome-unknown evidence: --
- Audit payload minimization and surface correlation: --
- Flow in-band provenance: --
- All-open parity: --
- Gate output: --
- Deviations/blockers: --

### R10 -- e2e, evaluation, and cutover

- Status: NOT STARTED.
- Full common/extension/package gates: --
- Lightbox and mixed-version extension matrix: --
- Real-extension smoke: --
- Visible Windows evidence: --
- Visible Linux evidence: --
- Native vs legacy model-journey measurements: --
- Claude and Codex model-journey measurements: --
- Accepted shipping default/fallback decision: --
- Release/docs/compatibility synchronization: --
- Deviations/blockers: --

## Decision and deviation log

Record every discovered mismatch between the accepted design and the live tree. Do not silently
repair the batch document after implementation has begun; append the decision, its authority, and
its effect on later stages.

| # | Date | Stage | Finding or deviation | Authority and disposition |
| --- | --- | --- | --- | --- |
| 1 | 2026-08-08 | R0 | The primer is proposed research, so production work has no authority yet. | R0 blocks before code until the owner accepts the required ADR or ADR set. |

## External and owner gates

- Accepting or amending ADRs is an owner decision.
- Changing the automatic default away from the current 25-tool profile is an owner decision after
  R10 evidence.
- Installing or distributing a Codex plugin/runtime integration requires explicit owner approval.
- Chrome Web Store submission, package publication, tags, release assets, website deployment,
  external comments, and directory updates remain draft-then-confirm actions.
- Machine-local install and visible-browser facts belong in `local/MACHINE-STATE.md` or
  `local/NOTES.md`, not in tracked evidence when they contain sensitive identifiers.
