# Browser operation kernel: BOOTSTRAP

Historical bootstrap for the browser-operation batch. ADR-0102 superseded the profile rollout and
is now implemented in the current worktree: one 24-tool Ghostlight surface, one typed operation
domain, one governance/scheduling path, and one private mechanism port. Use `LEDGER.md` for the
close evidence. The R0-R10 instructions below remain only as implementation history.

## Start here

1. Read `AGENTS.md`, `docs/MEMORY.md`, and `docs/STATUS.md`.
2. Read every accepted ADR named by the active stage, especially ADR-0034, ADR-0037, ADR-0038,
   ADR-0042, ADR-0050, ADR-0069, ADR-0078, ADR-0080, ADR-0093, ADR-0094, ADR-0096, and ADR-0099.
3. Read `docs/ubiquitous-language.md`, then read
   `docs/research/21-client-tool-surface-discovery-2026-08.md`,
   `docs/research/22-canonical-browser-operation-primer.md`, and the exact capture artifacts under
   `docs/research/tool-surfaces/` as evidence and history.
4. Read this file and `LEDGER.md`. Resume only from `RESUME HERE`.
5. Re-read the live files named by the active stage. Paths and line numbers in research are hints;
   the live tree and accepted ADRs are authoritative.

## Authority

Conflicts resolve in this order:

1. The live tree and accepted ADRs.
2. The ADR or ADR set accepted in R0 for the canonical operation, surface-profile, readiness,
   bridge-major, mechanism-wire, and adapter trust boundaries.
3. `docs/ubiquitous-language.md` as the current working product-language proposal.
4. Exact captured declarations and behavior observations, each kept in its evidence class.
5. The research primers as historical rationale.
6. This bootstrap and the ledger.

Captured vendor language can prove that a job or failure mode exists. It cannot override the
working native name, grouping, description, or schema merely because a vendor exposed it first.

Stop if R0 cannot establish accepted authority for a production change. Do not implement a
research recommendation merely because this batch describes its likely sequence.

## Target dependency direction

```text
external call
  -> one edge-owned SurfaceProfile and bounded SurfaceSession
  -> one protocol-neutral BrowserOperation
  -> core validation, governance, scheduling, execution, and audit
  -> typed browser MechanismId calls
  -> one canonical BrowserResult
  -> edge or runtime result rendering
```

The intended ownership is:

- `ghostlight-transport`: bridge-versioned operation and result DTOs, semantic ids, handles, and
  shared default-version vocabulary.
- `ghostlight-core`: operation descriptors, validation, RAWX requirements, resource resolution,
  scheduling, handlers, post-processing, and audit identity.
- `ghostlight-mcp-connector`: flat surface declarations, profile selection, external validation,
  normalization, catalog rendering, and result rendering.
- Browser execution shore and extension: typed mechanisms plus a bounded negotiated legacy alias
  path during adapter skew. They know no surface profile or policy.
- Codex runtime integration: a trusted stateful proxy module whose terminal methods call ordinary
  canonical Ghostlight operations. It is not a second policy engine or unrestricted service-side
  JavaScript runtime.

## Invariants

- Preserve the current 25 names, ordering, schemas, annotations, examples, and result behavior as
  `ghostlight-legacy/v1` until a separately accepted and evaluated default cutover.
- Never advertise native and compatibility duplicates in one catalog. One request or session sees
  exactly one profile.
- `clientInfo` and every adapter fingerprint select presentation only. They never grant authority,
  choose a workspace or browser, change scheduling, weaken policy, or suppress audit.
- `WorkspaceHandle` remains the service bearer authority. Tab, document, locator, capture, event,
  and artifact handles are verification or resource handles, never substitute authority.
- A surface adapter normalizes one external call to one canonical operation. Vendor batches map to
  `browser.flow`; adapters never dispatch governed subcalls around the pipeline.
- Canonical operations, including every concrete action variant, own RAWX, resource, scheduling,
  readiness, result, and audit semantics. Physical mechanism ids never become policy keys.
- All-open stays first-class. Correctness scheduling, typed results, and readiness apply equally in
  governed and ungoverned modes; governance remains an overlay.
- The extension stays policy-free and client-blind. It contains Chrome mechanism and bounded wire
  compatibility only.
- Page content and page-defined metadata remain untrusted output. No page text, target name, form
  value, URL, screenshot, raw bearer handle, or content-derived hash enters audit.
- Preserve local-only ingress, the visible authenticated Chromium profile, browser-shore topology,
  explicit owned-tab close, the no-phone-home promise, and browser-adapter independent versioning.
