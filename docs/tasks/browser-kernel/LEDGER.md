# Browser operation kernel: LEDGER

Durable progress for the Ghostlight operation-kernel rebuild. The original R0-R10 profile plan is
retained below as history; ADR-0102 replaced it with one Ghostlight surface and one operation path.

## RESUME HERE

- State: SUPERSEDED by ADR-0103. Preserve this ledger as evidence for the 0.9 architecture
  experiment; do not continue its R10 cutover.
- Current stage: closed without release.
- Next action: capture the 0.9 experiment, then write the Ghostlight 1.0 bill of intent.
- Blocking condition: none. The product direction changed by owner decision.
- Shipping contract: public `v0.8.0` remains stable while 1.0 is rebuilt cleanly.
- Last green gate: 2026-08-09 workspace tests including 631 core tests, strict workspace Clippy,
  209 extension tests,
  extension syntax checks, and all 36 real-process Lightbox scenarios passed.

## Stage table

| Stage | Status | Closing commit(s) | Release checkpoint | Notes |
| --- | --- | --- | --- | --- |
| R0 authority and inventory | COMPLETE | 5439e24c + a2a52ab7 | Current product unchanged | ADR-0101, immutable catalog/guide oracle, green baselines |
| R1 canonical operation bridge | COMPLETE | c3c4020d | Current 25 tools through temporary decoder | Bridge major 2 fails loudly across old/new peers |
| R2 legacy surface profile | COMPLETE | 641b332f | `ghostlight-legacy/v1` remains default | Exact declarations and rendering are edge-owned |
| R3 typed mechanism isolation | COMPLETE | fe55aceb | Old extension wire only | Core dispatches typed mechanisms; one bounded adapter owns old aliases |
| R4 extension mechanism skew | COMPLETE | (this commit) | Negotiated new wire plus legacy fallback | All three skew paths pass |
| R5 one Ghostlight surface, readiness, and action-kernel rebuild | COMPLETE | current worktree | Exact 24-tool Ghostlight contract | ADR-0102 replaced the profile plan |
| R6 client/profile selection | COMPLETE | current worktree | No selector exists | Superseded: one surface makes selection unnecessary |
| R7 Claude adapter | COMPLETE | current worktree | No dialect exists | Superseded: all clients call Ghostlight directly |
| R8 Codex adapter | COMPLETE | current worktree | No dialect exists | Superseded: all clients call Ghostlight directly |
| R9 governance and audit equivalence | COMPLETE | current worktree | One operation path | Governance and audit see Ghostlight operations only |
| R10 e2e and cutover | IN PROGRESS | current worktree | Ghostlight is unconditional | Automated 36/36 Lightbox pass; one local extension reload and successful live atomic-open journey remain |

Allowed status values are `NOT STARTED`, `IN PROGRESS`, `BLOCKED`, and `COMPLETE`. At most one
stage may be `IN PROGRESS`.

## Completion evidence matrix

An area is complete only when the evidence is linked to a commit, test output, fixture, capture,
or dated live record. A prose assertion is not evidence.

