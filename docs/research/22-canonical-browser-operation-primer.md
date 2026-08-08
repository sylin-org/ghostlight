# Canonical browser operation primer

**Date:** 2026-08-08
**Status:** Proposed research primer. Not an ADR and not implementation authorization.

## Recommendation

Build one typed Ghostlight browser-operation kernel and make the Ghostlight-native surface a
one-to-one projection of it. Keep compatibility surfaces outside that kernel. A compatibility
surface may rename an operation, accept a looser input shape, supply profile defaults, and render
the canonical result. Each external call still normalizes to exactly one canonical operation; a
vendor batch normalizes to `browser.flow`. The adapter never directly dispatches a sequence of
governed operations and may not classify, authorize, schedule, route, or audit browser work.

Use two adapter classes:

1. A flat surface adapter for versioned tool dictionaries such as the captured Claude Cowork
   surface and Playwright MCP.
2. A stateful runtime adapter for programmable object models such as the captured Codex Chrome
   runtime. This adapter owns proxy and handle lifetimes. Pure locator, capability, and cached
   documentation builders stay local; every terminal browser observation or effect resolves to
   one canonical Ghostlight operation before governance or browser execution.

The default profile should be `ghostlight-native/v1`. Today's 25-tool catalog should become an
explicit compatibility profile during migration. Vendor profiles are evidence- and
evaluation-tuned. They are not described as training-matched unless a vendor publishes that
contract.

## Why this separation matters

The current registry combines three different concerns in one `ToolDescriptor`:

- model-facing name, description, schema, example, and annotations;
- semantic behavior, capability requirements, workspace use, result meaning, and defaults; and
- physical extension dispatch, scheduling, post-processing, and post-dispatch markers.

That was a useful consolidation when one public surface and one extension vocabulary were the
same thing. It becomes the wrong dependency direction once several client surfaces can express
the same browser intent.

The target dependency direction is:

```text
client call
  -> selected SurfaceProfile and SurfaceSession
  -> canonical BrowserOperation
  -> validation, governance, scheduling, and audit
  -> browser Mechanism calls
  -> canonical BrowserResult
  -> SurfaceProfile result rendering and state update
```

The extension remains policy-free. It executes Chrome and page mechanisms. It does not know
Claude, Codex, Gemini, Playwright, MCP client names, RAWX policy, or surface profiles.

One-to-one means the native surface and the semantic kernel share the same typed operation
families. It does not mean that a mixed action loses its discriminant. Conceptually:

```text
BrowserOperation
  = Context(ContextIntent)
  | Tabs(TabsIntent)
  | Navigate(NavigateIntent)
  | Snapshot(SnapshotIntent)
  | Read(ReadIntent)
  | Find(FindIntent)
  | Screenshot(ScreenshotIntent)
  | Act(ActIntent)
  | Fill(FillIntent)
  | Wait(WaitIntent)
  | Flow(FlowIntent)
  | Dialog(DialogIntent)
```

`ActIntent::Click` and `ActIntent::PressKey`, for example, remain distinct concrete operation
variants for classification, scheduling, result meaning, and audit. Physical CDP input commands
are mechanism ids below that semantic identity, never alternate operation names.

## Intended capability map

The investigated products use different names, but their intended jobs converge on these planes.

| Plane | Purpose | Representative prior art | Canonical Ghostlight home |
|---|---|---|---|
| Binding | Discover a usable browser and preserve one explicit workspace | Claude browser selection; Codex `Browsers`; Gemini local/remote modes | Surface session plus optional `multi_browser` pack |
| Topology | List, create, focus, and explicitly close owned tabs | Claude tab tools; Codex `Tabs`; Playwright `browser_tabs` | `browser_tabs` |
| Navigation | Initiate URL/history change and report final landing readiness | Every investigated surface | `browser_navigate` |
| Structured observation | Give the model small, stable action targets | Playwright accessibility snapshot; agent-browser refs; Codex locators | `browser_snapshot` and `browser_find` |
| Text observation | Read prose without a screenshot or full action tree | Claude page text; Stagehand extract; Browser Use extraction | `browser_read` |
| Visual observation | Capture appearance and bind later coordinates to that exact view | OpenAI/Gemini CUA; every browser surface | `browser_screenshot`; precision pack |
| Deterministic action | Act on one exact or uniquely resolved target | Stagehand act; Playwright element tools; Ghostlight `act_on` | `browser_act` |
| Form intent | Resolve several fields, refuse ambiguity, optionally submit | Claude/Ghostlight form tools; Playwright fill form | `browser_fill` |
| Page readiness | Wait for a condition and/or a quiet document | Playwright waits/settle; Codex wait APIs; Ghostlight settle detector | Shared readiness contract and `browser_wait` |
| Composition | Remove model turns while preserving per-step evidence | Claude batch; OpenAI batched actions; Codex programmatic calls | `browser_flow` |
| Modal recovery | Inspect or deliberately resolve blocking browser dialogs | Playwright/Codex dialogs | `browser_dialog` |
| Transfer | Put client bytes or a captured artifact into a page | Claude/Ghostlight uploads; Playwright chooser flow | `files` pack |
| Diagnostics | Inspect bounded console or network evidence | Playwright/DevTools/Codex logs | `diagnostics` pack |
| Escape hatch | Execute page-context code with an honest hazard classification | Claude JavaScript and Playwright unsafe code; Codex evaluation is a narrower contract | `execute` pack |
| Human visibility | Narrate, highlight, hand off, stop, and resume | Ghostlight presentation; Gemini/Microsoft takeover | Presentation/control plane, never model-granted authority |
| Governed evidence | Return receipts, provenance, bounded tab deltas, recordings, and content-free audit correlation | Ghostlight audit and presentation controls; receipts and artifacts also appear elsewhere | Common result contract and `media` pack |