- Bridge changes fail loudly by major version. Extension mechanism changes negotiate a feature and
  preserve the declared old-service/new-extension and new-service/old-extension skew window.
- Navigation success and readiness remain separate. A proven commit may return successful
  navigation with readiness timed out or unavailable. An unproven effect never becomes soft
  success, and cancellation never claims rollback.
- Claude and Codex profiles expose only behavior Ghostlight can honor. Unsupported shortcuts,
  finalization, user-tab claiming, clipboard, auth, WebMCP, raw CDP, host access, and telemetry are
  omitted or rejected according to the accepted profile contract, never faked.
- No phone-home behavior, arbitrary host Node access, broad personal-tab discovery, credential
  entry, or protected-secret theater enters any profile.
- ASCII only. Preserve unrelated dirty work. Never read `/private/` or `saps/`.

## Release discipline

- Stages run strictly R0 through R10. Do not start a stage until the prior row is `COMPLETE` in
  `LEDGER.md`.
- A stage is a release checkpoint, not necessarily one commit. Keep every commit logical and green;
  record the closing commit or commit range and all deviations in the ledger.
- The shipping default remains `ghostlight-legacy/v1` through R9. Native and vendor candidates are
  explicit opt-ins until R10's accepted cutover decision and journey evidence.
- Do not retain two internal authorities during migration. A temporary decoder may preserve the
  old external identity, but validation, governance, scheduling, and audit have one canonical path.
- Do not publish, tag, merge to `main`, submit a store build, install a Codex plugin, or change an
  external listing without separate owner confirmation.

## Ordered stages

| Stage | Releasable outcome | Required exit evidence |
| --- | --- | --- |
| R0 | Accepted authority plus immutable inventory and baseline oracles; no production change | ADR acceptance; capture hashes; current catalog/result/RAWX/scheduling/workspace map; bridge and extension-wire transcripts; all baseline gates green |
| R1 | Canonical operation/result DTOs and a fail-loud bridge-major cutover, with a temporary legacy decoder preserving all 25 tools | DTO round trips; old/new bridge mismatch tests; recursive flow normalization; architecture checks; exact current catalog and behavior unchanged |
| R2 | Edge-owned `ghostlight-legacy/v1`; core owns operation availability and execution facts only | Byte-exact legacy declarations on both MCP revisions; legacy results and errors unchanged; no model-facing prose/vendor name drives core execution |
| R3 | Core dispatch uses typed `MechanismId`; outbound translation still emits the existing extension wire | Complete operation-to-mechanism map; legacy alias coverage; no extension change required; all browser journeys unchanged |
| R4 | Negotiated mechanism request wire with bounded bidirectional skew | New/new, new/old, and old/new matrix; unknown feature fails or falls back explicitly; extension remains profile- and policy-free |
| R5 | Strict `ghostlight-native/v1` and shared readiness contract available by explicit opt-in only | Twelve core declarations and honest packs; workspace/addressing tests; readiness state machine and result tests; native journey baseline; legacy still default and exact |
| R6 | Exact profile selection and state isolation, still with legacy as the shipping fallback | Override, authenticated handshake, exact allowlist, version/fingerprint, and fallback tests; concurrent profiles do not share state; `clientInfo` has zero authority effect |
| R7 | Versioned Claude flat profile or supported subset, opt-in until it wins evaluation | All 22 captured declarations dispositioned; every advertised variant maps exactly; unsupported browser/shortcut behavior omitted honestly; Claude journey and audit equivalence |
| R8 | Frozen Codex runtime module over the existing persistent kernel, opt-in and generation-safe | Complete 136-member mapping ledger; proxy/locator lifecycle tests; terminal-call canonical dispatch; cancellation/reset tests; prohibited host/telemetry/credential paths absent |
| R9 | Adversarial governance, audit, readiness, restriction, and all-open equivalence across every implemented profile | RAWX/resource/scheduler parity matrix; payload-free audit; flow source provenance; denial/hold/cancel/unknown tests; no adapter bypass path |
| R10 | Full process, extension-skew, visible-browser, and model-journey evidence; owner-approved default decision | Full gates and packaging; Lightbox and real extension e2e; Windows and Linux visible journeys; measured native/legacy/Claude/Codex comparison; accepted fallback/default disposition |

## Stage notes

### R0: authority and inventory

Freeze evidence before extracting code:

- Current 25-tool declaration order, identity, descriptions, annotations, examples, input/output
  schemas, workspace use, RAWX variants, scheduling, handlers, post-processing, page provenance,
  and text/structured result behavior.
