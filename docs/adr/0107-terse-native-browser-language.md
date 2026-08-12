# ADR-0107: Terse native browser language

- Status: Accepted
- Date: 2026-08-11
- Supersedes: ADR-0101 Decisions 2 and 3, Decision 4's 12-tool native catalog, Decision 7,
  Decision 8's edge-owned profile declarations, and the profile rollout in Decision 10
- Amends: ADR-0078 Decision 4 and its non-goal for generic console and network diagnostics
- Builds on: ADR-0053, ADR-0073, ADR-0074, ADR-0089, ADR-0094, ADR-0096, ADR-0101,
  and ADR-0103

## Context

The clean-room 1.0 build proved the product architecture with a direct 24-tool catalog. A complete
language pass then exposed two different kinds of cost.

First, several names split one obvious browser concept into adjacent verbs. Listing, focusing, and
closing tabs appeared as three tools. Opening and navigating appeared as two. History and reload
appeared as two. Zoom was isolated while viewport resize was missing. A capable model can learn
those seams, but a smaller model spends tool-choice capacity on distinctions the arguments can
state more plainly.

Second, some advertised JSON schemas were looser than the real decoder. Click, hover, drag,
screenshot, wait, and dialog constraints lived in prose or validation rather than in the schema a
model sees. One-line tool descriptions had no property guidance, examples, output schemas, or MCP
annotations. This made short calls possible but made invalid calls too easy.

The same review revisited three useful 0.8 capabilities that did not survive the clean-room
rebuild: ephemeral GIF recording, console observation, and network observation. Their product jobs
still matter, but their old names and argument shapes do not. `gif_creator`,
`read_console_messages`, and `read_network_requests` exposed mechanics, numeric tab ids, regexes,
destructive reads, and several mutually exclusive booleans. The recording implementation itself
had already reached the right architecture in ADR-0073: service-owned, transactional,
memory-only, bounded, and truthful.

ADR-0101 proposed an adaptive profile system, a 25-tool compatibility profile, and a 12-tool
native fallback. None of that catalog or profile rollout shipped in the clean-room tree. The
separation between model language, product operation, and browser mechanism remains useful. The
profile machinery is not required to obtain it, and it would add substantial architecture before
one default Ghostlight language has been evaluated.

## Decision

### 1. Ship one 22-tool Ghostlight language

Ghostlight 1.0 has one model-facing catalog for compatible MCP clients:

| Tool | Job |
| --- | --- |
| `browser_tabs` | List, focus, or close controlled tabs. |
| `browser_navigate` | Navigate an existing tab or create and navigate a new one. |
| `browser_history` | Go back, go forward, or reload. |
| `browser_window` | Set zoom or resize the controlled browser window. |
| `browser_read` | Read bounded useful page text. |
| `browser_inspect` | Inspect controls or page structure. |
| `browser_find` | Find current semantic targets by text. |
| `browser_screenshot` | Capture a viewport, page, or target and mint a view handle. |
| `browser_click` | Click a semantic target or a point in a current view. |
| `browser_scroll` | Scroll the page or reveal a semantic target. |
| `browser_hover` | Hover a semantic target or a point in a current view. |
| `browser_fill_form` | Fill several ordinary fields and optionally submit. |
| `browser_type_text` | Type text through browser input events. |
| `browser_press_key` | Send one keyboard action. |
| `browser_drag` | Drag between semantic targets or points in a current view. |
| `browser_wait` | Wait for one explicit observable condition. |
| `browser_dialog` | Inspect or resolve a JavaScript dialog. |
| `browser_upload` | Upload bounded local files to an ordinary file input. |
| `browser_evaluate` | Evaluate bounded page-context JavaScript. |
| `browser_sequence` | Run a short ordered sequence of known operations. |
| `browser_record` | Own the memory-only recording lifecycle and save its GIF. |
| `browser_diagnose` | Read opt-in bounded console and network evidence. |

There is no simultaneously advertised legacy dialect, automatic client-name classifier, vendor
profile, or profile union. Every compatible MCP client sees this catalog. A future compatibility
surface requires its own evidence and decision; it is not dormant infrastructure in 1.0.

This supersedes the unreleased catalog and profile rollout portions of ADR-0101 named above. It
retains the architectural rule that the orchestrator owns product language and semantics, the MCP
connector owns generic protocol behavior, and the extension owns policy-free Chromium
mechanisms. A model-facing name still does not become a browser wire command by convention.

### 2. Use action enums only for cohesive families