Several exposed vendor tools are coordination features rather than browser primitives. Claude
shortcuts and client planners belong in a client product or a saved-workflow capability. They
must not become fake Chrome mechanisms.

## Canonical principles

1. One intent has one canonical operation identity even when it uses several browser mechanisms.
2. The public native name and the canonical operation name are the same except for the internal
   `browser.` namespace. This is the promised one-to-one native projection.
3. Semantic constraints are stronger than any compatibility schema. A loose Claude schema can be
   accepted at the edge and normalized into a strict canonical intent.
4. Opaque handles carry generation and ownership. Physical Chrome ids never become authority.
5. Action success and page readiness are separate facts.
6. Action completion is not proof that the user's business goal succeeded or that a remote system
   committed a transaction.
7. Every failure says whether an effect was dispatched and whether retry can duplicate it.
8. Page text and page-defined metadata remain untrusted output, never policy input.
9. Large observations are bounded and resumable. Every truncation returns `more` and a cursor or
   one narrow next call.
10. The ordinary surface stays small. Specialist packs are fixed before catalog publication.
11. One connection or surface session sees exactly one dialect. Duplicate dialects are never
    advertised together.
12. Defaults are versioned semantic behavior, not unrecorded implementation convenience.

## Workspace and addressing contract

`WorkspaceHandle` is the service bearer authority for all browser state. A `TabHandle` proves
only that one tab belongs to that workspace; it never substitutes for the workspace. Every
stateful canonical input and result carries `workspace` after normalization.

A sessionful 2025-11-25 edge or Codex runtime may inject its one implicit workspace and its
workspace-bound current tab. A request-stateless edge must require `workspace` on every stateful
call. The only exceptions are declared creators: `browser_tabs(action:new)` can mint a workspace
and blank tab, and `browser_navigate({url})` can mint a workspace and tab before navigating. Both
return the new workspace. Human browser focus never supplies or changes this binding.

This means a profile's exact schema support can be MCP-revision-specific. An external dictionary
that has no way to carry request-stateless continuity is not an exact 2026-07-28 profile; that
edge falls back to the native revision-specific projection rather than secretly inventing shared
state.

## `ghostlight-native/v1` core

The proposed default has 12 tools. Prefixing every name with `browser_` makes ownership clear in
clients that merge several MCP servers, while the internal ids use dotted semantic names.

### 1. `browser_context` -> `browser.context`

Optional workspace only. Returns backend health, selected owned tab for that workspace, native
profile and default versions, enabled packs, output limits, current non-sensitive governance
posture, and the RAWX vocabulary. This is diagnostic and never a required bootstrap call.

### 2. `browser_tabs` -> `browser.tabs`

Actions:

- `list`
- `new`, which creates one blank owned tab and optionally makes it current
- `focus`, with one explicit owned tab
- `close`, with one explicit owned tab

`new` does not also navigate. `browser_navigate` owns create-and-navigate convenience so one
operation owns URL validation, landing checks, readiness, and its result. Codex turn finalization
is real prior art but is not a native v1 action: Ghostlight currently requires explicit close and
has no canonical auto-cleanup capability. A future finalizer needs its own ADR and must preserve
the distinction between agent-created tabs and claimed user tabs.

### 3. `browser_navigate` -> `browser.navigate`

Conceptual input:

```json
{
  "workspace": "w_...",
  "tab": "t_...",
  "url": "https://example.com",
  "force": false,
  "readiness": {
    "settle": true,
    "timeout_ms": 10000,
    "min_ms": 0
  }
}
```

Exactly one of `url` or `history: back|forward|reload` is accepted. If a URL navigation omits
`tab`, Ghostlight binds the workspace-selected owned tab at admission. If no workspace or tab
exists, URL navigation creates both and returns them. History navigation always requires an
existing workspace and tab. Human focus cannot retarget admitted work. Multi-tab flows require
explicit tab handles on each page step.

### 4. `browser_snapshot` -> `browser.snapshot`

Returns a bounded accessibility structure with fresh actionable refs. `interactive` is the cheap
default. `all` is explicit. Scope, depth, item limit, and cursor are bounded. The result includes
tab, document, and revision identifiers.

### 5. `browser_read` -> `browser.read`

Returns plain readable page or scoped-element text. It does not pretend to be an action index.
The default maximum is 20,000 characters, with explicit truncation and cursor continuation.

### 6. `browser_find` -> `browser.find`

Performs a cheap targeted accessible-name or visible-text search. It returns at most 20 ranked
candidates, surrounding context, `more`, and fresh refs. Find never mutates. A later semantic
action refuses to mutate unless its highest-ranked match resolves uniquely.

### 7. `browser_screenshot` -> `browser.screenshot`

