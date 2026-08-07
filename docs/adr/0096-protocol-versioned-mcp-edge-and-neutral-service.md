# ADR-0096: Protocol-versioned MCP edge and neutral service boundary

Status: Accepted

Date: 2026-08-04

Supersedes: ADR-0045 Decision 2's raw-MCP handshake replay, ADR-0051 Phase 3's merged
agent role, ADR-0049's support for `2024-11-05`, `2025-03-26`, and `2025-06-18` plus
pre-initialize tool compatibility, and ADR-0068's proposed cancellation design with the narrower
cancellation behavior below

Amends: ADR-0024 Decision 2, ADR-0030 Decisions 1/2/4/6/8, ADR-0033 Decisions 1/6,
ADR-0034 Decisions 2/6, ADR-0045 Decisions 1/3, ADR-0046 Decisions 1-3,
ADR-0047 Decisions 2-5, ADR-0049 Decisions 1/5, ADR-0060's lifecycle binding,
ADR-0062's restart-continuity scope, ADR-0065 Decision 4, ADR-0066 Decisions 1-5,
ADR-0080 Decisions 3/6/9,
ADR-0085 Decisions 2/3, ADR-0090's definition of a known tab, and ADR-0095 Decisions 1/2

Builds on: ADR-0044 and ADR-0077

## Naming amendment (2026-08-04)

The process topology and responsibilities below are unchanged. The two shore executables and
their crate directories now use explicit connector names:

- `ghostlight-mcp-connector` in `crates/mcp-connector/` owns the MCP stdio shore.
- `ghostlight-browser-connector` in `crates/browser-connector/` owns the Chromium native-host
  shore.
- `ghostlight` remains the persistent protocol-neutral service and CLI.

References below to `ghostlight-mcp`, `ghostlight-relay`, `crates/mcp/`, and `crates/relay/`
describe the names used when this decision was first implemented. They do not create compatibility
aliases or additional executables. Install and upgrade rewrite current client and native-host
registrations to the connector names. Historical `ghostlight-relay --role agent` recognition stays
only as an installer migration input.

### Closed host transport clarification (2026-08-04)

The MCP client owns the connector process and its stdio lifetime. A living
`ghostlight-mcp-connector` can reconnect its typed service bridge for future calls, but the
service cannot restore stdio after the MCP client closes or retires that connector. Service and
browser health therefore do not prove that one client's MCP transport is alive.

Install changes client configuration only. A client may keep its current connector, replace it,
or retain cached Ghostlight tool declarations after that process exits. A listed tool is not a
liveness signal. Recovery guidance belongs at the date-named MCP shore: after `Transport closed`,
the caller stops and reopens Ghostlight through that MCP client's normal reconnect or restart
mechanism. Starting a standalone connector does not repair the client's closed stdio and may
create a different workspace. Before retrying an effectful call, the caller inspects browser state
because the earlier call may have started.

Doctor reports the existing aggregate live-edge count as point-in-time information. It does not
add client identity, a connector registry, or another lifecycle mechanism solely for attribution.

## Context

Ghostlight currently passes the MCP client's raw stdio stream through
`ghostlight-relay --role agent` to the persistent service. The service captures and replays the
MCP initialize exchange, parses JSON-RPC, owns protocol session state, invokes tools, and renders
MCP responses.

That shape does not survive the difference between the exact MCP revisions Ghostlight now needs
to support:

- `2025-11-25` has an initialize/initialized lifecycle and connection-negotiated state.
- `2026-07-28` removes protocol sessions and initialize. It carries protocol version and client
  capabilities on every request, adds `server/discover`, requires `resultType`, moves list changes
  to `subscriptions/listen`, and requires explicit handles for cross-call application state.

Putting both models in the service would leak protocol dates and JSON-RPC lifecycle through
governance, scheduling, ownership, and browser execution. Keeping the edge as a byte pipe would
leave it unable to own either model cleanly.

The answer is not more services. The product needs three meaningful shores and one neutral
center. Existing compatibility behavior is not sacred. This decision deliberately removes
behavior when a stricter, smaller model is safer or clearer.

