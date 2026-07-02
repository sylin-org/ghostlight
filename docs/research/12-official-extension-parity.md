# Official-Extension Parity + Technique Harvest

Status: in progress (harvest workflow running at last checkpoint). This doc is the durable record
of the parity-verification thread and the plan to re-baseline our tool surface against the
**official** Claude-in-Chrome extension rather than the community reference.

## Why

The sacred contract (CLAUDE.md) is that our tool surface is **byte-identical to what the official
Claude in Chrome extension advertises**, because the model was trained against those schemas.
Until now our schemas came from the *community reference* (`reference/open-claude-in-chrome`, a
Node.js re-implementation). Verification proved the reference is a **lossy proxy** that carries its
own bugs, so "we match the reference" is weaker than the real goal. The official extension is the
ground truth.

## Parity findings vs the COMMUNITY reference (verify-vs-reference workflow)

Exercised all 13 tools live against Chrome. Three behavioral symptoms observed, and side-by-side
code reading showed **all three are inherited verbatim from the reference, NOT rewrite regressions**:

- **A) `read_network_requests` returns empty on first call**: both sides enable the CDP `Network`
  domain *lazily* (only inside the handler). `Network.enable` is not retroactive, so the page-load +
  pre-read fetches are never captured. Our wiring is actually *better* than the reference (joins
  `requestWillBeSent`+`responseReceived` by `requestId` vs the reference's method-guessing).
- **B) `read_console_messages` duplicates**: both listen to `Runtime.consoleAPICalled` AND
  `Console.messageAdded` with no dedup. Inherited double-count. **Fixed** (single Runtime source).
- **C) `find` matches only literal text**: identical whole-string substring algorithm to the
  reference over `role name text placeholder ariaLabel title type tag`. "Submit button" is not a
  literal substring, so it misses; "Example Domain" hits. The reference's own `find` description
  over-promises "search by purpose"; our sacred schema preserves that gap verbatim.

Real gaps the parity sweep found (relative to the reference), pending the official baseline:
- **`read_page` omits node attributes the reference emits**: `img src`, `aria-expanded/checked/
  selected`, `<select>` options. Medium: loses state signals the model reads. (`extension/content.js` ~121-130)
- **`form_input` checkbox/radio truthiness**: ours accepts `1`/`"1"`/nonzero as check; reference
  treats them false. This is a **deliberate earlier fix** (commit 0deef1c). Keep ours.
- Low-severity text/format/timing diffs (navigate `## Pages` list, `get_page_text` `Source:` line,
  tabs shape, hover settle delay, scroll coordinate validation, zoom mimeType): mostly deliberate
  lean choices; decide per-tool against the OFFICIAL, not the reference.

## The official extension (ground truth)

- Name "Claude", description "Claude in Chrome (Beta)", **version 1.0.78**, id
  `fcoeoabgfenejglbffodgkkbkcdhcgfn`.
- Installed at:
  `C:\Users\onose\AppData\Local\Google\Chrome\User Data\Default\Extensions\fcoeoabgfenejglbffodgkkbkcdhcgfn\1.0.78_0\`
- Architecture matches ours: MV3, `debugger` (CDP), `tabGroups`, `nativeMessaging`; content scripts
  `accessibility-tree.js` (all_urls) + `agent-visual-indicator.js`; service worker bridges to
  claude.ai / api.anthropic.com / `wss://bridge.claudeusercontent.com`.
- Key files (bundled/minified, but plain JS):
  - `assets/mcpPermissions-E9qdF7bb.js` (693 KB): **the MCP tool DEFINITIONS/schemas + the CDP
    execution logic**. The core harvest target (28,715 lines beautified).
  - `assets/accessibility-tree.js-CCweLwU2.js`: the `read_page`/`find`/`get_page_text` engine
    (220 lines beautified).
  - `assets/service-worker.ts-CRgYaSdM.js`: bootstrap / native-messaging bridge (2,380 lines).

### Re-extracting the official files for study (they live in the session scratchpad, ephemeral)

```
SRC=".../Extensions/fcoeoabgfenejglbffodgkkbkcdhcgfn/1.0.78_0/assets"
OUT="<scratchpad>/official-ext"; mkdir -p "$OUT"
cp "$SRC/mcpPermissions-E9qdF7bb.js"           "$OUT/mcpPermissions.min.js"
cp "$SRC/accessibility-tree.js-CCweLwU2.js"    "$OUT/accessibility-tree.min.js"
cp "$SRC/service-worker.ts-CRgYaSdM.js"        "$OUT/service-worker.min.js"
cp "$SRC/agent-visual-indicator.js-CW8zgsee.js" "$OUT/agent-visual-indicator.min.js"  # user-facing UI overlay
npx --yes js-beautify "$OUT/mcpPermissions.min.js" > "$OUT/mcpPermissions.pretty.js"   # etc.
```

