# G10: Take-the-wheel pause (a user hold the agent must honor)

## Goal

Give the user a pause control in the extension (a popup button and a keyboard shortcut)
that sends a mechanism-only hold/resume message to the binary over the existing native
channel. The BINARY holds the flag and decides: while held, every incoming tool call is
answered with a well-formed successful MCP text result stating that the user has paused
the session and the agent should wait for resume before acting. The phrasing makes agents
wait rather than retry-spin or error out. Resume releases cleanly. A hold that lasts
longer than a bounded period (a constant now, a config key later) appends a hint that
only the user can resume, from the extension. Audit records mark held calls. The engine
is truthful: the hold text never pretends an action happened.

This is part of ADR-0018 step 2 (sacred domains and the kill switch family). The panic
kill switch itself is G11, not this task.

## Depends on

- `docs/tasks/stage-2/00-shared-format.md` -- the reconciled format reference. Read it
  before writing any code. The load-bearing parts here are section 6 (audit record shape,
  which this task extends with one field), section 6.1 (`decision`, `duration_ms`
  semantics), and section 9 (the native-messaging envelope style the new hold messages
  follow: every request carries a caller-chosen string `id`, every response echoes it).
  Note: the hold messages defined below are NOT part of the section 9 settings protocol
  (`get_status` / `get_config` / `set_config_key`); they are a separate, minimal
  vocabulary that only shares its envelope style.
- G06 (audit wiring, ADR-0018 step 1) is the intended predecessor: it produces the
  per-call audit record writer this task marks held calls in. If the audit subsystem
  exists in the tree when you implement (a per-call record writer wired at the dispatch
  chokepoint), wiring the held marker is MANDATORY. If it does not exist, do NOT invent
  an audit subsystem: implement everything else, emit a `tracing::info!` line for held
  calls instead, and state plainly in your completion summary that the `held` audit
  marker awaits the audit task.
- All release-1 (stage-1) tasks in `docs/tasks/release-1/` are assumed landed. No other
  stage-2 task is a prerequisite. G10 does not need the config registry, the manifest
  engine, or sacred domains.

Because earlier stage-2 tasks may reshape `src/dispatch.rs`, `src/mcp/server.rs`, and
add `src/audit/` before G10 runs, the "Current behavior" section below records the tree
as it stands at authoring time. Re-read every file named below before changing it and
integrate against the code that is actually there.

## Project context

Browser MCP is governed browser automation. A single Rust binary is BOTH the MCP server
(JSON-RPC 2.0 over stdio, hand-rolled, tokio) and the Chrome native-messaging host; a
thin Manifest V3 extension executes CDP commands. Architecture:

```
MCP Client <--stdio--> Binary <--native messaging--> Extension <--CDP--> Browser
```

The two binary roles run as separate OS processes bridged by tokio-native named-pipe
(Windows) / Unix-domain-socket (elsewhere) IPC. The native-host process is a stateless
relay: it forwards native-messaging frames verbatim between Chrome (stdio) and the
mcp-server (IPC). Everything that decides lives in the mcp-server process.

Stage 1 hardened the engine. Stage 2 is the governance layer per ADR-0013 (separable
overlay; all-open stays first-class), ADR-0018 (observe-then-enforce sequencing),
ADR-0019 (layered configuration), and ADR-0020 (org policy experience). ADR-0018 step 2
names "a take-the-wheel pause and panic kill-switch honored mid-action" as small,
self-contained user-facing controls that land before the full manifest engine. G10 is
the pause; G11 is the kill switch.

Two invariants govern this task:

- The extension holds mechanism only (ADR-0005). The pause control reports a user
  gesture and renders state the binary reports back. The extension never decides whether
  a tool call runs; the binary holds the flag and answers held calls itself (a held call
  never reaches the extension at all).
- The engine is truthful. The pause reply says plainly that the action was NOT executed.
  It never pretends the action happened, and it never presents as an error.

Authority order: where `docs/SPEC.md` and the ADRs disagree, the ADRs win. The shared
format doc is the single source for file formats, field names, and locations.

## Current behavior

All facts verified against the working tree at authoring time.

`src/dispatch.rs` (31 lines) is the documented governance seam, still a no-op:
`PolicyDecision` (lines 13-17) has the single variant `Allow`;
`pub fn policy_check(_tool: &str) -> PolicyDecision` (lines 23-25) always allows;
`pub fn audit(_tool: &str) {}` (lines 27-30) does nothing.