## Decision

### 1. Use exactly three Ghostlight executables

```text
MCP client
  <stdio>
ghostlight-mcp
  <owner-only typed local IPC>
ghostlight service
  <existing browser IPC>
ghostlight-relay + policy-free Chromium extension
  <CDP>
browser
```

The executable responsibilities are:

1. `ghostlight-mcp` owns stdio, JSON-RPC, exact MCP revision behavior, request correlation,
   response rendering, and reconnect for future calls.
2. `ghostlight` owns the canonical tool registry, workspaces, governance, audit, scheduling,
   browser coordination, and protocol-neutral outcomes.
3. `ghostlight-relay` remains the browser native host. Its agent role is deleted. The extension
   remains policy-free browser mechanism.

There is no Agent API process, route broker, job manager, event bus, schema service, or fourth
runtime. Logical boundaries inside `ghostlight` remain code boundaries unless a real independent
lifecycle or trust boundary later earns another process.

The separation is structural. Executable entry points and crate dependencies enforce which shore
can reach which responsibilities. There is no process-global role enum, `OnceLock`, or runtime
role marker that asks a monolithic process which product it is pretending to be.

`ghostlight-mcp` retains `--instance <name>` and `GHOSTLIGHT_INSTANCE` endpoint selection. Named
instances remain a test-isolation seam under ADR-0044 and ADR-0065. Each engine still has one
adapter/control endpoint and one extension endpoint. Neither becomes public or network-facing.

At cutover, every MCP client entry and the ADR-0095 MCPB launcher starts `ghostlight-mcp`. The MCPB
contains `ghostlight-mcp`, `ghostlight`, and `ghostlight-relay` for each packaged platform because
first launch still installs the service and browser native host. The old agent command is not kept
as a fallback executable path.

### 2. Support exactly two date-named protocol handlers

One `ghostlight-mcp` binary contains shared JSON-RPC/stdio framing, one internal service client,
and two static modules:

- Rust `mcp_2025_11_25`; prose/external `mcp-2025-11-25`
- Rust `mcp_2026_07_28`; prose/external `mcp-2026-07-28`

No handler, module, or state machine is named `legacy`, `modern`, `v1`, or `v2`. There is no crate,
trait hierarchy, plugin registry, code generator, version DSL, or MCP SDK per revision. A small
enum and match select the handler.

The shared runtime owns only proven common behavior: JSON-RPC framing, stdio discipline, pending
request correlation, the internal bridge, and storage of service-supplied catalog projections.
Each date-named module owns its legal methods, lifecycle, metadata, capabilities, error mapping,
result envelopes, caching fields, and notifications.

Handler selection is deterministic:

- `server/discover` is served as the `2026-07-28` compatibility probe and does not bind the stdio
  process to an era.
- a valid `initialize` for `2025-11-25` selects `mcp_2025_11_25` for that stdio process;
- the first non-discovery request with valid `2026-07-28` per-request metadata selects
  `mcp_2026_07_28`;
- after selection, contradictory lifecycle or revision mixing is rejected;
- the `2026-07-28` handler still validates every request independently and never supplies missing
  metadata from the connection or an earlier call.

The `2025-11-25` handler requires initialize before tools, resources, or prompts are used. It keeps
only lifecycle exceptions allowed by the revision, such as ping. The currently accepted
`2024-11-05`, `2025-03-26`, and `2025-06-18` compatibility revisions are removed. A client asking
for one receives the normal `2025-11-25` negotiation response and may accept it or disconnect.
There are no compatibility profiles for behavior Ghostlight no longer intends to support.

The immutable official dated specifications and schemas are normative. The official conformance
suite is an additional external oracle when its runner supports the product transport under test.
Its current server runner accepts an HTTP URL, not a stdio command, so it is not evidence for this
stdio cutover and no such result is claimed. Official SDKs and reference implementations may be
studied for observable behavior and technique, but Ghostlight remains a hand-rolled
implementation. No MCP SDK is introduced and no reference code is copied.

