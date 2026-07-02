# Reference Analysis: open-claude-in-chrome

**Date:** 2026-07-01 · **Phase:** 0 (Reference Study) · **Subject:**
[noemica-io/open-claude-in-chrome](https://github.com/noemica-io/open-claude-in-chrome)
(clean-room reimplementation of Anthropic's Claude-in-Chrome; MIT; ~2,278 LoC across 6 files)

This documents what the reference does, so our Rust rewrite can preserve the sacred tool surface
and learn from the reference's hard-won solutions: **as a concern-surface, not a paradigm to
copy** (see `docs/research/NORTH-STAR.md`). The cloned source lives in
`reference/open-claude-in-chrome/` (gitignored). §8 lists what this study **validates** and
**challenges** in `docs/SPEC.md`.

Files studied: `host/mcp-server.js` (699), `host/native-host.js` (141),
`extension/background.js` (938), `extension/content.js` (463), `extension/manifest.json` (37),
`install.sh`, `test-prompt.md`, `README.md`.

---

## Section 1: Tool Schemas (18 tools)

Defined in `host/mcp-server.js` via `server.tool(name, description, zodShape, handler)`. The MCP
SDK converts the zod shape to the JSON Schema advertised in `tools/list`: **that emitted JSON
Schema is the sacred surface**, so Phase 1 must capture the reference's *actual* `tools/list`
output as the golden fixture (not just transcribe zod). Names and descriptions below are verbatim.

> **⚠ Naming correction for our SPEC:** the tab tools are **`tabs_context_mcp`** and
> **`tabs_create_mcp`** (with the `_mcp` suffix). Our SPEC §3 calls them `tabs_context` /
> `tabs_create`, **wrong**; must be corrected to preserve the trained surface.

### Classification (per SPEC §3: Observe / Mutate / Manage / Excluded)

| # | Tool (exact) | Params | Class | Notes |
|---|---|---|---|---|
| 1 | `tabs_context_mcp` | `createIfEmpty?` (bool) | **Observe** | Session metadata; must be called first. |
| 2 | `tabs_create_mcp` | *(none)* | **Mutate** | Opens a tab in the MCP group. |
| 3 | `navigate` | `url` (str), `tabId` (num) | **Observe** | Primary domain-enforcement point (overlay). `url` also accepts `"back"`/`"forward"`. |
| 4 | `computer` | `action` (enum, **13 values**), `tabId`, `coordinate?`, `duration?`, `modifiers?`, `ref?`, `region?`, `repeat?`, `scroll_direction?`, `scroll_amount?`, `start_coordinate?`, `text?` | **split** | Per-action tier, see below. |
| 5 | `find` | `query` (str), `tabId` | **Observe** | Read-only DOM query (content script). |
| 6 | `form_input` | `ref` (str), `value` (str\|bool\|num), `tabId` | **Mutate** | Shadow-DOM traversal (content script). |
| 7 | `get_page_text` | `tabId` | **Observe** | Article/main text (content script). |
| 8 | `gif_creator` | `action` (enum), `tabId`, `download?`, `filename?`, `options?` | **Excluded** | Stub: returns "not yet implemented". |
| 9 | `javascript_tool` | `action` (literal `"javascript_exec"`), `text` (str), `tabId` | **Mutate** | `Runtime.evaluate`. Always requires explicit grant. |
| 10 | `read_console_messages` | `tabId`, `pattern?`, `limit?`, `onlyErrors?`, `clear?` | **Observe** | Buffered event replay. |
| 11 | `read_network_requests` | `tabId`, `urlPattern?`, `limit?`, `clear?` | **Observe** | Buffered event replay. |
| 12 | `read_page` | `tabId`, `filter?` (`interactive`\|`all`), `depth?`, `ref_id?`, `max_chars?` | **Observe** | A11y tree w/ refs (content script). |
| 13 | `resize_window` | `width`, `height`, `tabId` | **Manage** | No security implication. |
| 14 | `shortcuts_list` | `tabId` | **Excluded** | Stub. |
| 15 | `shortcuts_execute` | `tabId`, `shortcutId?`, `command?` | **Excluded** | Stub. |
| 16 | `switch_browser` | *(none)* | **Excluded** | Stub. |
| 17 | `update_plan` | `domains` (str[]), `approach` (str[]) | **Manage** | Auto-approved pass-through. |
| 18 | `upload_image` | `imageId`, `tabId`, `ref?`, `coordinate?`, `filename?` | **Excluded** | **Non-functional in the reference** (returns "use file input directly"), not just "niche". |

### The `computer` action enum (13 actions, verbatim)

`left_click, right_click, double_click, triple_click, type, screenshot, wait, scroll, key,
left_click_drag, zoom, scroll_to, hover`

Per-action tier (extends SPEC §3.3, which is **incomplete**: it omits `triple_click`,
`left_click_drag`, `zoom`, `scroll_to`, and names drag as `drag`):

- **Observe:** `screenshot`, `wait`, `zoom` (zoom is just a full screenshot + region label).
- **Mutate:** `left_click`, `right_click`, `double_click`, `triple_click`, `type`, `key`,
  `scroll`, `hover`, `left_click_drag`, `scroll_to` (all dispatch input or move the viewport).

### Verbatim descriptions (the 13 tools we keep in v1.0)

- **`tabs_context_mcp`**: "Get context information about the current MCP tab group. Returns all
  tab IDs inside the group if it exists. CRITICAL: You must get the context at least once before
  using other browser automation tools so you know what tabs exist. Each new conversation should
  create its own new tab (using tabs_create_mcp) rather than reusing existing tabs, unless the
  user explicitly asks to use an existing tab."
  - `createIfEmpty` (bool, optional): "Creates a new MCP tab group if none exists, creates a new
    Window with a new tab group containing an empty tab … If a MCP tab group already exists, this
    parameter has no effect."
- **`tabs_create_mcp`**: "Creates a new empty tab in the MCP tab group. CRITICAL: You must get the
  context using tabs_context_mcp at least once before using other browser automation tools so you
  know what tabs exist." *(no params)*
- **`navigate`**: "Navigate to a URL, or go forward/back in browser history. If you don't have a
  valid tab ID, use tabs_context_mcp first to get available tabs."
  - `url` (str): "The URL to navigate to. Can be provided with or without protocol (defaults to
    https://). Use \"forward\" to go forward in history or \"back\" to go back in history."
  - `tabId` (num): "Tab ID to navigate. Must be a tab in the current group. …"
- **`computer`**: "Use a mouse and keyboard to interact with a web browser, and take screenshots.
  … * Whenever you intend to click on an element like an icon, you should consult a screenshot to
  determine the coordinates … * If you tried clicking … try adjusting your click location so that
  the tip of the cursor visually falls on the element … * Make sure to click any buttons, links,
  icons, etc with the cursor tip in the center of the element. …" (full per-action description in
  `mcp-server.js:498-520`, **capture verbatim in Phase 1**).
- **`find`**: "Find elements on the page using natural language. Can search for elements by their
  purpose (e.g., \"search bar\", \"login button\") or by text content … Returns up to 20 matching
  elements with references … If more than 20 matches exist, you'll be notified to use a more
  specific query. …"
- **`form_input`**: "Set values in form elements using element reference ID from the read_page
  tool. …": `ref`, `value` (`union(string, boolean, number)`), `tabId`.
- **`get_page_text`**: "Extract raw text content from the page, prioritizing article content.
  Ideal for reading articles, blog posts, or other text-heavy pages. Returns plain text without
  HTML formatting. …"
- **`javascript_tool`**: "Execute JavaScript code in the context of the current page. … Returns
  the result of the last expression or any thrown errors. …": `action` = literal
  `"javascript_exec"`; `text` = "The JavaScript code to execute. … Do NOT use 'return' statements
  - just write the expression … (e.g., 'window.myData.value' not 'return window.myData.value')."
- **`read_console_messages`**: "Read browser console messages … Returns console messages from the
  current domain only. … IMPORTANT: Always provide a pattern to filter messages …"
- **`read_network_requests`**: "Read HTTP network requests (XHR, Fetch, documents, images, etc.)
  … Requests are automatically cleared when the page navigates to a different domain. …"
- **`read_page`**: "Get an accessibility tree representation of elements on the page. By default
  returns all elements including non-visible ones. Output is limited to 50000 characters by
  default. If the output exceeds this limit, you will receive an error … Optionally filter for
  only interactive elements. …"
- **`resize_window`**: "Resize the current browser window to specified dimensions. …"
- **`update_plan`**: "Present a plan to the user for approval before taking actions. The user will
  see the domains you intend to visit and your approach. Once approved, you can proceed with
  actions on the approved domains without additional permission prompts.": `domains` (str[]),
  `approach` (str[]).

**Robustness pattern: pre-validation arg coercion** (`mcp-server.js:448-469`): before zod
validation, string args are coerced: `tabId` string→`Number`, and `coordinate`,
`start_coordinate`, `region` string→`JSON.parse`. MCP clients sometimes stringify structured
args; **our Rust deserialization must tolerate the same** (accept both `5` and `"5"` for tabId,
both `[x,y]` and `"[x,y]"` for coordinates).

---

## Section 2: CDP Commands (per tool)

`chrome.debugger` at protocol **1.3**. `ensureAttached(tabId)` attaches once then calls
`Emulation.setDeviceMetricsOverride {width, height, deviceScaleFactor: 1, mobile: false}`.
`ensureDomain(tabId, domain)` lazily enables `Console`/`Runtime`/`Network`.

| Tool / action | CDP method(s) | Notes |
|---|---|---|
| attach | `Emulation.setDeviceMetricsOverride` (dSF=1) | coordinate normalization (§5.2) |
| `computer` click/hover/drag | `Input.dispatchMouseEvent` (`mouseMoved`→`mousePressed`→`mouseReleased`; 50ms gaps; drag in 10 steps) | button/clickCount/modifiers |
| `computer` scroll | `Input.dispatchMouseEvent` type `mouseWheel` (delta = ticks×100) | + screenshot |
| `computer` type | `Input.insertText` **char-by-char** (10ms each) | not `dispatchKeyEvent` |
| `computer` key | `Input.dispatchKeyEvent` keyDown/keyUp | key-map + modifier bitmask (see §Bugs) |
| `computer` screenshot/scroll/zoom | `Page.captureScreenshot` (see §5) | |
| `computer` scroll_to | content msg `scrollToRef` **or** `Runtime.evaluate(window.scrollTo)` | |
| `javascript_tool` | `Runtime.evaluate {expression, returnByValue:true, awaitPromise:true}` | reads `exceptionDetails` |
| `read_console_messages` | `Console.enable`+`Runtime.enable`; buffer `Console.messageAdded` + `Runtime.consoleAPICalled` | |
| `read_network_requests` | `Network.enable`; buffer `Network.responseReceived` + `Network.requestWillBeSent` | |
| `read_page` / `get_page_text` / `find` / `form_input` | **content script** (`chrome.tabs.sendMessage`), NOT CDP | see §6 |
| `navigate` | `chrome.tabs.update`/`goBack`/`goForward` + `chrome.tabs.onUpdated` wait (10s cap) | Chrome API, not CDP |
| `tabs_*` | `chrome.tabs`/`chrome.tabGroups`/`chrome.windows` | Chrome API |
| `resize_window` | `chrome.windows.update {width, height}` | Chrome API |

**Design consequence:** the reference splits work: **CDP** for input/screenshot/JS/console/
network, **Chrome extension APIs** for tabs/windows/navigation, and a **content script** for all
DOM reads + form input. It is *not* a pure CDP executor.

---

## Section 3: Native Messaging Protocol + IPC (the 5-node reality)

```
Claude Code ──stdio MCP──> mcp-server.js ──TCP(:18765)──> native-host.js ──native msg──> Extension ──CDP──> Browser
```

**Two distinct framings:**
1. **Native messaging** (`native-host.js` ↔ extension, over the host's stdin/stdout): Chrome's
   4-byte **little-endian u32 length prefix + UTF-8 JSON** (`readUInt32LE`/`writeUInt32LE`).
2. **TCP relay** (`native-host.js` ↔ `mcp-server.js`, localhost:18765): **newline-delimited
   JSON**.

**Message types:** `tool_request {id, tool, args}` → `tool_response {id, result}` /
`tool_error {id, error}`; `heartbeat` (ignored); and the multi-session control messages
`client_hello` / `client_ack {clientId}` / `error`.

**Multi-session (primary/client), a real feature (README "Multiple Sessions"):** the first
`mcp-server.js` binds :18765 = **primary** (accepts the native host + client MCP servers);
later sessions fail to bind, become **clients**, and forward tool calls to the primary over TCP
(prefixed IDs `c<clientId>_<id>` for response routing). Connection classification: a new TCP peer
that sends `client_hello` within 500ms is a client; otherwise it's the native host.

**Robustness details worth porting:** 60s per-tool timeout (`mcp-server.js`); native-host
reconnect loop (500ms × 60 = 30s, then exit to avoid zombies); on native-host disconnect with
pending requests, the primary waits 5s and **re-sends un-acked requests** (`resent` flag) if the
host reconnects. Stale-pidfile cleanup that **does not kill live servers** (uses `process.kill(pid,
0)` liveness check).

> **⚠ Major architectural finding, see §8.1.** Chrome *spawns its own* native-messaging host
> process on `connectNative()`. So even our "single binary" is **two running instances** (MCP-
> server role, launched by Claude Code; native-host role, launched by Chrome) that must talk over
> a local IPC. We can drop Node and the TCP relay's *language*, but not the *two-process reality*
> or the need for an IPC + primary/client-style arbitration.

---

## Section 4: Extension Lifecycle (MV3 resilience)

- **Keepalive alarm** every **0.4 min (24s)**; on fire, reconnect native host if `nativePort` is
  null. `chrome.runtime.connectNative(NATIVE_HOST_NAME)` where `NATIVE_HOST_NAME =
  "com.anthropic.open_claude_in_chrome"`.
- **`onDisconnect` → reconnect after 2s**; `connectNativeHost()` guards against double-connect.
- **`self.addEventListener("unhandledrejection", e => e.preventDefault())`**: prevents an
  unhandled promise rejection from killing the service worker.
- **State recovery on SW restart:**
  - `recoverTabGroupState()` at startup: `chrome.tabGroups.query({title:"MCP"})` → rebuild
    `tabGroupId` + `tabGroupTabs`.
  - `isInGroup(tabId)` **always queries live `chrome.tabs.get`** (never trusts the in-memory
    `tabGroupTabs`), and re-recovers `tabGroupId` if lost.
  - `attachedTabs` (debugger attachments) is in-memory only, re-attached on demand via
    `ensureAttached` after a restart.
- **Tab group** titled `"MCP"`, color blue, created in its own window (`chrome.windows.create`).
- **Cleanup:** `chrome.tabs.onRemoved` detaches debugger + clears buffers; `chrome.debugger.
  onDetach` (user dismissing the debug bar) clears `attachedTabs`.

**This validates SPEC §2.4** (keepalive alarm, live-state tab-group recovery, debugger re-attach).

---

## Section 5: Screenshot Pipeline

`background.js takeScreenshot(tabId)`:
1. `ensureAttached` (which set `deviceScaleFactor: 1`, so capture is in CSS-pixel space matching
   `Input.dispatchMouseEvent` coordinates, no scaling math).
2. `Page.captureScreenshot {format: "jpeg", quality: 55, optimizeForSpeed: true,
   captureBeyondViewport: false}`.
3. If base64 length > **500000** (~375KB binary), recapture at **quality: 30**.
4. Store in `screenshotStore` (Map, keep last **10**), keyed `screenshot_<Date.now()>`.

**Per-action screenshot policy (matches our SPEC §6.3 / decision):** `screenshot`, `scroll`, and
`zoom` return `{type:"image", mimeType:"image/jpeg"}`; **all other actions return a text
confirmation** ("Clicked at (x, y)", "Typed …", etc.). So the reference **already does the
token-efficient thing**, see §8.3.

**This validates SPEC §6.1/§6.2** (JPEG 55→30 fallback at 512KB, `deviceScaleFactor:1`
normalization) essentially exactly.

---

## Section 6: Shadow DOM Handling (`content.js`)

All DOM reads run in a content script injected into the top frame (`all_frames: false`), guarded
by `window.__unblockedChromeLoaded`, exposing `window.__unblockedChrome` for the executeScript
fallback.

- **Element refs:** `getOrAssignRef(el)` → `ref_<n>`; stored as **`WeakRef`** in `elementMap` with
  a `WeakMap` reverse index, so refs persist across calls but don't leak memory. `resolveRef`
  deref's and prunes dead entries. Refs are **per-page/per-content-script-instance** (lost on
  navigation).
- **Accessibility tree** (`generateAccessibilityTree`): a **custom DOM walk** (not CDP's
  `Accessibility` domain) producing an indented text format `role "name" [ref] href=… value=…
  type=… expanded=… options=[…]`. Recurses **`el.shadowRoot.children`** as well as `el.children`.
  Custom `getRole` (tag→ARIA role), `getAccessibleName` (aria-label → labelledby → placeholder →
  title → alt → `<label>` → text), `isInteractive`, `isVisible`. `filter`/`depth`/`max_chars`
  (50000)/`ref_id` honored; truncation appends "... (truncated)".
- **`find`** (`findElements`): `collectAll(root)` recurses through **shadow roots**; substring
  match on `role name text placeholder ariaLabel title type tag`; returns up to 20
  `{ref, role, name, coordinates:[centerX,centerY]}`.
- **`form_input`** (`setFormValue`), the shadow-DOM fix:
  - `findInputInside(el)`: if the element is `input/textarea/select`, use it; else look in
    `el.shadowRoot`, else recurse children's shadow roots for the first `input,textarea,select`.
    **This is the "Reddit search-bar" web-component fix**: the ref points at a custom element
    whose real input is in shadow DOM.
  - Sets value via the **native prototype setter**
    (`Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,"value").set`) so React/Vue
    controlled inputs register the change; handles `select` (match option by value/text),
    `checkbox`/`radio` (click to toggle), `contentEditable`.
  - Dispatches `input` + `change` events with **`{bubbles:true, composed:true}`** so they cross
    the shadow boundary.

> **⚠ Thin-extension tension, see §8.2.** This is substantial extension-side logic. Our SPEC §2.4
> says the extension is "a thin, dumb CDP executor." Decision needed: replicate the content-script
> approach, or move reads to CDP (`Accessibility.getFullAXTree`, `DOM.*`) to keep the extension
> thin, noting the *output format* of `read_page` is likely part of the trained behavior.

---

## Section 7: Known Issues / Bugs (from the code)

The kickoff references "6 bugs from the build story"; that narrative lives in an external blog
(noemica.io/blog/reverse-engineered-claude-in-chrome), not the repo. Bugs observable **in the
code** (more authoritative for us):

1. **`zoom` mislabels + doesn't crop** (`background.js:629-642`): returns `mimeType:"image/png"`
   while the data is JPEG; comment says "client can crop": it returns the *full* screenshot, so
   `region` is decorative. (Our `zoom`, if kept, should either crop server-side or be honest.)
2. **`read_network_requests` method is unreliable** (`background.js:225`): for
   `Network.responseReceived` it sets `method: params.response.requestHeaders ? "?" : "GET"`.
   The real method is only on `requestWillBeSent`. Join the two events by `requestId`.
3. **Key handling is fragile** (`background.js:539-545`): `const resolvedKey = key.length === 1 ?
   key : key` is a no-op ternary; `windowsVirtualKeyCode: resolvedKey.charCodeAt(0)` is wrong for
   named keys (Enter/Tab/etc.). Use a proper key→VK map.
3b. **`MAX_SCREENSHOT_WIDTH/HEIGHT` (1280×800) are declared but never applied** in
   `takeScreenshot` (`background.js:314-315`): dead code; capture size is driven by the window
   dims in `setDeviceMetricsOverride`.
4. **`upload_image` is non-functional** (returns guidance text; never sets files). Real impl needs
   `DOM.setFileInputFiles` + a temp file. (Excluded in our SPEC, correctly.)
5. **`gif_creator` / `shortcuts_list` / `shortcuts_execute` / `switch_browser` are stubs** (return
   "not supported" text). Confirms SPEC §3.2 exclusions.
6. **Content-script single-frame** (`manifest.json all_frames:false`): reads/forms only work in
   the **top frame**: cross-origin iframes are invisible to `read_page`/`find`/`form_input`.
   (Relevant to our Fork 7a/7b per-frame handling.)

Design-level gaps we inherit unless addressed: navigate waits a flat 10s (no committed-URL
signal); `find` is naive substring matching; refs die silently on navigation.

---

## Section 8: What This Validates / Challenges in `docs/SPEC.md`

### 8.1 CHALLENGE (major): the "single process" model is not literally achievable (SPEC §2.2)
Chrome launches its own native-messaging host process. There will always be **two instances of
our binary**: the MCP-server role (spawned by Claude Code, stdio) and the native-host role
(spawned by Chrome, native-messaging stdio), bridged by a local IPC. SPEC §2.1's "3 processes, 2
protocol boundaries" and §2.2's "single process at the center of the star" **undercount**.
*Real simplification vs. the reference:* one Rust executable (dual-role by launch context), no
Node, and we can replace localhost-TCP with a **named pipe (Windows) / Unix domain socket**, but
the two-process reality, an IPC boundary, and **primary/client arbitration** remain. **Action:**
correct SPEC §2; decide the IPC transport and the multi-session policy (see 8.4) in Phase 1.

### 8.2 CHALLENGE: "thin extension" vs. the reference's content-script DOM engine (SPEC §2.4)
The reference puts a11y-tree generation, ref mapping, `find`, text extraction, and shadow-DOM form
input in a **content script** (~460 LoC). SPEC §2.4 says the extension is "a thin, dumb CDP
executor." **Decision (Phase 1):** (a) keep a content script for DOM reads (fast, matches trained
`read_page` output format, but not "thin"), or (b) move reads to CDP (`Accessibility.getFullAXTree`
+ `DOM.*`) to honor "thin", at the risk of a *different* `read_page` output the model wasn't
trained on. Output-format fidelity, not just schema fidelity, is at stake.

### 8.3 VALIDATES (and corrects CLAUDE.md): screenshot behavior (SPEC §6.3)
The reference **already** returns screenshots only on `screenshot`/`scroll`/`zoom` and text
elsewhere, and uses JPEG 55→30 at 512KB with `deviceScaleFactor:1`. This *matches* our SPEC §6.
**CLAUDE.md's "Screenshot Behavior" note is outdated**: it claims the reference "returns a
screenshot after every computer action," which this version does not. We **align with** the
reference here, not diverge.

### 8.4 CHALLENGE: multi-session is a real reference feature dropped by SPEC §10
The reference lets multiple Claude Code sessions share one browser (primary/client over TCP). SPEC
§10 ("one binary, one identity, one browser profile") implicitly drops this, but doesn't address
**one user running two Claude Code sessions**: both would spawn an MCP-server-role instance
contending for the same extension/IPC. **Decision:** either implement primary/client arbitration
over our IPC (inherit the reference's robustness), or explicitly fail the second session with a
clear message. Recommend at least graceful arbitration for v1.0 UX.

### 8.5 CORRECTION: tool names + `computer` action enum (SPEC §3)
`tabs_context` → **`tabs_context_mcp`**, `tabs_create` → **`tabs_create_mcp`**. The `computer`
enum has **13** actions; SPEC §3.3's classification omits `triple_click`, `left_click_drag`
(SPEC says `drag`), `zoom`, `scroll_to`. Correct §3.1/§3.3.

### 8.6 VALIDATES: native-messaging framing, MV3 resilience, coordinate normalization,
compression (SPEC §2.2/§2.4/§6). The 4-byte-LE protocol, keepalive alarm, live-state recovery,
`deviceScaleFactor:1`, and JPEG fallback are all confirmed and directly portable (as concerns, not
code).

### 8.7 INFORMS Fork 4 (installer) & Fork 7 (origin): `install.sh` takes extension IDs and
writes `allowed_origins`, and **explicitly punts on Windows** ("manually create the registry
entries"): exactly the gap our self-registering installer closes. The content script is
**top-frame only**, reinforcing that our per-frame committed-origin work (Fork 7a/7b) is net-new.

### 8.8 PORT: robustness patterns worth adopting as v1.0 requirements
Arg coercion (string→num/JSON pre-validation); 60s tool timeout; native-host reconnect w/ zombie
guard; re-send un-acked requests on host reconnect; liveness-checked pidfiles;
`unhandledrejection` suppression in the service worker.

---

## Appendix: Deltas from our target architecture

| Reference | Ours (v1.0 target) |
|---|---|
| Node.js `mcp-server.js` + `native-host.js` (2 processes) | One Rust binary, dual-role by launch context |
| TCP localhost:18765, newline-JSON | Named pipe / UDS (decide Phase 1) |
| `@modelcontextprotocol/sdk` (zod) | Hand-rolled MCP JSON-RPC (per CLAUDE.md, no SDK crate) |
| Content script for DOM reads | TBD (§8.2) |
| 18 tools incl. 5 stubs | 13 tools (stubs excluded), all-open, no governance in v1.0 |
| Host name `com.anthropic.open_claude_in_chrome` | Our own namespaced host name (decide Phase 1) |
| No Windows installer | Self-registering installer, Windows-first (Fork 4) |
