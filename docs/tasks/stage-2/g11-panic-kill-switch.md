# G11: Panic kill switch: sever the session in one gesture

## Goal

Give the user a panic kill switch: one click in the extension popup that severs the
agent's browser access immediately and completely. The kill control (distinct from any
pause control) detaches the chrome.debugger from every session tab, tears down the
native-messaging port, and signals the binary; the binary then fails every in-flight and
every subsequent tool call with a truthful hop-attributed error stating that the user
ended the session. The extension guarantees the debugger detach even if the service
worker restarts mid-kill, by persisting a kill marker in `chrome.storage.session` and
honoring it on every startup. Recovery is explicit: a fresh session begins only when the
user clicks reconnect in the popup. The binary writes one audit record for the kill
event.

This is ADR-0018 step 2 (the panic kill switch half; sacred domains and pause are
separate tasks).

## Depends on

- `docs/tasks/stage-2/00-shared-format.md` -- the reconciled format reference. Every
  field name, file location, and format in this task comes from it. Read it before
  writing any code. Load-bearing sections here: 1.4 (default audit file path), 3.4
  (`audit.enabled`, `audit.destination`, `audit.file.path` keys), 6 (audit record
  framing and the `event_id` / `ts` / `identity` / `client` / `manifest` field
  definitions reused by the kill record), 9 (the settings protocol this task must NOT
  implement).
- All release-1 tasks in `docs/tasks/release-1/` are assumed landed. In particular T06
  (hop-attributed errors) provides the `ToolError` type in `src/error.rs` with the
  `extension` constructor and the `next_step` builder, and changes `Browser::call` to
  return `Result<Value, ToolError>`. G11's kill error is built with that type. If
  `ToolError` does not exist in the tree, stop and land T06 first.
- The stage-2 audit flight-recorder task (G06 wires audit into dispatch; see the G05
  prompt's Goal note) -- it owns the audit destination resolution (`audit.enabled`,
  `audit.destination`, `audit.file.path`) and the JSON Lines writer. G11 appends the
  kill record through that machinery. If the audit subsystem has not landed, stop and
  land it first; do not invent a parallel audit writer inside this task.
- G10 (take-the-wheel pause) is NOT a prerequisite. G10 and G11 may land in either
  order. Both add controls to the extension popup: whichever lands first creates the
  popup files; the second integrates into them. Section 4 of Required behavior below
  covers both cases.

Because prerequisites reshape `src/browser.rs`, `src/error.rs`, `src/mcp/server.rs`,
and `extension/service-worker.js` before G11 runs, the "Current behavior" section below
records the tree as it stands today (pre-release-1). Do NOT trust it as the state you
will edit. Re-read every file named below before changing it, and integrate against the
code the prerequisites actually produced. Trust function names and prose over line
numbers.

## Project context

Browser MCP is a governed browser automation system. A single Rust binary is both the
MCP server (JSON-RPC 2.0 over stdio, hand-rolled, tokio) and the Chrome
native-messaging host. A thin Manifest V3 extension executes CDP commands. The chain
is:

    MCP Client <--stdio--> Binary <--native messaging--> Extension <--CDP--> Browser

The two binary roles run as separate OS processes bridged by tokio-native named-pipe
(Windows) or Unix-domain-socket (elsewhere) IPC: the mcp-server role (launched by the
MCP client) owns the IPC endpoint; the native-host role (launched by Chrome via
`connectNative`) relays native-messaging frames verbatim between the extension and the
mcp-server (`src/native/ipc.rs::relay_native_host`).

This is stage 2, the governance layer. Governance is a separable overlay (ADR-0013):
with no manifest and default config, tool-call behavior is byte-identical to the
all-open engine. Enforcement follows observe-then-enforce sequencing (ADR-0018): the
audit flight recorder lands first, then sacred domains and the kill switch, then the
full manifest engine. The kill switch is deliberately manifest-independent and
mode-independent: it is a user-authored severance that works identically in all-open
mode, under an observe manifest, and under an enforce manifest. It is not a policy
decision; it is the user physically taking the keys back. That is why the extension may
implement it: severing the extension's OWN debugger attachments and its OWN native port
is mechanism, not policy (ADR-0005: the extension holds mechanism only).

