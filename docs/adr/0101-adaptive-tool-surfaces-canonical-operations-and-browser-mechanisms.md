# ADR-0101: Adaptive tool surfaces over canonical operations and browser mechanisms

- Status: Accepted
- Date: 2026-08-08
- Amends: ADR-0094 Decisions 1 and 2, and ADR-0096 Decisions 3, 4, and its
  adversarial-minimality result
- Builds on: ADR-0005, ADR-0024, ADR-0034, ADR-0069, ADR-0080, and ADR-0093

## Context

Ghostlight currently uses one inherited tool vocabulary for three different jobs:

1. the names, schemas, descriptions, and examples an agent sees;
2. the operations the service validates, governs, schedules, audits, and executes; and
3. the commands the service sends to the browser extension.

That coupling was useful while exact Claude-in-Chrome compatibility was the only surface. It is
now the wrong boundary. A model paired with a first-party browser client may have been trained on
that client's exact tool names and schemas. Other MCP clients need a clear Ghostlight-native
surface whose descriptions, metadata, defaults, and recovery guidance optimize for reliable use.
Neither need should force the service or extension to keep vendor-derived semantic names.

The current layout also makes a harmless model-facing change look like an execution change. A
renamed internal browser command can disturb schema fidelity, while a better description remains
co-located with capability classification and scheduling. Composite tools are especially sharp:
their nested steps currently carry tool names back into the service, so translating only the
top-level call would leave the coupling intact.

MCP client information offers a useful content-negotiation hint. It is self-reported and cannot
establish identity or authority. The two supported MCP revisions also have different state
semantics. `2025-11-25` has one initialized connection-bound workspace. `2026-07-28` requires
request-local metadata and explicit workspace handles. Any adaptive surface must preserve those
differences rather than creating a new implicit session.

The desired architecture has one concern at each layer:

```text
MCP call in one SurfaceProfile
    -> canonical OperationId + canonical arguments
    -> governance, scheduling, and operation execution
    -> zero or more policy-free MechanismId browser commands
    -> canonical operation result
    -> result encoded in the same SurfaceProfile
```

## Decision

### 1. Separate the three vocabularies

Ghostlight has three explicit contracts.

`SurfaceProfile` is the flat model-facing contract at the MCP edge. It owns ordered tool
declarations, names, input and output schemas, descriptions, annotations, examples, call decoding,
and result encoding. It does not own capability requirements, policy, browser routing, or
execution.

A programmable object model may instead use a stateful runtime adapter. That adapter owns only
its external proxy, locator-plan, handle, and turn lifetimes. Pure object construction stays local.
Every terminal browser observation or effect still becomes one canonical operation before
governance or execution. A stateful runtime is not approximated by advertising a flat tool
dictionary under the same client name.

`OperationId` is the protocol-neutral product contract between the edge and service. Each
operation has canonical arguments and a canonical result vocabulary. The service operation
registry owns validation, workspace use, capability requirements, resource resolution,
scheduling, handler selection, provenance, post-processing, and result normalization.

`MechanismId` is the policy-free browser command contract between service execution and the
extension. A leaf operation may emit one mechanism. A semantic or composite operation may emit
none, one, or a response-dependent sequence. The mechanism layer reports physical outcomes; the
operation layer turns them into canonical product results.

These are not three copies of one registry. Each is authoritative for a different boundary:

- the edge is the only authority for model-facing surface declarations and translations;
- the service is the only authority for canonical operation availability and enforcement; and
- the browser adapter is the only authority for supported physical browser mechanisms.

An identifier from one layer must not be used as an identifier in another. In particular, a
surface tool name is not an `OperationId`, and an `OperationId` is not serialized to the extension
as a mechanism name by convention.

### 2. Make SurfaceProfile versioned and edge-local

Every flat surface has a stable profile id, a positive profile version, a supported MCP revision
set, and an evidence record. Two Ghostlight profiles have different jobs:

