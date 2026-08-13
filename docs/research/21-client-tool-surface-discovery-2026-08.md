# Client tool-surface discovery (2026-08)

**Date:** 2026-08-08
**Status:** Evidence capture. No product surface or implementation decision.

## Question

Ghostlight may eventually present one client-facing tool dialect for a known vendor-paired
client and a Ghostlight-native dialect for every other client. This research records the
declarations that selected clients actually expose before deciding whether compatibility
adapters are useful, supportable, or measurably better.

A declaration capture is not evidence of model training. It is evidence of one client-visible
contract at one point in time. This document therefore uses `compatibility dialect` or
`evaluation-tuned profile`, not `training-matched surface`.

## Evidence classes

Keep these classes separate:

1. **Exact declaration evidence:** a client-visible name, description, and schema captured
   without reconstruction.
2. **Observed behavior:** a request and result from a controlled invocation.
3. **Interpretation:** a proposed semantic mapping, default, or architecture consequence.

The preserved corpus contains both flat declaration evidence and one installed programmable
runtime schema. The owner also supplied a model report that a read-only check found a live Chrome
connection, but the underlying request, result, and response metadata were not included. That
report is behavior evidence, not an exact invocation capture. Client-side projection may differ
from an underlying MCP server declaration or generated runtime API.

## Corpus

| Artifact | Scope | Completion |
|---|---|---|
| [Claude Cowork / Claude-in-Chrome](tool-surfaces/claude-cowork-claude-in-chrome-2026-08-08.json) | The 22 deferred tools exposed by Claude Cowork through exact-name ToolSearch queries | Complete for the reported client-visible name, description, and input-schema surface |
| [Codex desktop filtered surface](tool-surfaces/codex-desktop-filtered-surface-2026-08-08.json) | Direct interfaces, the non-MCP nested inventory, and the retained `node_repl` Chrome gateway from the full 181-tool registry dump | Complete for the declared gateway |
| [Codex Chrome runtime API](tool-surfaces/codex-chrome-runtime-v26.803.41515-api.json) | Sanitized installed interface schema for the injected Chrome runtime | Complete for 22 interfaces, 59 declared types, and 136 members; packaged prose, machine paths, implementation code, and live tab data are excluded |
| [Playwright MCP default tools/list](tool-surfaces/playwright-mcp-v0.0.79-default-tools-list.ndjson) | Exact initialize and tools/list exchange from pinned `@playwright/mcp@0.0.79` | Complete for the 24 default tools |
| [Playwright MCP optional tools](tool-surfaces/playwright-mcp-v0.0.79-optional-tools.json) | Exact optional declarations mechanically separated from an all-capabilities tools/list exchange | Complete for 45 optional tools, 69 total with the default set |

The reusable [vendor capture prompt](tool-surfaces/CAPTURE-PROMPT.md) requests the same
declaration-only evidence without browser actions or browsing-data collection. Use it for Gemini
or a later client recapture rather than reconstructing a dictionary from prose.

The owner-provided Claude source file has SHA-256
`94419bb44a50fc8b8c36431eda629fb714bfe422863da00ac5eb2aea41bf684c`.
The tracked artifact has SHA-256
`b8d1285c3e89bee7982bc42e4e24813c77301228ab0b046207c30734e16f799f`.
The parsed JSON values are identical. The tracked copy only encodes seven literal U+2014
characters as JSON `\u2014` escapes to meet this repository's ASCII-only rule.

The owner-provided full Codex registry dump has SHA-256
`32e9dc4789469d1d3c1ee0f5c3e80b88ded876234134a4eb5ba3fdc2035ff418`.
It contains 181 nested callable tools, including 150 namespaced as MCP tools, plus nine direct
host and collaboration interfaces. The tracked filtered artifact has SHA-256
`62189d3bf61ccdeaa08daa3e8c990ba2e7940c74909027b21b6b3b43ae05ddac`.
It excludes 117 `codex_apps` projections, 25 Ghostlight tools, and five OpenAI Developer Docs
tools from the Codex browser analysis. It retains the three `node_repl` declarations because
their exact guidance identifies the Browser and Chrome plugins as intended runtimes.