### 3. Use one small internal bridge and no work platform

The MCP edge and service use one tagged, explicitly versioned vocabulary in the existing
`ghostlight-transport` crate. It is private, owner-only local IPC under ADR-0077, not a public
Agent API.

The minimum messages carry:

- bridge hello and fail-loud bridge-major compatibility;
- open and cleanly release the implicit `2025-11-25` workspace;
- a catalog projection request/response plus one `CatalogChanged` generation signal;
- `Start` with a bridge sequence, normalized operation, arguments, workspace handle when needed,
  and immutable request context;
- `Started` with a service-minted `WorkId`, or a semantic rejection;
- terminal semantic outcome;
- `Cancel` for an active `WorkId`.

The bridge sequence exists only until `Started` correlates the response. `WorkId` exists only for
the active call. The MCP JSON-RPC id remains entirely in `ghostlight-mcp`.

Inside the service, each admitted bridge stream has one bounded active-work map from `WorkId` to
its cancellation token. Each work future owns its normalized call state and exact response sender,
and uses the existing `CommandScheduler`; there is no global work table, retained-result window,
second queue, repository interface, or durable work database. The cancellation entry disappears
when the call settles. The stream's own writer routes the result, so there is no connection
registry or connection-generation type.

The normal path is:

```text
MCP request arrives at ghostlight-mcp
  -> Start(call sequence, click, arguments, workspace, request context)
  <- Started(call sequence, T01)
  -> existing service pipeline, governance, scheduler, and browser execution
  <- terminal semantic outcome for T01
  -> exact revision handler renders the original JSON-RPC response
```

`T01` is an illustrative label. The real `WorkId` is opaque and unique within the admitted
stream. PID is diagnostic only. OS peer credentials admit the local client, and
`ghostlight-mcp` retains the current anti-squat proof that it reached the real service. Neither
PID nor self-reported MCP client information grants authority.

### 4. Normalize application continuity as WorkspaceId

The service does not need a stable adapter/process identity. The existing `SessionGuid` concept
is replaced by one product identity with an honest lifetime: `WorkspaceId`.

`WorkspaceId` is an opaque, service-minted handle for the browser workspace and the state that
meaningfully follows it: owned tabs, group/window placement, attention circuit, and workspace
recovery. It is not a connection id, protocol session, caller identity, or authentication token.
The existing session/workspace registry is reshaped for this role; a generic handle manager is not
added.

Within the already admitted same-user boundary, `WorkspaceId` is an unguessable bearer capability
for that workspace. It is minted with a CSPRNG, compared exactly, and never written raw to logs or
audit. OS peer credentials and service anti-squat still establish the local trust boundary; the
handle supplies product ownership inside it.

For browser-adapter skew, the existing browser-wire field named `guid` may keep that spelling
while carrying `WorkspaceId`. Domain code and the MCP-service bridge use the honest name; covered
older adapters do not need a flag-day field rename.

That compatibility `guid` is the sole browser-extension routing key for a Ghostlight workspace.
Current tool and group frames do not send a top-level presentation/routing `clientKey`. The
extension may retain an additive parser for older service frames during adapter-version overlap,
but current routing never falls back to it. A human-readable client label may title a visible
group or enrich audit context; it never selects a workspace, group, tab, browser, scheduler key,
or authority. Two workspaces with the same client name therefore remain distinct all the way to
the extension.

Browser-adapter skew may separately preserve the scheduler resource spelling
`{kind: client_topology, clientKey: <WorkspaceId>}`. That nested field carries `WorkspaceId`, not
the human client label, and does not restore client-name presentation routing. Rust domain code
names the resource for what it means: workspace topology.

The two MCP shores normalize differently:

- `mcp_2025_11_25` obtains one `WorkspaceId` after successful initialization and supplies it on
  each stateful service call. This preserves the useful implicit workspace experience at that
  revision's shore.