`src/mcp/server.rs` (156 lines) is the dispatch caller. `handle_tools_call`
(lines 116-155) extracts `name` (line 122) and `arguments`, calls the no-op
`dispatch::policy_check(name)` and `dispatch::audit(name)` (lines 132-133), then
`browser.call(name, &args)` (line 135). Tool failures come back as an MCP `isError` text
result built with `text_content` from `src/mcp/types.rs` (lines 147-153), not as a
JSON-RPC error. There is no hold concept anywhere.

`src/browser.rs` (257 lines) is the mcp-server's handle to the extension:

- `Browser` (lines 33-40) holds `next_id`, `pending`, `outgoing`
  (`Arc<Mutex<Option<mpsc::UnboundedSender<Vec<u8>>>>>`), and `debug`. It is `Clone`;
  the IPC serve task and the MCP loop share one instance.
- `call` (lines 72-115) frames `{ id, type: "tool_request", tool, args }` with
  `host::encode` and awaits the correlated reply.
- `attach` (lines 120-150) splits the stream, spawns a writer draining `outgoing`, and
  routes incoming frames through `route_reply` until the stream closes.
- `route_reply` (lines 153-173) parses the frame, drops messages without an `id` as
  events (lines 158-160), drops unknown-id replies (lines 161-163), and otherwise
  resolves the pending call as `tool_response` / `tool_error`. There is no path today by
  which the extension can ask the binary anything.
- The inline tests (lines 182-256) show the house pattern for testing this file:
  `tokio::io::duplex`, a fake extension task speaking `host::read_message` /
  `host::write_message`, and a `wait_connected` helper.

`src/native/messages.rs` (21 lines) is doc-only prose describing the wire protocol
(`tool_request` / `tool_response` / `tool_error`). `src/native/host.rs` does the 4-byte
LE framing; `src/native/ipc.rs` is the transparent relay. Neither needs code changes for
this task: the relay forwards the new messages verbatim like any other frame.

`extension/manifest.json` (37 lines) declares permissions `tabs`, `debugger`,
`scripting`, `nativeMessaging`, `tabGroups`, `windows`, `storage`, `alarms`
(lines 8-17) and two content scripts. There is NO `action` key and NO `commands` key,
so there is no popup and no keyboard shortcut today.

`extension/service-worker.js` (569 lines): `connect` (lines 27-44) opens the native
port; `nativePort.onMessage` (lines 31-35) handles ONLY `tool_request`;
`onDisconnect` (lines 36-39) nulls the port and retries. There is no
`chrome.runtime.onMessage` listener in the service worker (only the two content scripts
listen on that channel, and content scripts do not receive `chrome.runtime.sendMessage`
traffic from extension pages). `dispatch` (lines 558-566) routes tool requests to
handlers. No hold logic exists.

`extension/agent-visual-indicator.js` is a CONTENT SCRIPT (phantom cursor + glow), not a
popup. Do not confuse it with the popup this task adds, and do not modify it.

`extension/popup.html` and `extension/popup.js` do not exist.

`Cargo.toml` dependencies: `tokio`, `serde`, `serde_json` (with `preserve_order`),
`clap`, `tracing`, `tracing-subscriber`, `thiserror`, `anyhow`, `dirs`. G10 adds none.

## Required behavior

Five pieces: the wire vocabulary, the binary-side flag and handler, the dispatch-side
pause reply, the extension surfaces, and the audit marker.

### 1. Hold message vocabulary (new reverse messages on the existing native channel)

Extension to binary (requests; `id` is a caller-chosen string, unique per request):

```json
{ "id": "<string>", "type": "get_hold" }
{ "id": "<string>", "type": "set_hold", "held": true }
{ "id": "<string>", "type": "toggle_hold" }
```

Binary to extension (responses; `id` is echoed):

```json
{ "id": "<echoed>", "type": "hold_state", "result": { "held": true } }
{ "id": "<echoed>", "type": "hold_error", "error": "set_hold requires a boolean 'held'" }
```

Rules:

- All three request types receive a `hold_state` reply carrying the state AFTER the
  request was applied (`get_hold` reports without changing; `set_hold` sets;
  `toggle_hold` flips atomically in the binary).
- A `set_hold` whose `held` member is missing or not a JSON boolean gets the
  `hold_error` reply above and changes nothing.
