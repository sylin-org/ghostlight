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
adapter inside the modular monolith, not another service or state authority. Every supported
orchestrator start initializes this desktop authority; there is no service-only launch mode.

## Local lifecycle

The orchestrator acquires an operating-system lifetime lease beside its runtime document before
opening listeners or initializing the desktop. The lease, not the replaceable discovery document,
admits the single local authority. Concurrent launch losers exit before creating a second runtime
identity or tray.

Both connectors call the same bridge-owned recovery operation after a service connection fails.
It observes the lifetime lease, honors a fresh deployment lock, and starts only the exact sibling
`ghostlight` executable with no application arguments and with detached null standard streams. The
connectors retain their existing reconnect behavior and learn no workbench or product semantics.

A no-argument launch first asks a running authority to reveal its workbench, then otherwise starts
the complete desktop authority, creates its tray, and backgrounds the workbench: minimized on
Windows and hidden on Linux. Connectors use that exact no-argument launch. The explicit local-human
`ghostlight open` intent composes the same bridge lifecycle operation with the same authenticated
activation request: when absent it demand-starts the ordinary no-argument sibling, waits for its
runtime, then reveals it. The request admits no workspace and adds no listener. No supported launch
can create an authority that refuses presentation by design.

The tray and authority do not depend on a permanent window. Native close destroys only the
disposable workbench, and native minimize remains compositor-owned. Windows Open focuses or
restores the existing window and rebuilds it when absent. Wayland cannot report or unset a
client's minimized state, so Linux Open coalesces, destroys any existing view, waits for Tauri's
destroyed event, and rebuilds from the canonical configuration. On Linux, abnormal WebKit renderer
termination discards that exact window after the signal callback; the next explicit Open creates a
fresh WebView without an automatic crash loop. Explicit Quit alone ends the desktop authority.

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

The service bridge has generic messages for hello, catalog, invoke, cancel, result, and error, plus
authenticated pre-session openings for workbench activation and content-free readiness inspection.
Invocation carries a tool name and JSON input. Catalog, result, and readiness projections are
opaque to the bridge; their meaning remains in the orchestrator. One small typed content vocabulary
lets the edge render bounded screenshots and GIFs without knowing which tool produced them. MCP
request ids and protocol revisions do not cross this bridge. Readiness inspection returns before
channel admission or workspace creation and never demand-starts the authority or writes audit.
The MCP edge generically renders each opaque result both as `structuredContent` and as compact JSON
after the authored ordinary-text summary. It never branches on a tool name or result field. This
keeps complete results available to clients that expose only ordinary content without moving
product vocabulary out of the orchestrator.

The browser bridge has generic messages for hello, primitive request, receipt, browser event,
cancel, and presentation. It carries a closed primitive vocabulary but no model-facing tool
names, product defaults, governance decisions, recovery text, or result envelopes.

The service edge bridge uses versioned newline-delimited JSON on an authenticated loopback
connection. The browser relay and adapter protocol use Chromium's four-byte little-endian length
prefix end to end, so the native host can forward bounded adapter frames without decoding them.
Each bridge hello rejects incompatible major versions before accepting work. Correlation ids are
opaque.

The browser hello also carries a persistent opaque adapter installation id, a restart-local adapter
epoch, an optional bounded product name, a reported attention fact, and versioned physical
capability declarations. They identify and describe one local extension installation for
diagnostics, compatibility, reconnect continuity, and routing. The installation id is the browser's
identity: it never grants authority and never replaces workspace ownership, but it is the key the
service routes on and the opaque handle a model may name. Human toolbar actions
cross the browser bridge as a closed control-intent vocabulary. The service applies them to its
authoritative runtime controls and publishes the resulting content-free control state back to the
adapter. Ended state is terminal for hold and resume; only explicit start-new-session intent creates
a new active runtime session.