- `ghostlight-legacy/v1` is the frozen compatibility oracle for the 25-tool catalog Ghostlight
  advertises before this decision is implemented. It is the extraction baseline and remains an
  explicit compatibility profile.
- `ghostlight-native/v1` is the one-to-one delight surface over the canonical operation kernel. It
  has the 12 core tools defined in Decision 4 and becomes the unknown-client fallback only after
  implementation and journey-evaluation gates pass.

During the extraction stages, the default temporarily remains `ghostlight-legacy/v1`. Switching
the unknown-client fallback to `ghostlight-native/v1` is a deliberate rollout gate, not a rename
of the 25-tool surface. Vendor-paired profiles are added only after their names, schemas,
descriptions, and result behavior have a dated discovery artifact and an accepted evaluation.
They are described as evidence-tuned, not training-matched, unless the vendor publishes that
training contract.

The MCP edge selects a profile in this strict order:

1. an explicit operator override naming an installed, revision-compatible profile;
2. an exact allowlisted match on `clientInfo.name`, an evidence-bounded client version range, and
   the negotiated MCP revision; or
3. `ghostlight-native/v1` as the final fallback profile.

Matching is exact and table-driven. There is no substring, model-name, executable-name, extension
id, or behavioral guess. A missing version, an unknown version, or a tuple outside the evidence
range does not activate a vendor profile. It uses the active Ghostlight fallback: legacy during
the bounded extraction, then native after the native rollout gate. An explicit but
revision-incompatible override fails with a corrective error instead of silently changing the
requested surface.

`clientInfo` remains untrusted presentation data. It may select an edge-local encoding in the same
way that a protocol version selects a wire grammar. It never grants authority, selects policy,
binds a workspace, chooses a browser, changes scheduling, or proves that the claimed vendor client
is present. Selecting any profile cannot make an otherwise denied operation run.

Profiles are immutable at their compatibility boundary. A changed tool name, parameter name,
parameter type, enum, ordering, or result contract requires a new profile version. Descriptions
and other guidance may improve deliberately under ADR-0094 when the profile promises Ghostlight
guidance rather than an exact captured description. The profile version and discovery artifact
make that promise explicit.

### 3. Select one surface at the lifetime the MCP revision permits

For MCP `2025-11-25`, the profile is selected during initialization and remains fixed for that
initialized connection. The edge preserves it across a service reconnect together with the
handler's implicit `WorkspaceId`. It is not reclassified from later calls.

The edge never unions two profiles or advertises duplicate dialects on one connection or request.

For MCP `2026-07-28`, the profile is selected independently from each request's immutable
metadata. A list request is rendered in that request's profile. A call request is decoded and its
result encoded in that same request's profile. Concurrent requests with different client
information cannot overwrite or inherit a shared profile slot. A call is never interpreted using
the profile selected for an earlier list request.

An edge cache key for a rendered catalog includes at least the profile id and version, MCP
revision, service catalog generation, and every authority or restriction fact already required by
ADR-0096. A profile selection does not create a service catalog generation. `CatalogChanged`
still means canonical service availability changed, after which the edge renders the selected
profile again.

Profile support is revision-specific. If an exact captured compatibility surface cannot carry the
explicit `workspaceId` continuity required by `2026-07-28`, that profile is not eligible for that
revision. Ghostlight does not add an undeclared parameter and still call the surface exact. It
also does not invent an implicit `2026-07-28` workspace. The revision-compatible Ghostlight
fallback remains available until a captured surface can express the required state honestly.

The selected `SurfaceProfile` implementation and its mutable state are not stored in
`WorkspaceRegistry`, `WorkContext`, scheduler keys, browser session state, or extension routing
state. One invocation may carry a bounded presentation tuple containing profile id, profile
version, and external call name for corrective copy and audit. That tuple is not a lookup or
execution key. `WorkspaceId` in the compatibility `guid` field remains the sole browser workspace
routing and ownership key.

### 4. Normalize every call into a semantic OperationId

`OperationId` uses Ghostlight's semantic product vocabulary rather than inherited surface names.
The native core is a one-to-one projection of these 12 operation families:

| `ghostlight-native/v1` tool | Canonical `OperationId` |
|---|---|
| `browser_context` | `browser.context` |
| `browser_tabs` | `browser.tabs` |
| `browser_navigate` | `browser.navigate` |
| `browser_snapshot` | `browser.snapshot` |
| `browser_read` | `browser.read` |
| `browser_find` | `browser.find` |
| `browser_screenshot` | `browser.screenshot` |
| `browser_act` | `browser.act` |
| `browser_fill` | `browser.fill` |
| `browser_wait` | `browser.wait` |
| `browser_flow` | `browser.flow` |
| `browser_dialog` | `browser.dialog` |

Specialist functionality joins the native core only through versioned capability packs fixed
before catalog rendering, such as files, diagnostics, execute, media, presentation, precision
input, or multi-browser support. A pack is advertised only when its canonical operations and
physical mechanisms exist. Unsupported work is omitted or rejected, never represented by a stub.

An operation family retains typed concrete intent variants. `browser.act` click, key input,
scroll, hover, drag, and other actions remain distinct variants for capability classification,
scheduling, result meaning, and audit. A compatibility profile may group or split those variants,
but it cannot erase their canonical discriminant.

The first implementation uses a closed, serializable `OperationId`, a typed intent discriminant,
and canonical JSON arguments and results validated against the operation registry. This creates a
real typed identity boundary without requiring a separate Rust argument struct for every operation
in the first migration. Families may gain stronger argument types later without changing a
surface profile.

The edge validates the selected surface schema, translates the call, and sends only canonical
operation data across the owner bridge. The service validates the canonical form again before
workspace admission, governance, or dispatch. An unknown surface tool fails at the edge. An
unknown or malformed canonical operation fails at the service before browser traffic.

Translations are recursive. A script or batch becomes a canonical composition containing
canonical operation steps. No nested surface `tool` or `name` string crosses the bridge. Each
composition step still re-enters the same operation pipeline, takes the authority snapshot owed by
ADR-0080, and receives the same scheduling and audit treatment as a direct call. Different surface
front doors may encode the same canonical composition result in their own trained result shapes.

The service projects ordered canonical operation availability and workspace-use facts after
governance and restriction filtering. The edge uses that projection to decide which tools in the
selected profile are truthful to advertise. A grouped surface tool is visible when at least one
mapped operation is reachable; per-call enforcement remains authoritative for every mapped
operation. The edge never edits a surface action enum to imply policy enforcement.

Successful service outcomes carry canonical operation results, not MCP `content` blocks or a
vendor result wrapper. Denials, holds, cancellation, not-dispatched outcomes, and outcome-unknown
states remain semantic terminal outcomes. The selected profile encoder renders them without
weakening their disposition or recovery guidance.

### 5. Put delight defaults in canonical operation semantics

A safe default that improves every surface belongs to the canonical operation, not to duplicated
compatibility adapters. Navigation to a new URL, reload, and equivalent navigation operations
default to adaptive settling enabled with a 10-second maximum budget unless the canonical call
explicitly supplies another supported policy. The implementation does not stack a legacy load
wait and a second settle wait; the one navigation-readiness deadline begins at dispatch.

Settling is a bounded postcondition observation, not proof that navigation failed. If Ghostlight
proves a final authorized document commit but the document does not reach the settle criterion
before the bound, navigation still succeeds and its canonical result records readiness as
`timed_out`. If no final commit is proven by the navigation deadline, the result is a known
no-effect failure when that is provable and otherwise `outcome_unknown`. A mechanism failure to
start navigation remains a failure. These states must not be collapsed.

Profiles may omit settle inputs and receive the canonical default. They encode settle metadata
only where their result contract permits it; preserving an exact legacy result shape is allowed.
A compatibility idiom that explicitly separates navigation from `waitForLoadState` or an
equivalent wait maps to a declared readiness policy and must not receive a duplicate hidden wait.
The default does not add a parameter to a vendor signature.