- Exact `2025-11-25` and `2026-07-28` catalog projections and workspace-addressing differences.
- Bridge major 1 `Start`, catalog, result, cancellation, and mismatch transcripts.
- Current extension `tool_request`, reply, chunking, executor-generation, feature-negotiation, and
  adapter-compatibility wire.
- Claude 22-tool, Codex outer/gateway, Codex 136-member runtime, and Playwright capture hashes with
  unobserved fields and behavior evidence kept separate.
- Current navigation, landing check, wait, settle, timeout, cancellation, and outcome-unknown
  behavior, including detector constants and browser-process recovery.

The ADR set must decide at least: canonical identities and result vocabulary; workspace/addressing;
readiness defaults; legacy/default migration; profile selection; bridge major; extension skew;
Claude supported contract; Codex plugin trust and distribution boundary; and ship/evaluation gates.

### R1-R2: operation bridge and legacy profile

R1 changes the private bridge and service execution path as one coordinated fail-loud cutover.
The temporary decoder accepts current external names only at the edge of the canonical pipeline.
`script` and `browser_batch` normalize every nested step before the bridge; no external tool name
survives as the enforcement key.

R2 moves model-facing declarations and rendering to the MCP edge. The service projects canonical
operation availability and grant filtering without owning another catalog. The legacy profile is
the exact regression oracle and remains the only automatic default.

### R3-R4: mechanism and extension wire

R3 creates one typed mechanism directory below semantic operations while continuing to serialize
the old extension frame. R4 adds the negotiated wire only after that seam is proven. Keep legacy
aliases bounded, documented, exhaustively mapped, and removable only after the adapter compatibility
window closes under ADR-0093.

### R5-R6: native surface and selection

R5 implements only physically supported core tools and packs. Optional packs do not advertise a
planned capability. Navigation uses one dispatch-to-readiness deadline, exact committed-document
landing checks, a soft settle timeout only after proven commit, and two-axis condition/settlement
evidence. Request-stateless projections carry explicit workspace authority.

R6 pins profile choice before catalog rendering. Missing, ambiguous, self-asserted-only, or
out-of-range evidence takes the accepted fallback. Stateful profile data is scoped by exact
profile, MCP revision, service generation, restriction context, and explicit workspace/session
handle. It never enters workspace routing.

### R7-R8: vendor adapters

R7 begins from the exact Claude capture. A subset receives its own honest profile id and exact
declaration set; it is not mislabeled as the complete vendor surface. Every schema quirk is
preserved only at the edge and normalizes into stricter canonical intent.

R8 uses the current Codex persistent Node kernel and a trusted Ghostlight module. Pure locator
builders and cached documentation may stay local; every terminal observation or effect calls one
canonical operation. The module cannot expose arbitrary imports, process, filesystem, network,
clipboard, credentials, raw CDP, telemetry, or broad user-tab access through Ghostlight.

### R9-R10: equivalence and cutover

R9 is not the first governance test. Every earlier stage must preserve governance. R9 is the final
cross-profile adversarial proof that no normalization, state, pack, or result renderer changes
authority or audit truth.

R10 changes the unknown-client fallback to native only if an accepted ADR authorizes it and the
pinned journeys show repeated success without a governance regression. If evidence does not pass,
keep legacy as fallback, record the failed gate, and do not call the batch complete.

## Exact common gates

Use an isolated target directory. Run this block before every stage-closing commit:

```powershell
$env:CARGO_TARGET_DIR = ".target-browser-kernel"
cargo fmt --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --locked --workspace
cargo test --locked --no-fail-fast --workspace
git diff --check
```

When a stage touches the extension or browser wire, also run:

```powershell
node --test "tests/extension/*.test.js"
Get-ChildItem extension -Recurse -Filter *.js | ForEach-Object {
    node --check $_.FullName
    if ($LASTEXITCODE -ne 0) { throw "node --check failed: $($_.FullName)" }
}
```

Every changed or new text/source file must pass an ASCII scan. Record the exact command and file
set in the ledger; never scan or disclose excluded personal directories.

## Exact focused gates by stage

The listed suites are additive to the common gates. Create the named new suite when it does not
exist yet; do not replace the full workspace gate with a focused run.