Returns an MCP image plus a frame/artifact id, CSS viewport, DPR, document id, scroll/layout
revision, and capture bounds. JPEG quality 55 remains the efficient default. Coordinate actions
must cite the frame id that defines their coordinate space.

### 8. `browser_act` -> `browser.act`

Performs one target action under one retained semantic lease. Target is exactly one of ref,
role/name, text, semantic query, or CSS. Core actions are click, double click, right click, hover,
scroll into view, focus, set value, target-bound press key, and drag. Value, key, and destination
are required only by the relevant action. Core targets cover the top document and same-origin
shadow trees. Cross-origin frame targeting returns unsupported before mutation until its
multi-origin governance contract has its own ADR.

With no postcondition, the operation returns the existing bounded first observation at roughly
300 ms. With `expect`, it uses the shared wait contract under the same retained lease. Semantic
ambiguity or a stale ref fails before mutation.

### 9. `browser_fill` -> `browser.fill`

Takes a bounded ordered array of `{target, value}` rather than a loose label map. Every target is
pre-resolved. Missing or ambiguous fields cause no mutation by default. `partial:true` explicitly
permits progress. Each target is revalidated immediately before mutation; if an earlier field
rerenders the form, Ghostlight returns truthful partial progress with no rollback. `submit:true`
uses the owning form or an explicit submit target. Targets have the same top-document and
same-origin-shadow boundary as `browser_act`. Field values are model-visible inputs but are never
echoed or audited. This is not a protected secret channel. Passwords, OTPs, recovery codes,
authentication codes, API keys, long-lived tokens, and other credential-class secrets are invalid
`browser_fill` inputs and fail before mutation. Adapters reject credential targets from inspected
form semantics; those values never enter model-visible fill arguments. A future protected-auth
profile must use a real human-facing secret broker or hand off.

### 10. `browser_wait` -> `browser.wait`

Waits for one bounded predicate, a bounded `all`/`any` predicate set, or settlement alone.
Predicates cover target/text presence or visibility, URL/title match, dialog state, and document
state. Defaults are settle true, 10-second maximum, and zero minimum.

Timeout normally returns `status:not_met`, not a transport failure. `on_timeout:error` gives a
strict flow step that halts under normal `on_error:stop` behavior.

### 11. `browser_flow` -> `browser.flow`

Runs up to 20 ordered canonical calls with no nesting. A later argument can use a reserved typed
reference variant:

```json
{ "$step": "find-save", "path": "/matches/0/ref" }
```

The `$step` object is interpreted as a reference only in schema positions whose argument AST
declares that variant. Literal object values in those positions use an explicit `literal` variant.
`path` is an RFC 6901 JSON Pointer, and the result records the source step id as in-band
provenance. This prevents page data from becoming an accidental reference.

In execute mode every step is independently validated, classified, authorized, scheduled,
audited, and reported under one parent correlation. Single-surface flows may retain the lease for
a bounded scheduling quantum. The flow is not a transaction and has no rollback.

`mode:execute` is the default. `mode:preflight` never dispatches the requested effects, but it may
use the same bounded read-only URL/resource probes as real pre-dispatch admission. A step that
depends on an earlier result cannot receive a real verdict without execution; it reports
`unresolved_dependency` or uses an explicit concrete preflight substitute. Preflight writes the
parent dry-run record and no per-step execution audit records. Navigation preflight explicitly
says that committed post-redirect landings are checked only at execution time. This preserves
Ghostlight's current dry-run advantage without a second planning tool.

### 12. `browser_dialog` -> `browser.dialog`

Actions are `status`, `accept`, `dismiss`, and `respond`; respond requires text. A modal blocks
target preparation, not only final click dispatch, so dialog handling remains separate from page
actions. Accept, dismiss, and respond require explicit current-task intent. Compatibility
profiles enforce their declared type matrix; the captured Codex runtime permits only dismiss on
alert and before-unload, accept or dismiss on confirm, and text response or dismiss on prompt.

## Capability packs

Packs are selected before `tools/list` and stay fixed for the surface session. A runtime
disconnect may make a declared capability temporarily unavailable, but it does not silently
replace the catalog.

| Pack | Tools | Boundary |
|---|---|---|
| `precision` | `browser_input`, `browser_viewport` | Coordinate pointer, keyboard, and scroll cite a screenshot frame; resize is browser-wide |
| `files` | `browser_upload`, `browser_download`, `browser_export`, `browser_artifacts` | Governed inbound files plus bounded download/export artifacts, explicit retention and consumption |
| `diagnostics` | `browser_console`, `browser_network` | Cursor-based bounded reads; response bodies and consume/clear are explicit |
| `execute` | `browser_evaluate` | Page-context JavaScript only, always Execute-classified, bounded timeout and result |
| `media` | `browser_record` | Memory-only start/status/stop/export/clear with bounded artifact lifetime |
| `presentation` | `browser_present`, `browser_visibility` | Narration, highlight, clear, and deliberate show/hide on the separate presentation lane |
| `multi_browser` | `browser_instances` | List/select/pair real backends only after the canonical capability exists |

Do not import Playwright's full test-context surface into an authenticated user browser. Cookie
rewrites, storage restore, network mocking, offline mode, raw tracing, and arbitrary CDP have
different trust and lifecycle assumptions.