Browsers are plural (ADR-0114). The service holds one connection per browser identity, so several
browsers are connected and worked in at once. A hello whose identity is already registered is the
same adapter arriving twice: it replaces that entry and closes the replaced stream, so a duplicate
connection collapses rather than lingering as a socket that silently discards work. Each workspace
binds to one browser for its life, physical tab ids resolve as `(browser, physical_id)`, and every
adapter event names the browser that produced it. A crossing with no binding takes an explicit
selection, then reported attention, then the sole connected browser, and otherwise refuses while
naming the candidates rather than choosing between two signed-in contexts. Attention is reported by
adapters that advertise it, never inferred from connection order, and a bound browser that
disconnects is waited for rather than replaced.

An adapter that advertises end-to-end liveness acknowledges content-free service heartbeats at the
extension shore, independently of browser operations. Relay attachment and adapter availability are
separate facts: a missed liveness window stops new dispatch while leaving the opaque connector and
socket available for a later acknowledgement. Every physical dispatch is followed by its own
probe, so a deadline with no acknowledgement quarantines that adapter while a silent operation
whose heartbeat was acknowledged does not. The browser connector remains unaware of both frames.

## Orchestrator contexts

### Language

Owns the one 24-tool catalog, JSON schemas, descriptions, examples, MCP annotations, decoder
defaults, validation, typed operations, output schemas, and mapping of domain outcomes to language
facts. Nothing outside this context defines a tool or an alternate client dialect. Conditional
requirements are present in the advertised schema rather than left only to runtime validation.

### Work

Owns invocation ids, lifecycle, deadlines, cancellation, sequences, the application executor,
unit-of-work state, uncertainty, and the single completion path.

### Workspace

Owns MCP sessions, controlled tabs, opaque tab and target handles, document generations,
view handles and viewport transforms, selection, ownership, leases, child-tab adoption, stale
detection, and release on disconnect.

### Recording

Owns the model-facing record actions, start and disclosure authorization, the choice of
destination, and the output budget. It holds no frames, no encoder, and no capture maintenance
loop. ADR-0108 places recording identity, frames, bounds, deadlines, stop, retention, and erase in
the extension; ADR-0109 places the GIF encode and its delivery there too.

### Governance

Owns authority loading, immutable snapshots, request restriction, capabilities, protected-host
ceiling, admission, landing decisions, runtime controls, holds, and content-minimized audit intent.
Operation handlers see only the governance facade.

### Browser

Defines the physical primitive port and observed facts: tabs, windows, navigation commits,
readiness, semantic targets, text, screenshots, input metadata, dialogs, recording requests and
receipts, bounded console and network entries, and effect receipts. Action receipts may include the
role and accessible name of the element actually used; the extension never phrases or governs it.
Transport and Chrome details
implement this port outside the context. Diagnostic product defaults, authorization, host filtering,
and model-facing results remain in the orchestrator. Bounded problem/all projection, literal
matching, opaque cursors, and URL sanitization terminate in the extension at the Chromium shore.

### Presentation

Defines content-free feedback events: operation start, target indication, progress, completion,
denial, and attention. Presentation reacts to domain events through a port. Its failures are
recorded but cannot affect authority or completion truth.

### Workbench

Owns the content-minimized desktop read model, bounded global search, high-signal notification
decisions, runtime-control intents, and explicit supported-harness management. It projects the
closed domain-event vocabulary through direct typed reactions and reconstructs bounded terminal
history from the existing durable audit file. The WebView owns only disposable view state.

Harness registration is an orchestrator-owned local-human capability. Each supported harness has
one explicit config resolver and schema. The resolver applies Windows or Linux environment
precedence before its documented fallback, including `CODEX_HOME` for Codex. Check is read-only.
Install and uninstall are serialized,
merge only the `ghostlight` entry, keep unrelated siblings, preserve JSONC and TOML comments,
create a backup, and refuse malformed, unreadable, or foreign-owned configuration rather than
guessing. The UI exposes no generic filesystem or process operation.

Linux desktop integration is another installer-owned local-human capability. A per-user install
owns one XDG desktop entry and one byte-identical icon. The entry invokes the exact versioned
orchestrator with `open`; update and uninstall touch only explicitly owned files. The Debian package
owns the equivalent system entry and does not create a per-user shadow. Browser package provenance
is read-only and typed separately from native-host registration, so a current manifest cannot make
a sandboxed or absent browser look usable.