| Area | Owning stage(s) | Required completion evidence | Status | Evidence |
| --- | --- | --- | --- | --- |
| Inventory and authority | R0 | Accepted ADR ids; hashes and provenance for all captures; exact current catalog/result/RAWX/scheduling/workspace map; bridge and extension-wire baselines | COMPLETE | ADR-0101; Research 21 capture table; `tests/golden/surfaces/ghostlight-legacy-v1*`; current directory plus bridge/extension regression suites; 2026-08-08 baseline gates |
| Ghostlight operation kernel | R1, R5 | Typed operation/result/handle/default round trips; exhaustive descriptors; no second execution identity | COMPLETE | `crates/transport/src/operation.rs`; `crates/core/src/operation/registry.rs`; exact 24-operation enum and registry |
| Operation bridge | R1 | Coordinated major cutover; old/new mismatch fails loudly; Start/catalog/result/cancel transcripts; recursive flow carries canonical operations only | COMPLETE | Bridge major 2 unit and integration transcripts in `crates/transport/src/bridge.rs`, both MCP revisions, `tests/hub_isolation.rs`, and `tests/operation_bridge.rs` |
| Inherited profile removal | R2, R5 | No second catalog, decoder, renderer, selector, guide, or goldens remain | COMPLETE | Deleted inherited surface files and tests; `ghostlight_surface_fidelity` proves the sole catalog |
| Mechanisms and extension skew | R3-R4 | Exhaustive operation-to-mechanism and legacy alias maps; negotiated new wire; new/new, new/old, and old/new process evidence | COMPLETE | R3 closes 57-operation planning, typed mechanisms/controls/events, the sole legacy-wire adapter, final-admission and exact-wire tests, and architecture guards. R4 adds exact `mechanismRequestV1` negotiation, ordered cross-language equality for all 43 mechanism ids, session-generation binding, and three named Lightbox skew scenarios |
| Ghostlight surface | R5 | Exact 24 declarations, input/output schemas, defaults, decoder, renderer, workspace addressing, and concise recovery | COMPLETE | `surface/data/ghostlight-v1*`; `ghostlight.rs`; `ghostlight_surface_fidelity`; both MCP revision handlers |
| Readiness | R5, R9 | Committed-document watcher; landing authorization; one deadline; ready/timeout/unavailable/unknown truth | COMPLETE | `navigation_readiness`; `readiness_contract`; extension readiness suite; Lightbox readiness journey |
| Surface selection removal | R6-R8 | One unconditional catalog with no client classifier or vendor dialect | COMPLETE | `surface/mod.rs`; architecture and advertisement tests |
| Governance and audit | R1-R9 | One RAWX/resource/scheduler/authority outcome per Ghostlight operation; all-open parity; payload-free audit; hold/cancel/unknown truth | COMPLETE | exhaustive registry, policy, audit, workspace, and Lightbox gates |
| End-to-end and cutover | R10 | Common gates, extension gates, real-process Lightbox, and local live activation | IN PROGRESS | Full workspace green, extension 209/209, and Lightbox 36/36; deployed old-worker skew fails safely with actionable recovery; successful local atomic-open journey awaits one extension reload |

## Gate log

Append one row whenever a stage closes or a blocking rerun changes the evidence.

| Date | Stage | Commit/tree | Common gates | Focused gates | Live/e2e evidence | Result and notes |
| --- | --- | --- | --- | --- | --- | --- |
| 2026-08-08 | R0 | 5439e24c + a2a52ab7 | `cargo test --workspace`: pass | `cargo test --locked --test surface_profile_golden`: 2 pass; `node --test tests/extension/*.test.js`: 164 pass; extension `node --check`: pass | Not required for docs/oracle stage | Current 25-tool product and extension wire unchanged |
| 2026-08-08 | R1 | c3c4020d | `cargo fmt --all -- --check`, strict workspace Clippy, workspace build, and full no-fail-fast workspace tests: pass | transport 90, core 760, connector 87, architecture 11, operation bridge 3, frozen surface 2, schema fidelity 17, advertisement 3, protocol 4, enforcement 11, and four migrated integration targets 12: all pass | Extension 164/164 and syntax checks for 29 JavaScript files pass; process/e2e not required at R1 | Canonical bridge major 2, typed results, recursive flow, provenance, cancellation, image validation, workspace equality, and exact legacy edge rendering are green |
| 2026-08-08 | R2 | 641b332f | `cargo fmt --all -- --check`, strict workspace Clippy, workspace build, and full no-fail-fast workspace tests: pass | connector 96, core 739, transport 90, `surface_profile_fidelity` 4, MCP protocol 4, schema fidelity 17, advertisement 3, and enforcement 11: all pass | Extension unchanged; process/e2e not required at R2 | Frozen 25-tool profile and explain copy are edge-owned; both revision handlers prove exact catalog/context/success/denial transcripts; core has no model-facing declaration or legacy call decoder |
| 2026-08-08 | R3 | fe55aceb | `cargo fmt --all -- --check`, strict workspace Clippy, workspace build, and full no-fail-fast workspace tests: pass | core 810, architecture 16, mechanism mapping 9, enforcement 11, and script 2: all pass | Extension 164/164, syntax checks for every extension JavaScript file, and MCPB package tests 5/5 pass; process/e2e not required at R3 | All 57 canonical operations have closed physical plans; typed final admission, reply classes, recording FIFO/cancellation, semantic result truth, and exact old-wire compatibility are green; three independent P0/P1 audits are clean |
| 2026-08-08 | R4 | (this commit) | `cargo fmt --all -- --check`, strict workspace Clippy, workspace build, and full no-fail-fast workspace tests: pass | core 821, architecture 16, mechanism mapping 10, Lightbox package 5, and extension 177: all pass | `mechanism_wire_new_new`, `mechanism_wire_new_service_old_extension`, and `mechanism_wire_old_service_new_extension`: pass on the settled tree | Exact feature negotiation, legacy fallback, reconnect isolation, semantic tab URL/chunk behavior, source adapter 0.8.1 compatibility, and the mixed-version matrix are green; independent P0/P1 audit is clean |
| 2026-08-09 | R5-R10 | current worktree | full workspace and strict workspace Clippy pass; core 631/631 | Ghostlight surface, mechanism mapping, readiness, workspace addressing including concurrent creators and atomic open, architecture including the non-inferential completion guard, and extension 209/209 pass | Lightbox 36/36; guarded local deployment healthy; old loaded adapter live probe rejects atomic open before dispatch and suggests reloading the extension; earlier simultaneous Wikipedia Tree and Fractal canopy creators returned distinct exact handles and both cleanup closes committed | ADR-0102 is implemented: one 24-tool Ghostlight surface, typed operation-owned execution and results, one non-inferential completion chokepoint, exact generation-bound adapter capabilities, no inherited or vendor dialect, and retired service clutter removed |

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