The files pack is a capability target, not a claim that every physical path exists today. It must
cover page-triggered downloads, targeted media acquisition, page/content export, and artifact
lifecycle before a Codex profile advertises the corresponding members. File chooser and download
listeners are armed before the triggering action and are one-shot. A missing capability is
reported as unsupported rather than simulated with arbitrary host filesystem access.

The native Execute pack is intentionally honest unrestricted page-context execution. Codex's
captured `evaluate` and `evaluateAll` methods promise a narrower read-only page scope. A Codex
adapter must prove and enforce that promise or omit those methods; it must not silently map them
to `browser_evaluate` merely because both accept JavaScript.

WebMCP remains a deferred watch item under ADR-0043, not a v1 capability pack. A dynamic
page-defined catalog and classifier need a new ADR after that decision's re-evaluation triggers
are met. The Codex `webmcp` capability is therefore unsupported rather than approximated.

## Shared navigation and readiness contract

The default is a maximum adaptive budget, not a fixed ten-second sleep:

```json
{
  "settle": true,
  "timeout_ms": 10000,
  "min_ms": 0
}
```

The canonical state machine is:

```text
validate workspace/tab shape and bind one surface lease plus one AuthoritySnapshot
  -> validate and authorize the requested target
  -> run final hold, panic, attention, and ownership admission
  -> atomically arm the exact-document watcher and dispatch navigation
  -> for each committed top-level DocumentHandle surfaced by the watcher,
     probe its URL and run the post-landing check before page-content/readiness observation
  -> on real deny, best-effort park and return blocked; shadow deny stays transparent
  -> settle only the latest allowed document within the dispatch-to-readiness deadline
  -> if its DocumentHandle changes, discard old readiness and repeat within that deadline
  -> reverify the same final document, render, and complete audit
```

Ghostlight authorizes the requested target before dispatch. It does not claim to prevent or
pre-authorize HTTP redirect hops. It post-checks each committed top-level document it can observe
before consuming that document's content or starting settlement. The same immutable authority
snapshot covers target authorization, landing checks, readiness, result, and audit.

`timeout_ms` is the operation's absolute `navigation_readiness_deadline`, beginning when the
navigation mechanism is dispatched. It does not replace or collapse ADR-0080's separate queue,
caller-response, extension-execution, drain, or quarantine deadlines. `min_ms` must not exceed
`timeout_ms`. Do not stack today's ten-second load wait and another ten-second DOM settle wait.
The mechanism must expose navigation commit separately from load completion for this contract to
be implementable.

The v1 settle predicate reuses Ghostlight's adaptive DOM mutation decay: 500 ms windows, an
adaptive threshold, three consecutive quiet windows, and one minimum observation window. Network
idle is not the default because long-lived requests make it pathological.

Readiness records condition and settlement as separate axes:

```json
{
  "status": "ready",
  "condition": { "requested": true, "met": true },
  "settlement": { "requested": true, "status": "settled" },
  "elapsed_ms": 1850
}
```

The aggregate status is `ready|timed_out|unavailable|not_requested`. Omit an unrequested axis.
Condition plus default settlement means AND. Navigation normally requests settlement only.

Canonical outcomes are:

- A proven final authorized commit whose document settled: `status:ok`, settlement `settled`.
- A proven final authorized commit whose settlement exhausted the deadline: `status:ok`,
  readiness `timed_out`, settlement `not_settled`.
- A proven final authorized commit whose DOM cannot be observed, such as a protected page or PDF:
  `status:ok`, readiness and settlement `unavailable`.
- `settle:false`: success after the final authorized commit with readiness `not_requested`.
- Pre-dispatch policy denial: `status:blocked`, `effect:none`,
  `retry:after_state_change`.
- Post-commit landing denial: `status:blocked`, `effect:committed`,
  `retry:after_state_change`, no readiness observation, with the best-effort parking result
  reported separately.
- No proven commit by the navigation deadline: a known no-effect failure when proven, otherwise
  `outcome_unknown` with unsafe retry. It is never soft timeout success.

A soft readiness timeout applies to navigation. A standalone wait has a requested observation as
its primary goal and returns `not_met` or an error according to `on_timeout`. An action with an
acknowledged effect but unmet expectation returns `partial` because the action already may have
changed the page.

Codex compatibility preserves its explicit `goto` then `waitForLoadState` idiom. The canonical
navigation result may still contain readiness, but the adapter must avoid imposing a duplicate
hidden ten-second wait when the trained client explicitly controls readiness.

## Common result invariant

Every native tool emits concise text plus a versioned structured result. Irrelevant fields are
omitted rather than set to null.

```json
{
  "schema": "ghostlight.browser.result/1",
  "operation": "browser.act",
  "operation_id": "o_...",
  "status": "ok",
  "effect": "committed",
  "workspace": { "id": "w_..." },
  "tab": { "id": "t_...", "url": "https://example.com", "title": "Example" },
  "page": { "document": "d_...", "revision": 12 },
  "data": {
    "interaction_receipt": {
      "action": "click",
      "target_assurance": "ref"
    }
  },
  "tab_delta": { "opened": [], "closed": [], "more": false },
  "provenance": {
    "untrusted_fields": ["/tab/url", "/tab/title"],
    "top_origin": "https://example.com",
    "session_nonce": "..."
  }
}
```