## Discipline (hard boundary)

We harvest the observable **interface** (tool names/params/enums/description strings) and the
**techniques** (CDP command sequences, algorithms) and **reimplement leanly**. We do **NOT** copy
official code into our repo (it is Anthropic proprietary; our repo is intended open-source). The
beautified official files stay in the throwaway scratchpad, never tracked. Interface + intent, not
code. Consistent with the project's "not a port" principle.

## Harvest results (official v1.0.78): the apply plan

Source: the `harvest-official-extension` workflow (4 read-only study agents, high confidence). All
13 of our tools have a 1:1 official counterpart: no tool missing/extra. The model-facing schema is
the `toAnthropicSchema()` return in `mcpPermissions.pretty.js` (NOT the internal `parameters` object,
which has agent-internal placeholders). Line refs below are into the beautified official files
(re-extract per the recipe above; they are ephemeral).

### A. Schema corrections: `src/mcp/schemas/tools.json` (sacred surface; re-baseline the golden fixture in `tests/tool_schema_fidelity.rs`)

1. **[HIGH] navigate: add `force` boolean**: "If the page shows a 'Leave site?' dialog because of
   unsaved changes, discard those changes and navigate anyway. Defaults to false..." (official 22718).
2. **[HIGH] get_page_text: add `max_chars` (number, default 50000)** + description ends "Output is
   limited to 50000 characters by default. If the output exceeds this limit, you will receive an
   error suggesting alternatives." (official 22217-22232).
3. **[HIGH] computer.duration.maximum: 30 -> 10** + text "Maximum 10 seconds." (official const `se`=10).
4. **[MED] javascript_tool.action: remove the `const`**: official is `{type:"string",
   description:"Must be set to 'javascript_exec'"}` with no const (official 21718).
5. **[MED] javascript_tool.text: adopt REPL wording**: "Evaluated in the page context with REPL
   semantics: top-level `await` works, and the result of the last expression is returned
   automatically -- write the expression (e.g. `window.myData.value`, or
   `await fetch(url).then(r=>r.json())`) rather than `return ...`" (official 21724). NOTE: adopting
   this implies our js engine should actually support top-level await.
6. **[MED] tabs_create_mcp: description is exactly "Creates a new empty tab in the MCP tab group."**:
   drop our extra "CRITICAL: ..." sentence (official 27922).
7. **[MED, decide] Description prose uses the BARE names `tabs_context`/`tabs_create`** everywhere
   (tool NAMES stay `_mcp`-suffixed). We (via the community reference) rewrote all prose to `_mcp`.
   This is a trained-token divergence across ALL 13 descriptions: match the official (bare in prose).
8. **[LOW] computer.action enum order**: `[left_click, right_click, type, screenshot, wait, scroll,
   key, left_click_drag, double_click, triple_click, zoom, scroll_to, hover]` (official 21360).
9. **[LOW] read_page description**: remove our inserted "by default" (official 23071).
   KEEP-OURS: our `computer.description` correctly matches the advertised form (omits the
   `{self.display_width_px}` resolution line); form_input/read_console/read_network/resize_window/
   update_plan params are field-identical.

### B. Extension behavior/technique adoptions: `extension/content.js`, `extension/service-worker.js`

read_page / accessibility engine (content.js):
- **[HIGH] [DONE]** **Emit `<select>` option children** with `(selected)` + `value="..."`: "single most
  load-bearing content gap"; without it the model is blind to dropdown choices (official a11y 157-162).
- **[HIGH, SECURITY] [DONE]** **Redact sensitive values**: gate on `type=password`/`hidden` + sensitive
  autocomplete (cc-number, cc-csc, one-time-code, new/current-password...) -> emit "[value redacted]",
  and suppress select options when redacted. OURS previously emitted raw `input.value` unconditionally,
  **leaking passwords/OTP/CC into the a11y tree** (official 37-43,89,92 vs old content.js:126). Fixed via a
  lean `sensitive()` gate (also ancestor-aware: options of a sensitive `<select>` inherit sensitivity).
  Verified with a jsdom harness (passwords/OTP/hidden/CC redacted; non-sensitive values + select options
  preserved). RESIDUAL (low sev, pre-existing): `find` still names unlabeled CONTAINER elements by their
  aggregate `textContent`, which can include a sensitive select's option LABELS (never a secret VALUE or
  the user's selection: input values are not in textContent). Goes away with the deferred find redesign.