- Status: COMPLETE.
- Edge-owned declaration evidence: `crates/mcp-connector/src/surface/ghostlight_legacy.rs` and
  its embedded catalog, guide, and explain assets match the frozen `ghostlight-legacy/v1` goldens.
- Both-revision catalog evidence: real handler/correlation transcripts in
  `mcp_2025_11_25.rs` and `mcp_2026_07_28.rs` prove the full ordered catalog through each revision;
  the transformed 2026 catalog also has a pinned byte length and fingerprint.
- Result/error rendering evidence: the same handler transcripts prove exact context/explain,
  ordinary read success, and policy-denial output. Context rendering rejects invalid family/intent
  pairs, repeated operation facts, repeated capabilities, and noncanonical capability order.
- Core dependency-boundary evidence: the old browser declaration, advertisement, tools, and
  decoder modules are deleted. `tests/surface_profile_fidelity.rs` proves core production code has
  no model-facing catalog/prose/decoder dependency. Historical audit replay remains one bounded
  canonical-first compatibility table; the old extension serializer is explicitly deferred to R3.
- Gate output: full workspace formatting, strict Clippy, build, and no-fail-fast tests passed in
  `.target-browser-kernel-r2-final`; connector 96, core 739, and transport 90 tests passed; diff and
  ASCII checks passed.
- Deviations/blockers: none. `ghostlight-legacy/v1` remains the sole automatic default.

### R3 -- typed mechanism isolation

- Status: COMPLETE.
- Mechanism directory and exhaustive map: `crates/core/src/browser/mechanism.rs` defines 43
  mechanism ids, 6 control ids, 2 event ids, and 7 closed auxiliary purposes. The 57 canonical
  operation descriptors are exhaustively classified as 38 direct, 14 dynamic, 2 composition,
  and 3 local plans. `tests/mechanism_mapping.rs` proves every descriptor, dynamic request/control
  set, and physical id has exactly one declared owner.
- Legacy serializer/alias evidence: `crates/core/src/hub/outbound/legacy_mechanism.rs` is the sole
  production owner of legacy extension aliases, presence-sensitive field translation, reply-class
  parsing, controls, and recording events. Exhaustive adapter and architecture tests prove exact
  coverage, fail-closed unknowns, and no raw alias, constructor, serializer, or enqueue escape.
- Browser behavior parity: browser dispatch retains typed origin through final panic, hold, and
  attention admission. One FIFO plus bounded ordinary permits preserves atomic frame order while
  exact-generation recording cancellation bypasses capacity without overtaking earlier work.
  Tests cover response-class mismatch, staged-start interruption, offline clear, one-shot frame
  rejection, finalization races, direct/flow result equivalence, edge-owned held copy, and
  mechanism-phase audit replay exclusion. The extension wire and handlers remain unchanged.
- Gate output: full workspace formatting, strict all-target Clippy, build, and no-fail-fast tests
  passed in `.target-browser-kernel-r2-final`; focused core 810, architecture 16, mechanism map 9,
  enforcement 11, and script 2 tests passed. Extension 164/164, syntax checks for every extension
  JavaScript file, MCPB package tests 5/5, diff check, and a 33-file ASCII scan passed. Independent
  acceptance, mechanism, and exact-wire audits found no P0/P1 blocker.
- Deviations/blockers: none. R3 deliberately retains only the old extension wire; negotiated typed
  extension requests and mixed-version evidence are R4 work.