- `mcp_2026_07_28` never infers workspace continuity from stdio or process identity. The
  context-creating tab tools mint and return `workspaceId`. Every later stateful tool passes it as
  an ordinary optional tool argument. Explicit tab, image, recording, and similar handles are
  verified as members of that workspace; they never substitute for its ownership check.

An explicit input `tabId` is verification-only. The workspace registry must already own it;
unknown and cross-workspace ids return the same denial before governance lookup or any browser
frame. Ownership can grow only when the browser shore settles a successful, exactly correlated
`tabs_context_mcp` or `tabs_create_mcp` result. It atomically adopts only the declared root
`structuredContent.tabId` and `structuredContent.tabs[].tabId` values after mapping them through
the exact browser slot. Generic recursive field discovery and admission-time first-touch adoption
are prohibited. A browser-process restart purges that browser's tab membership before its
replacement is published; a service restart loses all in-memory membership as described below.

If a `2026-07-28` stateful call cannot resolve a workspace from its arguments, it returns a
corrective error that points to the context-creating tool. It never borrows the workspace of a
previous request. Adding optional `workspaceId` parameters follows the existing additive growth
rule: no trained tool or existing parameter name, type, enum, or ordering changes.

Workspace state remains bounded. Active work pins it. A clean `2025-11-25` edge shutdown releases
its implicit workspace. An unclean bridge loss starts the same bounded idle grace used for an
explicit `2026-07-28` workspace, allowing the retained handle to reattach without making
workspaces permanent. Exact bounds are implementation constants tested for cleanup and
active-work safety. A service-process restart loses the in-memory registry: the `2025-11-25` edge
obtains a fresh implicit workspace for future calls, while a `2026-07-28` client receives the
normal invalid-handle correction and explicitly creates another.

Every call becomes one immutable protocol-neutral `WorkContext` before registry lookup,
authorization, scheduling, or execution. It contains only product facts the service consumes:
workspace, operation, client presentation for this call, and a validated tighten-only restriction
when present. It contains no MCP revision, JSON-RPC id, lifecycle method, or wire capability bag.

For `2026-07-28`, protocol version, capabilities, client information, log level, and optional
restriction are read from that request every time. Concurrent requests cannot overwrite a shared
client slot. `clientInfo` is presentation/audit context only. Current service authority is applied
at every scheduled action; a workspace never freezes an old policy snapshot.

The service remains the sole tool-catalog authority. It projects the same ordered schemas,
annotations, examples, and grant filtering to both handlers. A `2025-11-25` initialized workspace
can cache its view until `CatalogChanged`. A `2026-07-28` list request gets the view for that
request's immutable restriction and the required cache metadata. A restriction-dependent result
uses `cacheScope: private` and `ttlMs: 0` unless the cache key demonstrably includes the complete
restriction and authority context. The MCP executable never owns a second registry.

### 5. Implement only the cancellation and reconnect behavior that earns state

`2026-07-28` request cancellation and subscription closure are part of the base handler, not
future scaffolding. This decision accepts the narrow behavior proposed by ADR-0068:

- `ghostlight-mcp` maps the MCP request id to the active `WorkId` and suppresses an obsolete
  response after cancellation;
- queued work retires before dispatch when possible;
- a composition stops after its current atomic step;
- an already dispatched browser effect is not rolled back or repeated; it drains and audits;
- cancellation is cooperative and never reported as proof that an effect did not happen.

The same service mechanism may honor `2025-11-25` cancellation without moving protocol parsing
inward. `subscriptions/listen` cancellation closes that handler's long-lived response and releases
its edge-local subscription state.

Reconnect is deliberately simpler than ADR-0045's raw-stream model. If the edge/service bridge
breaks, `ghostlight-mcp` completes every pending synchronous request with a revision-appropriate
transport failure whose semantic disposition is `outcome_unknown` whenever execution may have
started. It never replays the call and never waits for a result after reconnect. If the old service
is still alive, accepted work receives disconnect cancellation, then follows the same queued,
atomic, composition, drain, and audit rules above.

