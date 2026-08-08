# Browser operation kernel: LEDGER

Durable progress for the canonical browser-operation, native surface, and compatibility-adapter
batch. Update this file before starting a stage, after every material finding, and when closing or
blocking a stage.

## RESUME HERE

- State: ACTIVE; ADR-0101 authorizes the staged implementation and R1 is complete.
- Current stage: R2 -- legacy surface profile, ready to start.
- Next action: remove the remaining model-facing declaration, legacy decoder, and trained-name
  dependencies from the core while `ghostlight-legacy/v1` remains byte exact at the edge.
- Blocking condition: none.
- Shipping default: current 25-tool surface. `ghostlight-native/v1`, Claude, and Codex profiles are
  candidates only.
- Last green gate: 2026-08-08 R1 full Rust, extension, formatting, Clippy, build, diff, and ASCII
  gates passed in `.target-browser-kernel-r1-core`.

## Stage table

| Stage | Status | Closing commit(s) | Release checkpoint | Notes |
| --- | --- | --- | --- | --- |
| R0 authority and inventory | COMPLETE | 5439e24c + a2a52ab7 | Current product unchanged | ADR-0101, immutable catalog/guide oracle, green baselines |
| R1 canonical operation bridge | COMPLETE | (this commit) | Current 25 tools through temporary decoder | Bridge major 2 fails loudly across old/new peers |
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
| Inventory and authority | R0 | Accepted ADR ids; hashes and provenance for all captures; exact current catalog/result/RAWX/scheduling/workspace map; bridge and extension-wire baselines | COMPLETE | ADR-0101; Research 21 capture table; `tests/golden/surfaces/ghostlight-legacy-v1*`; current directory plus bridge/extension regression suites; 2026-08-08 baseline gates |
| Canonical operation kernel | R1 | Typed operation/result/handle/default round trips; exhaustive concrete variant descriptors; no vendor or model-facing name as an execution key | COMPLETE | `crates/transport/src/operation.rs`; `crates/core/src/operation/registry.rs`; 26 operation families, 60 closed intents, and all 52 legacy variants covered by tests |
| Operation bridge | R1 | Coordinated major cutover; old/new mismatch fails loudly; Start/catalog/result/cancel transcripts; recursive flow carries canonical operations only | COMPLETE | Bridge major 2 unit and integration transcripts in `crates/transport/src/bridge.rs`, both MCP revisions, `tests/hub_isolation.rs`, and `tests/operation_bridge.rs` |
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
| 2026-08-08 | R0 | 5439e24c + a2a52ab7 | `cargo test --workspace`: pass | `cargo test --locked --test surface_profile_golden`: 2 pass; `node --test tests/extension/*.test.js`: 164 pass; extension `node --check`: pass | Not required for docs/oracle stage | Current 25-tool product and extension wire unchanged |
| 2026-08-08 | R1 | (this commit) | `cargo fmt --all -- --check`, strict workspace Clippy, workspace build, and full no-fail-fast workspace tests: pass | transport 90, core 760, connector 87, architecture 11, operation bridge 3, frozen surface 2, schema fidelity 17, advertisement 3, protocol 4, enforcement 11, and four migrated integration targets 12: all pass | Extension 164/164 and syntax checks for 29 JavaScript files pass; process/e2e not required at R1 | Canonical bridge major 2, typed results, recursive flow, provenance, cancellation, image validation, workspace equality, and exact legacy edge rendering are green |

## Stage records

### R0 -- authority and inventory

- Status: COMPLETE.
- Accepted ADR authority: ADR-0101.
- Capture hashes and evidence classes: `docs/research/21-client-tool-surface-discovery-2026-08.md`
  and its tracked `docs/research/tool-surfaces/` artifacts.
- Current 25-tool inventory fixture:
  `tests/golden/surfaces/ghostlight-legacy-v1.json` SHA-256
  `4e68638b0a85ef5dc5dacbf0420091fd5eebb4968d2b764ab4b1c9a78a8e5293`; exact guide SHA-256
  `cb35c48350599625016ddede5737580dce164a756961aa3406ba02623ffc4223`.
- Bridge-major-1 transcript fixture: exact serde/wire assertions in
  `crates/transport/src/bridge.rs` at commit 5439e24c and MCP transcript coverage in
  `tests/mcp_protocol.rs`.
- Extension legacy-wire fixture: `tests/extension/execution-response.test.js`,
  `surface-executor.test.js`, and `wire-chunks.test.js` at commit 5439e24c.
- Readiness and recovery baseline: `extension/lib/settle.js`,
  `tests/extension/settle.test.js`, and current navigation tests at commit 5439e24c.
- Baseline gate output: `cargo test --workspace` passed; extension 164/164 passed; all extension
  JavaScript parsed; focused surface golden passed 2/2.
- Deviations/blockers: none.

### R1 -- canonical operation bridge

- Status: COMPLETE.
- Operation/result DTO evidence: `crates/transport/src/operation.rs` has closed operation and
  intent ids, canonical operations/results, typed flow results, bounded handles, provenance,
  cancellation effects, readiness facts, and validated image parts. Its 90 transport tests pass.
- Descriptor and variant coverage: `crates/core/src/operation/registry.rs` is the execution lookup
  authority. Tests cover 26 operation families, 60 intents, valid family/intent pairs, all 52
  legacy variants, action-specific normalization, RAWX/resource/scheduling facts, and truthful
  success dispositions.
- Bridge-major and mismatch evidence: the owner bridge is major 2 and carries operation
  availability, typed Start, canonical result, and cancellation effect. Unit transcripts plus
  `tests/hub_isolation.rs` prove both old/new directions fail before catalog or browser work.
- Recursive composition evidence: `crates/core/src/tool/flow.rs`, edge flow-hint tests, and
  `tests/operation_bridge.rs` prove nested steps cross as canonical operations only, re-enter
  governance, retain typed termination/effect state, and cannot be mislabeled by a result renderer.
- Current-surface compatibility evidence: the edge-owned legacy declarations and guide match the
  frozen 25-tool assets byte for byte. Both MCP revisions reconstruct legacy text, images,
  structured content, provenance, flow shapes, workspace behavior, and error semantics from the
  canonical result.
- Gate output: full workspace tests, formatting, strict Clippy, workspace build, extension
  164/164, 29 JavaScript syntax checks, diff check, and a 46-file ASCII scan passed on 2026-08-08.
- Deviations/blockers: the edge profile extraction needed to land in R1 as part of the coordinated
  bridge-major cutover. Core still retains bounded legacy declaration and serializer seams; R2
  removes them before any new profile is enabled. No blocker remains.

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