### 6. Keep browser mechanisms semantic and policy-free

The service sends typed mechanism requests keyed by `MechanismId`. Browser outbound code may
switch on that enum for delivery, resource proof, screenshot handling, and compatibility
serialization. It does not switch on model-facing tool names.

The extension dispatches mechanism identifiers to Chrome API and CDP implementations. It contains
no `SurfaceProfile`, client classifier, capability requirements, governance decision, audit
policy, or model guidance. It continues to receive `WorkspaceId` only through the existing
compatibility routing field and remains policy-free under ADR-0005.

The new mechanism wire is additive and feature-negotiated. A new adapter advertises a versioned
mechanism-request feature and accepts both semantic mechanism requests and the covered legacy
`tool_request` aliases during the adapter compatibility window. The service sends semantic
mechanisms only to a browser session that advertised the feature. It serializes the isolated
legacy alias for an older covered adapter. Legacy aliases are compatibility data, not a second
execution path.

Removing an alias requires the normal ADR-0093 compatibility-range evidence. A service release
must not create an extension flag day.

### 7. Preserve the legacy surface byte-for-byte through extraction

The architecture extraction does not itself change the current Ghostlight tool contract. At the
cutover baseline, `ghostlight-legacy/v1` emits the same tool count, order, names, parameter names,
types, enums, descriptions, annotations, examples, input schemas, and output schemas as the
pre-extraction registry, subject only to the existing MCP-revision workspace augmentation and
authority filtering.

The trained identity snapshot remains the oracle for `ghostlight-legacy/v1`. Exact MCP transcript
tests remain the oracle for protocol envelopes. Existing extension behavior remains the oracle
when the service uses a legacy mechanism alias. `ghostlight-native/v1` has its own declaration,
operation-mapping, result, and journey oracles.

This byte-for-byte migration gate does not repeal ADR-0094. A later deliberate guidance change may
update descriptions and its regression snapshot without changing trained identity. It must be
reviewed as a guidance change, not hidden inside the architectural move.

ADR-0094's inherited trained identity boundary now applies to `ghostlight-legacy/v1`, not to the
new native surface or to internal operation or mechanism names. Its advisory annotation rules
remain unchanged, but the owning declaration moves from the service `ToolDescriptor` to the edge
`SurfaceProfile`. ADR-0094's rejection of runtime changes made only for a registry score remains
unchanged.

### 8. Amend catalog authority without duplicating it

ADR-0096 said the service was the sole tool-catalog authority and the MCP edge never owned a
second registry. This decision narrows those statements:

- the service remains the sole authority for canonical operations, availability, governance
  requirements, workspace behavior, and execution metadata;
- the MCP edge becomes the sole authority for versioned model-facing surface declarations and
  translation; and
- neither side may duplicate the other's layer as a fallback registry.

ADR-0096's rejection of compatibility profiles is amended only for these edge-local,
evidence-gated `SurfaceProfile` encodings. Its rejection of protocol behavior shims, implicit
application identity, duplicate same-layer catalogs, and service-side MCP lifecycle remains in
force. The service remains protocol-neutral and the MCP connector still does not depend on core.

The operation vocabulary shared across the typed owner bridge belongs in the transport contract.
It is product data, not MCP vocabulary. Changing `Start` from a surface name and raw arguments to
a canonical operation, and changing catalog declarations to operation availability, breaks the
current owner-bridge wire. The implementation increments `BRIDGE_MAJOR` from 1 to 2. Old/new
combinations fail loudly during hello. They do not guess, partially decode, or silently fall back.
The connector and service update is coordinated through the existing local supervisor and release
path. Permanent dual-major bridge support is not added.

### 9. Keep governance and audit canonical

Capability classification, sacred-domain checks, policy decisions, hold and panic behavior,
attention state, resource resolution, scheduling, post-dispatch checks, and result provenance use
only the canonical operation and current service authority. Two surface calls that translate to
the same operation and arguments receive the same decision and execution semantics.