The status vocabulary is
`ok|partial|not_met|blocked|held|attention_required|cancelled|not_dispatched|outcome_unknown|unavailable`.
Effect is `none|dispatched|committed|unknown`. Retry is
`safe|unsafe|after_state_change` and is omitted when no corrective retry guidance applies. These
are semantic facts, not optimistic labels. `dispatched` is used only when send/accept is proven
but terminal acknowledgement is not; it cannot accompany `status:ok`.

`ok` means the browser mechanism completed. It does not mean a purchase, submission, or remote
business operation succeeded. `committed` means Ghostlight received the defined browser
acknowledgement, not that the page caused a remote transaction. Browser-created child tabs are
adopted before their handles appear in a result.

A stale-ref result instead uses `status:blocked`, `effect:none`, and
`retry:after_state_change`, with recovery pointing to `browser_snapshot` for the same workspace
and tab. Hold, attention, and cancellation remain distinct because they have different queue,
human-control, and retry semantics. A user takeover retires queued work without dispatch; it does
not by itself make an already acknowledged effect unknown.

Provenance applies only to the named page-derived fields or bounded text subtrees. It never marks
service-authored `status`, `effect`, retry guidance, ids, or policy facts as untrusted. Omit it
when a result contains no page-derived payload.

## Opaque state and handle model

The canonical service owns these generation-aware handle classes:

- `WorkspaceHandle`: service-minted bearer authority, owner binding, liveness, and generation.
- `BrowserHandle`: exact backend or extension instance, browser family/type, browser-process
  generation, and current transport availability.
- `TabHandle`: verification-only workspace membership plus browser generation and native tab
  identity. It is never authority without `WorkspaceHandle`.
- `DocumentHandle`: changes on navigation, reload, and history traversal.
- `ElementRef`: document and observation revision plus resolution kind.
- `CaptureFrameRef`: document, viewport, DPR, scroll, and layout revision for coordinate work.
- `ArtifactRef`: workspace, type, size, retention deadline, and consumed state.
- `EventRef`: armed chooser, download, dialog, auth, or other one-shot resource.

The stateful surface adapter, not the canonical service, additionally owns:

- `LocatorPlanHandle`: an immutable tab-bound selector, frame-selector, filter, and composition
  plan. Builders transform this local plan without browser work. A terminal call normalizes the
  plan into one canonical target intent, which resolves under the current document lease.
- `TurnEpoch`: the vendor runtime turn identity plus `open|finalized` state. A future sanctioned
  Codex finalizer fences this epoch only. Native v1 has no finalizer and never creates a terminal
  surface-session state from tab cleanup.
- `UserTabClaimSnapshot`: the browser/extension instance, listing generation, provider tab id,
  title, and URL used to validate an explicit claim without trusting a reused numeric id.
- capability and documentation proxies keyed by browser connection and effective API version.

Vendor-visible numeric tab ids, node ids, and image ids map to resolved handles through adapter
state. A Codex/Playwright locator object maps to the adapter-local `LocatorPlanHandle`, not
`ElementRef`; it can survive ordinary DOM mutation and produces a fresh resolved target only at a
terminal operation. None of these vendor values becomes authority itself.

Invalidation is explicit:

- A native-port or extension-worker disconnect makes the surface temporarily unavailable and may
  leave dispatched work uncertain. It does not invalidate descendants while the same browser
  process generation can reconcile them. Only a proven browser-process generation change does.
- Empty tab lists and a missing optional Playwright helper do not invalidate the browser binding.
- Tab close or ownership loss invalidates the tab and every descendant.
- Navigation invalidates document-bound refs, capture frames, frame identities, dialogs,
  chooser/event refs, auth requests, page-asset inventories, and origin-scoped state. It does not
  invalidate workspace-scoped artifacts or necessarily invalidate lazy locator plans.
- Frame detachment invalidates resolved frame identities. A declarative frame-selector plan may
  be reused, but its next terminal call must resolve the frame again under the current document.
- DOM/layout/scroll/viewport revision invalidates node or coordinate evidence as applicable, not
  a lazy locator plan that will re-resolve.
- Chooser and download event handles are one-shot and expire after consumption, navigation, or
  tab close. Their listeners must be armed before the triggering action.
- An active dialog handle carries its observed dialog type and is consumed by one valid response
  or invalidated when the dialog or owning document disappears.
- Tab claiming uses a fresh user-tab listing object and never heals a reused provider tab id from
  stale title/URL/browser evidence.
- Kernel reset invalidates Codex proxy bindings, not physical browser state.
- User takeover retires queued work as not dispatched. Active effects drain; they become unknown
  only if terminal proof is lost. Re-observe before the next action after control returns.
- A timeout after dispatch never triggers automatic retry. It is unknown unless a terminal proof
  says otherwise.
- If a future Codex finalizer is sanctioned, it fences the current turn epoch, not the browser
  session; the next turn may reuse the browser binding.

No adapter may silently heal a stale ref into a different current target. It returns the current
revision and one corrective observation call.

## Surface profile and state contract

Each `SurfaceProfile` contains:

- stable profile id and version;
- provenance and tested client/version ranges;
- ordered external declarations and capability packs;
- input normalization from external call to typed canonical operation;
- result and error rendering from canonical outcome;
- a serializable or explicitly ephemeral surface-state schema;
- lifecycle and invalidation hooks;
- exact canonical operation mappings for every external variant; and
- a declared set of unsupported behaviors. Unsupported behavior is omitted or rejected, never
  represented by a stub.