- Request/reply only. The binary never pushes unsolicited hold messages.
- Document these shapes in the module doc of `src/native/messages.rs`, alongside the
  existing `tool_request` / `tool_response` / `tool_error` prose, in the same style.
  State there that the native-host relays them verbatim and only the mcp-server
  interprets them.

### 2. Binary: the hold flag and the request handler (`src/browser.rs`)

Add to `Browser`:

- A field `held: Arc<Mutex<Option<std::time::Instant>>>`. `None` means not held;
  `Some(t)` means held since `t`. Initialize to `None` in both constructors.
- `pub fn held_for(&self) -> Option<std::time::Duration>` -- `Some(elapsed)` while held.
- `pub fn set_held(&self, held: bool) -> bool` -- sets the flag and returns the
  resulting state. Setting `true` while already held MUST preserve the original
  `Instant` (the hint timer must not reset on a repeated pause gesture). Setting `false`
  clears it.
- `pub fn toggle_held(&self) -> bool` -- flips atomically under the mutex and returns
  the new state.

Handle the requests in `route_reply`: before the pending-reply lookup, branch on the
message `type`. If it is `get_hold`, `set_hold`, or `toggle_hold` AND the message has a
string `id`, apply the request to the flag and send the `hold_state` (or `hold_error`)
reply back over the same connection, then return. Add a small private helper that
serializes a reply `Value`, frames it with `host::encode`, and enqueues it on the
`outgoing` sender if one is present (drop silently if the connection is already gone).
All other messages flow through the existing logic unchanged.

Hold-state lifetime rules (test each):

- The flag lives only in the mcp-server process memory. It is NOT persisted to disk and
  does NOT survive a binary restart (a fresh session starts unheld).
- The flag is NOT cleared when the extension disconnects or reconnects. The user paused;
  a service-worker death must not silently resume the agent.
- Emit one `tracing::info!` line on every state change (e.g. `user hold engaged` /
  `user hold released`).

### 3. Binary: the pause reply at the dispatch chokepoint

In `src/dispatch.rs` add:

- ```rust
  /// How long a hold may last before the pause reply appends the resume hint.
  /// A constant for now; a future registry key (`engine.hold.hint_after_ms`) will
  /// make it configurable. Do NOT register that key in this task.
  pub const HOLD_HINT_AFTER: std::time::Duration = std::time::Duration::from_secs(120);
  ```
- A pure function:
  ```rust
  pub fn hold_message(tool: &str, action: Option<&str>, held_for: std::time::Duration) -> String
  ```
  The tool label `<label>` renders as `computer (<action>)` when `tool` is `"computer"`
  and `action` is `Some` (matching the denial-format convention in shared-format
  section 7.2); otherwise it is the tool name. The base text is EXACTLY:

  ```
  Paused: the user has taken control of the browser (take-the-wheel). The '<label>' call was NOT executed. This is not an error, and retrying will not help: every browser tool call receives this same reply until the user resumes. Stop issuing browser tool calls, tell the user the session is paused and you are waiting, and continue only after the user says they have resumed.
  ```

  When `held_for >= HOLD_HINT_AFTER`, append a single space and EXACTLY:

  ```
  This session has been paused for more than 2 minutes. Only the user can resume it, from the Browser MCP extension: the popup Pause/Resume button or the toggle keyboard shortcut.
  ```

  If you change `HOLD_HINT_AFTER`, the "2 minutes" wording must be kept in agreement;
  a unit test must pin that agreement (compute the expected phrase from the constant or
  assert both together).

In `src/mcp/server.rs`, in `handle_tools_call`, AFTER extracting `name` and `args` and
BEFORE `dispatch::policy_check` / `dispatch::audit` / `browser.call`:

```rust
if let Some(held_for) = browser.held_for() {
    let action = args.get("action").and_then(Value::as_str);
    // audit marker per section 5 below
    return JsonRpcResponse::success(
        id,
        text_content(dispatch::hold_message(name, action, held_for)),
    );
}
```

Rules:

- The reply is a SUCCESSFUL MCP tool result: one `{type:"text"}` content item, no
  `isError` field, never a JSON-RPC error. Agents must be able to read it and wait.
- The hold check short-circuits before policy and before any extension traffic. A held
  call is never sent to the extension, never evaluated by policy, and is never queued or
  deferred: it is answered immediately, and resuming does NOT replay it. The agent
  re-issues calls itself after resume.