| Stage | Required focused commands or suites |
| --- | --- |
| R0 | `cargo test --locked --test tool_schema_fidelity`; `cargo test --locked --test tool_advertisement`; `cargo test --locked --test mcp_protocol`; baseline extension suite |
| R1 | `cargo test --locked -p ghostlight-transport`; `cargo test --locked -p ghostlight-mcp-connector`; `cargo test --locked -p ghostlight-core`; `cargo test --locked --test architecture`; new `operation_bridge` round-trip, major-mismatch, recursive-flow, cancellation, and result tests |
| R2 | `cargo test --locked --test tool_schema_fidelity`; `cargo test --locked --test tool_advertisement`; `cargo test --locked --test mcp_protocol`; new `surface_profile_fidelity` legacy catalog/result tests for both revisions |
| R3 | `cargo test --locked -p ghostlight-core`; `cargo test --locked --test tool_enforcement`; `cargo test --locked --test script_tool`; new `mechanism_mapping` exhaustive operation/variant/legacy-alias tests |
| R4 | Extension suite and syntax block; new `tests/extension/mechanism-wire.test.js`; Lightbox scenarios `mechanism_wire_new_new`, `mechanism_wire_new_service_old_extension`, and `mechanism_wire_old_service_new_extension` |
| R5 | `cargo test --locked --test tool_schema_fidelity`; new `native_surface_fidelity`, `workspace_addressing`, and `readiness_contract` suites; `node --test tests/extension/settle.test.js`; native opt-in Lightbox journeys |
| R6 | `cargo test --locked -p ghostlight-mcp-connector`; new `client_profile_selection` suite covering precedence, version bounds, request-stateless isolation, restrictions, concurrent profiles, and authority invariance |
| R7 | New `claude_profile_fidelity` and `adapter_equivalence` suites; all 22 captured tools have a tested disposition; Claude-profile Lightbox journeys and denial/audit twins |
| R8 | `node --test "tests/codex/*.test.js"`; new Rust `adapter_equivalence` Codex cases; complete 136-member mapping check; reset, disconnect, cancellation, locator-generation, documentation-filter, and prohibited-capability tests |
| R9 | `cargo test --locked --test all_open_golden`; `cargo test --locked --test tool_enforcement`; `cargo test --locked --test audit_recorder`; `cargo test --locked --test provenance`; `cargo test --locked --test policy_simulate`; `cargo test --locked --test shadow_mode`; `cargo test --locked --test hub_isolation`; `cargo test --locked --test hub_queue`; cross-profile equivalence matrix |
| R10 | Full common and extension gates; `cargo run --locked -p ghostlight-lightbox -- run --all`; package gates below; real-extension e2e and visible-browser/model journey matrix |

R10 package and real-extension gates:

```powershell
npm test --prefix packaging/npm
node --test "packaging/mcpb/test/*.test.js"
npx --yes @anthropic-ai/mcpb@2.1.2 validate packaging/mcpb/manifest.json
npm ci --prefix tests/e2e
npx --prefix tests/e2e playwright install chromium
node --test tests/e2e/free-surface-baseline.test.mjs
node tests/e2e/run-smoke.mjs
node tests/e2e/run-smoke.mjs --free-surface-baseline
```

Extend the e2e harness before R10 so it runs named legacy, native, Claude, and Codex-runtime
journeys without advertising multiple profiles to one client. Visible Ghostlight verification uses
the user's ordinary local browser and the real installed stack; Playwright proves only the harness
boundary.

## Stop conditions

Stop, record the exact blocker in `LEDGER.md`, and do not skip ahead if:

- an accepted ADR does not authorize the active production change;
- the current 25-tool identity or result contract cannot remain exact at a release checkpoint;
- old/new bridge peers do not fail loudly or extension skew cannot fall back truthfully;
- an adapter would need authority, raw handles, page content, or vendor identity to enter routing,
  scheduling, governance, or audit decisions;
- a vendor declaration has no honest canonical or unsupported disposition;
- a Codex method would require host access, credentials, telemetry, raw CDP, or a capability that
  Ghostlight does not own;
- a readiness timeout would turn an unproven navigation effect into success;
- page payload enters audit or diagnostics persistence;
- native journey evidence does not justify the requested default cutover;
- required visible-browser, cross-platform, or extension-skew evidence is unavailable.

## Completion criteria

R0-R10 are complete only when:

- every stage closes at a releasable commit with all required gates recorded;
- the completion evidence matrix in `LEDGER.md` has no `NOT STARTED`, `IN PROGRESS`, or unexplained
  `N/A` row;
- legacy fidelity, canonical bridge, mechanism skew, native surface, selection, Claude, Codex,
  governance/audit, readiness, Lightbox, and visible-browser evidence all pass;
- the accepted ADR disposition and journey data agree on the shipping default;
- durable docs, `docs/STATUS.md`, changelog, packaging, compatibility declarations, and release
  notes reflect the implemented state;
- no external publication or release is claimed without owner confirmation.