- **[MED] [DONE]** **select accessible-name = selected option text** (official 65-67); sensitive selects
  fall through to a label instead.
- **[MED] [DONE]** **Emit inline `placeholder` attr** on element lines (official 156).
- get_page_text: **[MED] use `element.innerText`** (not cloned textContent), richer selector list
  (article-body/entry-content/content-body variants), pick LARGEST-innerText candidate, label
  "Source element: <tag>", return actionable over-limit / <10-char errors (official 22140-22182).
- find: **[decide]** official `find` is **LLM-backed** (feeds full a11y tree to a `small_fast` model,
  returns purpose-ranked matches + a "reason"). Ours is offline whole-string substring. We lack a
  model-sampling channel, so adopting LLM-find is a big architectural call. Minimum improvement:
  tokenize (every-token-present) so "login button" matches a button named "Sign in". KEEP our x/y
  coords in the result (useful; official omits them).

computer / screenshot pipeline (service-worker.js), biggest technique divergence:
- **[HIGH] [DONE]** **Token-budget screenshot downscale**: official caps to `ceil(w/28)*ceil(h/28)<=1568`
  tokens and <=1568px longest side (canvas), then steps JPEG quality 0.75 -> 0.10 by 0.05 until under
  ~1.05MB base64. OURS previously captured the raw viewport (q55, single 30 fallback >500KB) with **NO
  pixel cap** (official 13887-13910,19973-20059). Fixed: capture native (jpeg q80) -> downscale to the
  same token/longest-side budget via OffscreenCanvas in the service worker -> encode jpeg 0.55 (0.30
  fallback if >1.1MB b64).
- **[HIGH, DECIDED -> full official model, user-approved 2026-07-01] [DONE]** **Coordinate model**:
  official NEVER uses `Emulation.setDeviceMetricsOverride`; it probes `innerWidth/innerHeight/
  devicePixelRatio` per screenshot, captures at native DPR, stores a per-tab ScreenshotContext, and
  rescales model coords via `Mv()=round(v*viewport/screenshot)` before dispatch (official
  14079-14101,19874-19963,20730-20734). We removed `deviceScaleFactor:1`/device-metrics override
  entirely: `probeViewport` (CDP Runtime.evaluate) + per-tab `screenshotCtx {vpW,vpH,shotW,shotH}` +
  `rescaleCoord` applied to model-provided coords (a.coordinate / start_coordinate / drag / scroll);
  ref-derived coords (getBoundingClientRect) are NOT rescaled (they are already CSS px). Removed the
  `resize_window` device-metrics refresh (now just invalidates stale ScreenshotContext). Rescale math
  verified numerically (corner->corner, center->center; longest side <=1568). CLAUDE.md updated.
  RESIDUAL: `zoom` still returns a full (downscaled) screenshot rather than cropping `region`. That
  is a step-5 item; the model-coord region would rescale via the same context when implemented.
- **[MED] real `zoom`**: ours ignores `a.region` and returns a full-viewport jpeg, so zoom does not
  zoom; official crops the region on a PNG canvas (official 21086-21174).
- **[MED] double/triple click** send an incrementing `clickCount` sequence (not a lone clickCount:2/3);
  **[MED] type** via real keyDown/keyUp with key fields (code/windowsVirtualKeyCode/location/
  unmodifiedText) so keystroke handlers fire (insertText only as fallback); **[MED] key reload chords**
  (ctrl/cmd+r, f5) -> `chrome.tabs.reload({bypassCache})` (ours silently no-ops reload); **[MED] scroll**
  verify >5px moved + content-script wheel fallback; **[MED] mouse** send `buttons` bitmask + `force:0.5`
  while held (official 19603-21078).

console / network capture (service-worker.js):
- **[CONFIRMED keep] Our single-source console (Runtime.consoleAPICalled only, never Console domain) is
  byte-for-byte the official's design.** The community-reference double-count fix we shipped (8c41a15)
  is correct. Symptom B resolved.
- **[CONFIRMED keep] Lazy `Network.enable` on read matches the official**: it ALSO returns empty for
  page-load traffic and tells the agent to refresh. Symptom A is expected behavior, not a bug.
- **[MED] Append empty-result guidance** to read_console/read_network: "tracking starts when this tool
  is first called; if the page loaded before, refresh to capture page-load events" (official 22787/22914).
- **[MED] Handle `Network.loadingFailed`** -> set status (official uses 503) so failed requests don't
  stay "(pending)" forever.
