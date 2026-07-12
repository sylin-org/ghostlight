# ADR-0058: Per-browser identity, focus-chain routing, and composite tab identifiers

## Status

Accepted. Amends ADR-0030 Decision 1 (PINS.md SS1): the extension endpoint's "no hello,
server-speaks-first, singleton peer" contract is repealed for the singleton assumption only: the
extension is no longer assumed to be one process for the service's whole lifetime.

## Context

The service has always assumed exactly one browser is attached. `Browser` (`crates/core/src/hub/
outbound/browser.rs`) holds a single `outgoing: Arc<Mutex<Option<Sender>>>` slot; `Browser::attach`
enforces this with an atomic claim, rejecting a second connection outright
(`AttachOutcome::AlreadyAttached`) rather than admitting it. `chrome.runtime.connectNative`'s native
host id (`org.sylin.ghostlight`) is the same for every Chrome/Edge profile, so nothing distinguishes
one browser instance from another even at the transport level. ADR-0030 Decision 1 made this
explicit and deliberate: "the extension is a singleton, spoken-to, sacred-wire peer" -- the reason
the extension endpoint carries no hello, unlike the adapter/control endpoint (PINS.md SS1).

This broke down during 2026-07-11 live-testing of the denial-notification feature (see
`docs/adr/0018*` g11 kill-switch background and this session's connectivity debugging): a stray
service instance from an earlier debugging session held a live browser attachment that a second,
intended service instance could never observe, and there was no way to tell -- from `ghostlight
doctor` or from the wire -- that two independent, valid attachments existed at all, or which one a
given MCP client's tool call would actually reach. Separately, the underlying user need is real and
common: a user may run more than one browser profile (or more than one browser entirely) with the
extension loaded, and today the system cannot address them independently -- the second one is
silently dropped.

## Decision

### 1. Identity: a session-hello on the extension endpoint

The extension endpoint gains a hello frame, following the SAME envelope shape the adapter/control
endpoint already uses (`crates/transport/src/handshake.rs`), with a new role:

```json
{ "hub": 1, "role": "browser", "relayPid": <u32>, "browserPid": <u32>, "browserCreated": <u64> }
```

- `role`: a new PINNED constant `ROLE_BROWSER = "browser"`.
- `relayPid`: the relay process's own pid (`std::process::id()`), diagnostic only.
- `browserPid` / `browserCreated`: the relay's PARENT process identity -- i.e. the browser
  (Chrome/Edge) that spawned it via `connectNative` -- captured with the EXISTING
  `ghostlight_transport::proc::parent()` / `ProcId { pid, created }` the agent role already uses for
  its own parent-death watchdog (`crates/relay/src/main.rs::run_agent`). Reusing `ProcId` (not a
  bare pid) means the same creation-time match `ghostlight doctor` already relies on
  (`crates/core/src/hub/manage/doctor.rs::classify`) also protects browser identity against PID
  reuse: a dead browser's pid, reused by an unrelated process, is never mistaken for a live session.

The relay sends this ONE frame immediately after connecting, before the generic byte-relay loop
starts (`ipc::relay_native_host_over`). The service's `Browser::attach` reads it FIRST -- the SAME
"peer speaks first" pattern H2 already established for the adapter/control endpoint (PINS.md SS1)
-- before admitting the connection into the session map. This is the part of ADR-0030 Decision 1
this ADR repeals: the extension endpoint is no longer server-speaks-first-only, because it is no
longer assumed to have exactly one possible peer. `host.rs`'s 4-byte-LE framing itself is
UNCHANGED; the hello rides on top of it exactly like the adapter/control hello does.

A hello for a `browserPid` already present in the session map REPLACES the existing entry (does not
reject): a service-worker relaunch, extension reload, or overlapping reconnect from the SAME browser
is a fresh, more-authoritative session for that identity, not a stray. A hello for a NEW `browserPid`
is ADDED as a new, independent session -- this is the actual multi-browser support.

### 2. Lifecycle: a parent-death watchdog on the browser-role relay

`crates/relay/src/main.rs::run_browser` gains the identical watchdog treatment `run_agent` already
has: capture `proc::parent()` at startup, race `watchdog::wait_until_orphaned(parent)` against the
relay loop in a `tokio::select!`. If the browser process itself dies, the relay exits immediately on
a positive signal, rather than depending solely on stdin/pipe EOF detection (today's only signal,
and the weaker of the two: "the process that spawned me is gone" is more direct than "my pipe
stopped producing bytes").

### 3. Routing: a session map + a focus-chain tie-breaker

`Browser::outgoing: Arc<Mutex<Option<Sender>>>` becomes a map keyed by `browserPid`:

```rust
struct BrowserSession {
    sender: mpsc::UnboundedSender<Vec<u8>>,
    created: u64,   // ProcId.created, for stale-pid-reuse detection
    generation: u64, // this attach()'s own monotonic id, for safe self-removal on disconnect
}
sessions: Arc<Mutex<HashMap<u32, BrowserSession>>>,
focus_chain: Arc<Mutex<Vec<u32>>>, // front = most recently focused, still-attached browser
```

`Browser::is_connected()` becomes "at least one session is present." A session's reader loop, on
exit, removes its OWN entry from `sessions` (and from `focus_chain`) only if the map's current entry
for that `browserPid` still carries ITS `generation` -- guarding the ABA case where a reconnect from
the same browser has already replaced the entry by the time the old reader loop notices its stream
died. This is the standard compare-before-remove pattern; no new primitive is invented.

`call()`, `notify()`, `request_group()`, `tab_url()`, and `send_and_await()` all gain a target
`browserPid` argument (or resolve one internally). Resolution order, applied ONCE at the single true
dispatch entry point (`crate::mcp::pipeline::run_tool_call`):

1. If the inbound `args.tabId` is present, DECODE it (see 4 below) to get `(browser_pid,
   native_tab_id)` directly -- the call is inherently addressed to whichever browser owns that tab.
2. Otherwise (no tabId yet -- `tabs_create_mcp`/`tabs_context_mcp` with `createIfEmpty`, or
   `navigate`'s auto-bootstrap with no managed tabs yet): use the front of `focus_chain`, skipping
   any entry no longer present in `sessions`.
3. If the chain is empty (no browser has ever reported focus) but exactly one session is attached,
   use it -- the common single-browser case must never depend on focus-event plumbing working.
4. If multiple sessions are attached and none has ever reported focus, fall back to
   `sessions`'s arbitrary-but-deterministic iteration order (documented as best-effort; a real
   ambiguity in this state is inherent, not a bug to chase further).

Focus itself: the extension listens for `chrome.windows.onFocusChanged` and sends a new fire-and-
forget wire message, `{ "type": "focus" }`, whenever one of ITS OWN windows gains focus (chosen over
OS-level window z-order specifically to avoid unsafe, platform-specific window enumeration --
Win32 `GetForegroundWindow`, Cocoa, and X11/Wayland compositor APIs that in several Wayland setups
refuse to expose global window state at all; `chrome.windows.getLastFocused`/`onFocusChanged` answer
the same question from inside the one process that already knows it, portably, with no unsafe code).
No `browserPid` field is needed in the message itself -- it travels over an already-identified
session, so the service already knows which browser sent it. Blur is never reported and never
tracked: only "gained focus" events move an entry to the front of the chain; the chain's own
recency ordering already answers "who was focused most recently, among those still alive" without
needing to model "currently focused" as a separate boolean. On attach, before any `onFocusChanged`
event may ever fire again, the extension checks `chrome.windows.getLastFocused()` once and reports
its focus state immediately -- covering the cold-start case (a browser that was already focused
before it connected).

### 4. Composite tab identifiers, encoded as a JSON number

`tabId` is `"type": "number"` in the frozen tool schemas (`crates/core/src/browser/directory.rs`,
multiple sites) and every existing call site parses it with `Value::as_i64()`. ADR-0034 Decision 7
permits additive tools/parameters but forbids changing an existing trained field's type or format --
a string-composite id (`"12345-5"`) would violate that. Instead the browser identity is encoded
ARITHMETICALLY into the same JSON number space (`crates/core/src/constants.rs::tab_id`):

```rust
pub const MULTIPLIER: i64 = 1i64 << 32; // 2^32
pub fn encode(browser_pid: u32, native_tab_id: i64) -> i64 {
    (browser_pid as i64) * MULTIPLIER + native_tab_id
}
pub fn decode(composite: i64) -> (u32, i64) {
    ((composite / MULTIPLIER).max(0) as u32, composite.rem_euclid(MULTIPLIER))
}
```

`MULTIPLIER` was originally pinned at `10_000_000`, on the assumption that Chrome's tab id is a
small per-launch counter. LIVE VERIFICATION (2026-07-11, against a real browser) disproved that:
the extension reported a native tab id of `1_246_199_197` -- over a billion -- which overflowed
past the old bound and corrupted the decoded pid (`21840` decoded back as `21964`, silently
misrouting a `navigate` call to a "browser not connected" error against a tab whose real owner
was, in fact, still attached). Chrome's tab id is a signed 32-bit int internally, not a small
counter; `MULTIPLIER = 2^32` covers its FULL possible range cleanly, and still keeps
`browser_pid * MULTIPLIER` within JavaScript's safe-integer range (2^53) for any pid up to
`~2.1 million` -- far above any pid a real OS assigns. The model never sees or needs to understand
this: nothing in the trained schema or its description promises a specific magnitude, and the
model was only ever trained to copy `tabId` verbatim between calls, never to reason about its
value.

Decode and routing happen INSIDE `Browser::call`/`notify`/`tab_url`/`request_group` themselves
(`crates/core/src/hub/outbound/browser.rs`), not at `pipeline.rs`'s entry: each of these methods
decodes whatever composite tabId it was given FRESH, every call, from the caller's own
still-composite argument -- never rewritten in place into a shared `args` value. This is what
keeps a `browser_batch`/`script` sub-step safe on re-entry: `Browser::call` builds a LOCAL,
owned copy of `args` with the tabId field replaced by the decoded NATIVE value for the actual
wire send, while the caller's own `args` reference (which a recursive re-entry might reuse for
its own default tabId) is never touched. The extension (mechanism-only, per the architecture's
standing rule) never learns the encoding exists -- it only ever receives native ids.

Encode happens on every extension RESPONSE, generically, inside `Browser::call` (the
`encode_tab_ids`/`encode_tab_ids_in_value` helpers): it walks the result `Value` (both
`structuredContent` and any `content[].text` block that parses as JSON) and rewrites every
`"tabId"` key holding a plain number to its composite form using the call's own resolved
`browser_pid`, before returning to the caller. A generic walker (rather than hand-editing
`tabs_create_mcp`/`tabs_context_mcp` specifically) is deliberate: it covers every current
tabId-reporting tool and any future one without a matching manual edit. `gif_creator.rs` is the
one caller with its OWN local state (`RecordingStore`) keyed by the extension's native tab id
(fed by native `gif_frame` events); it decodes locally for that bookkeeping while still passing
the original composite value into `Browser::call`, which decodes it again independently -- cheap,
and correctness-safe because decode never mutates the value it reads.

### 5. `ghostlight doctor`: list every connected browser

The control-channel `StatusReply` (`crates/transport/src/ipc.rs`) gains a `browsers: Vec<BrowserInfo>`
field (`BrowserInfo { pid: u32, focused: bool }`), replacing the single `extension_connected: bool`
as the SOURCE of truth (`extension_connected` stays, derived as `!browsers.is_empty()`, for wire
back-compat with an older client reading a newer service's reply mid-upgrade). `doctor`'s "IPC
endpoint" section renders a "Browsers:" sub-list, one line per attached browser (pid, focused or
not), instead of the single yes/no line. A live tab count per browser is deliberately not included:
the service has no source for it without a round-trip doctor's synchronous one-shot query does not
make -- a future addition if wanted, not a gap in this pass.

## Explicitly out of scope for this change

`Browser::held` (take-the-wheel pause, g10), `Browser::killed` (panic kill switch, g11),
`screenshot_cache`, `kill_hooks`, and gif-recording state all remain GLOBAL -- shared across every
attached browser, exactly as today -- rather than becoming per-browser-session state. Making the
panic kill switch or take-the-wheel pause target one specific browser instead of the whole process is
a real, separate product question (does killing browser A's session also sever browser B's?) that
this ADR does not answer; it is left for a follow-up pass if multi-browser use surfaces a concrete
need for it. This keeps the current change scoped to identity, routing, and diagnostics only.

## Consequences

- A second (or third) simultaneously-attached browser is now possible and independently addressable,
  where today it is silently dropped.
- `ghostlight doctor` can finally answer "which browser(s) is this actually talking to," closing the
  exact diagnostic gap that made 2026-07-11's connectivity debugging slow.
- The extension endpoint's wire contract changes for the first time since ADR-0030: existing
  fake-extension test doubles that connect without sending a hello now fail to attach, and every
  test exercising `Browser::attach` needs updating to send one. This is the expected, bounded cost
  of repealing the singleton assumption.
- `held`/`killed`/screenshot-cache/gif-recording staying global is a known, intentional limitation
  of this pass, not an oversight -- see "Explicitly out of scope" above.