Tauri commands form a small typed inbound adapter over `WorkbenchFacade`. File work runs outside
the UI event loop. The native notification port is best effort and content-free. WebView, tray,
notification, and recoverable Tauri failures cannot change domain truth; startup or event-loop
failure ends the desktop authority instead of leaving an invisible process.

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

The closed adapter vocabulary is: list tabs, focus tab, atomically open and group a URL, navigate,
traverse history, reload, close tab, read text, read a composed document, inspect, find, screenshot,
screenshot region, describe targets,
activate a locator or physical point, scroll, set zoom, resize a window, hover, fill, type text,
press key, drag, upload supplied bytes, evaluate script, observe condition, inspect dialog, handle
dialog, start and stop screencast capture, observe bounded console and network entries, cancel, and
present. Receipts state whether no effect, a committed effect, or an uncertain effect was observed.
Browser events report document commits, readiness, dialog state, child-tab creation, tab close,
bounded diagnostic entries, control intent, and disconnect.

Ordinary product features compose these primitives only in the orchestrator. A bridge or adapter
change requires a new physical Chromium capability or a bridge protocol requirement.

The composed semantic layer is the physical page-observation boundary. Inside each injected
http(s) frame, the extension walks rendered elements and text through open shadow roots and assigned
slots while excluding hidden content and editable values. Read, inspect, find, fallback accessible
names, and text waits share that model. The service worker joins text and rootless trees in stable
frame order under one page-wide character or node ceiling. Explicit article mode probes only the
top document first and uses the composed full-page read when no useful article exists. Closed
shadow roots stay closed, and child-frame origins do not become result or audit fields.

Pointer geometry follows the same composed surface. Frame-box discovery reaches embeds inside open
roots. Point hit testing descends through open roots, then the service worker follows the embed at
the point through Chromium's parent-frame tree. CDP effects keep top-viewport coordinates while
DOM-local effects keep the deepest frame-local coordinates. Ambiguous embed ownership refuses
instead of guessing.

Screenshot receipts include the exact CSS viewport origin, CSS dimensions, device scale, zoom,
and output scale used for the image. The workspace context turns that physical transform into an
opaque short-lived view handle. The executor resolves image points and rectangles back to CSS
coordinates before sending pointer or region-capture primitives. A region capture validates the
source viewport and returns a new transform, which makes another region capture chainable. A
commit, zoom change, viewport mismatch, or newer screenshot invalidates the old view.

File upload is a cross-boundary capability with explicit owners. The language accepts bounded
absolute paths. The orchestrator validates and reads only those files after governance admission,
then sends bounded names, media types, and bytes through the physical bridge. The extension creates
browser `File` objects for the already selected file input. The relay never reads paths or files.

Recording follows one-owner semantics, and the owner is the browser. The extension's plural
registry owns Chrome screencast start, frame acknowledgement, identity, compressed frames, fixed
bounds, autonomous stop, frozen retention, erase, the truthful recording indicator, and the
animated GIF encode. The orchestrator sends only start, status, stop, export, and discard requests.
The extension folds byte-identical successive JPEGs into one visual span before retention, so
capture time and compressed bytes are the ordinary limits. While recording, presentation disables
only the perpetual controlled-scope glow; transient action feedback remains available.

An export names one of three destinations and one output budget. Attaching to a page target and
writing through the browser's download mechanism both finish inside Chromium; only a client return
carries bytes out, and only that destination is bounded by the transfer ceiling. Encoding runs in
an offscreen document, because Chrome may evict a service worker mid-encode and because object
URLs do not exist in one. Fidelity is traded to meet a budget, never coverage: dropping a frame
folds its time into the frame before it, so a thinned replay still spans and plays for as long as
the work it recorded. Relays remain opaque. Recording frames never cross a process boundary at
all, and no recording bytes enter extension storage, service storage, logs, audit, or restart
state.