Each `SurfaceSession` contains only presentation/runtime state:

- selected profile and selection reason;
- profile, runtime/kernel, browser-connection, and capability-catalog generations;
- the current `TurnEpoch`, including open/finalized state only for a compatibility runtime whose
  finalizer is actually sanctioned;
- browser family/type, selected alias, selection reason, and any hard user selection constraint;
- the service-issued workspace handle retained only where the protocol is sessionful;
- external-to-canonical handle bindings;
- browser, tab, document, locator-plan, capture-frame, artifact, and event proxies;
- fresh user-tab claim snapshots and exclusive claim-lease state only when claiming is supported;
- per-browser documentation fingerprint and complete-documentation-read flag;
- a current-tab convenience binding scoped to the exact workspace;
- per-tab mutation queues and pending operation identity;
- armed navigation, chooser, and download state plus active typed-dialog and pending-notification
  state;
- browser visibility plus temporary viewport-reset obligations;
- auth request, page-asset, WebMCP, and CDP state only when those capabilities are genuinely
  implemented; and
- pending outer program/cancellation state, including unawaited asynchronous work.

It may retain an opaque service-issued workspace handle but cannot mint, validate, widen, or route
by it. It owns no grant, policy, sacred-domain decision, scheduler, or audit decision.

## Layered classifier

Selection precedence is:

1. Explicit local configuration or launch override.
2. An exact authenticated product integration handshake, when a future plugin provides one.
3. Exact allowlisted `clientInfo.name` plus a tested version range and any required protocol or
   capability fingerprint.
4. `ghostlight-native/v1` fallback.

Do not use substring, brand, nearest-version, or guessed-model rules. Unknown, missing, ambiguous,
or out-of-range signals fall back safely and record the reason locally.

`clientInfo` is self-asserted. It can choose schema presentation and a runtime adapter. It can
never grant a capability, weaken policy, change workspace ownership, choose a physical browser,
relax audit, or disclose state.

For MCP 2025-11-25, resolve and pin the surface at initialization. For request-stateless MCP
2026-07-28, resolve independently from each request's immutable context. Cache identity includes
profile, MCP revision, service generation, and restriction context; concurrent profiles never
share mutable adapter state. A profile that needs state not carried in the request or an explicit
profile-scoped handle is unsupported on that revision. Never borrow another request's client
identity or store profile choice in workspace routing.

## Compatibility implications

### Claude Cowork / Claude-in-Chrome

The captured 22-tool dictionary is feasible as a flat profile only where Ghostlight owns the
capability. Sixteen tools map directly or compositionally. `tabs_close_mcp` maps to
`browser.tabs(close)`. Browser discovery/selection needs the real `multi_browser` capability.
Shortcut list/execute needs a real saved-workflow subsystem and must not be faked.

The captured schemas omit several required fields, types, exclusivity constraints, and
`additionalProperties:false`. Preserve those quirks only at the compatibility edge. Normalize and
validate the stronger canonical intent before governance.

### Codex Chrome runtime

This is not a flat MCP profile. It needs a Codex plugin/runtime module using the existing
persistent Node kernel, a frozen Ghostlight proxy object model, effective documentation, dynamic
capability filtering, and generation-aware browser/tab/locator state. Pure locator composition,
capability lookup, and cached documentation access stay local. Each terminal observation or
effect makes one typed operation call. Never audit only the enclosing JavaScript source.

Do not map a bare Codex `clientInfo` to a fake flat browser dictionary. With the runtime plugin
installed, the model continues to see Codex's native `node_repl` affordance; the trusted module
uses an internal Ghostlight connection and explicitly targets canonical operations. Without that
authenticated integration, ordinary Ghostlight falls back to its native MCP surface.

Do not expose host Node imports, process/fs/network access, raw CDP, broad personal-tab discovery,
system clipboard, telemetry, or credential entry through a fake compatibility layer. A capability
is absent until Ghostlight can honor its actual safety and lifecycle contract.

An exact 136-member mapping ledger is required before calling this profile complete. The intended
dispositions are:

| Codex runtime family | Canonical disposition |
|---|---|
| Browser discovery, selection, documentation, session naming | `browser_context` plus real `multi_browser`; naming and pure proxy construction stay adapter-local |
| Controlled tab list/get/new/select/close | `browser_tabs` |
| Tab `goto`, back, forward, and reload | `browser_navigate` |
| Tab URL/title, screenshot, and JavaScript dialogs | Bounded read result, `browser_screenshot`, and `browser_dialog` with the captured type matrix |
| Turn `finalize`, `markDeliverable`, and `markHandoff` | Deferred until an explicit cleanup ADR exists; any future fence is `TurnEpoch`-scoped, never surface-session terminal |
| User-tab `openTabs` and `claimTab` | Deferred until a user-tab-claim ADR defines fresh snapshot matching, exclusive lease, and release behavior |
| `BrowserUser.history` | Omit until a separately governed sensitive browser-history capability exists; tab history navigation does not substitute for it |
| Playwright locator builders | Local immutable `LocatorPlanHandle`; terminal reads/actions use snapshot/find/act/wait |
| Playwright terminal reads, actions, navigation expectations, and waits | Snapshot/find/act/wait; action-triggered navigation and event listeners arm atomically before dispatch |
| Visible DOM and node actions | Snapshot/find/act with document-generation checks |
| Coordinate CUA | Precision `browser_input` with a cited capture frame |
| `Tabs.content` | Omit until a governed temporary-tab batch-read and cleanup operation exists; do not replace it with host-network fetches |
| Content export, Google Workspace export, `PlaywrightDownload.path`, media download, and `pageAssets` | Files pack only after governed output-artifact acquisition, retention, and consumption exist |
| File chooser and download events | Adapter-local one-shot `EventRef` plus files-pack terminal operations; omit until both sides are implemented |
| Console logs | Diagnostics pack |
| Viewport and visibility | Precision viewport and presentation visibility |
| Read-only `evaluate`/`evaluateAll` | Omit until the promised non-mutating scope is enforceable |
| Clipboard | Omit unless a workspace-scoped virtual clipboard exists |
| `browserAuth` | Omit until a protected human secret broker exists |
| WebMCP | Deferred under ADR-0043 |
| Raw CDP | Omit from the governed/default facade; possible explicit Execute capability only |
| `botDetection.report` | Reject: internal telemetry conflicts with the no-phone-home promise |

This grouped table covers the captured core interfaces and dynamic capabilities, but it is not a
substitute for the required member-by-member ledger and version-pinned conformance tests.

The captured `BrowserDocumentation` interface is orphaned from the typed `Agent` graph. Do not
implement it solely because it appears in the interface inventory.

### Playwright MCP

Playwright is strong naming and workflow prior art, but not a wholesale compatibility target for
the user's authenticated browser. Its accessibility snapshot, target pairing, list/detail output,
actionable errors, opt-in packs, and settle default should influence native semantics. Storage and
network mutation, test tracing, and unsafe code remain outside the default.

### Gemini and Microsoft browser products

Public sources establish capabilities and human-control flows, not model-visible dictionaries.
Do not manufacture exact profiles. Confirmation, takeover, resume, and stop are control-plane/UI
states. The model may observe `blocked` or `held`; it cannot approve itself.

## Implementation boundary and migration sequence

The smallest sound dependency direction keeps four registries distinct without duplicating
authority:

1. `ghostlight-transport` owns the protocol-neutral typed `BrowserOperation` DTOs, semantic ids,
   shared defaults, and canonical result vocabulary used on both sides of the local bridge.
2. `ghostlight-core` owns `OperationDescriptor`: canonical validation, concrete variant-to-RAWX
   requirements, resource resolution, scheduling, handler, post-processing, and operation
   availability. It never sees a model-facing description or vendor name as an execution key.
3. `ghostlight-mcp-connector` owns flat `SurfaceProfile` declarations, exact/version-bounded
   classification, external validation/normalization, catalog rendering, and result rendering.
   It consumes service-projected operation availability and still has no dependency on core.
4. The browser executor owns typed `MechanismId` requests. The extension dispatches those ids and
   retains a bounded legacy alias table during adapter skew. It knows no surface profile or policy.

Surface selection and mutable adapter state never enter `WorkspaceRegistry`, `WorkContext`
routing, scheduler keys, the extension `guid`, or a governance decision. A surface invocation can
carry bounded presentation facts such as profile id and external name for corrective copy and
audit, but canonical operation identity alone drives enforcement.

Changing the typed bridge `Start` and catalog projection is a wire break. The implementation must
bump bridge major 1 or deliberately dual-stack it; silently reusing major 1 is not compatible.
The simplest plan is a coordinated fail-loud bridge-major bump with old-edge/new-service and
new-edge/old-service tests. The extension wire changes separately under ADR-0093: negotiate a new
mechanism-request feature, emit the legacy `tool_request` form to old adapters, and accept both
forms throughout the declared skew window.

Keep every migration stage releasable:

1. Accept the ADRs, preserve these captures as the oracle, and pin today's tool and extension
   wires before moving them.
2. Add typed operations and convert the bridge, work context, pipeline, audit, and recursive
   compositions while a temporary identity decoder keeps the current 25-tool behavior exact.
3. Move current declarations into an edge-local `ghostlight-legacy/v1` surface. The service
   projects operation availability; the edge renders the byte-identical current catalog.
4. Isolate typed browser mechanisms in core while serializing the legacy extension wire.
5. Add the negotiated extension mechanism wire and its legacy alias translator.
6. Implement and evaluate `ghostlight-native/v1`; make it the unknown-client fallback only after
   its ADR explicitly amends the current trained-surface default and journey gates pass.
7. Enable one captured flat adapter at a time, with Claude first only if missing multi-browser and
   shortcut behaviors are either honestly implemented or omitted by a versioned supported subset.
8. Deliver Codex as a plugin/runtime module over the existing persistent Node kernel. Its frozen
   proxies call the ordinary Ghostlight connector or another typed local adapter path; do not add
   a fake MCP `js` tool or a second unrestricted JavaScript engine to the service.

Nested compatibility calls are normalized recursively. `script` and `browser_batch` become two
surface encodings of one canonical `browser.flow`; no nested external name survives the bridge.
Every execute-mode step retains its own canonical authorization and audit identity.

## Current Ghostlight migration map

