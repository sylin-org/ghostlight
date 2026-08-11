# Ghostlight 1.0 architecture

## Shape and dependencies

Ghostlight has four process or trust-boundary components and one shared wire crate:

```text
MCP client
  -> ghostlight-mcp-connector
  -> typed local service bridge
  -> ghostlight orchestrator service
  -> typed local browser bridge
  -> ghostlight-browser-connector
  -> Chromium native messaging
  -> policy-free extension
  -> Chrome APIs and page-local observation

local human
  -> bundled Tauri workbench
  -> typed WorkbenchFacade
  -> the same orchestrator contexts
```

- `crates/bridge` defines the small versioned service and browser wire vocabularies plus the one
  shared local service-lifecycle seam.
- `crates/orchestrator` is the domain-driven modular monolith and service process.
- `crates/mcp-connector` is the hand-rolled JSON-RPC MCP stdio edge.
- `crates/browser-connector` is a frame relay between native messaging and the service.
- `extension` implements typed physical primitives, browser topology mechanism, content-free
  feedback, and the local toolbar/options experience.

Only process, lifecycle, and trust boundaries justify these crates. Product contexts remain
modules inside the orchestrator. The dependency direction points inward to `bridge` types and
orchestrator ports. The orchestrator does not depend on MCP or Chromium APIs.

The `ghostlight` executable also hosts the Tauri 2 desktop event loop. Tauri is a presentation
adapter inside the modular monolith, not another service or state authority. `--headless` starts
the same orchestrator without a WebView, tray, or native notifications.

## Local lifecycle

The orchestrator acquires an operating-system lifetime lease beside its runtime document before
opening listeners or initializing the desktop. The lease, not the replaceable discovery document,
admits the single local authority. Concurrent launch losers exit before creating a second runtime
identity or tray.

Both connectors call the same bridge-owned recovery operation after a service connection fails.
It observes the lifetime lease, honors a fresh deployment lock, and starts only the exact sibling
`ghostlight --background` executable with detached null standard streams. The connectors retain
their existing reconnect behavior and learn no workbench or product semantics.

A direct/default launch, or the compatible `--show` spelling, first asks the running authority to
reveal its workbench through an authenticated first message on the existing service bridge. That
request admits no workspace and adds no listener. If no authority exists, the launch starts the
desktop visibly. `--background` starts the same desktop authority hidden with a tray;
`--headless` is the explicit service-only mode and cannot reveal a workbench.

## Fringe stability

Ghostlight's fringes are independently versioned compatibility products. Their size is not
constrained; their reasons to change are. The MCP connector changes only for MCP negotiation and
protocol compatibility and local service connection lifetime. The browser connector changes only
for native messaging, local relay authentication, framing, discovery, and connection lifetime.
The extension changes only for
Chromium mechanisms, browser-execution integrity and recovery, or the preserved Ghostlight user
experience. Any product feature expressible through existing physical browser capabilities changes
only the orchestrator.

The browser connector treats adapter frames as opaque bounded bytes. A small relay protocol owns
local authentication and backend availability separately from the adapter protocol. The relay
keeps Chromium's native port alive while the service is unavailable, reconnects to the current
runtime endpoint, repeats only the relay and cached adapter handshakes, and never interprets or
replays a physical operation.

The extension is a policy-free browser execution engine, not merely a collection of API calls. It
owns browser-local operation state, safe mechanism retries, duplicate-operation suppression,
physical observation, factual receipts, adapter resynchronization, and fail-safe local human
control. It does not own model-facing tools, workspace authority, governance, product recovery, or
journey composition. A physical effect is repeated only when the browser engine can prove that it
was not dispatched or that the physical sub-step is idempotent. Insufficient evidence produces an
unknown disposition.

Compatibility uses separate axes for the external MCP revision, the service edge bridge, the
browser relay, and the adapter protocol. Adapter behavior is selected by explicitly advertised
physical capabilities, never by parsing an implementation version. A new service may continue to
use an older adapter for every capability that adapter advertises.

## Stable bridges

The service bridge has generic messages for hello, catalog, invoke, cancel, result, and error.
Invocation carries a tool name and JSON input. Catalog and result payloads are opaque to the MCP
edge. A small typed content vocabulary lets the edge render bounded images without knowing which
tool produced them. MCP request ids and protocol revisions do not cross this bridge.