The owner-provided Codex Chrome runtime dump has SHA-256
`c3c471d0d2a237a42ce2b486b858fed3e8472a106125c4cf1fb871e27db1cdd6` and identifies installed
plugin version `26.803.41515`. The tracked interface-only artifact has SHA-256
`0a3a7a6e3c5f6abf961c0a0ce06b5617e70e2d9a74ad34c380f84b94ec42c6a8`. The tracked copy excludes
machine-local paths, packaged control prose, examples, implementation code, and live browsing
data. It preserves the exact installed interface schema and its support-override metadata.

The pinned Playwright default exchange has SHA-256
`791e01b66b8a2e6555109982dcebcd5e6fcb3ee4a51e5a813ca975bdd6b1f015`. The optional-tool
artifact has SHA-256
`385318e76b9d0f36ed92ee1b43fb755af5c6f73d5578c7dbb96376c7e1930fee`. Both were acquired by
running `@playwright/mcp@0.0.79`; the initialized server identified its engine as
`1.63.0-alpha-2026-08-05`.

## Claude Cowork / Claude-in-Chrome

The capture contains 22 unique tools with ordinals 1 through 22 and `complete: true`.
It reports Cowork mode in the Claude desktop app, model `claude-opus-5`, and acquisition by
deferred ToolSearch. Client version, extension version, browser version, and underlying MCP
server identity remain unobserved.

The client exposed only a namespaced name, description, and draft-07 input schema. It did not
expose a server-declared name, output schema, annotations, `_meta`, or other declaration fields.
Null values in the artifact mean unobserved, not absent from the underlying server.

### Surface shape

The surface combines several concern levels:

- Page observation and action: `read_page`, `get_page_text`, `find`, `computer`,
  `form_input`, `file_upload`, `upload_image`, and `javascript_tool`.
- Navigation and tab lifecycle: `navigate`, `tabs_context_mcp`, `tabs_create_mcp`, and
  `tabs_close_mcp`.
- Diagnostics and media: `read_console_messages`, `read_network_requests`, `resize_window`,
  and `gif_creator`.
- Orchestration: `browser_batch`, `shortcuts_list`, and `shortcuts_execute`.
- Client and device selection: `list_connected_browsers`, `select_browser`, and
  `switch_browser`.

This is useful counter-evidence to treating every exposed tool as one browser-engine primitive.
The last two groups are product coordination and workflow facilities. They should not become
physical Chrome commands merely because one vendor presents them beside page actions.

### High-signal schema details

- `computer` has the same 13-action enum as current Ghostlight and additionally exposes
  `save_to_disk`. Its required fields are `action` and `tabId`.
- `navigate` declares `tabId` and `url` but has no `required` array. Its description supplies
  standalone tab creation and selection behavior that the schema does not encode.
- `browser_batch.actions.items.required` contains `input` but not `name`.
  It also declares no minimum array length.
- `find` requires `tabId` but not `query`. `form_input.value` has no declared type, and its
  target `ref` is optional by schema.
- `javascript_tool` requires only `tabId`; both `action` and `text` are optional by schema.
- `select_browser` describes `deviceId` as its selector but does not require it in the schema.
- `shortcuts_execute` requires only `tabId`; `command` and `shortcutId` are both optional in
  the captured schema.
- `gif_creator` prose mentions `coordinate`, but the schema declares neither `coordinate` nor
  `ref`.
- `upload_image` requires only `tabId`; even `imageId` is optional by schema.
- All 22 root schemas omit `additionalProperties`, so draft-07 permits unknown fields.
- Several prose constraints, including `upload_image` target exclusivity, are not represented
  structurally. The artifact preserves those gaps rather than repairing them.

These are compatibility facts, not quality recommendations. A Ghostlight-native schema should
remain precise even when an adapter accepts a looser vendor shape.

### Name-set delta from current Ghostlight

