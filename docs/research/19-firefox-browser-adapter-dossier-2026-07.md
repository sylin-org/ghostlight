# Firefox browser-adapter dossier

**Status:** research and architecture input, not a decision

**Date:** 2026-07-15

**Scope:** Firefox in the interactive user's ordinary local browser context

**Primary question:** can Ghostlight keep one model-facing tool language while different browser
adapters implement it honestly through vendor-specific mechanisms?

## Executive conclusion

Firefox is a credible Ghostlight target, but not as a drop-in replacement for the current
Chromium extension.

There are two technically viable Firefox shapes:

1. **Extension-only Firefox.** A Firefox WebExtension can use native messaging, tabs, windows,
   content scripts, screenshots, webRequest, and Firefox-specific APIs. This preserves an ordinary
   user profile and needs no automation flag. It is a strong fit for identity, presentation, tab
   management, page reads, and many DOM operations. It is a weaker fit for trusted input, complete
   console/network instrumentation, dialogs, and recording because Firefox does not implement
   Chromium's `debugger` extension API.
2. **Hybrid Firefox.** A thin Firefox extension provides identity, focus, native messaging, and
   Ghostlight's visual language while Marionette/WebDriver BiDi provides browser control. This can
   cover most of Ghostlight's current semantic surface with higher input fidelity and exposes a
   valuable additional diagnostics surface. Mozilla's own Firefox DevTools MCP proves that an
   existing Firefox process can be automated with cookies, logins, and open tabs intact. The cost
   is material: Firefox must have Marionette enabled, `navigator.webdriver` becomes true, browser
   fingerprint signals change, and the local automation endpoint has no built-in authentication or
   encryption.

The right abstraction is a **browser adapter**, but it should not receive raw tool names and JSON
and translate them ad hoc. Ghostlight should first resolve a tool call into a typed, vendor-neutral
browser operation. An adapter then declares whether that operation is native, composed, degraded,
experimental, unsupported, or outside the product boundary and executes it through one or more
vendor mechanisms.

The model-facing tool contract remains stable. Browser-specific truth belongs in the connection
capability report, `initialize.instructions`, and a dynamic adapter section in `explain`. The 13
trained tool schemas and descriptions remain byte-stable.

For multiple connected browsers, focus is useful but not sufficient. The recommended order is:

1. a tab's encoded browser owner;
2. an explicit browser selection held as MCP-session affinity;
3. a single compatible connected browser;
4. the most recently focused compatible browser, if that signal is current and unambiguous;
5. a bounded disambiguation response.

Ghostlight must never silently move an operation to a different browser merely because that
browser supports it. Authentication state and page context differ between browsers.

The recommended next step is a bounded Firefox proof of concept, not a general adapter refactor.
The proof should establish the semantic operation seam, test extension-only and hybrid modes on
the Linux host, and answer the unresolved pairing, trusted-input, recording, and live-session
instrumentation questions before an ADR commits the product.

## Research posture

This dossier applies Ghostlight's existing product boundaries:

- the browser is local;
- the interactive user's visible, authenticated context is the point;
- headless, isolated, cloud, and remote browser execution are not product targets;
- governance remains in the Rust service, never in an extension;
- no adapter may phone home;
- unsupported behavior must be stated, not simulated poorly and called parity.

The WebDriver BiDi specification is used to describe the standards surface. A command appearing in
the specification does not prove that a particular Firefox release implements it. Mozilla's own
Firefox DevTools MCP and Firefox source documentation provide the stronger evidence for currently
usable Firefox behavior. A proof of concept must negotiate actual runtime capabilities instead of
assuming the whole specification.

## Primary sources