The browser bridge has generic messages for hello, primitive request, receipt, browser event,
cancel, and presentation. It carries a closed primitive vocabulary but no model-facing tool
names, product defaults, governance decisions, recovery text, or result envelopes.

The service edge bridge uses versioned newline-delimited JSON on an authenticated loopback
connection. The browser relay and adapter protocol use Chromium's four-byte little-endian length
prefix end to end, so the native host can forward bounded adapter frames without decoding them.
Each bridge hello rejects incompatible major versions before accepting work. Correlation ids are
opaque.

The browser hello also carries a persistent opaque adapter installation id, a restart-local adapter
epoch, and versioned physical capability declarations. They identify and describe one local
extension installation for diagnostics, compatibility, and reconnect continuity. They are never
model-facing, never grant authority, and never replace workspace ownership. Human toolbar actions
cross the browser bridge as a closed control-intent vocabulary. The service applies them to its
authoritative runtime controls and publishes the resulting content-free control state back to the
adapter. Ended state is terminal for hold and resume; only explicit start-new-session intent creates
a new active runtime session.

## Orchestrator contexts

### Language

Owns the catalog, JSON schemas, descriptions, decoder defaults, validation, typed operations,
and mapping of domain outcomes to language facts. Nothing outside this context defines a tool.

### Work

Owns invocation ids, lifecycle, deadlines, cancellation, sequences, the application executor,
unit-of-work state, uncertainty, and the single completion path.

### Workspace

Owns MCP sessions, controlled tabs, opaque tab and target handles, document generations,
view handles and viewport transforms, selection, ownership, leases, child-tab adoption, stale
detection, and release on disconnect.

### Governance

Owns authority loading, immutable snapshots, request restriction, capabilities, protected-host
ceiling, admission, landing decisions, runtime controls, holds, and payload-free audit intent.
Operation handlers see only the governance facade.

### Browser

Defines the physical primitive port and observed facts: tabs, navigation commits, readiness,
semantic targets, text, screenshots, input metadata, dialogs, and effect receipts. Transport and
Chrome details implement this port outside the context.

### Presentation

Defines content-free feedback events: operation start, target indication, progress, completion,
denial, and attention. Presentation reacts to domain events through a port. Its failures are
recorded but cannot affect authority or completion truth.

### Workbench

Owns the payload-free desktop read model, bounded global search, high-signal notification
decisions, runtime-control intents, and explicit supported-harness management. It projects the
closed domain-event vocabulary through direct typed reactions and reconstructs bounded terminal
history from the existing durable audit file. The WebView owns only disposable view state.

Harness registration is an orchestrator-owned local-human capability. Each supported harness has
one explicit config location and schema. Check is read-only. Install and uninstall are serialized,
merge only the `ghostlight` entry, keep unrelated siblings, preserve JSONC and TOML comments,
create a backup, and refuse malformed, unreadable, or foreign-owned configuration rather than
guessing. The UI exposes no generic filesystem or process operation.

Tauri commands form a small typed inbound adapter over `WorkbenchFacade`. File work runs outside
the UI event loop. The native notification port is best effort and content-free. WebView, tray,
notification, and recoverable Tauri failures cannot change domain truth; startup or event-loop
failure leaves the orchestrator running headlessly.

## Chokepoints and unit of work

Every direct call and sequence enters `ApplicationExecutor::execute`. One invocation owns one
unit of work until exactly one completion is committed.

The synchronous path is:

1. Decode and validate through language.
2. Snapshot configured and managed authority, then apply request restrictions.
3. Start work and emit `WorkStarted`.
4. Resolve ownership and acquire a workspace lease.
5. Ask the governance facade at the final boundary for each required capability or landing.
6. Dispatch physical effects only through the browser port.
7. Apply observed receipts to the workspace aggregate.
8. Complete through `CompletionGate`, which accepts one terminal outcome only.
9. Release the lease and synchronously deliver audit and presentation reactions.

An operation handler cannot mutate another context directly, call a transport, bypass
governance, or construct a client result around `CompletionGate`.

## Closed domain events

The in-process vocabulary is:

- `WorkStarted`
- `TabCreated`
- `DocumentCommitted`
- `TargetIndicated`
- `WorkProgressed`
- `HoldEntered`
- `AttentionRequired`
- `WorkBlocked`
- `WorkCompleted`

Events describe completed state changes. They separate audit, presentation, workspace
bookkeeping, and lifecycle reactions. They do not grant authority, dispatch effects, or create
client success. Reactions are direct typed function calls over the closed enum, not a bus.

## Browser primitives

The closed adapter vocabulary is: list tabs, focus tab, atomically open and group a URL, navigate, traverse history,
reload, close tab, read text, inspect, find, screenshot, describe targets, activate a locator or
physical point, scroll, set zoom, hover, fill, type text, press key, drag, upload supplied bytes,
evaluate script, observe condition, inspect dialog, handle dialog, cancel, and present. Receipts
state whether no effect, a committed effect, or an uncertain effect was observed. Browser events
report document commits, readiness, dialog state, child-tab creation, tab close, control intent,
and disconnect.

Ordinary product features compose these primitives only in the orchestrator. A bridge or adapter
change requires a new physical Chromium capability or a bridge protocol requirement.

Screenshot receipts include the exact CSS viewport origin, CSS dimensions, device scale, zoom,
and output scale used for the image. The workspace context turns that physical transform into an
opaque short-lived view handle. The executor resolves image coordinates back to CSS coordinates
before sending a pointer primitive. A commit, zoom change, viewport mismatch, or newer screenshot
invalidates the old view.

File upload is a cross-boundary capability with explicit owners. The language accepts bounded
absolute paths. The orchestrator validates and reads only those files after governance admission,
then sends bounded names, media types, and bytes through the physical bridge. The extension creates
browser `File` objects for the already selected file input. The relay never reads paths or files.

## Governance

No policy file configured means all four capabilities are allowed. Browser-internal schemes,
extension management pages, loopback addresses, and link-local metadata endpoints are an
independent protected ceiling. A local policy may further allow or deny host patterns and
capabilities. An optional managed authority layer can only tighten local authority. If a managed
layer is configured but missing, malformed, expired, or not marked as managed, effective
authority denies all work.

`GHOSTLIGHT_POLICY_FILE` selects an optional local version-1 JSON authority document.
`GHOSTLIGHT_MANAGED_AUTHORITY_FILE` selects an optional managed version-1 document, which must
set `managed: true` and contain a future `expires_unix_ms`. Each document can contain
`allow_capabilities`, `deny_capabilities`, `allow_hosts`, `deny_hosts`, and the optional boolean
`allow_tab_close`. Missing allow lists and a missing tab-close constraint mean no additional
restriction. A false tab-close constraint is monotonic across local and managed layers; a later
layer cannot expand it. Malformed configured layers fail closed. Host patterns are exact names or
`*.` suffix patterns and never override the protected ceiling.

`GHOSTLIGHT_RUNTIME_CONTROL_FILE` optionally names a local text file read at every final browser
boundary. Its exact states are `active`, `held`, `attention`, and `ended`. A missing or
invalid configured control file enters hold. These controls never expand the immutable authority
snapshot.

One immutable effective snapshot is stored in the unit of work. Request restrictions intersect
with it. Runtime hold, attention, end-session, and cancellation controls are checked immediately
before browser dispatch and on browser events.

Navigation prepares commit observation before dispatch. Every committed URL is checked before
content or readiness is accepted. A denied new-tab landing is closed only when tab-close authority
and the adapter's local physical interlock both permit that compensation. Otherwise it remains
visible and the result reports that compensation did not occur. A denied landing in an existing
tab enters hold and returns a committed, non-repeat-safe blocked outcome. Redirects receive the
same check.

Audit records include time, invocation id, workspace id, tool, capability, authority version,
decision, status, effect class, and reason code. They exclude all request and page payloads.

## Failure and recovery

Disconnect before a physical request has no effect. Disconnect after dispatch without a decisive
receipt yields `unknown`, never failure or success. Cancellation follows the same boundary.
Stale tabs and targets fail before dispatch. Sequence failure reports completed step count and
never replays completed steps. Recovery suggestions come from typed reason codes and effect class,
not page content.

## Extension product architecture

The extension has four small responsibilities with explicit boundaries:

1. The service worker owns native connection lifecycle, physical primitive dispatch, persistent
   adapter identity, and an opaque map from physical tabs to service-supplied workspace ids.
2. The page adapter owns document-local semantic discovery, open-shadow-tree traversal, target
   locators, actionability checks, physical DOM effects, observation, and presentation mounting.
3. The popup reports content-free connection, control, and work state. It sends human control
   intent but cannot decide policy or synthesize model results.
4. The options page owns adapter-local feedback, caption, diagnostic, and physical tab-preservation
   preferences only. A physical preference may refuse a primitive but can never grant authority.

Workspace tab grouping is browser mechanism. `OpenTab` carries the governed URL, the owning
workspace already present on every browser request, and the presentation-only exact title
`Ghostlight - <client label>`. The adapter resolves placement before opening: it reuses one
browser-wide exact-title group wherever that group currently resides, otherwise reuses a window
containing another Ghostlight group, otherwise creates a dedicated normal window containing the
requested URL. Destination resolution is serialized so concurrent opens cannot create duplicate
groups. The first tab for a workspace brings the Ghostlight window into view; later tabs do not
repeatedly steal window focus. The adapter colors the group blue, records the opaque workspace
association, and opens without an externally visible blank-tab step. Group ids are restart-local
hints and exact-title discovery repairs stale hints. The label never routes or grants authority. A
newly opened tab is adopted only when its opener is mapped unambiguously; the service validates
parent ownership before adding the child to the workspace aggregate. Moving a group or tab between
windows does not change ownership.

The service worker persists only the installation id, adapter-local preferences, enough opaque
topology to recover after worker suspension, and a bounded content-free operation disposition
journal. URLs, page content, form values, scripts, file data, screenshots, receipts, and policy are
never stored. Popup status is derived from live native connection state, relay availability, and
service-published control state. The native relay owns ordinary backend reconnection; the
alarm-backed extension loop is the fallback when the native host itself ends. Both paths repeat the
same adapter identity and capability declaration.

Model-driven close is dual gated. The orchestrator first admits the action capability and the
monotonic tab-close policy constraint. The extension then checks its default-on preserve-tabs
preference immediately before `chrome.tabs.remove`. A local refusal uses the existing content-free
adapter error channel and is rendered by the orchestrator as a blocked no-effect result. The local
preference is not advertised as a physical capability, does not renegotiate the adapter, and
cannot override service authority. Direct human closure through Chromium remains outside this
model-driven path.

The operation journal records only the current service epoch, correlation id, and one of accepted,
dispatched, completed, failed, or uncertain. Duplicate accepted work and known no-effect failures
may resume. Dispatched, completed-without-an-in-memory-receipt, and uncertain work report unknown
instead of repeating. Terminal records remain until the service acknowledges receipt, which keeps
normal operation bounded without evicting an unacknowledged effect.

Presentation is a document-local state machine. Signals carry only opaque ids, a closed event
kind, a closed Ghostlight activity treatment, fixed Ghostlight-authored phase and detail text,
and an optional physical locator. One small renderer implements the established sky-blue visual
vocabulary and owns its fixed palette, shapes, timing, easing, and reduced-motion treatments. It
has no broker, policy, catalog, or orchestration. Controlled scope and guardrail feedback remain
visible independently of decorative preferences. Screenshot capture hides the layer before pixels
are read. A new document remounts clean state; a terminal signal clears transient effects. A
bounded browser-local queue retains unseen denial signals for the relevant controlled tab without
changing focus, coalesces repeated notices per tab, and expires them after ten minutes. The toolbar
badge marks an unseen denial until the tab becomes visible and the five-second ribbon renders.

The manifest is part of the architecture contract. It preserves the established product name,
description, development key, host name, toolbar title, shortcut description, and byte-identical
icon assets. It connects the established toolbar and options experiences while including only
permissions required by the responsibilities above. Extension source remains framework-free
because its state and UI are small and Chrome-native.

## Unpacked extension continuity

For the narrow compatibility question consulted from the archived public development instructions:
Chromium loads `extension/` directly with id `cjcmhepmagomefjggkcohdbfemacojoa`, connects through
`org.sylin.ghostlight`, and reloads explicitly after changes. Repo-built shores remain under
`target/release`. No prior implementation code was reused.