Sixteen names overlap after removing the client namespace:

`browser_batch`, `computer`, `file_upload`, `find`, `form_input`, `get_page_text`,
`gif_creator`, `javascript_tool`, `navigate`, `read_console_messages`,
`read_network_requests`, `read_page`, `resize_window`, `tabs_context_mcp`,
`tabs_create_mcp`, and `upload_image`.

Claude-only names are:

`list_connected_browsers`, `select_browser`, `shortcuts_execute`, `shortcuts_list`,
`switch_browser`, and `tabs_close_mcp`.

Current Ghostlight-only names are:

`act_on`, `dialog`, `explain`, `form_fill`, `narrate`, `script`, `tab_control`,
`update_plan`, and `wait_for`.

The six Claude-only names do not have equal implementation cost. `tabs_close_mcp` is a direct
projection of canonical tab-close semantics. Browser discovery, selection, and switching need
real multi-browser discovery and pairing. The shortcut tools depend on a Claude side-panel
workflow runtime. A schema adapter alone cannot implement the latter two groups honestly.

Name overlap does not imply schema or behavior equality. For example, Ghostlight currently
requires `tabId` for `navigate`, while the captured Claude description defines an implicit-tab
standalone path. Ghostlight's `tab_control` also subsumes explicit close behavior without using
the Claude `tabs_close_mcp` name.

## Codex desktop and Chrome gateway

The supplied Codex desktop dump exposes a two-layer surface:

1. Nine outer declarations: `functions.exec`, the state-gated `functions.wait`, the mode-gated
   `functions.request_user_input`, and six collaboration tools.
2. A runtime catalog of 181 nested name and description entries callable inside
   `functions.exec`.

The full registry includes installed and projected tools that are not part of Codex's native
browser vocabulary. Prefix filtering is insufficient because the actual Chrome-aware gateway
also has an MCP-prefixed name. The derived artifact therefore uses provenance and semantics:
registered Ghostlight, app, and documentation servers are excluded; `node_repl` is retained.

### Model-facing Chrome entry point

`mcp__node_repl__js` is the only declared executor that expresses Chrome actions. Its exact
description says to use the persistent Node-backed kernel with the Chrome Plugin and to prefer
that path over Computer Use unless the user asks otherwise. Its declaration is:

```text
code: string
timeout_ms?: number
title?: string
```

The kernel supports top-level await, persistent bindings, dynamic module imports, and a default
30-second timeout. `mcp__node_repl__js_add_node_module_dir` adds a persistent module search root,
and `mcp__node_repl__js_reset` clears the kernel. `functions.exec` is the outer orchestration
wrapper; `functions.wait` is conditional support when a yielded execution remains active.

This means the Codex Chrome dialect is programmatic, persistent, and stateful. Browser verbs are
methods on a dynamically supplied JavaScript or Playwright-style runtime behind `js`, not
individual model-visible tool declarations. The separately supplied installed runtime schema
fills the prior object-model gap. It does not prove that a particular browser was live for a
particular call.

### Installed runtime shape

The captured runtime declares 22 interfaces, 59 named types, and 136 members. Its meaningful
layers are:

- Discovery and session: `Agent`, `Browsers`, `Browser`, and `BrowserUser` bootstrap a browser,
  select a backend, name a session, and distinguish controlled tabs from user tabs.
- Tab lifecycle: `Tabs` and `Tab` list, create, get, select, claim, close, finalize, navigate,
  inspect title/URL, handle dialogs, and capture screenshots.
- Targeting ladder: `PlaywrightAPI` and locators provide semantic actions first;
  `DomCUAAPI` provides fresh DOM-node actions; `CUAAPI` provides coordinate fallback.
- Event resources: download and file-chooser objects must be armed before their triggering
  action. Clipboard, logs, and dialogs carry their own bounded state.
- Dynamic capabilities and documentation: browser and tab capability collections plus
  per-browser effective documentation describe only what the current backend supports.