Diagnostics are off until an authorized `browser_diagnose` call enables bounded volatile rings for
one owned tab. The extension captures only the Chrome event facts that must originate there, owns
the bounded problem/all projection, literal matching, opaque cursor, and URL-sanitization
mechanisms, and never writes them to extension storage. The orchestrator owns authorization and
applies cross-origin authority plus final filtering before any model disclosure. Reads are
non-destructive and visually quiet. Diagnostic payloads never enter audit or presentation.

Process diagnostics are a separate, local surface (ADR-0145). When activated, all three
executables append bounded, content-free operational JSONL -- connection lifecycle, demand-start
outcomes, negotiation, operation boundaries -- into one directory beside the runtime file.
Activation is layered and eventual: `GHOSTLIGHT_DIAGNOSTICS_DIR` pins a process on at birth, and
a presence-only `diagnostics.on` marker beside the runtime file is applied live by an OS watch
with a 2-second safety-net re-check, whichever fires first. An OS person toggles it by hand,
with `ghostlight diagnostics on|off`, from the extension popup through the runtime-control path,
or from the workbench Status card, which also opens the folder through a native-process act.
Records carry no URLs, page content, payloads, or credentials, so the folder is safe to share;
the schema is pinned by test, retention is automatic, and the governance audit remains the only
decision record.

## Governance

No policy configured means all four independent RAWX capabilities are open. Browser-internal
schemes, extension management pages, loopback addresses, and link-local metadata endpoints remain
an independent protected ceiling.

`GHOSTLIGHT_POLICY_FILE` selects an optional strict schema-3 local policy. Ordered grants combine
host allow and deny patterns with complete independent RAWX sets. Exact hosts outrank longer
suffix wildcards, which outrank `*`; an exact tie denies. Managed policy, local policy, and request
restrictions intersect. Sacred destinations compose by union. False tab-close and target-name
settings are monotonic. Observe mode records ordinary would-deny decisions while protected
destinations continue to enforce. A malformed cold source fails closed; an invalid replacement
keeps the last valid authority for future snapshots.

Signed managed policy is opt-in through `%PROGRAMDATA%\Ghostlight\managed.json` on Windows or
`/etc/ghostlight/managed.json` on Linux. With no bootstrap, no policy network work occurs. A
bootstrap names a local file or HTTPS source plus the organization's public verification key and
may add a bearer token, CA pin, and polling interval. Ed25519 is required; when ML-DSA-65 is
configured, both signatures must verify. Monotonic publish sequence blocks rollback. Verified
bundles are cached and verified again on read. Bad or unreachable updates retain last-known-good;
a configured cold start without a valid source or cache fails closed. Signed policy has no time
expiry. The workbench Policy Passport surfaces content-minimized provenance and signed
organization contacts without policy rules or credentials.

`GHOSTLIGHT_RUNTIME_CONTROL_FILE` optionally names a local text file read at every final browser
boundary. Its exact states are `active`, `hold`, `attention`, and `end_session`. A missing or
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

Audit records include time, invocation id, workspace id, tool, complete RAWX requirement set,
authority version, managed sequence, mode, deciding tier, grant, rule, denial id, decision,
status, effect class, and reason code. They exclude request and page payloads except the governed
host and optional bounded target name described by the active privacy setting.

## Failure and recovery

Disconnect before a physical request has no effect. Disconnect after dispatch without a decisive
receipt yields `unknown`, never failure or success. Cancellation follows the same boundary.
Stale tabs and targets fail before dispatch. Sequence failure reports completed step count and
never replays completed steps. Recovery suggestions come from typed reason codes and effect class,
not page content.

An unanswered post-dispatch liveness probe does not revise that invocation's unknown effect. It
changes only the availability fact used by later invocations, which then stop before dispatch until
the extension acknowledges a new probe or reconnects.

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
service-published control state. The service's negotiated heartbeat supplies the end-to-end
availability fact and ordinary local traffic that keeps the idle adapter shore observable. The
native relay owns ordinary backend reconnection; the alarm-backed extension loop is the fallback
when the native host itself ends. Both paths repeat the same adapter identity and capability
declaration.

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