The edge reconnects only for future calls. It preserves the selected MCP handler and, for
`2025-11-25`, its `WorkspaceId`; it does not replay MCP initialize bytes into the service. First
connect and reconnect remain bounded and keep the existing supervisor self-heal. Parent-death
cleanup and doctor/reaper behavior move from the old agent relay to `ghostlight-mcp`.

A caller response deadline does not discard the service pipeline. At 60 seconds the bridge emits
one `outcome_unknown` terminal response, then keeps that same per-call future alive privately. An
effectful browser delivery has a 180-second settlement bound. Within that bounded window the
original landing check, post-processing, audit scope, scheduler lease, and workspace lease still
settle. There is no second client response, replay, result registry, or detached job identity.
Executor-generation proof is retained in the existing pending entry and scheduler quarantine, so
a later terminal acknowledgement clears uncertainty only for the exact executor, command,
request, and resource.

A service process crash cannot guarantee a terminal audit record for work whose in-memory scope
died with it. Ghostlight does not claim otherwise. Persisted audit events remain append-only, the
new service never fabricates success or retries the mutation, and the edge reports the unresolved
call as unknown. Exactly-once terminal audit remains required when the service process survives
long enough to settle the work, including edge disconnect and cancellation.

Browser-side reconnect is unchanged: `ghostlight-relay` keeps its Chrome reader, extension
identity replay, native framing, feature negotiation, multi-browser routing, and patient service
reconnect.

### 6. Do not confuse base work with optional MCP Tasks

An active `WorkId` is not an MCP Task. Ghostlight does not advertise the official Tasks extension
in this decision and adds no persistence for it.

Tasks require a separate accepted ADR against the then-current extension, client support,
durability, security, cancellation, ownership, TTL, and crash requirements. That future decision
may reuse `WorkId` or may not; this ADR does not pre-commit its storage or identity model.

The `2026-07-28` handler implements the base protocol, discovery, required result and cache fields,
and the subscribe/notify behavior Ghostlight actually advertises. It can render
`input_required` only after a concrete multi-round-trip product flow is accepted and implemented.
Unimplemented optional capabilities are absent from discovery rather than represented by empty
frameworks.

### 7. Preserve product invariants, not incidental compatibility

The break-and-rebuild must preserve these load-bearing product properties:

- the trained tool identity surface and additive-only growth rule;
- one canonical tool catalog, tool order, schemas, annotations, structured results, screenshots,
  receipts, provenance, and truthful errors;
- all-open as first-class behavior;
- governance, current-authority checks, sacred/hold/panic/attention behavior, redaction, and audit;
- tab ownership, explicit workspace isolation, window/group placement, stale recovery,
  multi-browser routing, presentation, recording, and bounded blob transport;
- ADR-0080 resource binding, fairness, lanes, leases, reentrant composition, draining, and
  uncertain-surface quarantine;
- local-only ingress, same-user peer admission, anti-squat proof, policy-free extension, one
  installed engine, browser-adapter version skew, and never-phone-home.

It intentionally does not preserve:

- support for MCP revisions older than `2025-11-25`;
- tool calls before the `2025-11-25` lifecycle reaches operation;
- implicit process-scoped state in `2026-07-28`;
- transparent completion of an in-flight synchronous call across an edge/service bridge break;
- byte-identical envelopes where exact revision compliance requires a better response;
- `ghostlight-relay --role agent` or raw MCP ingress in the service.

Tests that pin removed behavior are rewritten or deleted under this ADR. Tests that protect a
product invariant, safety property, or trained identity stay and must pass through the new path.

## Adversarial minimality result

Every retained mechanism has one concrete job:

- three executables exist because MCP lifecycle, persistent product state, and Chromium native
  hosting have independent lifetimes;
- two exact-date modules exist because their legal state machines conflict;
- `WorkspaceId` exists because application state crosses calls while connections do not;
- `WorkId` and one per-stream active map exist because cancellation must address accepted work;
- one catalog generation signal exists because Ghostlight advertises list changes;
- the existing scheduler exists because browser correctness requires resource ordering.