The state contract is as important as the method signatures. Browser disconnection invalidates
every descendant handle. Navigation invalidates node, frame, dialog, coordinate, page-asset,
WebMCP, and origin-scoped state. A kernel reset invalidates JavaScript bindings without proving
that physical tabs disappeared. Locator builders can survive as declarative recipes, but resolve
against the current document at action time. Tab finalization is a terminal fence with different
cleanup for agent-created and claimed user tabs.

That contract cannot be reproduced honestly by advertising a lone `js` function. A faithful
Codex integration needs the persistent kernel, proxy objects, effective documentation, capability
negotiation, and generation-aware state. The lowest-risk integration is a Codex plugin/runtime
module that uses Codex's existing Node kernel and sends each proxy method to Ghostlight as a typed
canonical operation. A second general-purpose JavaScript engine inside Ghostlight is possible but
would duplicate the wrong layer.

`web__run` remains an independent web-retrieval multiplexer, not authenticated Chrome control.
`codex_app__open_in_codex` can present a browser tab in the Codex UI but explicitly delegates
inspection and interaction to other tools.

Official OpenAI documentation confirms that the Codex Chrome extension supports browser work in
signed-in Chrome context across background tabs. It does not publish the model-facing runtime
schema. The installed capture supplies that local version's runtime surface. The owner's reported
live-connection check is consistent with the declared gateway, but remains separate from the
schema evidence because the raw invocation result was not retained.

## Playwright MCP v0.0.79

The pinned live capture contains 24 default tools and 45 opt-in tools. The default surface covers
navigation, tabs, accessibility snapshots, targeted interaction, form fill, waits, screenshots,
console and network reads, dialogs, uploads, and an explicitly named unsafe code escape hatch.
Optional packs add configuration, routing/network state, cookies and web storage, DevTools
recording and annotation, coordinate vision actions, PDF, and test assertions.

Several design choices explain the surface:

- Accessibility snapshots and stable targets are the deterministic action substrate.
  Screenshots are for appearance, not the default action locator.
- Many element calls pair a human-readable element description with an exact target. The former
  helps permission and intent presentation; the latter gives deterministic execution identity.
- `browser_find` returns bounded snippets and paths instead of replaying a whole snapshot.
- Network requests use list-then-detail, and large reads can write a file instead of flooding
  model context.
- `browser_tabs` combines list, create, close, and select behind one action union.
- Capability packs keep specialist schemas out of the ordinary context. The official rationale
  is lower token cost, fewer hallucinated calls, and lower latency.
- The unsafe escape hatch says `unsafe` in its callable name and description.
- Version 0.0.79 adds a default 500 ms settle timeout after actions, evidence that readiness can
  be an execution-pipeline default without repeating one flag across every client schema.

The exact capture also exposes documentation drift. Current web documentation lists some tools
that the pinned source marks skill-only and omits some released tools. Exact `tools/list`
evidence, package version, and source tag therefore belong together.

## Gemini in Chrome and Gemini API Computer Use

Google publishes product capabilities for Gemini in Chrome, not its model-visible tool
declarations. Public documentation establishes shared-tab reading and comparison, multi-tab work,
auto-browse navigation/click/fill, local and remote browser modes, visible task indicators,
takeover and resume, and confirmations for sensitive actions. It does not establish tool names,
JSON schemas, descriptions, ordering, or result envelopes. There is no honest Gemini-in-Chrome
dialect to implement until a consenting client yields an empirical capture.

Gemini API Computer Use is a separate, public contract. The current browser action vocabulary
contains coordinate clicks, mouse and keyboard state, typing, drag, screenshot, scroll, history,
navigation, and wait. Coordinates are normalized to 0 through 999 and each action carries an
intent string. The host returns the current URL and a fresh screenshot, and safety decisions may
require confirmation. Its older 2.5 preview used a materially different vocabulary. Any Gemini
API adapter therefore needs an explicit contract/model version and must not be labeled Gemini in
Chrome.

## Other major public surfaces and counterpatterns

These are prior art, not captures of first-party Chrome product internals.