Two invariants govern this task:

- All-open stays first-class. Absent a kill gesture, every tool call behaves
  byte-identically to today. The kill switch adds a popup and event handling but
  changes nothing on the normal path.
- The engine is truthful. After a kill, the binary says plainly that the user ended the
  session; it never reports a generic connection failure when it knows the real cause,
  and it never pretends the session is recoverable without the user's explicit action.

## Current behavior

All facts verified against the working tree at authoring time (pre-release-1; see the
caveat under "Depends on").

`extension/manifest.json` (37 lines):

- Has NO `action` key: there is no popup, no toolbar button behavior beyond the
  default. No `popup.html` or `popup.js` exists in `extension/`.
- The `"storage"` permission is already declared (line 15), as is `"alarms"`
  (line 16). `chrome.storage` is not used anywhere in the extension today (grep for
  `storage` in `extension/` matches only the manifest).

`extension/service-worker.js` (569 lines):

- `NATIVE_HOST = "org.sylin.browser_mcp"` (line 8); `nativePort` (line 11); the
  `attached` map of tabId to CDP state (line 13); `attaching` in-flight attach promises
  (line 54); `consoleBuffer`, `networkBuffer`, `screenshotCtx` maps (lines 14-16).
- Keepalive: `chrome.alarms.create("keepalive", { periodInMinutes: 0.4 })` (line 22)
  and an `onAlarm` listener that calls `connect()` whenever `nativePort` is null
  (lines 23-25).
- `connect()` (lines 27-44): opens the native port, dispatches incoming
  `tool_request` messages, and on `onDisconnect` nulls the port and schedules
  `setTimeout(connect, 2000)` (lines 36-39). There is no condition that ever refuses
  to reconnect.
- `ensureAttached(tabId)` (lines 55-64) populates `attached` via
  `chrome.debugger.attach({ tabId }, "1.3")`.
- `chrome.tabs.onRemoved` (lines 125-133) detaches per closed tab;
  `chrome.debugger.onDetach` (line 134) drops the map entry.
- `sleep(ms)` helper exists (lines 242-244).
- `dispatch(id, tool, args)` (lines 558-566) routes tool requests; top-level
  `connect();` runs at every service-worker start (line 568).
- There is NO `chrome.runtime.onMessage` listener in the worker, no kill or pause
  logic, and no storage use.

`src/browser.rs` (257 lines) -- the mcp-server's handle to the extension:

- `TOOL_TIMEOUT` 60s (line 25). `Browser` struct (lines 33-40) holds `next_id`,
  `pending`, `outgoing`, `debug`. No killed state exists.
- `Browser::call` (lines 72-115): fails with message
  `browser extension is not connected` when no native-host is attached (lines 90-96).
  T06 reshapes these errors into `ToolError` values; re-read the file.
- `Browser::attach` (lines 120-150): marks connected, routes replies until the stream
  closes, then fails every pending call with `extension disconnected` (lines 147-149).
- `route_reply` (lines 153-173): parses a reply; a message WITHOUT an `id` returns
  early as "an event/heartbeat, not a tool reply" (lines 158-160). This is where the
  kill event will be recognized.

`src/native/messages.rs` (21 lines): doc-only module for the binary <-> extension wire
protocol. It states that replies without an `id` are ignored by the mcp-server in v1.0
(lines 19-20). T06 adds the `hop`/`detail` fields to `tool_error` here.

`src/native/ipc.rs::relay_native_host` (lines 43-71): the native-host role relays
frames verbatim in both directions; it never parses them. G11 changes nothing here.
`src/main.rs::run_native_host_role` (lines 212-226) ends with `std::process::exit(0)`
(line 225) -- the zombie fix; G11 changes nothing here either.