Everything else proposed during exploration is cut: no stable adapter id, PID binding, global
connection registry or application-level connection identity, global work table, retained
synchronous results, route broker, job platform, event bus, kernel crate, version crates, generic
ports framework, duplicate catalog, compatibility profiles, Task database, or speculative
continuation machinery. Transport-local generations still reject stale writes and stale browser
detach events; they never become application continuity. The one bounded continuation is the
original per-call future after its caller deadline, not a new continuation system.

Deleting any of the six retained mechanisms would remove a required protocol behavior, product
state, or correctness property. Adding another mechanism requires the same proof.

## Migration and gates

Implementation is one staged replacement, not two permanent stacks:

1. Pin the product invariants and the exact `2025-11-25`/`2026-07-28` protocol transcripts that
   must survive. Mark the intentional behavior breaks above in tests and release notes.
2. Extract protocol-neutral `WorkContext`, catalog projection, and semantic outcomes inside the
   existing service without creating a new core package.
3. Add the minimal typed bridge, `WorkspaceId`, active-work cancellation map, and pure seam tests.
4. Build `ghostlight-mcp` with strict `mcp_2025_11_25` behavior.
5. Add `mcp_2026_07_28`, explicit workspace handles, discovery, caching, subscriptions, and
   per-request metadata isolation.
6. Cut installer, MCPB, doctor, supervisor, dev loop, and Lightbox to the three-executable path;
   delete the old agent role, raw MCP service ingress, and handshake replay in the same cutover.

Cutover requires:

- immutable official dated-schema/spec-driven review plus exact stdio transcript tests for both
  revisions and every advertised optional capability;
- trained tool-schema fidelity and shared catalog projection from both handlers;
- all-open, governance, audit, ownership, workspace, scheduler, presentation, recording,
  browser-wire, and extension suites through the neutral boundary;
- two clients reusing one JSON-RPC id without cross-routing;
- concurrent `2026-07-28` requests with different metadata without cross-stamping;
- explicit workspace creation, reuse, isolation, expiry, and no-handle correction;
- cross-workspace tab/asset-handle mismatch rejection and raw-workspace-handle redaction;
- cancellation races before queueing, while queued, during an atomic effect, and between composed
  steps, with no replay and truthful audit;
- edge disconnect and service crash tests that produce no hang, duplicate effect, false success,
  or impossible audit claim;
- parent death, anti-squat, bounded reconnect/self-heal, browser reconnect, and an architecture
  assertion that PID is absent from routing and authority keys;
- exactly one MCP entry pointing to `ghostlight-mcp`, one adapter/control endpoint, one extension
  endpoint, one native-host path, and continued `compatibility.json` coverage;
- an architecture check that service execution code cannot name MCP revisions, JSON-RPC, stdio,
  or MCP Tasks, and that `ghostlight-mcp` cannot depend on governance or browser execution code.

The official conformance runner is an additional future gate when it can target Ghostlight's
shipping stdio transport. Its current server command accepts only an HTTP URL, so a runner result
is neither a cutover gate nor claimed evidence for this implementation.

The implementation batch under `docs/tasks/protocol-versioned-mcp-edge/` records cutover and gate
evidence. It is a work ledger, not another runtime layer.

## Implementation record (2026-08-04)

The accepted replacement is present in the working tree:

- `crates/mcp/` builds `ghostlight-mcp`; its `mcp_2025_11_25` and `mcp_2026_07_28`
  modules own the two exact wire state machines.
- `ghostlight_transport::bridge` is the one typed private bridge. JSON-RPC ids and MCP revision
  state do not cross it.
- `ghostlight_core::work`, `hub::bridge`, `hub::workspace`, and `tool` form the neutral service
  path. The former core `mcp` module and raw MCP service ingress are removed.
- `ghostlight-relay` has only the Chromium native-host path. MCP-client entries launch
  `ghostlight-mcp` with no role flag.
- The executable and crate graph carries the role boundary; the former process-global role marker
  is deleted rather than replaced.