### R4 -- extension mechanism skew

- Status: COMPLETE.
- Negotiated feature and compatibility range: exact case-sensitive `mechanismRequestV1` selects
  `{id,type:"mechanism_request",mechanism,input,...}` for one browser-session generation. Source
  adapter 0.8.1 and adapter 0.8.0 both declare service block 0.8; the public adapter remains 0.8.0.
- New service/new extension: Lightbox `mechanism_wire_new_new` crosses the real service, MCP edge,
  browser IPC, and fake-extension boundary and observes canonical `workspace.tabs.inspect` with no
  legacy alias fields.
- New service/old extension: Lightbox `mechanism_wire_new_service_old_extension` omits the feature
  and observes the exact covered `tabs_context_mcp` request through the bounded serializer.
- Old service/new extension: Lightbox `mechanism_wire_old_service_new_extension` executes the
  exported extension dual reader under Node and proves an old `tool_request` passes through by
  identity. This is honest source-adapter evidence, not a claim that an archival binary ran.
- Unknown-feature and reconnect evidence: core tests prove absent, unknown, case-changed, and
  near-match features select legacy; a replacement connection inherits no feature; prepared
  requests cannot cross either grammar transition; semantic tab URL retains its reply class; and
  large requests retain the exact session's chunk capability. A semantic-wire hold/attention test
  proves final admission still emits no frame.
- Gate output: core 821/821, architecture 16/16, cross-language mechanism mapping 10/10, Lightbox
  package 5/5, extension 177/177, changed JavaScript syntax, all three named skew scenarios,
  formatting, strict Clippy, workspace build/tests, diff, and ASCII checks pass.
- Deviations/blockers: no blocker. If an archival old service binary becomes available, the
  old-service/new-extension Node proof can be strengthened to a full process scenario.

### R5-R10 -- one-surface rebuild and close

- Status: R5-R9 COMPLETE; R10 IN PROGRESS pending local extension activation proof.
- Product language: accepted `docs/ubiquitous-language.md` and its quick reference.
- Governance language: accepted `docs/governance-language.md` and its quick reference.
- Authority: ADR-0102 replaces ADR-0101's profile rollout with one Ghostlight surface.
- Operation domain: exact 24-operation enum, argument types, defaults, results, and exhaustive
  registry. Grouped families and intents are gone.
- Action path: one bridge call, one workspace resolver, one scheduler/admission chokepoint, one
  handler dispatch, and one private mechanism port. Each operation constructs its own closed result
  from typed execution facts. Sequence children re-enter the same executor, and completion only
  binds opaque handles and serializes.
- Surface: one ordered declaration set, decoder, result renderer, guide, and both-revision MCP
  lifecycle. The inherited surface and vendor selectors are deleted.
- Governance: policy, protected-host, restriction, hold, end-session, attention, landing,
  readiness, audit, and uncertainty truth remain service-owned and share the same operation path.
- Cleanup: recording, GIF, demo, upload-image, update-plan, raw catalog/ref/validation modules,
  and their obsolete tests are removed from the service.
- Evidence: workspace tests including core 631/631 and strict Clippy pass; extension 209/209 plus
  syntax checks pass;
  Lightbox 36/36 real-process scenarios pass.

## Decision and deviation log

Record every discovered mismatch between the accepted design and the live tree. Do not silently
repair the batch document after implementation has begun; append the decision, its authority, and
its effect on later stages.

| # | Date | Stage | Finding or deviation | Authority and disposition |
| --- | --- | --- | --- | --- |
| 1 | 2026-08-08 | R0 | The primer is proposed research, so production work has no authority yet. | R0 blocks before code until the owner accepts the required ADR or ADR set. |
| 2 | 2026-08-09 | R5 | Vendor-profile compatibility added complexity without a measured benefit. | Owner accepted ADR-0102: one Ghostlight surface, no inherited or vendor dialect. |

## External and owner gates

- Accepting or amending ADRs is an owner decision.
- The 24-tool Ghostlight contract is unconditional in this worktree. A future dialect requires a
  new ADR and measured journey evidence.
- Chrome Web Store submission, package publication, tags, release assets, website deployment,
  external comments, and directory updates remain draft-then-confirm actions.
- Machine-local install and visible-browser facts belong in `local/MACHINE-STATE.md` or
  `local/NOTES.md`, not in tracked evidence when they contain sensitive identifiers.