Five tools use one required `action` string because each is one small state machine or one
resource family:

- `browser_tabs`: `list`, `focus`, or `close`;
- `browser_history`: `back`, `forward`, or `reload`;
- `browser_window`: `zoom` or `resize`;
- `browser_dialog`: `status`, `accept`, `dismiss`, or `respond`; and
- `browser_record`: `start`, `status`, `stop`, `save`, or `discard`.

Their schemas use discriminated branches so irrelevant fields are invalid. They do not encode
verbs as booleans, overload one property's type, or rely on mutually exclusive optional keys.
Tab focus and close require `tab`; list accepts none. History accepts optional `tab` and timeout;
`bypass_cache` is valid only for reload. Window zoom requires integer `percent` from 25 to 500 and
is Read-classified. Window resize requires integer `width` from 320 to 7680 and `height` from 240
to 4320, accepts optional `tab`, and is Action-classified because it visibly changes browser
chrome. Dialog `text` is required only for respond and is invalid for status, accept, and dismiss.

The ordinary interaction tools remain narrow. Click, hover, drag, key input, form filling, and
text typing are not folded into one large `browser_act` union. A smaller tool count is not useful
when it replaces an obvious tool choice with a difficult argument choice.

### 3. Make navigation terse without hiding tab creation

`browser_navigate` requires `url` and accepts optional `tab`, `new_tab`, and `timeout_ms`.
`new_tab` defaults to `false`.

- `new_tab:true` always creates a controlled tab and navigates it. It cannot be combined with
  `tab`.
- With `tab`, Ghostlight navigates that exact controlled tab.
- With neither, Ghostlight uses the only controlled tab, or the sole active controlled tab when
  ownership is unambiguous.
- With no controlled tab, omission creates one and navigates it.
- Ambiguous selection is rejected before dispatch and points to `browser_tabs`.

This keeps the common call at `{"url":"https://example.com"}` while making new-tab intent
explicit when existing browser state makes it matter.

### 4. Make schemas teach the valid call

The catalog is the model's primary instruction surface. Every tool declaration therefore carries:

- a concise purpose and sibling-selection rule where tools overlap;
- property descriptions, enum meanings, executable defaults, and one shortest valid example;
- typo closure at every object level;
- discriminated `oneOf` branches for conditional requirements;
- a truthful output schema and standard MCP annotations; and
- the same required fields, defaults, and bounds as the decoder.

Opaque handles keep meaningful prefixes: `tab_`, `target_`, `view_`, `recording_`, and bounded
diagnostic cursors. Optional `tab` keeps the existing unambiguous-selection rule. Every tool keeps
the flat, optional `restrict_hosts` and `restrict_capabilities` fields. These restrictions can
only reduce authority.

The result envelope and typed outcome voice remain unchanged. A completed result sentence, safe
next steps, and content-free measurements still come from one typed outcome under ADR-0103.

### 5. Restore recording as `browser_record`

`browser_record` is the terse surface over ADR-0073's reliable ephemeral recording lease. The
service owns the state machine, volatile frame store, deadlines, encoder, and delivery truth. The
extension owns only Chrome screencast start, frame acknowledgement, final-frame stop, and the
truthful recording indicator. Generic bounded blob delivery remains below the tool layer under
ADR-0074.

The actions are:

- `start`: begin a transactional memory-only recording on an optional unambiguously selected tab;
- `status`: report state, deadlines, frame count, byte count, and stop reason;
- `stop`: cross the final-frame barrier and freeze the recording;
- `save`: auto-stop when needed, encode one immutable GIF, and either return it to the client or
  attach it to an optional semantic `target`; and
- `discard`: erase the captured bytes.

`recording` may be omitted for status, stop, save, or discard only when exactly one owned recording
can be resolved. Otherwise Ghostlight requires the `recording_` handle and returns corrective
owned handles without exposing another session's state. `target` is valid only for save. Save
without a target returns one bounded MCP GIF image block. Save with a target performs a page
delivery and distinguishes prepared, dispatched-unverified, and outcome-unknown states. It never
claims that the page or a remote service accepted the file.

Start requires Read against the source tab. Client save requires Read against the owned recording.
Target save requires Write against the destination tab. Status, stop, and discard require no new
capability so cleanup remains possible after browser loss or authority reduction. Ownership,
revocation, idle and hard deadlines, frozen retention, memory bounds, immutable repeatable encoding,
and best-effort zeroization remain exactly as ADR-0073 decided. The frozen encoding can be
prepared repeatedly; a target delivery is a separate Write effect and is not thereby repeat-safe.