`src/mcp/server.rs::handle_tools_call` (lines 116-155): renders a `Browser::call`
failure as an `isError: true` tool result (pre-T06: `Error: {e}`, lines 146-153;
post-T06: the `[hop: ...]` format).

`src/error.rs`: `enum Error` (line 10) with `NativeMessaging(String)` (line 17). T06
adds `ToolError` below it with constructors `invalid_request`, `binary`, `ipc`,
`extension`, `cdp`, `page` and the `next_step(self, step) -> Self` builder.

`src/dispatch.rs` (31 lines): the documented no-op policy/audit seams. The stage-2
audit task replaces the `audit` seam; there is no `src/audit/` directory today.

## Required behavior

G11 delivers five things: the persistent kill marker and gated reconnect in the
extension, the kill sequence itself, the popup control, the `session_killed` wire event
and the binary's killed state with its truthful error, and the audit record. All policy
stays in the binary; everything the extension does here is severing its own mechanism.

### 1. Extension: kill marker and gated reconnect

The single source of truth for "the user killed the session" on the extension side is
one `chrome.storage.session` key:

- Key: `"session_killed"`, value `true` when a kill is in force; the key is REMOVED
  (not set to false) on explicit reconnect.
- `chrome.storage.session` is deliberate: it survives service-worker restarts (the
  mid-kill guarantee) but not a full browser restart. Closing the browser detaches all
  debuggers and closes the native port by construction, so after a browser relaunch
  the marker is gone and normal connect behavior resumes; that relaunch IS a fresh
  session. Do not use `chrome.storage.local` or `sync` for this marker.

Gate every reconnect path through the marker. Change `connect()` to an async function
with this exact shape of guard logic:

    async function connect() {
      if (nativePort) return;
      const s = await chrome.storage.session.get("session_killed");
      if (s.session_killed) return; // killed: only an explicit user reconnect resumes
      if (nativePort) return;       // re-check: another caller may have won the await
      ... existing connectNative logic ...
    }

All three existing callers (the top-level startup call, the keepalive `onAlarm`
listener, and the `setTimeout(connect, 2000)` in `onDisconnect`) go through this guard
unchanged; callers do not need to await it. Reading storage inside `connect()` (rather
than mirroring it in a module variable) is required: a module variable resets to its
initial value on every service-worker restart and can race the async read, and the
keepalive alarm can fire in that window.

### 2. Extension: the kill sequence

Add a `killSession()` async function in `extension/service-worker.js`. Steps, in this
exact order (the order is load-bearing):

1. Persist the marker FIRST: `await chrome.storage.session.set({ session_killed:
   true })`. If the service worker dies at any point after this line, startup recovery
   (section 3) completes the kill.
2. Signal the binary, best-effort, while the port is still open: if `nativePort` is
   non-null, post `{ type: "session_killed" }` (no `id`; it is an event, not a reply)
   inside try/catch, then `await sleep(100)` so the frame flushes before the port is
   torn down. If the port is already null (binary not running), skip this step; the
   kill is still valid as a local severance.
3. Detach every debugger attachment:
   - for each tabId in the in-memory `attached` map, `await
     chrome.debugger.detach({ tabId })` inside try/catch (swallow errors; the tab may
     be gone);
   - then sweep `await chrome.debugger.getTargets()` and for every target with
     `attached === true` and a `tabId`, detach it the same way. The sweep covers
     attachments made before a service-worker restart that the in-memory map has
     forgotten. Detach calls on targets attached by something else (DevTools) fail
     and are swallowed; that is fine.
4. Clear in-memory session state: `attached.clear()`, `attaching.clear()`,
   `consoleBuffer.clear()`, `networkBuffer.clear()`, `screenshotCtx.clear()`. Do NOT
   close any tab, do NOT ungroup or remove the Browser MCP tab group, and do NOT touch
   `groupId` (the user's tabs are the user's).
5. Tear down the native port: if `nativePort` is non-null,
   `try { nativePort.disconnect(); } catch {}` then `nativePort = null;`. (Chrome does
   not fire our own `onDisconnect` for a self-initiated disconnect, and even if a
   reconnect timer were pending, the section-1 guard blocks it.)