- Only `tools/call` is held. `initialize`, `tools/list`, and `ping` behave exactly as
  before while held.
- A call already in flight when the hold engages completes normally. G10 does not cancel
  in-flight work; hard interruption is the kill switch (G11).

### 4. Extension: popup, keyboard shortcut, badge (mechanism only)

`extension/manifest.json` -- add exactly these two top-level keys (no new permissions;
`action` and `commands` require none):

```json
"action": { "default_title": "Browser MCP", "default_popup": "popup.html" },
"commands": {
  "toggle-hold": {
    "suggested_key": { "default": "Alt+Shift+P" },
    "description": "Pause or resume agent browsing (take the wheel)"
  }
}
```

`extension/service-worker.js` -- add:

- A pending-request map for hold replies (`Map` of id to resolver) and a sequence
  counter. Ids are extension-chosen strings (for example `"h1"`, `"h2"`); they never
  collide with tool ids because tool ids are chosen by the binary and hold replies are
  matched only against this map.
- `holdRequest(payload)`: returns a Promise. Assigns an id, stores the resolver, posts
  the message on `nativePort`. Resolves with the reply's `result` object on
  `hold_state`, and with `null` on `hold_error`, on a 1500 ms timeout, or when there is
  no connected port (attempt `connect()` and still resolve `null`; do not wait for the
  reconnect). `null` means "no active session" to the callers.
- Extend the existing `nativePort.onMessage` listener: if the message `type` is
  `hold_state` or `hold_error` and its `id` is in the pending map, resolve and delete
  the entry. `tool_request` handling is unchanged.
- `chrome.commands.onCommand` listener: on `"toggle-hold"`, send
  `{ type: "toggle_hold" }` via `holdRequest` and update the badge from the reply.
- `chrome.runtime.onMessage` listener for the popup (return `true` to answer
  asynchronously; return `false` for unrecognized message types):
  - `{ type: "getHoldState" }` -> `holdRequest({ type: "get_hold" })`
  - `{ type: "setHold", held: <bool> }` -> `holdRequest({ type: "set_hold", held })`
  - Both respond to the popup with `{ session: <bool>, held: <bool> }` where `session`
    is `false` (and `held` is `false`) when `holdRequest` resolved `null`.
- Badge: when a `hold_state` reply reports `held: true`, set
  `chrome.action.setBadgeText({ text: "II" })` and
  `chrome.action.setBadgeBackgroundColor({ color: "#D97757" })`; when it reports
  `held: false`, clear the badge text (`""`). In the port `onDisconnect` handler, clear
  the badge (state unknown without a session).
- CRITICAL: do NOT gate `tool_request` dispatch on any held state in the service worker.
  The binary decides; while held it simply never sends `tool_request`. The extension
  holds no policy.

`extension/popup.html` + `extension/popup.js` (new files, vanilla JS, ASCII only, no
inline scripts -- MV3 CSP requires `<script src="popup.js"></script>`):

- On open, the popup sends `{ type: "getHoldState" }` and renders one of three states:
  - `session: false`: status text `No active browsing session.`, button disabled.
  - `held: true`: status text `Agent browsing is PAUSED.`, button label
    `Resume agent browsing`.
  - `held: false`: status text `Agent browsing is allowed.`, button label
    `Pause agent browsing (take the wheel)`.
- Clicking the button sends `{ type: "setHold", held: <negation of rendered state> }`
  and re-renders from the response. Explicit set (not toggle) so the action matches what
  the user saw.
- Include one small note line naming the keyboard shortcut (`Alt+Shift+P by default;
  change it at chrome://extensions/shortcuts`).
- The popup renders binary-reported state and submits gestures. It caches nothing (no
  `chrome.storage`, no persisted state) and decides nothing.

### 5. Audit: mark held calls

If the audit subsystem has landed (see Depends on), a held call still produces exactly
one audit record (shared-format section 6: one record per tool call, no exceptions).
On that record:

- `decision` is `"allow"` (the call was not policy-denied; policy was never consulted).
- `duration_ms` is `0` (the tool did not execute; same convention as calls denied
  before dispatch, shared-format 6.1).
- A new boolean field `held` is `true`. Add `held` to the record shape with value
  `false` on ALL other records (always present, never omitted).