### 6. Restore diagnostics as one opt-in bounded read

`browser_diagnose` combines console and network observation because the user intent is usually
"why did this page fail?" Its default call is `{}` on an unambiguously selected tab. Defaults are
`source:"both"` and `detail:"problems"`.

Inputs are:

- `source`: `both`, `console`, or `network`;
- `detail`: `problems` or `all`;
- `match`: an optional case-insensitive literal substring, never a regular expression;
- `after`: an opaque non-destructive cursor returned by an earlier diagnostic read;
- `limit`: a maximum from 1 to 200, default 50; and
- optional `tab` and request restrictions.

`problems` includes console warnings, console errors, exceptions, failed requests, and HTTP error
responses. `all` also includes ordinary console events and successful requests. Results are
ordered, bounded, cursor-based, and report truncation or eviction. There is no `clear` input and a
read never consumes history.

Observation is off until an authorized `browser_diagnose` call enables it for that controlled
tab. The first call may therefore return no earlier evidence and tells the model to reproduce or
reload when appropriate. Once enabled, the extension keeps only bounded volatile rings for the
owned tab. Tab closure, ownership loss, browser disconnect, service exit, or expiry removes them.
They are never written to extension storage, audit, logs, or restart state.

Console text is length-bounded. Network evidence excludes headers, bodies, cookies, authorization,
post data, query strings, and fragments. It returns only bounded method, governed origin and path,
resource kind, status, and failure category. Cross-origin entries pass the same host authority
ceiling before disclosure. Denied detail is omitted with a content-free count. Diagnostic payloads
are model-visible untrusted page evidence, never policy input or audit content.

This explicitly amends ADR-0078 Decision 4 and its non-goal. Diagnostics remain absent from
automatic recovery capsules and are not always on. The accepted capability is one deliberate,
Read-classified, visually quiet tool with a bounded lifecycle.

### 7. Evaluate the language, not only its size

The release gate exercises every documented shortest call with lower-capability-model fixtures.
It also compares representative models on first-call validity, correct tool selection, correction
turns, stale-handle recovery, task completion, unsafe calls, and total tool/result bytes.

The 22-tool count is a consequence of cohesive intent boundaries, not the target metric. A future
merge or split needs journey evidence that it improves use without weakening governance or
outcome truth.

## Consequences

- One regular Ghostlight vocabulary serves every compatible MCP client.
- Five related tool groups become cohesive action families while precise interaction verbs remain
  easy to select.
- Recording and page diagnosis return without importing their cryptic 0.8 names or destructive
  input ergonomics.
- The catalog grows richer even as calls stay terse: constraints move into schemas instead of
  decoder-only errors.
- The orchestrator gains recording and diagnostic product state. The extension gains only the
  Chrome mechanisms and bounded volatile buffers that must live there.
- The 1.0 implementation must replace the current 24-tool catalog atomically. It must not
  advertise old and new Ghostlight names together.

## Rejected alternatives

### Keep the 24-tool clean-room catalog and add three historical tools

Rejected because it preserves avoidable choice seams and restores poor names. It would also grow
the catalog to 27 while missing the chance to make the language regular.

### Ship the ADR-0101 12-tool native profile beside a legacy profile

Rejected for 1.0 because profile negotiation, translation, catalog caching, and client
classification add architecture before the default language has earned it. The useful separation
of language, operation, and browser mechanism does not require simultaneous dialects.

### Merge every interaction into `browser_act`

Rejected because the action enum and conditional fields become harder for smaller models than
choosing `browser_click`, `browser_hover`, `browser_drag`, or `browser_press_key` directly.

### Use intent keys such as `{"start":true}` for recording

Rejected because one required enum keeps every invocation the same shape. It avoids overloaded
boolean-or-string values and makes irrelevant-field validation straightforward.

### Restore separate console and network tools

Rejected because they duplicate filtering, cursor, lifecycle, and recovery semantics. One
diagnostic intent with a `source` selector is smaller and teaches the default problem-focused read.

### Capture diagnostics continuously

Rejected because always-on observation expands sensitive-data lifetime and browser work for users
who did not request diagnosis. Explicit opt-in keeps the capability useful and bounded.

## Superseding decision

ADR-0108 replaces this ADR's service-owned recording coordinator with one plural, volatile
extension-owned registry. The `browser_record` model surface stays the same. The orchestrator still
owns language, governance, rendering, and delivery truth; the extension now owns recording ids,
frames, capture bounds, stop, retention, and erase.