Audit records add the canonical `OperationId` and concrete intent variant as the replayable
decision identity. They may retain the external tool name and bounded profile id as presentation
fields so a human can diagnose what the client called. Those presentation fields cannot
participate in authorization, workspace ownership, routing, or simulation decisions. Policy
simulation replays the canonical operation and variant; historical records without them may use
one explicit legacy translation table.

Profile result encoders may improve wording and preserve trained shapes. They may not turn a
denial into success, invite retry after an uncertain effect, suppress a material side effect, or
claim a stronger settle or delivery result than the canonical outcome.

No raw `WorkspaceId` is added to logs or audit. Existing redaction, local-only storage, and
never-phone-home rules remain unchanged.

### 10. Roll out by stable seams and evidence gates

Implementation proceeds in stages. Each stage leaves one working stack and passes its own
compatibility gate.

1. Pin the current `ghostlight-legacy/v1` `tools/list`, exact MCP transcripts, canonical call
   outcomes, and legacy browser wire. Accept the semantic `OperationId` and `MechanismId`
   vocabulary before moving behavior.
2. In one coordinated bridge-major-2 cutover, introduce canonical operations and operation
   availability, move `ghostlight-legacy/v1` rendering and translation to the MCP edge, and convert
   direct and nested calls. The visible legacy surface and extension wire remain unchanged.
3. Prove byte-for-byte legacy and all-open equality, then delete the co-located service surface
   declarations. This cleanup does not change bridge major 2 or create another catalog format.
4. Route operation handlers through typed mechanisms while still serializing the legacy extension
   aliases. No model-facing or adapter-visible change is required for this stage.
5. Add the feature-negotiated mechanism wire and dual-reading adapter. Verify old adapter/new
   service and new adapter/covered old service combinations required by ADR-0093.
6. Implement the 12-tool `ghostlight-native/v1` projection and its canonical result rendering.
   Unknown clients continue to receive legacy until the native declaration, revision, workspace,
   and journey gates pass. Switch the fallback only as a deliberate release change; keep legacy
   explicitly selectable.
7. Add external adapters by the separate rollout classes below. Unknown or unsupported clients
   receive `ghostlight-native/v1` after its fallback gate.
8. Remove a legacy mechanism alias only after the declared adapter compatibility range excludes
   every adapter that needs it.

The investigated external surfaces are not one interchangeable adapter class:

- The captured 22-tool Claude Cowork and Claude-in-Chrome dictionary is a flat compatibility
  profile. It may use the exact allowlisted `clientInfo` classifier after every advertised variant
  maps and the missing multi-browser or saved-workflow capabilities are omitted rather than
  faked.
- The official Playwright MCP declarations form a flat evidence profile for explicit evaluation
  of names, target pairing, results, capability packs, and readiness. Playwright MCP is a server,
  not proof of one paired client identity, so this evidence profile is not automatically selected
  from ordinary client information. Promoting it to a compatibility target requires its own
  client evidence and rollout gate.
- The captured Codex Chrome surface is a 136-member stateful runtime, not a flat MCP profile. A
  Codex runtime adapter requires its authenticated integration, versioned proxy object model,
  handle and turn lifetimes, and a complete member-level mapping ledger. Each terminal call is
  audited as its canonical operation, never only as enclosing JavaScript. Bare Codex `clientInfo`
  must not select a fabricated flat dictionary. Without the runtime integration, Codex receives
  the normal native MCP fallback.
- Gemini-in-Chrome evidence establishes capabilities and human-control flows but no exact
  model-visible dictionary. Ghostlight therefore creates no exact Gemini profile. Gemini clients
  receive the native fallback unless later primary evidence supports a versioned profile.

Automatic activation of a flat vendor profile requires all of the following:

- a dated raw discovery artifact with tool names, order, complete schemas, descriptions, and
  result evidence from the paired client version range;
- a reviewed total mapping from each advertised call shape to canonical operations, including
  nested composition and unsupported-capability behavior;