- OpenAI's current built-in `computer` contract is one tool whose response contains ordered
  batches of nine action variants: click, double click, drag, keypress, move, screenshot, scroll,
  type, and wait. The host returns a fresh screenshot after execution. OpenAI also explicitly
  documents custom Playwright, Selenium, VNC, MCP, and code-execution harnesses as intended model
  environments. That makes exact built-in-schema imitation an evaluation hypothesis, not a
  prerequisite for model competence.
- Browser Use v0.13.7 exposes 16 source-inspectable MCP tools: navigate, click, type, scroll, back,
  state, content extraction, HTML, screenshot, three tab calls, an autonomous retry fallback, and
  three session-lifecycle calls. Its source removed a JSON Schema `oneOf` from click because it
  broke Claude clients. This is direct evidence for client-specific schema rendering over a
  stronger canonical constraint.
- agent-browser v0.33.2 combines a persistent daemon, stable `@eN` element refs, stable tab
  handles, and selectable core/network/state/debug/tabs/react/mobile/all profiles. Its public MCP
  documentation exposes a small common core and paginated profile discovery. The useful lesson is
  progressive capability disclosure, not its ungoverned `extraArgs` escape hatch.
- Stagehand v3 is the semantic counterpattern: `observe`, `act`, `extract`, and autonomous `agent`
  over DOM, coordinate, or hybrid modes. It optimizes a stable semantic abstraction and provider
  adapters rather than cloning each vendor dictionary.
- Chrome DevTools MCP v1.6.0 publishes 52 tools across input, navigation, emulation, performance,
  network, debugging, memory, extensions, third-party, and WebMCP groups. Its official slim mode
  is exactly navigate, evaluate, and screenshot. This is strong prior art for a small default and
  optional diagnostic depth, although its disposable debugging context and default telemetry or
  update behavior are not Ghostlight assumptions.
- Microsoft publishes capability contracts rather than model tool declarations. Copilot Vision
  observes and highlights without acting; Browse and Cowork navigate, select, scroll, and type,
  with per-site/action permission, confirmation, and human handoff. One vendor family therefore
  still needs runtime-mode classification rather than one guessed schema.

The recurring design is one execution substrate with either capability packs, semantic adapters,
or a programmable runtime. None supports advertising several duplicate dialects at once.

## Architecture implications

The evidence strengthens the proposed separation, with guardrails.

1. **Surface profile.** One model-facing declaration set owns vendor-compatible names,
   descriptions, schemas, and default translation. Exactly one profile is exposed per
   connection.
2. **Canonical semantic operation.** A typed Ghostlight vocabulary owns intent, stable result
   meaning, readiness defaults, scheduling, governance classification, and audit identity.
   The Ghostlight-native profile should project this vocabulary one-to-one.
3. **Physical mechanism.** Chrome and CDP actions implement the operation without inheriting
   Claude, Codex, or Ghostlight-facing names.

Known-client classification can choose a default surface profile, but only as schema and runtime
adaptation. Use an explicit override first, then an exact allowlisted `clientInfo.name` plus a
tested version range, then the Ghostlight-native fallback. A profile must never grant authority,
weaken policy, select a workspace, or change audit obligations. A connection-bound protocol may
pin the profile at initialization. A request-stateless protocol must resolve it from immutable
request context or carry an explicit surface handle; it must not borrow another request's choice.

Stable delight defaults belong to canonical operations, not duplicated adapter handlers. A
navigation operation can, for example, own one bounded readiness budget and return successful
navigation after a proven commit with separate condition and settlement facts plus aggregate
`ready`, `timed_out`, or `unavailable` readiness. A vendor profile may omit those fields from its
input signature while still mapping to the same internal default. The profile and semantic-default
versions must remain explicit because behavior is part of the contract even when the input schema
does not change.

A compatibility adapter may be synthetic and fully functional, but it cannot advertise a
vendor tool whose physical capability Ghostlight lacks. Claude's browser selection and shortcut
tools are the immediate examples. Implement the canonical capability first, or omit that
profile until it can satisfy the declaration honestly.