- [Firefox remote protocols](https://firefox-source-docs.mozilla.org/remote/index.html)
- [Firefox Remote Agent security](https://firefox-source-docs.mozilla.org/remote/Security.html)
- [Firefox Remote Agent preferences](https://firefox-source-docs.mozilla.org/remote/Prefs.html)
- [geckodriver flags and connect-existing](https://firefox-source-docs.mozilla.org/testing/geckodriver/Flags.html)
- [geckodriver profiles](https://firefox-source-docs.mozilla.org/testing/geckodriver/Profiles.html)
- [WebDriver BiDi specification](https://w3c.github.io/webdriver-bidi/)
- [Mozilla Firefox DevTools MCP](https://github.com/mozilla/firefox-devtools-mcp)
- [Mozilla's Firefox DevTools MCP documentation](https://firefox-source-docs.mozilla.org/ai-agent-tools/firefox-devtools-mcp.html)
- [Firefox WebExtension native messaging](https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Native_messaging)
- [Firefox tabs API](https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/API/tabs)
- [Firefox content scripts](https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Content_scripts)
- [Firefox tab capture](https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/API/tabs/captureTab)
- [Firefox DevTools extension API](https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/API/devtools)
- [Chrome incompatibilities in Firefox WebExtensions](https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Chrome_incompatibilities)
- [Event.isTrusted](https://developer.mozilla.org/en-US/docs/Web/API/Event/isTrusted)

## What Firefox actually offers

### WebExtensions

Firefox supports the parts of the extension architecture Ghostlight most needs for a familiar user
experience:

- native messaging over extension-bound manifests (`allowed_extensions` rather than Chromium's
  `allowed_origins`);
- tab and window enumeration, creation, activation, navigation, and focus events;
- content-script injection and page messaging;
- screenshot capture;
- webRequest observation and interception;
- cookies, downloads, history, contextual identities, privacy, proxy, clipboard, and browser
  settings APIs;
- DevTools pages that can inspect one explicitly inspected window.

This is enough to build a Firefox-native identity and presentation adapter. The current Chrome
extension cannot simply be repackaged, however. Firefox does not implement the Chromium
`chrome.debugger` API, which is Ghostlight's current path to CDP. API and lifecycle differences also
require a real Firefox package rather than a manifest-only fork.

An extension-only controller can invoke DOM methods and dispatch DOM events. Those events are not
equivalent to browser-generated input: `Event.isTrusted` is false for events dispatched by
`dispatchEvent()` and for `HTMLElement.click()`. Sites that depend on trusted input or browser
default behavior can therefore fail. Extension-only input is **degraded**, not full parity.

### Marionette and WebDriver Classic

Marionette is Firefox's longstanding automation protocol. geckodriver normally starts a new
Firefox process and temporary profile, but `--connect-existing` connects to an already running
Firefox that was started with `--marionette`. Mozilla's Firefox DevTools MCP documents this exact
mode for a real browsing session with cookies, logins, and open tabs intact.

That is strategically important: Firefox automation is not limited to a disposable test browser.
It can meet Ghostlight's user-context premise.

It also carries costs that Chromium Ghostlight does not currently impose:

- the user must start Firefox with Marionette enabled or persist that preference;
- automation changes browser preferences unless explicitly constrained;
- `navigator.webdriver` and other fingerprint signals change;
- Mozilla warns not to leave Marionette enabled during ordinary browsing;
- connect-existing in Mozilla's current MCP lacks the BiDi-dependent console and network event
  features;
- the controller endpoint is designed for local automation, not as an authenticated application
  boundary.

### WebDriver BiDi

WebDriver BiDi is the strongest long-term cross-browser control surface. The standard includes:

- browsing-context creation, activation, navigation, reload, close, history traversal, viewport,
  screenshot, print, prompts, and context-tree inspection;
- DOM location by CSS, XPath, inner text, and accessibility-related locators;
- script evaluation, function calls, realms, preload scripts, and serialization;
- keyboard, pointer, wheel, drag-like action sequences, and file input;
- console events;
- network observation, response/request data collection, interception, header changes, cache
  behavior, authentication, failure, and response provision;
- cookie read, set, and delete;
- user contexts, client windows, and download behavior;
- geolocation, locale, timezone, media, color-scheme, screen, and other emulation controls;
- WebExtension installation and removal.

This surface is broader than Ghostlight today. It is also evolving. Ghostlight should negotiate
actual command support at connection time and should distinguish standardized, implemented, and
experimentally enabled features.

Firefox 141 removed its CDP implementation; WebDriver BiDi is now the Remote Agent protocol. A
Firefox strategy should therefore target BiDi and Marionette, not a Firefox-CDP compatibility
layer.

### Firefox Remote Debugging Protocol

Firefox also has its internal Remote Debugging Protocol, used by Firefox DevTools actors. It can
reach deep debugger and browser internals but is Firefox-specific, less suitable as the primary
portable adapter seam, and can move independently of a web standard. It is useful as an optional
Firefox diagnostics backend when BiDi lacks a capability, not as the default Ghostlight contract.

### Mozilla's own MCP as feasibility evidence

Mozilla's Firefox DevTools MCP is not a design template for Ghostlight, but it is unusually strong
feasibility evidence. Its current surface includes:

- pages, snapshots, UID-based click/hover/fill/drag/upload, and form fill;
- console and network reads;
- page and element screenshots;
- page script evaluation;
- dialog handling, history, and viewport changes;
- WebExtension install, uninstall, and listing;
- Firefox preferences, process output, restart, and build information;
- Gecko profiler start/stop;
- privileged Firefox UI context access;
- Firefox for Android through ADB.

Several of those capabilities are explicitly opt-in because they expand authority. Ghostlight
should preserve that separation and apply RAWX governance to any future public tool, rather than
copying Mozilla's surface wholesale.

## Support vocabulary

Every adapter capability should carry one of these states:

| State | Meaning |
|---|---|
| **Native** | The vendor mechanism directly provides the required semantics and fidelity. |
| **Composed** | Ghostlight can provide the semantics by combining native operations without a material fidelity loss. |
| **Degraded** | The operation is useful but observably weaker, less complete, or behaviorally different. |
| **Experimental** | The mechanism exists but runtime/version support or reliability is not yet sufficient for a stable claim. |
| **Unsupported** | The connected adapter cannot perform the operation. |
| **Boundary-excluded** | Technically possible, but intentionally outside Ghostlight's product or security boundary. |

These states are connection facts, not marketing labels. A single Firefox installation can report
different states depending on whether only the extension is connected, Marionette is enabled,
BiDi is available, experimental commands are enabled, or system access was granted.

## Current Ghostlight surface mapped to Firefox

The following table maps every currently advertised Ghostlight tool. "Extension" means a
Firefox WebExtension plus Ghostlight native messaging. "Hybrid" means that extension plus a
paired Marionette/BiDi controller. Local orchestration tools do not depend on either vendor.

| Ghostlight tool | Semantic job | Firefox extension | Firefox hybrid | Important caveat |
|---|---|---|---|---|
| `tabs_context_mcp` | List and describe tabs | Native | Native | Vendor, adapter mode, profile label, and capability state should be added to the internal surface model. |
| `tabs_create_mcp` | Create a tab | Native | Native | BiDi context IDs must normalize to Ghostlight composite tab IDs. |
| `navigate` | Navigate a tab | Native | Native | Privileged Firefox URLs remain restricted unless system access is explicitly enabled. |
| `computer` | Pointer, keyboard, scrolling, screenshot, wait, zoom | Degraded | Native/Composed | Extension DOM input is not trusted input. BiDi actions provide the stronger path. Zoom needs Firefox-specific handling. |
| `find` | Locate text or elements | Composed | Native/Composed | Normalize locator results and ref lifetime; do not expose vendor node handles. |
| `form_input` | Set one form control | Degraded | Native/Composed | Extension event synthesis can differ from real input. |
| `get_page_text` | Extract visible page text | Native/Composed | Native/Composed | Keep frame and origin provenance identical to Chromium. |
| `javascript_tool` | Evaluate arbitrary page JavaScript | Native/Composed | Native | Extension world selection and BiDi realm selection must have explicit semantics. |
| `read_console_messages` | Read console output | Degraded | Degraded in connect-existing | A DevTools extension page can inspect a selected page; Mozilla's current connect-existing mode lacks BiDi console events. |
| `read_network_requests` | Read network activity | Degraded | Degraded in connect-existing | webRequest can observe future traffic but differs from CDP. Mozilla's current connect-existing mode lacks BiDi network events. |
| `read_page` | Return a structured page representation | Composed | Composed | Mozilla proves snapshot feasibility, but Ghostlight's ref, frame, token, and provenance contract must be reproduced. |
| `resize_window` | Resize the visible browser window | Native | Native | Distinguish outer window size from BiDi viewport emulation. |
| `update_plan` | Update model-visible task plan | Native local | Native local | No browser dependency. |
| `narrate` | Present a transient agent message | Native | Native | This belongs to the Firefox extension presentation channel, not BiDi. |
| `wait_for` | Wait for page state | Composed | Composed | Service orchestration should remain adapter-neutral. |
| `script` | Execute a sequence of Ghostlight tools | Native local | Native local | Each inner operation resolves against the pinned browser context. |
| `form_fill` | Fill several fields and optionally submit | Degraded | Composed | Extension-only submission inherits synthetic-input limitations. |
| `act_on` | Semantic action with optional postcondition | Degraded | Composed | Adapter must preserve target resolution and postcondition receipts. |
| `dialog` | Inspect and answer browser dialogs | Unsupported | Native | WebDriver BiDi exposes prompt events and handling. |
| `tab_control` | Focus, reload, or close a tab | Native | Native | Focus reports should update the service's browser recency only after the operation succeeds. |
| `file_upload` | Set files on an input | Degraded | Native | BiDi `input.setFiles` is the reliable path; path governance remains in the service. |
| `browser_batch` | Execute a batch with bounded results | Native local | Native local | Resolve and pin one browser for the batch; never reroute inner calls mid-batch. |
| `upload_image` | Reuse a cached capture as page input | Degraded | Composed | Screenshot provenance and file placement must remain tied to the same browser/session. |
| `gif_creator` | Record and export a visible tab | Experimental | Experimental | Mozilla's current MCP does not demonstrate recording. BiDi screencast support and Firefox capture performance require measurement. |
| `explain` | Explain capabilities and requirements | Native local | Native local | Static tool truth can remain deterministic; append a dynamic connected-adapter section. |

### Parity summary

An extension-only Firefox adapter can credibly cover identity, focus, presentation, tab lifecycle,
page reads, page scripts, screenshots, and a useful subset of DOM actions. It should not claim full
parity for input, dialogs, console/network, uploads, or recording.

A hybrid adapter can plausibly cover nearly the entire current surface. The main unresolved parity
items are:

- pairing the extension identity with the correct Marionette/BiDi browser instance and profile;
- console and network events in a real connect-existing session;
- Chromium-equivalent recording and capture performance;
- exact behavior of zoom, downloads, restricted pages, nested frames, and accessibility snapshots;
- preserving Ghostlight's visual presentation while BiDi performs the action;
- proving that automation preference changes do not disturb the user's intended browsing context.

## Capabilities Firefox exposes beyond Ghostlight

Availability below means a credible Firefox mechanism exists. It does not mean Ghostlight should
add a public tool.

| Capability | Firefox mechanism | Support posture | Ghostlight opportunity or concern |
|---|---|---|---|
| Element screenshot | BiDi screenshot clip; Mozilla MCP `screenshot_by_uid` | Native/Composed | Strong model-token and user-delight opportunity if normalized across adapters. |
| Full-page screenshot | BiDi screenshot origin/clip behavior | Native/Experimental | Useful if output remains token-bounded and provenance-rich. Verify Firefox implementation. |
| Print to PDF | BiDi `browsingContext.print` | Native/Experimental | Good documentation workflow primitive; classify as read plus local write depending on delivery. |
| Back/forward history | BiDi history traversal; tabs APIs | Native | Could be additive `tab_control` actions if model demand justifies it. |
| Response bodies | BiDi network data collectors and `network.getData` | Experimental | High model value; high secret and payload risk. Needs minimization, bounds, and governance review. |
| Request interception and mocking | BiDi network intercept/continue/fail/provide-response | Native/Experimental | Powerful testing surface, but changes page behavior. Likely write/action and opt-in. |
| Extra headers and authentication | BiDi network header/auth commands | Native/Experimental | Useful for testing; risky in authenticated user context. Never silently persist. |
| Cache control and offline simulation | BiDi network cache behavior and emulation | Native/Experimental | Better suited to explicit testing mode than ordinary user automation. |
| Cookies | BiDi storage; Firefox cookies API | Native | Highly sensitive. A direct cookie tool is unnecessary for ordinary UI automation and should remain excluded by default. |
| Local/session storage | Script realms; extension content scripts | Composed | Same concern as cookies; useful for diagnosis but easy to overexpose. |
| Download control and events | BiDi download behavior/events; Firefox downloads API | Native/Experimental | Could provide governed download receipts without scraping UI. File-system authority must stay separate. |
| Browser user contexts | BiDi user contexts | Native/Experimental | Standards-level isolated contexts conflict with the current ordinary-profile premise unless explicitly user-created. |
| Firefox containers | Firefox contextual identities API | Native vendor-specific | Interesting user-context feature: route work into existing named containers without creating a cloud/headless isolation product. Requires explicit selection. |
| Private browsing | Firefox extension incognito access; BiDi contexts | Native vendor-specific | Privacy semantics and extension permissions make this opt-in; never auto-select. |
| Geolocation | BiDi emulation | Native/Experimental | Useful testing primitive; should be reversible and visibly disclosed. |
| Locale and timezone | BiDi emulation | Native/Experimental | Useful documentation/localization testing; avoid leaking across unrelated tabs or sessions. |
| Viewport and screen emulation | BiDi viewport/emulation | Native/Experimental | Distinguish this from resizing the user's real window. |
| Media and color preferences | BiDi emulation | Native/Experimental | Useful accessibility and visual QA surface. |
| User prompts and file dialogs | BiDi events and commands | Native | Could improve truthful waiting and receipts even without a new public tool. |
| Preload scripts | BiDi script preload | Native/Experimental | Powerful but persistent within selected contexts. Execute capability and explicit lifecycle required. |
| WebExtension install/uninstall | BiDi WebExtension module; Mozilla MCP | Native | Governance-sensitive and outside normal browsing tasks. Exclude from default surface. |
| Firefox preference read/write | Marionette privileged context; Mozilla MCP | Native vendor-specific | System-level authority. Boundary-excluded for ordinary Ghostlight operation. |
| Privileged Firefox UI scripting | Remote system access; Mozilla MCP chrome contexts | Native vendor-specific | Grants unrestricted Gecko and host/device access. Boundary-excluded by default. |
| Gecko profiler | Firefox profiler; Mozilla MCP | Native vendor-specific | Valuable diagnostics tool for Firefox development, not a general browser action. |
| Browser process logs and MOZ_LOG | Firefox process environment/output; Mozilla MCP | Native vendor-specific | Useful only when Ghostlight launches or supervises Firefox. Does not fit attach-only ordinary use without restart. |
| Firefox build/channel information | capabilities; Mozilla MCP | Native | Good diagnostic metadata for `doctor` and adapter explanations. |
| Firefox for Android | geckodriver plus ADB; Mozilla MCP | Native vendor-specific | Technically compelling, but remote device control is outside the current local desktop browser boundary. Watch, do not ship. |
| Accessibility inspection | BiDi locators plus DOM/script composition | Composed/Experimental | Strong model-token opportunity if it improves `read_page`; test fidelity before claiming an accessibility tree. |
| Clipboard | Firefox clipboard extension APIs | Native vendor-specific | Sensitive cross-application data. Needs explicit permissions and likely no default model-facing tool. |
| Bookmarks and history database | Firefox extension APIs | Native vendor-specific | User-data surface unrelated to page automation. Exclude unless a separate product decision is made. |
| Proxy, DNS, privacy, and browser settings | Firefox extension APIs | Native vendor-specific | Broad browser administration, not ordinary page automation. Boundary-excluded. |

### Strategic low-hanging fruit

The most attractive cross-browser additions are not the most privileged Firefox features. They
are semantic primitives that improve model payloads and remain useful on Chromium:

1. **Element screenshot.** Return a tightly cropped, ref-linked image rather than a full viewport.
2. **PDF export.** Turn a visible authenticated document into an explicit, governed artifact.
3. **Download receipts.** Report filename, media type, source URL, size, and final path without
   making the model inspect the downloads UI.
4. **Back/forward traversal.** Small addition, clear semantics, common recovery need.
5. **Capability-aware diagnostics.** Tell the model and user exactly why console, network, record,
   or privileged pages are unavailable in the current adapter mode.
6. **Firefox containers as explicit browser context.** This respects user-owned context better than
   creating hidden isolated browsers, but only if selection is visible and deliberate.

Network response bodies, interception, cookie access, preferences, and privileged UI scripting are
technically interesting but not low-risk free-surface wins. They expand authority and secret
exposure more than they improve ordinary automation delight.

## Browser-adapter architecture

### The boundary

The adapter boundary should sit below Ghostlight's semantic browser domain and above vendor
protocols:

```text
MCP tool call
    |
    v
tool registry + validation + RAWX governance
    |
    v
typed BrowserOperation
    |
    v
browser resolver -> one BrowserInstance
    |
    v
BrowserAdapter
    |-- Chromium: extension + CDP
    |-- Firefox extension-only: WebExtension APIs
    `-- Firefox hybrid: WebExtension APIs + Marionette/BiDi
```

The adapter stays policy-free. It reports mechanism facts and executes an already-authorized
operation. Capability classification, policy, audit, user holds, batching, and session ownership
remain service concerns.

### Do not translate raw tools

A tempting interface is:

```text
adapter.call(tool_name, arbitrary_json)
```

That preserves today's extension wire shape but makes the abstraction cosmetic. Tool semantics,
vendor quirks, normalization, and error handling would spread across adapters as stringly typed
conditionals.

The stronger seam is a typed internal vocabulary such as:

```text
BrowserOperation::Navigate
BrowserOperation::ReadPage
BrowserOperation::PointerAction
BrowserOperation::SetFormValue
BrowserOperation::EvaluateScript
BrowserOperation::CaptureScreenshot
BrowserOperation::ReadNetwork
BrowserOperation::PresentEffect
```

This is illustrative, not a proposed final Rust enum. The proof of concept should discover the
smallest useful operation set from real Chromium and Firefox implementations before an ADR freezes
it.

### Adapter descriptor

Each connected browser instance should provide a versioned descriptor containing at least:

| Field | Purpose |
|---|---|
| `browser_id` | Persistent instance/profile identity, distinct from vendor. |
| `vendor` | Chromium, Firefox, or a future vendor family. |
| `browser_name` and `browser_version` | Diagnostics and version-gated behavior. |
| `adapter_version` | Compatibility and support diagnostics. |
| `mode` | For example Chromium CDP, Firefox extension-only, or Firefox hybrid. |
| `features` | Versioned operation support with support state and constraints. |
| `presentation` | Whether narration, effects, controlled border, capture cues, and denials are available. |
| `transport_trust` | Extension-bound native messaging, local unauthenticated automation port, or another explicit posture. |
| `profile_label` | Optional user-facing label such as "Firefox - Personal"; never inferred from private profile contents. |

Capabilities should be negotiated once on connection and refreshed when the adapter's mode
changes. Dispatch can then fail locally and clearly instead of probing by error on every call.

### Results and provenance

Adapters should return normalized domain results, not vendor protocol packets. Every result and
audit record should be able to identify:

- browser ID, vendor, version, and adapter mode;
- Ghostlight composite tab ID and document identity;
- semantic operation and support state used;
- whether the result was native, composed, or degraded;
- important fidelity caveats;
- the underlying mechanism in diagnostic mode only.

The user-facing and model-facing result should normally remain compact. Full vendor traces belong
in bounded debug artifacts, not MCP payloads.

### One browser, several mechanisms

"Adapter" must mean one logical browser instance, not one socket. Firefox hybrid mode can need:

- native messaging for stable browser identity and presentation;
- WebExtension APIs for tabs, focus, and Firefox-only features;
- Marionette/WebDriver BiDi for trusted input and instrumentation.

The adapter owns that composition. The browser domain should not decide per call whether to use a
content script, BiDi, or Marionette.

### Pairing is the hardest Firefox integration problem

Ghostlight already trusts an extension-minted persistent `browserId` and assigns it a stable slot.
A separate Marionette/BiDi connection does not inherently prove which extension profile it belongs
to. With two Firefox profiles, "one extension plus one port" is no longer enough.

A proof of concept must evaluate pairing designs such as:

- explicit user pairing between a Firefox browser ID and a configured Marionette port;
- a Ghostlight-created Firefox launch shortcut/profile entry that binds a known port to one browser
  identity without creating a disposable browser;
- a browser-visible one-time pairing ceremony observed by both channels;
- a Firefox-supported instance capability that can be safely correlated without reading private
  profile data.

Do not infer pairing from process order, focus, title, or "the only Firefox we saw" once the product
claims multi-profile correctness. Do not enable unrestricted system access merely to make pairing
convenient.

## Dynamic tool explanations without schema drift

The user's intuition is correct: the same tool can have different operational caveats depending on
the connected browser. The wrong implementation would rewrite tool descriptions per connection.

Current Ghostlight facts matter:

- `ToolDescriptor.advertised_description` is static registry data;
- `tools/list` is rendered from that registry;
- `explain_text()` is deterministic and pinned by tests;
- the trained 13 tool schemas and descriptions are sacred.

The recommended split is:

1. **Stable semantic contract.** Tool names, trained descriptions, schemas, and core meaning do not
   change by browser.
2. **Dynamic environment guidance.** `initialize.instructions` includes a compact connected-browser
   summary: selected browser, mode, important limitations, and how to select another browser.
3. **Dynamic `explain` appendix.** Keep the current deterministic RAWX directory intact, then append
   an "Active browser adapters" section derived from descriptors. For example: "Firefox - Personal:
   page input native through WebDriver; console/network unavailable in connect-existing mode; GIF
   recording experimental."
4. **Structured diagnostics.** A browser status/list action returns exact support states for clients
   or models that need detail. This is better than inflating every tool description.
5. **Truthful errors.** If a selected browser cannot perform a tool, return a bounded unsupported
   response with the limitation and available choices. Never silently emulate an unsafe substitute.

For additive tools, Ghostlight may choose not to advertise a tool when no connected capability can
ever support it during that session. For the sacred surface, the safer product gate is stronger:
do not call a browser adapter "supported" until its essential sacred operations pass parity tests.
An unavailable sacred operation must still fail truthfully.

## Selecting among several browsers

### What Ghostlight already does

ADRs 0058 and 0061 already solve an important part of the problem:

- each extension profile mints a persistent browser ID;
- the service assigns a stable browser slot;
- composite tab IDs encode the owning slot;
- a call naming a composite tab always routes to that owner;
- a call without a tab currently routes to the most recently focused connected browser;
- browser extensions report window focus through browser APIs rather than OS window enumeration.

This design is vendor-portable. Firefox exposes window focus events too. "Window focus" here means
browser-window focus on Windows, macOS, or Linux, not a dependency on the Microsoft Windows OS.

### Why focus alone is not enough

Several cases defeat a focus-only rule:

- The user focuses Firefox to read something, then returns to the MCP client and asks a task that
  belongs to an existing Chrome workflow.
- Firefox is focused most recently but lacks recording, while Chrome supports it.
- Two browser profiles report focus events close together or Firefox emits duplicate focus events.
- A long-running `script` starts in Chrome while the user focuses Firefox midway through it.
- A tool has no `tabId` but should remain in the browser chosen earlier in this MCP session.

Focus is an excellent first-contact hint. It is not durable task identity.

### Proposed resolver

Resolve the browser once at the semantic-operation boundary:

1. **Tab owner wins.** If the call carries a composite tab ID, its encoded browser instance is the
   only target. If that instance lacks the capability, fail there. Never switch.
2. **Pinned operation group wins.** A `script`, `browser_batch`, recording, or multi-step semantic
   action pins its browser for its whole lifetime.
3. **Explicit MCP-session selection wins.** If the user or model selected "Firefox - Personal", use
   it until changed or disconnected.
4. **Filter by compatibility.** Consider only browser instances that truthfully support the
   requested operation at the required fidelity. This prevents choosing an extension-only Firefox
   for a dialog action it cannot perform.
5. **One compatible browser is safe.** Use it when no explicit context exists.
6. **Fresh focus can bootstrap affinity.** Among several compatible browsers, select the most
   recently focused browser when the focus signal is unambiguous, then establish MCP-session
   affinity.
7. **Otherwise disambiguate.** Return a short list with stable labels, vendor, focused state, and
   the relevant capability difference. Do not choose by hash order or slot number.

There is one deliberate exception to capability filtering: if an explicit tab or session browser
is already selected and lacks the operation, do not jump to another compatible browser. Report the
gap and let the caller explicitly select or open context in the other browser.

### How to express explicit selection

Repeating a browser selector on all 25 tools would waste tokens and weaken the simplicity of "the
same tool call lands the same way." Better options are:

- an additive browser-context tool with `list`, `use`, `status`, and `auto` actions;
- a client configuration default such as a stable browser ID or user label;
- a small optional selector only on tab-bootstrap calls, after which the composite tab ID carries
  ownership;
- a human control in the future Console that sets the current MCP session's browser affinity.

The dossier recommends the first option for explicit model control and a human control for user
control. The exact public shape requires an ADR because it changes routing behavior and the model
surface.

### Focus state needs richer diagnostics

`BrowserInfo` currently contains only `slot` and `focused`, while `BrowserSession` stores the sender,
generation, and one wire feature. A multi-vendor system needs the adapter descriptor above plus:

- last-focus timestamp or monotonic sequence;
- connection health and controller-channel health;
- current selection/affinity per MCP session;
- whether focus came from a cold-start check or a later event;
- vendor-specific duplicate-event coalescing.

These are service-domain facts. The extension still reports only mechanism events.

## Security and governance comparison

### Extension-bound Firefox

Firefox native messaging preserves the strongest part of Ghostlight's current posture: only the
installed extension identity named in the host manifest can launch the native host. The service can
continue using same-user OS IPC, its local governance engine, audit, session identity, user hold,
and kill behavior.

Extension permissions and Firefox-specific APIs still require a new trust review. Broad page
access, webRequest, downloads, cookies, contextual identities, privacy settings, and DevTools each
expand what a compromised extension could observe or change. The Firefox package should request
only what its negotiated stable surface actually uses.

### Marionette and Remote Agent

Mozilla states that Remote Agent:

- starts only through an explicit command-line flag;
- defaults to loopback;
- can control input, inject scripts, inspect network/console data, and extract cookies;
- has no authentication or message encryption;
- should not be exposed to untrusted hosts;
- can gain unrestricted Gecko and host/device access when system access is enabled.

Loopback is necessary but not equivalent to extension-bound admission. Another process running as
the same user can attempt to connect. This does not invalidate Firefox for Ghostlight: the project
already assumes that same-user malware can take the browser. It does mean governance claims must be
precise. Ghostlight can govern its own MCP clients and commands, but it cannot claim to govern every
local process that can reach an enabled Marionette/Remote Agent port.

Recommended mitigations for a hybrid experiment:

- bind loopback only;
- use a random high port where Firefox permits it and never expose a route off-host;
- enable the controller only for an explicit test session, then restart Firefox normally;
- do not enable system access;
- display hybrid-controller state visibly in the extension and `doctor`;
- detect and report controller loss or an unexpected second client;
- document the `navigator.webdriver` and site-compatibility effect;
- keep all Ghostlight policy and audit decisions before adapter dispatch.

### Governance advantage of adapters

A capability-negotiated adapter gives governance more truth, not less. Policy and audit can know:

- which browser and profile context received the action;
- whether the mechanism was native or degraded;
- whether arbitrary script, network interception, cookie access, or privileged context was even
  enabled;
- whether the browser changed during a workflow;
- which adapter limitation caused a denial or unsupported result.

That is preferable to pretending all browsers share one opaque transport.

## Recommended proof of concept

The proof should run on the existing Linux test host and remain unshipped.

### Track A: Firefox extension-only

Build the smallest Firefox package that can:

1. mint and persist a browser ID;
2. connect through Firefox native messaging;
3. report focus and enumerate tabs;
4. render Ghostlight's controlled border, narration, denial, scan, field, click, typing, JavaScript,
   and screenshot signatures;
5. create, select, navigate, reload, and close tabs;
6. read page text and a structured page snapshot;
7. capture a screenshot;
8. attempt click, type, form input, upload, dialogs, console, network, and recording, recording the
   exact fidelity boundary rather than filling gaps with claims.

### Track B: Firefox hybrid

Against an ordinary visible Firefox profile started with `--marionette`:

1. connect to the existing session without launching a second profile;
2. pair the controller with the correct extension browser ID;
3. implement trusted pointer, keyboard, wheel, drag, form, upload, dialog, script, screenshot, and
   page-read operations;
4. test console and network availability and compare it with Mozilla's documented connect-existing
   limitation;
5. test nested frames, cross-origin frames, navigation replacement, stale refs, prompts, downloads,
   restricted pages, and multiple windows;
6. measure page snapshot size, screenshot cost, latency, and recording feasibility;
7. record preference changes, `navigator.webdriver`, site compatibility, and restart cleanup;
8. connect two Firefox profiles and one Chromium profile to force correct pairing and routing.

### Shared acceptance gates

- The trained tool surface is unchanged.
- Every advertised operation has a measured support state.
- No operation silently switches browsers.
- A tab ID always routes to its owning adapter.
- `script`, batch, and recording pin one browser.
- Browser identity survives extension/worker reconnects.
- Focus bootstraps selection, then explicit/session affinity is stable.
- Unsupported and degraded results are compact and truthful.
- Visual feedback remains extension-owned and policy-free.
- Governance and audit precede vendor dispatch.
- No remote listener, telemetry, or hidden isolated profile is introduced.

### Stop conditions

Stop before a product ADR if any of these remain true:

- a normal authenticated Firefox session cannot be controlled without persistent disruptive
  automation changes;
- two profiles cannot be paired reliably;
- the core sacred operations require unbounded privileged system access;
- Firefox control can only be made reliable through a headless or disposable profile;
- the extension cannot provide Ghostlight's visible safety and delight language;
- hybrid mode cannot state a defensible local security boundary.

## Decisions a future ADR must make

This dossier does not decide:

1. whether Firefox becomes a supported product target;
2. whether extension-only degraded mode is independently useful or only a development step;
3. whether hybrid Firefox's launch and fingerprint costs are acceptable;
4. the final typed `BrowserOperation` vocabulary and adapter trait;
5. the browser capability handshake and versioning format;
6. the extension-to-controller pairing ceremony;
7. the explicit browser-selection surface and session-affinity lifecycle;
8. whether current focus fallback is amended to disambiguate rather than deterministically choose;
9. which expanded Firefox capabilities, if any, become public Ghostlight tools;
10. how adapter-specific support appears in `initialize`, `explain`, `doctor`, Console, and audit;
11. the minimum parity bar required before the word "Firefox support" is used publicly.

## Final recommendation

Proceed to a two-track Firefox proof of concept after current release work settles.

Do not begin by generalizing every Chromium call. First prove five load-bearing seams in real
Firefox:

1. stable browser identity and correct multi-profile pairing;
2. typed semantic operation mapping;
3. trusted input in the user's real session;
4. extension-owned presentation alongside BiDi-owned control;
5. capability-aware, affinity-preserving browser selection.

If those hold, a browser-adapter architecture is strategically worthwhile. It would make Firefox
possible, make Chromium assumptions visible, and give Ghostlight a truthful way to add future
browser-specific strengths without fragmenting the model-facing language. If they do not hold, the
same proof still yields value: it marks exactly where Chromium is a product requirement rather than
an accidental implementation detail.