- **[MED] Capture `Runtime.exceptionThrown`** as a synthetic console `exception` entry so `onlyErrors`
  surfaces uncaught page errors (our read filter already matches `exception` but nothing produces it).
- **[MED] Reset per-tab console+network buffers on domain change**: ours leaks cross-domain data,
  contradicting our own schema text "current domain only / cleared on navigation".
- [LOW] guard debugger attach against `chrome://`/`chrome-extension://`; persist tracking-enabled
  across re-attach.

### C. Deliberately keep ours (do NOT change)

- Console single-source + lazy network enable (matches official); JPEG 55/30 (CLAUDE.md pinned; note
  official uses a finer 0.75->0.10 ladder); single native-messaging port with id-correlated framing;
  tab-group recovery by title; "Browser MCP" blue branding; shadow-DOM traversal in read_page/find
  (ours does MORE than the official, an improvement); `form_input` broader checkbox truthiness (commit
  0deef1c); find returning x/y center coords.

### D. UI capabilities: user-facing parity (NEW: requested for true 1:1 parity)

The official extension is more than a headless CDP executor: it shows the USER what the agent is
doing. This matches our North Star's "end user watching" delight persona and is part of true parity.
Our extension currently has **no visual indicator at all** (manifest `content_scripts` is just
`content.js`); we must add this.

Official evidence (harvest):
- A dedicated content script `assets/agent-visual-indicator.js-CW8zgsee.js` (manifest: matches
  `<all_urls>`, `run_at: document_idle`) renders an on-page **agent-activity indicator**.
- The `computer` handler sends a **phantom-cursor** content message before every mouse dispatch, and
  waits for cursor settle (up to 250ms) so the user sees where the agent's pointer moves/clicks
  (mcpPermissions ~19604-19612, 19650).
- Before `Page.captureScreenshot` it **hides the on-page indicator** and waits ~50ms, so the model's
  screenshot is clean of the overlay (mcpPermissions ~19860). Restores it after.

Plan (reimplement the CONCEPT leanly; do NOT copy Anthropic's overlay code): **[DONE]**
- Added `extension/agent-visual-indicator.js` content script (all_urls, document_idle): (a) a phantom
  **cursor** (own SVG arrow, tip at the target, Claude-orange with a white outline + glow) that moves
  to each dispatch coordinate with a smooth transition; (b) a subtle **"agent active" glow border**
  that self-fades after ~4s of inactivity. Respects prefers-reduced-motion.
- Driven by the service worker: `moveCursor` (UPDATE_PHANTOM_CURSOR with the rescaled CSS-px coord)
  before each mouse dispatch (click/hover/drag/scroll), awaiting the cursor settle so the user sees it
  arrive; `showActivity` (SHOW_AGENT_INDICATORS) at the start of every computer action. Screenshots
  send HIDE_FOR_TOOL_USE + 40ms before capture and SHOW_AFTER_TOOL_USE after, so the model's image
  stays clean.
- Overlay elements (`browser-mcp-*` ids) are excluded from read_page/find in content.js (zero-latency;
  no need to hide them for text reads). content.js's message listener now returns false for non-its
  messages so the two content scripts don't both answer.
- Omitted the official's Stop button + static "chat" pill: those are product controls for the
  OFFICIAL's in-browser agent (STOP_AGENT / SWITCH_TO_MAIN_TAB) and do not apply to our external-client
  model. The official itself suppresses the Stop button in isMcp mode. Lean by design.
- Harvested from the official `agent-visual-indicator.js-CW8zgsee.js` + the mcpPermissions
  phantom-cursor/hide call sites (19604-19611, 19637, 19860); reimplemented, not copied.

### Sequencing

1. [DONE, commit 60bf334] Schema corrections (A) + re-baseline `tests/tool_schema_fidelity.rs` golden
   fixture. Pure Rust; the fidelity test is the guard: update the fixture to the official surface and
   keep the tests passing.
2. [DONE] Extension redaction (B, security) + `<select>` options: highest user/safety value.
3. [DONE] **UI visual cursor + agent-active indicator (D)**: user-facing parity + "watching" delight.
   The cursor renders at the same rescaled CSS-px coordinate `rescaleCoord()` produces for dispatch.
4. [DONE] Screenshot token-budget + coordinate-model decision (B). DECIDED: full official model
   (probe viewport/DPR, downscale to budget, ScreenshotContext, rescale model coords). Done before 3
   so the cursor consumes the rescaled dispatch coord.
5. The remaining MED technique adoptions (zoom, click/type/key, network loadingFailed/exception,
   domain-reset, empty-result guidance).
6. Reload the unpacked extension in Chrome to test each behavior change (Rust side unaffected).