- `tool`, `action`, and `rw` are filled as the audit task normally fills them. If the
  audit path would need an extension round trip to resolve `domain` for a held call, use
  `null` instead; a held call must not touch the extension.

Also append this exact row to the field table in section 6.1 of
`docs/tasks/stage-2/00-shared-format.md` (additive edit only; touch nothing else in
that file):

```
| `held` | boolean | `true` when the call was answered with the take-the-wheel pause text instead of executing (user hold, G10); on held records `decision` is `"allow"` and `duration_ms` is `0`. `false` on all other records. |
```

If the audit subsystem has NOT landed: skip this whole section (including the format-doc
row), emit `tracing::info!(tool, "tool call answered with user-hold pause")` at the
chokepoint instead, and report the deferral in your completion summary.

### 6. Tests

Unit tests inline (`#[cfg(test)]`), integration tests in `tests/` if needed. Minimum:

1. `dispatch::hold_message`: base text contains `NOT executed` and starts with
   `Paused:`; no hint below `HOLD_HINT_AFTER`; hint present at exactly `HOLD_HINT_AFTER`
   and above; the hint wording agrees with the constant; `computer` with
   `Some("left_click")` renders `computer (left_click)`; a plain tool renders its name.
2. `Browser` hold state: `set_held(true)` then `held_for()` is `Some`;
   `set_held(false)` clears; `toggle_held` returns `true` then `false`; calling
   `set_held(true)` twice with a ~30 ms sleep between preserves the original start
   (`held_for()` >= 30 ms after the second set).
3. `route_reply` handling, using the existing duplex + fake-extension pattern in
   `src/browser.rs`: a framed `set_hold` (held true) gets a framed `hold_state` reply
   with `held: true` and flips `held_for()` to `Some`; `get_hold` reports without
   changing state; `toggle_hold` flips; `set_hold` with a non-boolean `held` gets
   `hold_error` and changes nothing; hold survives the fake extension closing its end
   of the duplex (after `attach` returns, `held_for()` is still `Some`).
4. `handle_tools_call` with a held `Browser` and NO extension connected returns a
   success result whose single text block starts with `Paused:` and which has no
   `isError` member (this also proves the hold check precedes the
   "extension not connected" failure path). With hold released and no extension, the
   existing `Error: ...` `isError` result is returned unchanged.
5. All-open / no-hold invariant: with the hold never engaged, every existing test passes
   unchanged, including `tests/tool_schema_fidelity.rs` and `tests/mcp_protocol.rs`.
6. If the audit marker was wired: a held call writes one record with
   `decision: "allow"`, `held: true`, `duration_ms: 0`; a normal allowed call writes
   `held: false`.

## Constraints

1. NEVER modify `src/mcp/schemas/tools.json`, tool names, parameters, or description
   strings. `tests/tool_schema_fidelity.rs` must pass unchanged. G10 adds no tool and
   filters nothing from `tools/list`.
2. The extension holds mechanism only: no policy, access, or redaction decision in any
   extension JS. The popup and shortcut report the user gesture; the badge and popup
   render state the binary reports. The service worker must NOT gate or drop
   `tool_request` messages based on any held state.
3. All-open stays first-class: with no manifest and default config, and the hold never
   engaged, behavior is byte-identical to today. The hold path only activates on an
   explicit user gesture and releases cleanly on resume.
4. ASCII only in all code and docs, including this task's new files, comments, message
   strings, popup text, and the format-doc row: no em-dashes, no arrows, no curly
   quotes. Use ` -- ` (double hyphen) where the codebase uses it.
5. The engine is truthful: the pause reply states the call was NOT executed, presents as
   neither an error nor a success-of-the-action, and instructs the agent to wait for the
   user rather than retry. Use the exact strings in section 3.
6. No new runtime dependencies in `Cargo.toml` (including dev-dependencies). The
   extension stays vanilla JS (no bundler, no framework, no library).
7. Rust 2021 edition; `thiserror` for library error types (this task should not need a
   new error variant; the hold path has no fallible surface toward the MCP client); doc
   comments on every new public item; `cargo fmt` clean;
   `cargo clippy --all-targets -- -D warnings` clean. Unit tests inline, integration
   tests in `tests/`.
8. Do NOT copy code from the official Anthropic extension, the reference repo, or any
   other project; implement from the behavior described here.

Task-specific:

9. The hold is a user gesture, not a policy decision: do not route it through
   `policy_check`, do not create a `Deny` variant for it, and do not involve the
   manifest or config registry. It short-circuits before both no-op seams.
10. Held calls are never queued, deferred, or replayed. Resume changes future calls
    only.
11. No in-flight cancellation: a call already sent to the extension completes. That is
    G11's territory.
12. The hold flag lives only in mcp-server process memory: no disk persistence, no
    `chrome.storage`, no survival across binary restarts, and no clearing on extension
    disconnect.
13. Request/reply only on the wire: the binary never pushes unsolicited `hold_state`
    messages, and the extension re-queries (popup open, shortcut reply) instead of
    assuming.
14. `src/native/host.rs` and `src/native/ipc.rs` code is untouched; only the
    `src/native/messages.rs` module doc gains the new message documentation.

## Verification

1. From the repo root: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and
   `cargo test` all clean. `tests/tool_schema_fidelity.rs` passes without any edit.
2. Rebuild the binary. If `target/debug/browser-mcp.exe` is locked by a running session,
   rename it aside (`mv target/debug/browser-mcp.exe
   target/debug/browser-mcp.exe.old-1`) and rebuild. Binary changes need an MCP client
   restart; extension changes need a reload at `chrome://extensions` (and the popup /
   shortcut appear only after the reload).
3. Manual flow with Claude Code connected: open the extension popup; it shows
   `Agent browsing is allowed.` Click `Pause agent browsing (take the wheel)`; the
   status flips to `Agent browsing is PAUSED.` and the toolbar badge shows `II`.
4. Ask the agent to take a screenshot. The agent receives the `Paused:` text (not an
   error), the extension receives no `tool_request` for it, and the agent reports it is
   waiting instead of retry-looping.
5. If the audit subsystem landed: the held call's record shows `decision: "allow"`,
   `held: true`, `duration_ms: 0`. Otherwise the server log shows the
   `tool call answered with user-hold pause` trace line.
6. Click `Resume agent browsing` (or press the shortcut). The badge clears. The same
   tool call now executes normally with its normal result.
7. Press the keyboard shortcut (default `Alt+Shift+P`) with the popup closed: the badge
   toggles; re-opening the popup shows the matching state.
8. Hint: pause, wait past 2 minutes (or temporarily lower `HOLD_HINT_AFTER` in a local
   build for the check; restore it before committing), issue a call, and confirm the
   reply carries the appended only-the-user-can-resume hint.
9. Kill the service worker from `chrome://extensions` while paused, let it restart, and
   confirm a tool call is STILL answered with the `Paused:` text (the binary flag
   survived the extension restart).
10. With no MCP session running, open the popup: it shows
    `No active browsing session.` with the button disabled, within about 1.5 seconds.

## Out of scope

- The panic kill switch (G11): no cancellation of in-flight tool calls, no CDP debugger
  detach, no session teardown, no hard stop of any kind. The pause only answers FUTURE
  calls.
- Consent cards, per-action approval prompts, or any allow/deny UI. The pause is a
  single global hold, not a decision surface.
- Any policy decision in the extension. The extension only reports the user gesture and
  renders binary-reported state; it never inspects, gates, or drops tool traffic.
- The native-messaging settings protocol of shared-format section 9 (`get_status`,
  `get_config`, `set_config_key`): do not implement it, and do not fold hold state into
  a `get_status` reply here. If that protocol already exists when you run, still keep
  the hold vocabulary separate as specified; merging surfaces is a later task's call.
- Registering `engine.hold.hint_after_ms` (or any key) in the config registry, presets,
  layered resolution, or org locks (ADR-0019 tasks own the registry). The bound stays a
  constant.
- Auto-resume, hold timeouts, or expiry: the bounded period only appends a hint text; a
  hold never releases itself.
- Persisting hold state to disk or `chrome.storage`, or restoring it across binary
  restarts.
- The audit subsystem itself (record shape, destinations, JSON Lines framing): G10 only
  sets the `held` marker on records an existing writer produces, per section 5.
- An options page, popup styling beyond a minimal readable layout, or any extension UI
  beyond `popup.html`, `popup.js`, the badge, the `action` key, and the `commands` key.
- Any edit to `src/mcp/schemas/tools.json`, `extension/content.js`,
  `extension/agent-visual-indicator.js`, `src/native/host.rs`, `src/native/ipc.rs`, the
  installer, or the debug/status subsystem.