After the marker is set, incoming `tool_request` messages (a race window exists between
step 1 and step 5) must be refused: in the native-port `onMessage` handler path, when
the marker is in force, reply with a `tool_error` whose error string is exactly
`The user ended the browser session (kill switch)` instead of dispatching. Implement
this with a module-level boolean set synchronously at the top of `killSession()` (set
it in the same tick, before the first await) and also set by startup recovery; this
boolean is an optimization for the hot path only -- the storage marker remains the
source of truth for reconnect gating.

### 3. Extension: startup recovery (honor the marker)

The service worker's startup path (today the bare `connect();` at the bottom of the
file) becomes an async init:

    async function init() {
      const s = await chrome.storage.session.get("session_killed");
      if (s.session_killed) {
        // A kill is in force (possibly interrupted by a worker restart): finish it.
        ... set the module-level killed boolean ...
        ... run the debugger detach sweep of section 2 step 3 ...
        return; // no connect; recovery is explicit
      }
      connect();
    }
    init();

This is what guarantees the debugger detach even if the service worker restarts
mid-kill: the marker was persisted before any detach began, and every worker start
re-runs the sweep until the user explicitly reconnects.

### 4. Extension: the popup kill control

If `extension/popup.html` does not exist (G10 has not landed), create it, create
`extension/popup.js`, and add to `extension/manifest.json`:

    "action": { "default_title": "Browser MCP", "default_popup": "popup.html" }

If G10 already created the popup, add the kill control to the existing files and do not
duplicate the `action` key. Either way the kill control is visually and functionally
distinct from any pause control: its own button, never a shared toggle, destructive
(red) styling. MV3 forbids inline scripts: `popup.html` loads `popup.js` via
`<script src="popup.js"></script>`; styling is a small inline `<style>` block or plain
attributes, no frameworks, no external assets.

Popup <-> worker protocol (three `chrome.runtime.sendMessage` message types, handled by
a `chrome.runtime.onMessage` listener in the service worker; the listener must call
`sendResponse` and `return true` for async handling -- returning a Promise does not
deliver a response in Chrome MV3):

- `{ type: "GET_SESSION_STATE" }` -> respond
  `{ killed: <bool>, connected: <bool>, attachedTabs: <number> }` where `killed` is the
  storage marker, `connected` is `nativePort !== null`, and `attachedTabs` is
  `attached.size`. Mechanism facts only.
- `{ type: "KILL_SESSION" }` -> run `killSession()` (section 2), then respond with the
  new session state object.
- `{ type: "RECONNECT_SESSION" }` -> `await
  chrome.storage.session.remove("session_killed")`, clear the module-level killed
  boolean, call `connect()`, then respond with the new session state object.

The listener ignores (returns without responding to) any other message type, so it
cannot interfere with G10's messages or future ones.

Popup rendering (`popup.js` requests `GET_SESSION_STATE` on open and re-renders after
every action):

- Active view (killed false): a status line -- exactly `Connected to the binary.` or
  `Not connected to the binary.` -- plus exactly `Debugger attached to N tab(s).` with
  N substituted; and the kill button, id `kill-button`, label exactly
  `End session now`. ONE click runs the kill. No confirmation dialog, no hold-to-
  confirm, no second step: this is a panic control and the title of this task is "one
  gesture".
- Killed view (killed true): text exactly
  `Session ended. Browser access is severed until you start a new session.` and a
  button, id `reconnect-button`, label exactly `Start new session`, which sends
  `RECONNECT_SESSION`.

The popup makes no policy decisions, renders no governance state, and does not talk to
the binary; it only messages its own service worker. The shared-format section 9
settings protocol (`get_status` / `get_config` / `set_config_key`) is a DIFFERENT
surface owned by another task; do not implement or call any of it here.

### 5. Wire protocol: the `session_killed` event

Extension -> binary, over the existing native-messaging channel, relayed verbatim by
the native-host role:

    { "type": "session_killed" }