- golden declaration and translation tests for that profile;
- protocol-revision and workspace compatibility without undeclared parameters or implicit state;
- journey evaluations against `ghostlight-native/v1` for selection accuracy, completion,
  recovery, call count, and unsafe or invalid calls; and
- no weaker governance, audit, scheduling, delivery, or uncertainty semantics.

A profile that is incomplete, lossy, or merely plausible may remain available for explicit
development evaluation. It is not allowlisted for automatic selection. A profile is disabled or
its version range is narrowed when new evidence invalidates its capture.

`ghostlight-native/v1` is also evaluated before becoming fallback. It is the delight-oriented
general surface, not a claim of one universal optimum for every model. Its descriptions and
metadata should maximize purpose, tool choice, side-effect awareness, stable recovery, and low
ambiguity while its canonical operation mapping remains one-to-one and complete.

## Consequences

- Evidence-matched clients can receive an appropriate flat profile or stateful runtime without
  making external vocabulary an internal architecture.
- Unknown clients receive `ghostlight-native/v1` after its explicit fallback gate rather than a
  guessed vendor dialect. `ghostlight-legacy/v1` remains the compatibility oracle.
- Governance, scheduling, and audit have one canonical identity even when several tool signatures
  reach the same browser behavior.
- The extension becomes easier to keep thin because it implements browser mechanisms, not model
  tools.
- Surface and mechanism compatibility can evolve independently, with explicit profile versions,
  bridge major compatibility, and ADR-0093 adapter ranges.
- The first extraction is substantial and touches composition, catalogs, result encoding, audit,
  and browser delivery. Staging and byte-for-byte gates are therefore mandatory.
- Exact compatibility profiles may be unavailable on a newer MCP revision until their captured
  state model can express Ghostlight's workspace invariant. Correctness wins over automatic
  matching.

## Rejected alternatives

### Keep the inherited tool names as the canonical operation and extension vocabulary

Rejected because it preserves the coupling this decision is meant to remove. It also makes a
future surface translator cosmetic: nested calls, governance text, scheduling helpers, and
extension handlers would still depend on one vendor's names.

### Put profiles and client classification in the service

Rejected because it leaks MCP client metadata and model-facing compatibility into the
protocol-neutral center. It also invites profile state to become workspace or authority state.

### Store the selected profile on a workspace

Rejected because a workspace is browser continuity, not client or model identity. It would break
concurrent `2026-07-28` request isolation and make a presentation choice affect routing state.

### Fuzzy-match clientInfo or treat it as authentication

Rejected because `clientInfo` is self-reported and version drift is material to a trained schema.
Exact allowlisted negotiation is useful; identity, authority, and heuristic guessing are not.

### Copy each vendor's complete execution pipeline

Rejected because governance, scheduling, browser delivery, and audit would drift. Profiles share
one canonical operation pipeline and differ only at the edge translation boundary.

### Use surface-name strings as OperationId

Rejected because a string newtype would make the bridge look typed while preserving the semantic
coupling. Mixed action tools must normalize to distinct semantic operations.

### Add Ghostlight workspace fields to an exact vendor schema

Rejected because the result would not match the captured training surface. On `2026-07-28`, a
profile either expresses explicit continuity honestly or is not eligible.

### Infer a `2026-07-28` workspace from connection, client, tab, or prior request

Rejected by ADR-0096 and this decision. It weakens ownership and creates request-order state that
the revision deliberately removed.

### Put capability classification or policy in the extension

Rejected by ADR-0005. A mechanism reports what the browser did or could not do. The service alone
decides whether it may be attempted.

### Cut the extension over to MechanismId in one release

Rejected because the service and browser adapter version independently. Feature negotiation and a
bounded legacy alias table preserve the declared compatibility range without retaining two
execution implementations.

### Enable a vendor profile from names and schemas alone

Rejected because descriptions, output behavior, workspace semantics, nested calls, and recovery
guidance also shape model success. Discovery plus journey evidence is the activation gate.