- Browser routing uses only `WorkspaceId` in the compatibility `guid` field. Human client labels
  are presentation/audit data, and current tool/group frames omit the former top-level
  presentation/routing `clientKey`.
- Install, doctor, demos, release archives, package managers, npm, MCPB, the dev loop, Lightbox,
  and real-process tests are cut over to the three sibling executables.
- The adversarial runtime pass keeps the same service work future alive after an outward timeout,
  binds late terminal proof to the negotiated executor generation, atomically bounds each browser
  writer queue, orders hold/panic against the final tool enqueue under one safety lock, purges
  browser-process-local tab ownership on restart, and prevents a stale detach from removing its
  replacement's focus route. These changes reuse the existing work future, pending entry,
  scheduler quarantine, and workspace registry rather than adding a result service or route
  broker.
- The service counts active work independently from bridge streams, so idle shutdown cannot kill a
  late-settling call after its MCP shore has gone away. A single browser lifecycle gate orders
  attach, detach, stateful events, reply settlement, and process restart. A replaced connection may
  settle only the exact pending request it received; stateful events require the current live
  connection, and a process restart fails old pending calls before publishing the replacement.
- Input tab handles are verification-only. Successful creator replies establish membership
  synchronously at the browser shore before the result escapes; creator extraction is exact and
  cross-workspace adoption is atomic. Final multi-frame enqueue rechecks the exact connection's
  chunk capability, so an adapter replacement cannot inherit a predecessor's negotiation.
- Browser relay dial, relay hello, and cached extension identity replay form one bounded opening
  attempt. A stale Windows pipe that accepts a dial and closes during either write is retried
  without ending Chrome's native port, consuming the cached identity, or losing queued frames.

The adversarial architecture checks encode the negative boundary: service execution cannot name
MCP dates, JSON-RPC, stdio, or MCP Tasks; the edge cannot depend on service core; PID is absent from
work and workspace authority; and the shipped topology has exactly the three product executables.
Final gate results and any remaining corrections stay in the implementation batch ledger. This
record does not claim publication or a conformance-runner result.

## Consequences

- Ghostlight returns to three executables, but each now owns a distinct lifecycle. Conceptual paths
  shrink because raw MCP no longer enters the service.
- `2025-11-25` remains convenient through its one implicit workspace. `2026-07-28` becomes truly
  request-stateless and can host multiple explicit workspaces in one client process.
- Some older clients stop working, and pre-initialize tool calls are rejected. This is an accepted
  compatibility break in exchange for two exact, testable protocol state machines.
- A bridge failure ends pending synchronous requests truthfully instead of requiring a result
  registry. Future requests recover after bounded reconnect.
- The service keeps all application policy and browser correctness. The MCP edge stays narrow and
  the browser adapter stays policy-free.
- Tasks remain a strategic follow-on, not a falsely advertised consequence of internal work ids.

## References

- [MCP 2025-11-25 lifecycle](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle)
- [Immutable MCP 2025-11-25 schema](https://raw.githubusercontent.com/modelcontextprotocol/modelcontextprotocol/2025-11-25/schema/2025-11-25/schema.json)
- [MCP 2026-07-28 key changes](https://modelcontextprotocol.io/specification/2026-07-28/changelog)
- [MCP 2026-07-28 base protocol](https://modelcontextprotocol.io/specification/2026-07-28/basic)
- [MCP 2026-07-28 server discovery](https://modelcontextprotocol.io/specification/2026-07-28/server/discover)
- [Immutable MCP 2026-07-28 schema](https://raw.githubusercontent.com/modelcontextprotocol/modelcontextprotocol/2026-07-28/schema/2026-07-28/schema.json)
- [Official TypeScript SDK 2026-07-28 migration notes](https://github.com/modelcontextprotocol/typescript-sdk/blob/main/docs/migration/support-2026-07-28.md)
- [MCP Tasks extension overview](https://modelcontextprotocol.io/extensions/tasks/overview)
- [Official MCP conformance suite](https://github.com/modelcontextprotocol/conformance)