No `id` field: it is an event, not a tool reply. Update the module docs in
`src/native/messages.rs` to document it (one line in the extension -> binary section
plus a sentence: an event without an `id`; the mcp-server marks the session killed,
fails all in-flight and subsequent tool calls until a fresh native-host connection
attaches, and writes one audit record). No change to the framing, the IPC transport, or
`relay_native_host`.

### 6. Binary: killed state on the Browser handle

In `src/browser.rs`:

- Add a `killed: Arc<AtomicBool>` field to `Browser` (initialized false in the
  constructors).
- In `route_reply`, BEFORE the existing early return for id-less messages, recognize
  the event: if the parsed reply has no `id` and its `type` is `"session_killed"`,
  handle the kill and return. Handling the kill means, exactly once per false-to-true
  transition (use `swap` / `compare_exchange` so duplicate frames are idempotent):
  1. set `killed` to true;
  2. drain every pending call, failing each with the kill error of section 7;
  3. invoke the kill hook (below) so the mcp-server writes the audit record.
- Kill hook: add

      /// Register a hook invoked exactly once each time the extension reports the
      /// user ended the session (the "session_killed" event). The mcp-server role
      /// uses this to write the kill audit record.
      pub fn on_session_killed(&self, hook: impl Fn() + Send + Sync + 'static)

  backed by an `Arc<Mutex<Option<Box<dyn Fn() + Send + Sync>>>>` field. If no hook is
  registered, handling still sets the flag and drains pending. If the audit writer the
  flight-recorder task exposes is async, the hook body may send on an unbounded
  `tokio::sync::mpsc` channel drained by a task spawned where the hook is registered;
  the mechanism is the implementer's choice, but the record must be written exactly
  once per kill event and only by the mcp-server role.
- In `Browser::call`, as the FIRST check (before the pending-map insert and before the
  not-connected check), return the kill error when `killed` is true. Ordering matters:
  after the kill the port drops and `outgoing` becomes `None`; the killed check must
  win over the generic not-connected error, because the binary knows the real cause
  and the engine is truthful.
- In `Browser::attach`, at the top (a new native-host stream means the extension
  reconnected, which -- because of the section-1 gate -- only happens after the user's
  explicit reconnect or a full browser restart), reset `killed` to false. This is the
  "fresh session begins only when the user reconnects" rule on the binary side.
- The disconnect drain that already exists at the end of `attach` runs after a kill
  too; by then the pending map is already empty (the kill drained it), so no special
  casing is needed there.

Register the hook where the mcp-server role constructs the audit subsystem (re-read
the flight-recorder task's code to find the writer handle; today that wiring point is
`run_server` in `src/main.rs` / `mcp::server::run`).

Note: the killed flag lives in the mcp-server process and does not persist across an
MCP client restart. That is acceptable: after a restart the extension still refuses to
reconnect (the storage marker gates it), so calls fail with the not-connected error
until the user reconnects. Truthful either way.

### 7. Binary: the kill error

Built with release-1 T06's `ToolError`, exactly:

    ToolError::extension("The user ended the browser session (kill switch)")
        .next_step("ask the user to reconnect from the Browser MCP extension popup, then retry")

which renders, byte for byte:

    [hop: extension] The user ended the browser session (kill switch). Next step: ask the user to reconnect from the Browser MCP extension popup, then retry.

This exact error is used for (a) draining in-flight calls when the event arrives and
(b) every subsequent `Browser::call` while `killed` is true. `handle_tools_call`
renders it as an `isError: true` tool result through the existing T06 path; no change
in `src/mcp/server.rs` is expected beyond what the prerequisites already made.

The hop is `extension` because the severance happened at the extension hop, by the
user's hand; the message says so plainly. Never soften it to a generic connection
error, and never auto-retry.

### 8. Binary: the kill audit record

When the kill hook fires, the mcp-server appends exactly one record to the audit
destination resolved by the flight-recorder task (`audit.enabled`,
`audit.destination`, `audit.file.path`; shared-format sections 1.4 and 3.4). If
`audit.enabled` resolves false, no record is written (that is the user's or org's
configuration choice); also emit `tracing::info!("session killed by the user")` in
every case so the operational log has the event regardless.

The record is a session event, not a tool call. Shared-format section 6 defines
tool-call records; this record is additive to that stream and deliberately
distinguishable: it carries an `event` field and carries NO `tool`, `action`, `rw`,
`domain`, `decision`, `grant_id`, `denial_id`, or `duration_ms` field. Reuse the
section 6.1 definitions verbatim for the fields it shares. Fields, in this insertion
order (serde_json is built with `preserve_order`, so insertion order is serialization
order):

| Field | Value |
|---|---|
| `event_id` | UUID v4, lowercase, hyphenated (section 6.1). |
| `ts` | RFC 3339 UTC timestamp with millisecond precision (section 6.1). |
| `identity` | per section 6.1: from the active manifest's `identity` block, or `null`. |
| `client` | per section 6.1: from the MCP `initialize` `clientInfo`, or `null`. |
| `event` | the exact string `"session_killed"`. |
| `manifest` | per section 6.1: `{ name, version, hash }` of the active manifest, or `null`. |

One compact JSON object, one line, LF-terminated, appended like every other record. If
the flight-recorder task already landed a session-event record mechanism with its own
discriminator, reuse it instead of inventing this shape twice -- but the `event` value
`"session_killed"` and the shared fields above are fixed either way. Downstream
consumers that expect tool-call records (`policy simulate`, the activity ledger) must
skip records that carry an `event` field; if such a consumer already exists when G11
lands, add that skip with a one-line comment, otherwise leave it to those tasks.

If the extension could not signal (the binary was not running at kill time), no record
is written; the binary cannot log what it never saw, and the extension never writes
audit records (SPEC 7.4 trust boundary; shared-format section 6).

### 9. Tests

Rust tests are feasible for the whole binary side; extension behavior gets the manual
script in Verification. Add at minimum:

1. `src/browser.rs` unit tests, using the existing duplex-stream fake-extension
   pattern (`tokio::io::duplex`, `host::write_message`):
   - kill fails in-flight calls: attach a fake extension; start a `browser.call` on a
     spawned task; have the fake read the request and then send the frame
     `{"type":"session_killed"}` (no id); assert the call fails and the error string
     starts with `[hop: extension]` and contains
     `The user ended the browser session (kill switch)`.
   - kill fails subsequent calls fast: after the kill frame, a new `browser.call`
     fails with the same message without any frame being sent to the fake and without
     waiting on `TOOL_TIMEOUT` (wrap in a short `tokio::time::timeout`, for example
     1s, to prove it fails immediately).
   - the kill error beats the not-connected error: after the kill frame AND after the
     stream is dropped (attach returned), `browser.call` still reports the kill
     message, not `not connected`.
   - fresh attach clears the kill: after a kill, attach a NEW duplex stream with a
     fake that answers normally; assert a call round-trips with its normal result.
   - the hook fires exactly once: register a hook incrementing an
     `Arc<AtomicUsize>`; send `{"type":"session_killed"}` twice on the same
     connection; assert the counter is 1 and no panic occurred.
2. An audit test at the seam the flight-recorder task provides (unit test against its
   writer if it is testable in-process, otherwise an integration test in `tests/` with
   `audit.file.path` pointed at a temp file): trigger the kill hook path and assert
   exactly one appended line parses as JSON with `event == "session_killed"`, a
   36-char lowercase `event_id`, an RFC 3339 `ts`, and the `identity` / `client` /
   `manifest` fields present (null is fine), and that the line contains NO `tool` and
   NO `decision` field.
3. All-open invariant: the existing protocol tests (`tests/mcp_protocol.rs`) and
   `tests/tool_schema_fidelity.rs` pass unchanged; no new test may require a manifest
   or any config to exercise the kill path.

## Constraints

1. NEVER modify `src/mcp/schemas/tools.json`, tool names, parameters, or description
   strings. `tests/tool_schema_fidelity.rs` must pass unchanged. G11 changes no tool
   schema text and does not add or remove tools. (`extension/manifest.json` is NOT the
   sacred file; adding the `action` key there is required and allowed.)
2. The extension holds mechanism only: no policy, access, or redaction decisions in
   extension JS. The kill switch qualifies because the extension severs only its OWN
   debugger attachments and its OWN native port at the user's direct gesture; it
   decides nothing about domains, tools, or grants. The popup renders mechanism facts
   (connected, attached-tab count, killed) and nothing else.
3. All-open stays first-class: with no manifest and default config, and absent a kill
   gesture, behavior is byte-identical to today. The kill switch itself works
   identically with and without a manifest and in every governance mode; it is never
   gated on policy.
4. ASCII only in all code and docs: no em-dashes, no arrows, no curly quotes, anywhere,
   including comments, popup text, and error strings. Use ` -- ` where the codebase
   uses it.
5. The engine is truthful: after a kill the binary reports the real cause with the
   exact section-7 string; it never auto-retries, never auto-reconnects, and never
   reports a generic failure when it knows the user ended the session. The extension
   never reconnects on its own while the marker is set. Recovery happens only through
   the explicit `Start new session` gesture (or a full browser restart, which is a
   physically fresh session).
6. No new runtime dependencies. `uuid` and the RFC 3339 time source arrive with the
   earlier stage-2 audit task; G11 adds nothing to `Cargo.toml`, including
   dev-dependencies. The extension stays vanilla JS: no bundler, no libraries, no
   framework in the popup.
7. Rust 2021 edition; `thiserror` stays the error mechanism (reuse T06's `ToolError`;
   define no new error enum); doc comments on every new public item; `cargo fmt`
   clean; `cargo clippy --all-targets -- -D warnings` clean. Unit tests inline,
   integration tests in `tests/`.
8. Do NOT copy code from the official Anthropic extension or any other project;
   implement the behavior described here from scratch.

Task-specific:

9. One gesture means one gesture: no confirmation dialog, no double-click requirement,
   no hold-to-confirm on `kill-button`.
10. Ordering inside `killSession()` is fixed: marker first, signal second (with the
    100 ms settle), detach third, state-clear fourth, port teardown last. The marker-
    first rule is the mid-kill restart guarantee; the signal-before-teardown rule is
    what makes the binary's attribution reliable.
11. The kill never closes, ungroups, or navigates any tab, never deletes the tab
    group, and never touches the user's browser beyond detaching the extension's own
    debugger attachments.
12. The killed check in `Browser::call` precedes the not-connected check; the killed
    flag transitions are idempotent (duplicate `session_killed` frames are harmless);
    the audit record and the hook fire exactly once per kill event.
13. Exact strings are a contract: the wire event `{"type":"session_killed"}`, the
    storage key `session_killed`, the audit `event` value `session_killed`, the
    popup message types `GET_SESSION_STATE` / `KILL_SESSION` / `RECONNECT_SESSION`,
    the button labels `End session now` and `Start new session`, the popup texts of
    section 4, the extension-side refusal string
    `The user ended the browser session (kill switch)`, and the rendered binary error
    of section 7. Produce them byte for byte.
14. No change to the IPC transport, `relay_native_host`, the native-host zombie-fix
    `std::process::exit(0)`, the keepalive alarm period, `TOOL_TIMEOUT`, or the
    installer.

## Verification

1. From the repo root: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and
   `cargo test` are all clean, including the new `src/browser.rs` kill tests and the
   unchanged `tests/tool_schema_fidelity.rs`.
2. Rebuild the binary. If `target/debug/browser-mcp.exe` is locked by a running
   session, rename it aside first (for example
   `mv target/debug/browser-mcp.exe target/debug/browser-mcp.exe.old-1`) and rebuild.
   Binary changes require an MCP client restart; extension changes require a reload at
   chrome://extensions (and the popup files are picked up by the same reload).
3. Manual kill, mid-flight (the core scenario):
   - Start a session from the MCP client: `tabs_create_mcp`, `navigate` to any http(s)
     page, `computer` `screenshot`. Chrome shows the "is debugging this browser"
     infobar (the debugger is attached).
   - Issue a slow call (for example `computer` with action `wait` and duration 20),
     and while it is in flight, click the Browser MCP toolbar icon and click
     `End session now` once.
   - Confirm, in order: the debugger infobar disappears (detach happened); the
     in-flight call returns exactly the section-7 error text; a further tool call
     returns the same text immediately (no 60s timeout); the popup now shows
     `Session ended. Browser access is severed until you start a new session.`
4. Audit: with `audit.enabled` resolving true, open the resolved audit file
   (shared-format 1.4 default:
   `%LOCALAPPDATA%\browser-mcp\audit.jsonl` on Windows) and confirm the last line is a
   compact JSON object with `"event":"session_killed"` and the section-8 field set,
   and no `tool` or `decision` field.
5. Mid-kill service-worker restart guarantee: kill the session, then force the worker
   down (chrome://extensions, the extension's "service worker" link, or wait for MV3
   idle teardown), then wake it (open the popup). Confirm the popup still shows the
   killed view, the worker did NOT reconnect (no debugger infobar reappears; a tool
   call from the client still fails), and the keepalive alarm firing does not
   reconnect either (wait at least one 24s alarm period).
6. Explicit recovery: click `Start new session`. Confirm the extension reconnects
   (within a few seconds), a `tabs_context_mcp` call from the client succeeds, and a
   fresh `navigate` + `screenshot` flow works end to end. Confirm the binary error is
   gone (calls no longer report the kill message).
7. Kill with the binary down: quit the MCP client (no mcp-server running), click
   `End session now`. Confirm the popup shows the killed view and no error surfaces;
   confirm on restart of the MCP client that tool calls fail with the not-connected
   error (the extension refuses to reconnect) until `Start new session` is clicked.
8. All-open invariant: without ever touching the kill button, run the normal flow
   (`tabs_create_mcp`, `navigate`, `screenshot`, `read_page`): every result is
   byte-identical to the pre-G11 build.

## Out of scope

- Pause / take-the-wheel (G10). No pause button, no pause state, no resume-from-pause
  semantics, no honoring of a pause mid-action. If G10's popup already exists, add the
  kill control beside its controls without modifying them. Kill and pause never share
  a control.
- Closing user tabs, removing tabs from the Browser MCP tab group, deleting the group,
  navigating any tab (including to about:blank), or otherwise altering the user's
  browser state beyond detaching the extension's own debugger attachments.
- Uninstalling or deregistering anything: no native-messaging host deregistration, no
  MCP client config changes, no extension disable/uninstall, no touching
  `src/install/`.
- Killing OS processes. The mcp-server process stays alive (it must, to answer with
  the truthful kill error), and the native-host exits only through the existing
  disconnect path. No signal handling, no process management.
- The shared-format section 9 settings protocol (`get_status` / `get_config` /
  `set_config_key`), the options page, and any binary <- popup communication. The
  popup talks only to its own service worker in this task.
- Sacred domains, manifests, grants, denial ids, and denial messages. A kill is not a
  policy denial: it produces no `denial_id`, no `Denied (D-...)` text, and no
  `decision` field. Do not route the kill through `policy_check`.
- Any change to `tools/list`, tool routing, tool result text on the normal path, or
  `src/mcp/schemas/tools.json`.
- Persistence of the kill across full browser restarts (`chrome.storage.local` /
  `sync`), multi-profile coordination, or any enterprise-forced kill. This is the
  user's session-scoped panic control.
- New dependencies in `Cargo.toml` (including dev-dependencies) and any extension
  build tooling.
- Changes to the IPC transport, `relay_native_host`, the zombie-fix exit, the
  keepalive alarm period, `TOOL_TIMEOUT`, the reconnect retry delays, or the debug /
  observability subsystem.