## Feasibility and desirability update

- A canonical operation kernel plus one-to-one Ghostlight profile is highly desirable and
  technically feasible.
- Translators for captured, versioned vendor schemas are feasible where all declared behavior
  maps to real canonical capabilities.
- `clientInfo`-selected defaults are feasible for exact, allowlisted client families. Automatic
  model inference remains unnecessary and unreliable.
- A Codex gateway-compatible overlay is technically different from a Claude schema adapter. The
  captured dynamic API confirms that it needs a controlled persistent JavaScript facade backed by
  canonical Ghostlight operations, preferably delivered through Codex's existing plugin and Node
  runtime rather than reimplemented inside the service.
- Maintaining speculative clones of hidden or fast-changing product surfaces is undesirable.
  Each profile must earn maintenance cost through journey evaluations.

## Capture and evaluation gate

Before accepting a vendor profile:

1. Record client, client version, model, extension or plugin version, browser version, date,
   acquisition method, and exact declaration evidence. Mark every unobserved field.
2. Preserve names, descriptions, schemas, annotations, metadata, and ordering separately from
   behavior observations. Never repair a vendor schema inside the evidence artifact.
3. Version-pin the proposed profile and map each external tool to a typed canonical operation or
   reject it as unsupported.
4. Run representative browser journeys across the vendor profile and Ghostlight-native profile.
   Measure task success, first-call validity, recovery turns, calls, context bytes, latency,
   readiness behavior, and governance correctness.
5. Ship only a repeatable winner. Preserve one visible surface per connection and a safe
   Ghostlight fallback.

## Open evidence work

- In the exact Codex task exposing `node_repl`, retain a sanitized invocation envelope proving a
  live extension-backed browser, if behavior evidence is needed. Keep only backend kind, a hashed
  browser or extension identifier, documentation fingerprint, and tab count. Do not retain URLs,
  titles, page text, screenshots, or raw request metadata.
- Capture Gemini's first-party Chrome declaration surface with the same evidence format, if the
  client exposes it. Until then, keep the record capability-only.
- Add harmless behavior probes only after declaration capture, with request and result records
  kept separate from inferred semantics.
- Re-capture profiles when a mapped client or extension leaves its tested version range.

## Sources and local anchors

- [OpenAI product changelog](https://learn.chatgpt.com/docs/changelog)
- [OpenAI computer use](https://developers.openai.com/api/docs/guides/tools-computer-use)
- [Playwright MCP v0.0.79 interface](https://raw.githubusercontent.com/microsoft/playwright-mcp/v0.0.79/README.md)
- [Gemini in Chrome auto browse](https://support.google.com/gemini/answer/16821166?hl=en)
- [Gemini API Computer Use](https://ai.google.dev/gemini-api/docs/computer-use)
- [Browser Use v0.13.7 MCP source](https://github.com/browser-use/browser-use/blob/0.13.7/browser_use/mcp/server.py)
- [agent-browser v0.33.2 interface](https://raw.githubusercontent.com/vercel-labs/agent-browser/v0.33.2/README.md)
- [Stagehand v3 act](https://docs.stagehand.dev/v3/basics/act)
- [Chrome DevTools MCP tool reference](https://raw.githubusercontent.com/ChromeDevTools/chrome-devtools-mcp/main/docs/tool-reference.md)
- [Microsoft Browse with Copilot](https://support.microsoft.com/en-us/microsoft-copilot/browse-with-copilot)
- [0.8 Ghostlight tool-surface regression fixture](https://github.com/sylin-org/ghostlight/blob/c01cc3276102471f3e18de2ae90cb90abf98ed88/tests/tool_schema_fidelity.rs)
- [ADR-0069: journey-first evaluation](../adr/0069-agent-journey-evaluation-artifacts.md)
- [ADR-0094: stable tool identity and mutable guidance](../adr/0094-agent-readable-tool-definitions.md)
- [ADR-0096: one exact service and catalog](../adr/0096-protocol-versioned-mcp-edge-and-neutral-service.md)