| Current surface | Canonical destination |
|---|---|
| `tabs_context_mcp`, `tabs_create_mcp`, `tab_control` | `browser_tabs` |
| `navigate` | `browser_navigate` |
| `read_page` | `browser_snapshot` |
| `get_page_text` | `browser_read` |
| `find` | `browser_find` |
| `computer` screenshot and region zoom | `browser_screenshot` |
| Ref/semantic `computer` click, hover, and scroll-to; `form_input`; `act_on` | `browser_act` |
| Screenshot-frame coordinate click/hover/drag/wheel and targetless focused type/key | Precision `browser_input` |
| Fixed `computer` wait | `browser_wait` |
| `form_fill` | `browser_fill` |
| `wait_for` | `browser_wait` |
| `script`, `browser_batch` | `browser_flow` |
| `dialog` | `browser_dialog` |
| `file_upload`, `upload_image` | `browser_upload` |
| `read_console_messages`, `read_network_requests` | diagnostics pack |
| `javascript_tool` | execute pack |
| `resize_window` | Precision `browser_viewport` |
| `gif_creator` | media pack |
| `narrate` | presentation pack |
| `explain` | `browser_context` plus denial recovery and flow preflight |
| `update_plan` | Compatibility-only client workflow echo; no browser operation |

The mapping is intentionally not a rename table. Several current tools are surface-level splits
of one semantic operation, and some are compatibility affordances rather than browser semantics.

## Audit and governance invariants

Authorization happens after normalization. The canonical operation owns RAWX requirements,
resource resolution, workspace use, scheduling, readiness behavior, post-processing, and audit
identity. A surface adapter cannot lower any of them.

Audit keeps the external fact without confusing it for the governed operation:

```text
operation: browser.act
surface_profile: claude-cowork/2026-08-08
surface_tool: computer
surface_action: left_click
```

No tool arguments, page text, target name, form value, screenshot, or raw bearer handle enter the
audit record. Internal mechanisms are correlated children of the one semantic intent, not
unrelated model actions. A `browser_flow` is one orchestration request containing explicitly
authored step intents. Each step keeps its canonical identity, authorization, audit, in-band
source provenance, parent correlation, and shared-lease facts.

## Desirability and feasibility

| Proposal part | Desirability | Feasibility | Judgment |
|---|---|---|---|
| Typed semantic kernel | Very high | High | The current registry/pipeline already contains most facts but needs separation |
| Native one-to-one profile | Very high | High | Best chance to reduce names, turns, stale refs, and ambiguous results |
| Claude flat adapter | High if eval-positive | Medium/high | Exact schema is captured; missing browser/shortcut capabilities must be omitted or built |
| Playwright-compatible flat adapter | Medium | Medium/high | Public schema is exact, but authenticated-browser semantics differ materially |
| Codex runtime adapter | High for Codex | Medium | Best as a Codex plugin over the existing Node runtime, not an MCP rename layer |
| Gemini/Microsoft exact profile | Unknown | Low today | No exact declarations are public or captured |
| Automatic client defaulting | High with safeguards | Medium/high | Exact allowlist plus version range is adequate for schema choice, never authority |
| Hidden settle defaults | High when truthful | High | Use one budget and structured timeout status; avoid duplicate waits in explicit-wait clients |
| Full vendor-surface cloning | Low | Low | Hidden contracts, version churn, missing capabilities, and false equivalence dominate |

## Evaluation and ship gates

Every profile must pass a version-pinned mapping test and browser journeys before becoming an
automatic default. Compare at least two client/model configurations on:

- task completion and wrong-effect rate;
- first-call schema validity;
- tool-selection errors and recovery turns;
- model turns, tool calls, input/output bytes, image bytes, latency, and cost;
- stale-ref and stale-coordinate failures;
- navigation readiness and timeout interpretation;
- denial, hold, cancellation, and outcome-unknown correctness;
- tab ownership, generation invalidation, and cleanup; and
- audit equivalence between the external dialect and canonical operation.

Ship an adapter only when it repeatedly beats or materially complements the native fallback.
Retire or fall back when a client leaves the tested range. Never show two equivalent profiles at
the same time.

## Primary sources

- [OpenAI computer use](https://developers.openai.com/api/docs/guides/tools-computer-use)
- [OpenAI product changelog](https://learn.chatgpt.com/docs/changelog)
- [Playwright MCP v0.0.79 interface](https://raw.githubusercontent.com/microsoft/playwright-mcp/v0.0.79/README.md)
- [Playwright MCP capabilities](https://playwright.dev/mcp/capabilities)
- [Gemini API Computer Use](https://ai.google.dev/gemini-api/docs/computer-use)
- [Gemini in Chrome auto browse](https://support.google.com/gemini/answer/16821166?hl=en)
- [Browser Use v0.13.7 MCP source](https://github.com/browser-use/browser-use/blob/0.13.7/browser_use/mcp/server.py)
- [agent-browser v0.33.2 interface](https://raw.githubusercontent.com/vercel-labs/agent-browser/v0.33.2/README.md)
- [Stagehand v3 act](https://docs.stagehand.dev/v3/basics/act)
- [Chrome DevTools MCP tool reference](https://raw.githubusercontent.com/ChromeDevTools/chrome-devtools-mcp/main/docs/tool-reference.md)
