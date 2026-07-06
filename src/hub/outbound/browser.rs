// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The `Browser` handle -- the mcp-server's view of the connected browser extension.
//!
//! A tool call becomes a framed request sent to the extension (through the native-host instance
//! over the local IPC) and a correlated response, awaited by id. This module is transport-agnostic:
//! [`Browser::attach`] takes any async duplex stream -- a real IPC connection in production, an
//! in-memory pipe in tests -- so the correlation logic is verifiable without a browser.
//!
//! Wire protocol (see also `transport/native/messages.rs`): the mcp-server sends
//! `{ "id", "type": "tool_request", "tool", "args" }`; the extension replies with
//! `{ "id", "type": "tool_response", "result" }` or
//! `{ "id", "type": "tool_error", "error", "hop"?, "detail"? }`. A `tool_error` is mapped to a
//! hop-attributed [`ToolError`] (see [`ToolError::from_extension_wire`]); `detail`, if present, is
//! logged with `tracing::debug!` and never reaches the tool result. Messages without an `id`
//! (events, heartbeats) are ignored here (Phase 3 buffers events).
//!
//! Tab-URL query (g13, [`Browser::tab_url`]): the mcp-server sends
//! `{ "id", "type": "tab_url_request", "tabId" }`; the extension replies with
//! `{ "id", "type": "tab_url_response", "result": { "url" } }`. This routes through the same
//! `pending` map and generic reply path as a tool call (any non-`tool_error` reply already
//! becomes `Ok(result)`); mechanism only, feeding the dispatch chokepoint's grant enforcement --
//! never a decision made by the extension.
//!
//! Take-the-wheel hold (g10, ADR-0018 step 2): the extension's popup/shortcut sends
//! `get_hold` / `set_hold` / `toggle_hold` requests over the same channel; [`Browser`] holds
//! the flag (mcp-server process memory only -- no disk persistence, no survival across a
//! restart, and NOT cleared by an extension disconnect/reconnect) and answers with a
//! `hold_state` (or `hold_error`) reply. The dispatch chokepoint (`transport::mcp::server`)
//! checks [`Browser::held_for`] before any policy or extension traffic; the flag itself
//! carries no policy meaning here, only a user gesture the chokepoint acts on.
//!
//! Panic kill switch (g11, ADR-0018 step 2): the extension signals `{"type":"session_killed"}`
//! (an event, no `id`) once it has severed its own debugger attachments and is tearing down the
//! native port. [`Browser`] latches a `killed` flag (idempotent: only the false-to-true
//! transition acts), fails every pending and future call with the truthful
//! `"The user ended the browser session (kill switch)"` [`ToolError`], and invokes every
//! registered kill hook exactly once per transition (a fan-out registry, ADR-0030 Decision 7: one
//! `session_killed` audit record per LIVE session's subject, since `held`/`killed`/`connected`
//! stay global on this one shared handle while sessions multiplex over it). A fresh
//! [`Browser::attach`] (only reachable after the extension's own storage-marker gate lets it
//! reconnect) clears the flag: a fresh session begins only on the user's explicit reconnect.
//!
//! Tab-group-per-session request ([`Browser::request_group`], H7, ADR-0030 Decision 6/7): the
//! mcp-server sends `{ "type": "group_request", "guid", "tabIds", "title" }`; the extension
//! groups exactly the named tabIds into that session's Chrome tab group and replies
//! `{ "type": "group_response", "guid", "ok" }`. Fire-and-forget -- neither message carries an
//! `id`, so no caller awaits a reply; [`Browser::route_reply`] drops an incoming `group_response`
//! as an ordinary id-less event, same as any other frame nothing is waiting for.

use crate::debug::DebugSink;
use crate::transport::native::host;
use crate::ToolError;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot, watch};

/// A kill hook: `Fn`, not `FnOnce`, because it is stored and may (in principle) be invoked more
/// than once across the `Browser`'s lifetime -- once per kill event, across however many kills
/// a single mcp-server process observes (each preceded by a fresh reconnect that clears the
/// flag). The false-to-true transition guard in [`Browser::route_reply`] is what makes each
/// individual kill fire it exactly once.
type KillHook = Box<dyn Fn() + Send + Sync>;

/// The kill-hook fan-out registry (ADR-0030 Decision 7): every live session's subject gets
/// exactly one `session_killed` audit record, keyed by an opaque monotonic id so a session-scoped
/// registration ([`Browser::register_session_kill_hook`]) can remove exactly its own entry when
/// its [`KillHookHandle`] drops. A permanent hook registered via [`Browser::on_session_killed`]
/// is never removed.
type KillHooks = Arc<Mutex<Vec<(u64, KillHook)>>>;

/// How long to wait for the extension to answer a single tool call before giving up.
const TOOL_TIMEOUT: Duration = Duration::from_secs(60);

/// Bounded reconnect grace window (ADR-0030 Decision 3, "D1 -- the honest singleton queue":
/// "truthful failure on a real drop"; PINNED in PINS.md SS4). STRICTLY LESS THAN
/// [`TOOL_TIMEOUT`]: a brief extension disconnect HOLDS the session's pending calls awaiting
/// reconnect instead of failing them the instant the stream closes; only a REAL drop (this
/// window elapsing with no reconnect) fails them, with the unchanged disconnect error text.
pub const GRACE_WINDOW: Duration = Duration::from_secs(10);

/// The truthful, hop-attributed error for every call while [`Browser::is_killed`] is true
/// (g11): the user severed the session; never a generic connection failure.
fn kill_error() -> ToolError {
    ToolError::extension("The user ended the browser session (kill switch)")
        .next_step("ask the user to reconnect from the Ghostlight extension popup, then retry")
}

/// Delivered to a waiting caller: `Ok(result)` or `Err(hop-attributed tool error)`.
type CallResult = std::result::Result<Value, ToolError>;
type Pending = Arc<Mutex<HashMap<String, oneshot::Sender<CallResult>>>>;

/// The outcome of [`Browser::attach`]: whether this connection became the active session or was
/// rejected because one is already attached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum AttachOutcome {
    /// This connection was the active session and has now detached (its stream closed).
    Detached,
    /// A session was already attached; this stray/extra connection was dropped without touching any
    /// `Browser` state.
    AlreadyAttached,
}

/// A session-scoped kill-hook registration (ADR-0030 Decision 7). Dropping the handle
/// unregisters the session's hook, so a session that has already ended records nothing on a
/// later kill. Returned by [`Browser::register_session_kill_hook`]; a live session holds it for
/// its whole lifetime.
#[must_use = "dropping the handle immediately unregisters the session kill hook"]
pub struct KillHookHandle {
    kill_hooks: KillHooks,
    id: u64,
}

impl Drop for KillHookHandle {
    fn drop(&mut self) {
        self.kill_hooks
            .lock()
            .unwrap()
            .retain(|(id, _)| *id != self.id);
    }
}

/// A cloneable handle the mcp-server uses to call tools on the extension.
#[derive(Clone)]
pub struct Browser {
    next_id: Arc<AtomicU64>,
    pending: Pending,
    /// `Some` when a native-host (and thus the extension) is connected; `None` otherwise.
    outgoing: Arc<Mutex<Option<mpsc::UnboundedSender<Vec<u8>>>>>,
    /// Readiness signal: `true` while a native-host / extension is attached. Lets callers await
    /// connectedness (see [`Browser::wait_connected`]) instead of polling [`Browser::is_connected`].
    connected: Arc<watch::Sender<bool>>,
    /// Observability sink (no-op unless debug mode is on).
    debug: DebugSink,
    /// Take-the-wheel hold (g10): `None` while not held; `Some(t)` since the instant the user
    /// engaged it. Process memory only -- never persisted, never cleared by a disconnect.
    held: Arc<Mutex<Option<Instant>>>,
    /// Panic kill switch (g11): `true` once the extension has reported the user ended the
    /// session, until the next [`Browser::attach`] (a fresh, explicit reconnect) clears it.
    killed: Arc<AtomicBool>,
    /// The kill-hook fan-out registry (ADR-0030 Decision 7): every entry fires exactly once per
    /// false-to-true `killed` transition. Starts empty until [`Browser::on_session_killed`] or
    /// [`Browser::register_session_kill_hook`] appends one.
    kill_hooks: KillHooks,
    /// Monotonic id source for `kill_hooks` entries (ADR-0030 Decision 7), so a
    /// [`KillHookHandle`] can remove exactly its own registration.
    next_hook_id: Arc<AtomicU64>,
}

impl Browser {
    /// Create a handle with no extension connected yet and debug disabled.
    pub fn new() -> Self {
        Self::with_debug(DebugSink::disabled())
    }

    /// Create a handle wired to an observability sink.
    pub fn with_debug(debug: DebugSink) -> Self {
        Self {
            next_id: Arc::new(AtomicU64::new(1)),
            pending: Arc::new(Mutex::new(HashMap::new())),
            outgoing: Arc::new(Mutex::new(None)),
            // Dropping the initial receiver is fine: updates use `send_replace`, which does not
            // require a live receiver (unlike `send`, which would fail and skip the update).
            connected: Arc::new(watch::channel(false).0),
            debug,
            held: Arc::new(Mutex::new(None)),
            killed: Arc::new(AtomicBool::new(false)),
            kill_hooks: Arc::new(Mutex::new(Vec::new())),
            next_hook_id: Arc::new(AtomicU64::new(1)),
        }
    }

    /// The observability sink (used by the mcp-server to record the MCP boundary).
    pub fn debug(&self) -> &DebugSink {
        &self.debug
    }

    /// Time since the take-the-wheel hold was engaged, or `None` while not held (g10).
    pub fn held_for(&self) -> Option<Duration> {
        self.held.lock().unwrap().map(|since| since.elapsed())
    }

    /// Set the hold flag and return the resulting state (g10). Setting `true` while already
    /// held is a no-op on the timer: the original engage instant is preserved (a repeated
    /// pause gesture must not reset the hint countdown). Logs exactly once per real
    /// transition, never on a no-op repeat.
    pub fn set_held(&self, held: bool) -> bool {
        let mut guard = self.held.lock().unwrap();
        let was_held = guard.is_some();
        if held && !was_held {
            *guard = Some(Instant::now());
            tracing::info!("user hold engaged");
        } else if !held && was_held {
            *guard = None;
            tracing::info!("user hold released");
        }
        held
    }

    /// Flip the hold flag atomically and return the new state (g10).
    pub fn toggle_held(&self) -> bool {
        let mut guard = self.held.lock().unwrap();
        let now_held = guard.is_none();
        if now_held {
            *guard = Some(Instant::now());
            tracing::info!("user hold engaged");
        } else {
            *guard = None;
            tracing::info!("user hold released");
        }
        now_held
    }

    /// True once the extension has reported the user ended the session (g11), until the next
    /// [`Browser::attach`] (a fresh, explicit reconnect) clears it.
    pub fn is_killed(&self) -> bool {
        self.killed.load(Ordering::SeqCst)
    }

    /// Register a PERMANENT hook invoked exactly once each time the extension reports the user
    /// ended the session (the `session_killed` event, g11): appended to the fan-out registry and
    /// never removed (ADR-0030 Decision 7, converting this from the pre-H2 single-consumer
    /// "registering a second hook replaces the first" behavior). Use
    /// [`Browser::register_session_kill_hook`] for a session-scoped registration that
    /// deregisters when the session ends.
    pub fn on_session_killed(&self, hook: impl Fn() + Send + Sync + 'static) {
        let id = self.next_hook_id.fetch_add(1, Ordering::Relaxed);
        self.kill_hooks.lock().unwrap().push((id, Box::new(hook)));
    }

    /// Register a REMOVABLE, session-scoped kill hook (ADR-0030 Decision 7): fires exactly once
    /// per false-to-true `killed` transition, same as [`Browser::on_session_killed`], but is
    /// deregistered as soon as the returned [`KillHookHandle`] drops -- so a session that has
    /// already ended records nothing on a later kill. `hold`/`killed`/`connected` stay GLOBAL
    /// (latched on this one shared `Browser`, never per session); only the audit-writing hook
    /// itself is session-scoped. A live session holds its handle for its whole lifetime.
    pub fn register_session_kill_hook(
        &self,
        hook: impl Fn() + Send + Sync + 'static,
    ) -> KillHookHandle {
        let id = self.next_hook_id.fetch_add(1, Ordering::Relaxed);
        self.kill_hooks.lock().unwrap().push((id, Box::new(hook)));
        KillHookHandle {
            kill_hooks: Arc::clone(&self.kill_hooks),
            id,
        }
    }

    /// True while a native-host / extension is connected.
    pub fn is_connected(&self) -> bool {
        self.outgoing.lock().unwrap().is_some()
    }

    /// Wait until a native-host / extension is attached, up to `timeout`. Returns `true`
    /// immediately when already connected, `true` when a connection arrives within the window,
    /// and `false` when the window elapses without one.
    pub async fn wait_connected(&self, timeout: Duration) -> bool {
        let mut rx = self.connected.subscribe();
        if *rx.borrow() {
            return true;
        }
        tokio::time::timeout(timeout, async {
            while rx.changed().await.is_ok() {
                if *rx.borrow() {
                    return true;
                }
            }
            false
        })
        .await
        .unwrap_or(false)
    }

    /// Invoke `tool` with `args` on the extension and await its result.
    ///
    /// Every failure is a hop-attributed [`ToolError`]: no extension connected, an encoding
    /// failure before the request left the process, the extension reporting a tool error (tagged
    /// `cdp`, `page`, or untagged and attributed to the `extension` hop), a mid-call disconnect,
    /// or a timeout.
    pub async fn call(&self, tool: &str, args: &Value) -> std::result::Result<Value, ToolError> {
        // The killed check precedes everything else, including the pending-map insert and the
        // not-connected check (g11 constraint 12): after a kill the port drops and `outgoing`
        // becomes `None`, so the generic not-connected error would otherwise win by accident.
        // The binary knows the real cause; the engine is truthful. No debug tool_begin/tool_end
        // pairing here: the call never began in any trackable sense.
        if self.killed.load(Ordering::SeqCst) {
            return Err(kill_error());
        }

        let id = self.next_id.fetch_add(1, Ordering::Relaxed).to_string();
        let request = json!({ "id": id, "type": "tool_request", "tool": tool, "args": args });
        let framed = match serde_json::to_vec(&request)
            .map_err(|e| e.to_string())
            .and_then(|bytes| host::encode(&bytes).map_err(|e| e.to_string()))
        {
            Ok(framed) => framed,
            Err(e) => {
                let err = ToolError::binary(format!("failed to encode the tool request: {e}"));
                self.debug.tool_begin(&id, tool);
                self.debug.tool_end(&id, false, &err.to_string());
                return Err(err);
            }
        };
        self.send_and_await(id, framed, tool).await
    }

    /// Query the current URL of tab `tab_id` from the extension (g13): mechanism only, reporting
    /// `chrome.tabs.get(tab_id).url` verbatim, never matched or interpreted here. The dispatch
    /// chokepoint uses this to resolve the governing domain for every tab-scoped tool other than
    /// `navigate`'s pre-check (which governs the target URL argument instead, before any tab
    /// exists to query) -- shared format doc section 4.3: the URL feeds policy only and is never
    /// trusted from tool call parameters. `Ok(None)` covers both an unknown/closed tab (the
    /// extension reports `url: null`) and a reply missing the expected shape; either way the
    /// caller fails closed.
    pub async fn tab_url(&self, tab_id: i64) -> std::result::Result<Option<String>, ToolError> {
        if self.killed.load(Ordering::SeqCst) {
            return Err(kill_error());
        }

        let id = self.next_id.fetch_add(1, Ordering::Relaxed).to_string();
        let request = json!({ "id": id, "type": "tab_url_request", "tabId": tab_id });
        let framed = match serde_json::to_vec(&request)
            .map_err(|e| e.to_string())
            .and_then(|bytes| host::encode(&bytes).map_err(|e| e.to_string()))
        {
            Ok(framed) => framed,
            Err(e) => {
                let err = ToolError::binary(format!("failed to encode the tab url request: {e}"));
                self.debug.tool_begin(&id, "tab_url_request");
                self.debug.tool_end(&id, false, &err.to_string());
                return Err(err);
            }
        };
        let result = self.send_and_await(id, framed, "tab_url_request").await?;
        Ok(result
            .get("url")
            .and_then(Value::as_str)
            .map(str::to_string))
    }

    /// Ask the extension to place `tab_ids` into `guid`'s Chrome tab group (H7, ADR-0030 Decision
    /// 6/7; PINS.md SS6). Fire-and-forget, the SAME posture `send_hold_reply` below uses: this
    /// is out-of-band PRESENTATION, never a tool call, so a missing connection or an encoding
    /// failure is a harmless no-op with nothing for a caller to await -- the pinned wire shape
    /// carries no `id` to correlate a `group_response` by, and [`Browser::route_reply`] already
    /// drops any id-less, non-`session_killed` frame as an ordinary event. `guid` is written
    /// verbatim into the outbound wire message (the pinned wire behavior itself) but MUST NOT be
    /// logged from this function or by any caller (ADR-0030 Decision 4: the GUID is secret
    /// material in every log/audit sink) -- and it is not: this function contains no `tracing`
    /// call naming any of its arguments.
    pub fn request_group(&self, guid: &str, tab_ids: &[i64], title: &str) {
        let request = json!({
            "type": "group_request",
            "guid": guid,
            "tabIds": tab_ids,
            "title": title,
        });
        let Ok(bytes) = serde_json::to_vec(&request) else {
            return;
        };
        let Ok(framed) = host::encode(&bytes) else {
            return;
        };
        if let Some(tx) = self.outgoing.lock().unwrap().as_ref() {
            let _ = tx.send(framed);
        }
    }

    /// Shared send-and-await core behind [`Browser::call`] and [`Browser::tab_url`] (g13):
    /// register the pending reply slot, enqueue the already-framed bytes if a native-host is
    /// connected (fail fast otherwise), and await the correlated reply up to [`TOOL_TIMEOUT`].
    /// Each caller frames its own request first, since their encode-failure messages differ.
    async fn send_and_await(&self, id: String, framed: Vec<u8>, debug_label: &str) -> CallResult {
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id.clone(), tx);
        self.debug.tool_begin(&id, debug_label);

        // Enqueue only if a native-host is connected; otherwise fail fast. The lock is scoped so it
        // is never held across the await below.
        let sent = {
            let outgoing = self.outgoing.lock().unwrap();
            match outgoing.as_ref() {
                Some(tx) => tx.send(framed).is_ok(),
                None => false,
            }
        };
        if !sent {
            self.pending.lock().unwrap().remove(&id);
            let err = ToolError::extension("Browser extension not connected");
            self.debug.tool_end(&id, false, &err.to_string());
            return Err(err);
        }
        self.debug.frame_out();

        let outcome = match tokio::time::timeout(TOOL_TIMEOUT, rx).await {
            Ok(Ok(Ok(result))) => Ok(result),
            Ok(Ok(Err(err))) => Err(err),
            Ok(Err(_closed)) => Err(ToolError::extension(
                "Browser extension disconnected before responding",
            )
            .next_step("retry the call; the extension reconnects automatically")),
            Err(_elapsed) => {
                self.pending.lock().unwrap().remove(&id);
                Err(ToolError::extension("Tool request timed out after 60s")
                    .next_step("check that Chrome is running and responsive, then retry"))
            }
        };
        match &outcome {
            Ok(v) => self.debug.tool_end(&id, true, &v.to_string()),
            Err(e) => self.debug.tool_end(&id, false, &e.to_string()),
        }
        outcome
    }

    /// Attach a connected native-host stream: spawn a writer draining outgoing frames to it and run
    /// a reader routing replies back to waiting callers.
    ///
    /// A single active session is enforced here with an atomic slot claim on `outgoing`. The first
    /// connection to arrive becomes the active session and returns [`AttachOutcome::Detached`] when
    /// its stream later closes. A connection that arrives while a session is already attached is a
    /// stray/extra one (a `doctor` probe, or a service-worker relaunch that overlaps the outgoing
    /// connection): it is rejected immediately with [`AttachOutcome::AlreadyAttached`] by dropping
    /// its stream halves (the peer then sees EOF and goes away) WITHOUT touching the live session's
    /// sender, connected flag, or pending calls. This is what lets
    /// [`crate::transport::native::ipc::serve`]
    /// accept connections ahead of time (spawning `attach` per connection) so the pipe always has a
    /// spare instance ready, instead of parking the accept loop for the whole session lifetime.
    ///
    /// On [`AttachOutcome::Detached`] the browser is marked disconnected and every pending call is
    /// failed. The single-slot claim is correct only because the reader loop below detects native-
    /// host death promptly (EOF/BrokenPipe, no heartbeat) and frees the slot on the way out.
    pub async fn attach<S>(&self, stream: S) -> AttachOutcome
    where
        S: AsyncRead + AsyncWrite + Send + 'static,
    {
        let (mut read_half, mut write_half) = tokio::io::split(stream);
        let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();

        // Atomic single-slot claim. If a session already holds the slot this is a stray/extra
        // connection: return without touching any `Browser` state. Dropping `read_half`/`write_half`
        // on return closes our end so the stray peer observes EOF. The guard is released at the end
        // of this block and is never held across an await.
        {
            let mut outgoing = self.outgoing.lock().unwrap();
            if outgoing.is_some() {
                return AttachOutcome::AlreadyAttached;
            }
            *outgoing = Some(tx);
        }
        // A new native-host stream becoming the live session means the extension reconnected --
        // which, because of the extension's own storage-marker gate, only happens after the
        // user's explicit reconnect or a full browser restart (g11). Either way that is a fresh
        // session: clear the kill flag.
        self.killed.store(false, Ordering::SeqCst);
        self.debug.set_connected(true);
        self.connected.send_replace(true);

        let writer = tokio::spawn(async move {
            while let Some(frame) = rx.recv().await {
                if write_half.write_all(&frame).await.is_err() || write_half.flush().await.is_err()
                {
                    break;
                }
            }
        });

        // Route replies until the stream closes cleanly (Ok(None)) or the transport errors
        // (Err(e)); the two are distinguished so pending calls learn WHY the loop ended.
        let drain_err = loop {
            match host::read_message(&mut read_half).await {
                Ok(Some(payload)) => {
                    self.debug.frame_in();
                    self.route_reply(&payload);
                }
                Ok(None) => {
                    break ToolError::extension("Browser extension disconnected before responding")
                        .next_step("retry the call; the extension reconnects automatically");
                }
                Err(e) => {
                    tracing::warn!(error = %e, "native-host stream read failed");
                    break ToolError::ipc(format!("IPC transport failed: {e}"));
                }
            }
        };

        *self.outgoing.lock().unwrap() = None;
        self.debug.set_connected(false);
        self.connected.send_replace(false);
        writer.abort();

        // ADR-0030 Decision 3 (H5): hold pending calls for a bounded grace window awaiting
        // reconnect instead of failing them the instant the stream closes. Spawned so `attach`
        // itself still returns `Detached` promptly regardless of the window's length -- neither
        // `ipc::serve`'s per-connection task nor any other caller here blocks on it.
        self.spawn_grace_drain(GRACE_WINDOW, drain_err);

        AttachOutcome::Detached
    }

    /// Hold pending calls for `window` awaiting reconnect (ADR-0030 Decision 3: "truthful failure
    /// on a real drop"). If [`Browser::wait_connected`] reports a reconnect (a fresh
    /// [`Browser::attach`] claims the slot again) within `window`, pending calls are left
    /// untouched -- each is still bounded by its own [`Browser::send_and_await`]/[`TOOL_TIMEOUT`].
    /// If `window` elapses with no reconnect, this IS a real drop: drain pending with `drain_err`,
    /// byte-identical to the pre-H5 immediate-fail error text, just delayed until the window has
    /// genuinely elapsed. A `session_killed` event during the window is unaffected: it can only
    /// arrive over a LIVE (reconnected) stream, and [`Browser::handle_session_killed`] already
    /// drains pending with [`kill_error`] independently and immediately -- if that already ran,
    /// this later, empty drain is a harmless no-op, and [`Browser::is_killed`] still wins for any
    /// subsequent call regardless of what this function does.
    fn spawn_grace_drain(&self, window: Duration, drain_err: ToolError) {
        let browser = self.clone();
        tokio::spawn(async move {
            if !browser.wait_connected(window).await {
                for (_, tx) in browser.pending.lock().unwrap().drain() {
                    let _ = tx.send(Err(drain_err.clone()));
                }
            }
        });
    }

    /// Route one framed message from the extension: the kill-switch event (g11:
    /// `session_killed`, no `id`), a hold request (g10: `get_hold` / `set_hold` /
    /// `toggle_hold`, answered here and returned early), or a reply to a waiting tool caller
    /// (by id). Messages without an id are otherwise events.
    fn route_reply(&self, payload: &[u8]) {
        let Ok(reply) = serde_json::from_slice::<Value>(payload) else {
            tracing::warn!("dropping unparseable extension reply");
            return;
        };

        let msg_type = reply.get("type").and_then(Value::as_str);

        if reply.get("id").is_none() && msg_type == Some("session_killed") {
            self.handle_session_killed();
            return;
        }

        if let (Some(id), Some(kind @ ("get_hold" | "set_hold" | "toggle_hold"))) =
            (reply.get("id").and_then(Value::as_str), msg_type)
        {
            self.handle_hold_request(id, kind, &reply);
            return;
        }

        let Some(id) = reply.get("id").and_then(Value::as_str) else {
            return; // an event/heartbeat, not a tool reply
        };
        let Some(tx) = self.pending.lock().unwrap().remove(id) else {
            return; // late or duplicate reply
        };
        let result = match reply.get("type").and_then(Value::as_str) {
            Some("tool_error") => {
                let message = reply
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("tool execution failed")
                    .to_string();
                let hop = reply.get("hop").and_then(Value::as_str);
                if let Some(detail) = reply.get("detail").and_then(Value::as_str) {
                    tracing::debug!(detail, "extension error detail");
                }
                Err(ToolError::from_extension_wire(hop, message))
            }
            _ => Ok(reply.get("result").cloned().unwrap_or(Value::Null)),
        };
        let _ = tx.send(result);
    }

    /// Apply one hold request (g10) and send the `hold_state` (or `hold_error`) reply back
    /// over the same connection. `get_hold` reports without changing state; `set_hold`
    /// requires a boolean `held` member (a missing or non-boolean value is a `hold_error` that
    /// changes nothing); `toggle_hold` flips atomically. Every request receives the state
    /// AFTER the request was applied.
    fn handle_hold_request(&self, id: &str, kind: &str, request: &Value) {
        let outcome = match kind {
            "get_hold" => Ok(self.held_for().is_some()),
            "toggle_hold" => Ok(self.toggle_held()),
            "set_hold" => match request.get("held").and_then(Value::as_bool) {
                Some(held) => Ok(self.set_held(held)),
                None => Err("set_hold requires a boolean 'held'"),
            },
            _ => unreachable!("matched only get_hold/set_hold/toggle_hold in route_reply"),
        };
        let reply = match outcome {
            Ok(held) => json!({ "id": id, "type": "hold_state", "result": { "held": held } }),
            Err(error) => json!({ "id": id, "type": "hold_error", "error": error }),
        };
        self.send_hold_reply(&reply);
    }

    /// Frame and enqueue a hold reply on the outgoing channel, dropping it silently if the
    /// connection is already gone (the same fire-and-forget posture as every other
    /// best-effort send in this module).
    fn send_hold_reply(&self, reply: &Value) {
        let Ok(bytes) = serde_json::to_vec(reply) else {
            tracing::warn!("failed to serialize a hold reply");
            return;
        };
        let Ok(framed) = host::encode(&bytes) else {
            tracing::warn!("failed to frame a hold reply");
            return;
        };
        if let Some(tx) = self.outgoing.lock().unwrap().as_ref() {
            let _ = tx.send(framed);
        }
    }

    /// Handle the `session_killed` event (g11): exactly once per false-to-true transition
    /// (`swap` makes duplicate frames on the same connection harmless), fail every pending
    /// call with the kill error, then invoke EVERY registered hook -- permanent and
    /// session-scoped alike -- exactly once (ADR-0030 Decision 7: "every live session's subject
    /// gets exactly one `session_killed` audit record"). The per-transition `swap` guard above is
    /// what makes each individual kill fan out once per hook, never twice. Handling still sets
    /// the flag and drains pending calls even if no hook is registered.
    fn handle_session_killed(&self) {
        if self.killed.swap(true, Ordering::SeqCst) {
            return; // already handled; a duplicate frame is a no-op
        }
        for (_, tx) in self.pending.lock().unwrap().drain() {
            let _ = tx.send(Err(kill_error()));
        }
        for (_, hook) in self.kill_hooks.lock().unwrap().iter() {
            hook();
        }
    }
}

impl Default for Browser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    async fn wait_connected(browser: &Browser) {
        for _ in 0..200 {
            if browser.is_connected() {
                return;
            }
            sleep(Duration::from_millis(5)).await;
        }
        panic!("browser never reported connected");
    }

    #[tokio::test]
    async fn call_round_trips_a_tool_response() {
        let (browser_side, mut ext_side) = tokio::io::duplex(64 * 1024);
        let browser = Browser::new();

        let attached = browser.clone();
        tokio::spawn(async move { attached.attach(browser_side).await });

        // Fake extension: read one framed request, reply with a result echoing the tool name.
        let fake_ext = tokio::spawn(async move {
            let req = host::read_message(&mut ext_side).await.unwrap().unwrap();
            let v: Value = serde_json::from_slice(&req).unwrap();
            let id = v["id"].as_str().unwrap();
            let reply =
                json!({ "id": id, "type": "tool_response", "result": { "echoed": v["tool"] } });
            host::write_message(&mut ext_side, &serde_json::to_vec(&reply).unwrap())
                .await
                .unwrap();
        });

        wait_connected(&browser).await;
        let result = browser
            .call("navigate", &json!({ "url": "https://example.com" }))
            .await
            .unwrap();
        assert_eq!(result, json!({ "echoed": "navigate" }));
        fake_ext.await.unwrap();
    }

    #[tokio::test]
    async fn call_surfaces_a_tool_error() {
        let (browser_side, mut ext_side) = tokio::io::duplex(64 * 1024);
        let browser = Browser::new();
        let attached = browser.clone();
        tokio::spawn(async move { attached.attach(browser_side).await });

        tokio::spawn(async move {
            let req = host::read_message(&mut ext_side).await.unwrap().unwrap();
            let v: Value = serde_json::from_slice(&req).unwrap();
            let reply = json!({ "id": v["id"], "type": "tool_error", "error": "boom" });
            host::write_message(&mut ext_side, &serde_json::to_vec(&reply).unwrap())
                .await
                .unwrap();
        });

        wait_connected(&browser).await;
        let err = browser
            .call("javascript_tool", &json!({}))
            .await
            .unwrap_err();
        let text = err.to_string();
        assert!(text.starts_with("[hop: extension]"), "{text}");
        assert!(text.contains("boom"), "{text}");
    }

    #[tokio::test]
    async fn call_without_a_connection_fails_fast() {
        let browser = Browser::new();
        let err = browser.call("navigate", &json!({})).await.unwrap_err();
        let text = err.to_string();
        assert!(text.starts_with("[hop: extension]"), "{text}");
        assert!(text.contains("not connected"), "{text}");
    }

    /// H7 supplementary (not task-named; the pinned H7 assertions live in
    /// `tests/extension/grouping.test.js`): `request_group` is a harmless no-op with no connected
    /// extension -- it must never panic or block a caller that has nothing to await.
    #[test]
    fn request_group_without_a_connection_is_a_harmless_no_op() {
        let browser = Browser::new();
        browser.request_group("11111111-1111-4111-8111-111111111111", &[101, 202], "title");
    }

    /// H7 supplementary: a connected fake extension receives EXACTLY the pinned wire shape --
    /// `type`/`guid`/`tabIds`/`title`, no `id` member (fire-and-forget; nothing correlates a
    /// reply) -- and sending a `group_response` back (also id-less) never wedges `route_reply`,
    /// which drops it as an ordinary event.
    #[tokio::test]
    async fn request_group_sends_the_pinned_shape_and_a_reply_is_a_harmless_event() {
        let (browser_side, mut ext_side) = tokio::io::duplex(64 * 1024);
        let browser = Browser::new();
        let attached = browser.clone();
        tokio::spawn(async move { attached.attach(browser_side).await });
        wait_connected(&browser).await;

        browser.request_group("11111111-1111-4111-8111-111111111111", &[101, 202], "title");

        let req = host::read_message(&mut ext_side).await.unwrap().unwrap();
        let v: Value = serde_json::from_slice(&req).unwrap();
        assert_eq!(v["type"], "group_request");
        assert_eq!(v["guid"], "11111111-1111-4111-8111-111111111111");
        assert_eq!(v["tabIds"], json!([101, 202]));
        assert_eq!(v["title"], "title");
        assert!(v.get("id").is_none(), "the pinned shape carries no id");

        let reply = json!({
            "type": "group_response",
            "guid": "11111111-1111-4111-8111-111111111111",
            "ok": true,
        });
        host::write_message(&mut ext_side, &serde_json::to_vec(&reply).unwrap())
            .await
            .unwrap();

        // Proof the id-less reply did not wedge anything: an ordinary tool call still round-trips
        // afterward.
        let fake_ext = tokio::spawn(async move {
            let req = host::read_message(&mut ext_side).await.unwrap().unwrap();
            let v: Value = serde_json::from_slice(&req).unwrap();
            let id = v["id"].as_str().unwrap();
            let reply = json!({ "id": id, "type": "tool_response", "result": { "ok": true } });
            host::write_message(&mut ext_side, &serde_json::to_vec(&reply).unwrap())
                .await
                .unwrap();
        });
        let result = browser.call("navigate", &json!({})).await.unwrap();
        assert_eq!(result, json!({ "ok": true }));
        fake_ext.await.unwrap();
    }

    #[tokio::test]
    async fn call_surfaces_a_cdp_tagged_tool_error_without_leaking_detail() {
        let (browser_side, mut ext_side) = tokio::io::duplex(64 * 1024);
        let browser = Browser::new();
        let attached = browser.clone();
        tokio::spawn(async move { attached.attach(browser_side).await });

        tokio::spawn(async move {
            let req = host::read_message(&mut ext_side).await.unwrap().unwrap();
            let v: Value = serde_json::from_slice(&req).unwrap();
            let reply = json!({
                "id": v["id"],
                "type": "tool_error",
                "error": "Input.dispatchMouseEvent failed: no target",
                "hop": "cdp",
                "detail": "verbose internals",
            });
            host::write_message(&mut ext_side, &serde_json::to_vec(&reply).unwrap())
                .await
                .unwrap();
        });

        wait_connected(&browser).await;
        let err = browser.call("computer", &json!({})).await.unwrap_err();
        let text = err.to_string();
        assert!(text.starts_with("[hop: cdp]"), "{text}");
        assert!(text.contains("Input.dispatchMouseEvent failed"), "{text}");
        assert!(!text.contains("verbose internals"), "{text}");
    }

    #[tokio::test]
    async fn call_surfaces_a_page_tagged_tool_error() {
        let (browser_side, mut ext_side) = tokio::io::duplex(64 * 1024);
        let browser = Browser::new();
        let attached = browser.clone();
        tokio::spawn(async move { attached.attach(browser_side).await });

        tokio::spawn(async move {
            let req = host::read_message(&mut ext_side).await.unwrap().unwrap();
            let v: Value = serde_json::from_slice(&req).unwrap();
            let reply = json!({
                "id": v["id"],
                "type": "tool_error",
                "error": "Element ref_5 not found",
                "hop": "page",
            });
            host::write_message(&mut ext_side, &serde_json::to_vec(&reply).unwrap())
                .await
                .unwrap();
        });

        wait_connected(&browser).await;
        let err = browser.call("form_input", &json!({})).await.unwrap_err();
        let text = err.to_string();
        assert!(text.starts_with("[hop: page]"), "{text}");
        assert!(text.contains("Element ref_5 not found"), "{text}");
    }

    #[tokio::test]
    async fn wait_connected_times_out_without_a_connection() {
        let browser = Browser::new();
        let ready = browser.wait_connected(Duration::from_millis(50)).await;
        assert!(!ready, "no extension ever attached; wait must time out");
    }

    #[tokio::test]
    async fn wait_connected_wakes_when_the_extension_attaches() {
        let (browser_side, _ext_side) = tokio::io::duplex(64 * 1024);
        let browser = Browser::new();

        let attached = browser.clone();
        tokio::spawn(async move {
            sleep(Duration::from_millis(50)).await;
            let _ = attached.attach(browser_side).await;
        });

        let ready = browser.wait_connected(Duration::from_secs(2)).await;
        assert!(ready, "wait_connected must wake once attach() connects");
    }

    #[tokio::test]
    async fn a_second_attach_is_rejected_without_disturbing_the_live_session() {
        let (first_side, mut first_ext) = tokio::io::duplex(64 * 1024);
        let (second_side, _second_ext) = tokio::io::duplex(64 * 1024);
        let browser = Browser::new();

        let attached = browser.clone();
        tokio::spawn(async move { attached.attach(first_side).await });
        wait_connected(&browser).await;

        // A connection arriving while a session is attached is a stray: it must be rejected and must
        // not clear the live session's sender or connected flag.
        let outcome = browser.attach(second_side).await;
        assert_eq!(outcome, AttachOutcome::AlreadyAttached);
        assert!(
            browser.is_connected(),
            "the live session must stay connected after a stray attach"
        );

        // ...and the live session still round-trips a call.
        let ext = tokio::spawn(async move {
            let req = host::read_message(&mut first_ext).await.unwrap().unwrap();
            let v: Value = serde_json::from_slice(&req).unwrap();
            let reply = json!({ "id": v["id"], "type": "tool_response", "result": { "ok": true } });
            host::write_message(&mut first_ext, &serde_json::to_vec(&reply).unwrap())
                .await
                .unwrap();
        });
        let result = browser.call("navigate", &json!({})).await.unwrap();
        assert_eq!(result, json!({ "ok": true }));
        ext.await.unwrap();
    }

    #[test]
    fn held_state_set_toggle_and_preserved_timer() {
        let browser = Browser::new();
        assert!(browser.held_for().is_none());

        assert!(browser.set_held(true));
        assert!(browser.held_for().is_some());

        assert!(!browser.set_held(false));
        assert!(browser.held_for().is_none());

        assert!(browser.toggle_held());
        assert!(browser.held_for().is_some());
        assert!(!browser.toggle_held());
        assert!(browser.held_for().is_none());
    }

    #[test]
    fn repeated_set_held_true_preserves_the_original_instant() {
        let browser = Browser::new();
        browser.set_held(true);
        std::thread::sleep(Duration::from_millis(30));
        browser.set_held(true);
        assert!(
            browser.held_for().unwrap() >= Duration::from_millis(30),
            "a repeated set_held(true) must not reset the engage instant"
        );
    }

    #[tokio::test]
    async fn hold_requests_are_answered_over_the_native_channel() {
        let (browser_side, mut ext_side) = tokio::io::duplex(64 * 1024);
        let browser = Browser::new();
        let attached = browser.clone();
        tokio::spawn(async move { attached.attach(browser_side).await });
        wait_connected(&browser).await;

        async fn send_and_read(ext_side: &mut tokio::io::DuplexStream, request: Value) -> Value {
            host::write_message(ext_side, &serde_json::to_vec(&request).unwrap())
                .await
                .unwrap();
            let reply = host::read_message(ext_side).await.unwrap().unwrap();
            serde_json::from_slice(&reply).unwrap()
        }

        let reply = send_and_read(
            &mut ext_side,
            json!({ "id": "h1", "type": "set_hold", "held": true }),
        )
        .await;
        assert_eq!(reply["id"], "h1");
        assert_eq!(reply["type"], "hold_state");
        assert_eq!(reply["result"]["held"], true);
        assert!(browser.held_for().is_some());

        let reply = send_and_read(&mut ext_side, json!({ "id": "h2", "type": "get_hold" })).await;
        assert_eq!(reply["type"], "hold_state");
        assert_eq!(
            reply["result"]["held"], true,
            "get_hold must not change state"
        );
        assert!(browser.held_for().is_some());

        let reply =
            send_and_read(&mut ext_side, json!({ "id": "h3", "type": "toggle_hold" })).await;
        assert_eq!(reply["result"]["held"], false);
        assert!(browser.held_for().is_none());

        let reply = send_and_read(
            &mut ext_side,
            json!({ "id": "h4", "type": "set_hold", "held": "not-a-bool" }),
        )
        .await;
        assert_eq!(reply["type"], "hold_error");
        assert_eq!(reply["error"], "set_hold requires a boolean 'held'");
        assert!(
            browser.held_for().is_none(),
            "an invalid set_hold must change nothing"
        );
    }

    #[tokio::test]
    async fn hold_survives_the_extension_disconnecting() {
        let (browser_side, ext_side) = tokio::io::duplex(64 * 1024);
        let browser = Browser::new();
        browser.set_held(true);

        let attached = browser.clone();
        let attach_task = tokio::spawn(async move { attached.attach(browser_side).await });
        wait_connected(&browser).await;

        drop(ext_side);
        let _ = attach_task.await.unwrap();

        assert!(
            browser.held_for().is_some(),
            "the hold must survive the extension disconnecting"
        );
    }

    /// Test 1a (g11 spec section 9): the kill event fails an in-flight call with the exact
    /// section-7 error, and the extension never sees a reply.
    #[tokio::test]
    async fn kill_fails_in_flight_calls() {
        let (browser_side, mut ext_side) = tokio::io::duplex(64 * 1024);
        let browser = Browser::new();
        let attached = browser.clone();
        tokio::spawn(async move { attached.attach(browser_side).await });
        wait_connected(&browser).await;

        let caller = browser.clone();
        let call_task = tokio::spawn(async move { caller.call("navigate", &json!({})).await });

        let req = host::read_message(&mut ext_side).await.unwrap().unwrap();
        let _: Value = serde_json::from_slice(&req).unwrap();
        host::write_message(
            &mut ext_side,
            &serde_json::to_vec(&json!({ "type": "session_killed" })).unwrap(),
        )
        .await
        .unwrap();

        let err = call_task.await.unwrap().unwrap_err();
        let text = err.to_string();
        assert!(text.starts_with("[hop: extension]"), "{text}");
        assert!(
            text.contains("The user ended the browser session (kill switch)"),
            "{text}"
        );
        assert!(browser.is_killed());
    }

    /// Test 1b: after the kill, a new call fails immediately with the same message -- no frame
    /// sent to the extension, no waiting on `TOOL_TIMEOUT`.
    #[tokio::test]
    async fn kill_fails_subsequent_calls_fast() {
        let (browser_side, mut ext_side) = tokio::io::duplex(64 * 1024);
        let browser = Browser::new();
        let attached = browser.clone();
        tokio::spawn(async move { attached.attach(browser_side).await });
        wait_connected(&browser).await;

        host::write_message(
            &mut ext_side,
            &serde_json::to_vec(&json!({ "type": "session_killed" })).unwrap(),
        )
        .await
        .unwrap();
        // Wait for the event to be routed before issuing the next call.
        for _ in 0..200 {
            if browser.is_killed() {
                break;
            }
            sleep(Duration::from_millis(5)).await;
        }
        assert!(browser.is_killed());

        let result =
            tokio::time::timeout(Duration::from_secs(1), browser.call("navigate", &json!({})))
                .await
                .expect("a killed call must fail immediately, not time out");
        let text = result.unwrap_err().to_string();
        assert!(
            text.contains("The user ended the browser session (kill switch)"),
            "{text}"
        );
    }

    /// Test 1c: the kill error beats the not-connected error even after the stream itself
    /// closes.
    #[tokio::test]
    async fn kill_error_outlives_the_disconnect() {
        let (browser_side, ext_side) = tokio::io::duplex(64 * 1024);
        let browser = Browser::new();
        let attached = browser.clone();
        let attach_task = tokio::spawn(async move { attached.attach(browser_side).await });
        wait_connected(&browser).await;

        let mut ext_side = ext_side;
        host::write_message(
            &mut ext_side,
            &serde_json::to_vec(&json!({ "type": "session_killed" })).unwrap(),
        )
        .await
        .unwrap();
        drop(ext_side);
        let _ = attach_task.await.unwrap();

        let err = browser.call("navigate", &json!({})).await.unwrap_err();
        assert!(
            err.to_string()
                .contains("The user ended the browser session (kill switch)"),
            "{err}"
        );
    }

    /// Test 1d: a fresh attach clears the kill; a call round-trips normally afterward.
    #[tokio::test]
    async fn fresh_attach_clears_the_kill() {
        let (first_side, mut first_ext) = tokio::io::duplex(64 * 1024);
        let browser = Browser::new();
        let attached = browser.clone();
        let first_attach = tokio::spawn(async move { attached.attach(first_side).await });
        wait_connected(&browser).await;

        host::write_message(
            &mut first_ext,
            &serde_json::to_vec(&json!({ "type": "session_killed" })).unwrap(),
        )
        .await
        .unwrap();
        for _ in 0..200 {
            if browser.is_killed() {
                break;
            }
            sleep(Duration::from_millis(5)).await;
        }
        assert!(browser.is_killed());

        // Tear down the first connection (a real "session ended") and wait for the slot to
        // free before attaching a fresh one; a stray attach while a session still holds the
        // slot is rejected without touching the kill flag.
        drop(first_ext);
        let _ = first_attach.await.unwrap();

        let (second_side, mut second_ext) = tokio::io::duplex(64 * 1024);
        let attached = browser.clone();
        tokio::spawn(async move { attached.attach(second_side).await });
        wait_connected(&browser).await;
        assert!(
            !browser.is_killed(),
            "a fresh attach must clear the kill flag"
        );

        let fake_ext = tokio::spawn(async move {
            let req = host::read_message(&mut second_ext).await.unwrap().unwrap();
            let v: Value = serde_json::from_slice(&req).unwrap();
            let reply = json!({ "id": v["id"], "type": "tool_response", "result": { "ok": true } });
            host::write_message(&mut second_ext, &serde_json::to_vec(&reply).unwrap())
                .await
                .unwrap();
        });
        let result = browser.call("navigate", &json!({})).await.unwrap();
        assert_eq!(result, json!({ "ok": true }));
        fake_ext.await.unwrap();
    }

    /// Test 1e: the hook fires exactly once even if two kill frames arrive on the same
    /// connection.
    #[tokio::test]
    async fn kill_hook_fires_exactly_once_per_transition() {
        let (browser_side, mut ext_side) = tokio::io::duplex(64 * 1024);
        let browser = Browser::new();
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let hook_count = Arc::clone(&count);
        browser.on_session_killed(move || {
            hook_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });

        let attached = browser.clone();
        tokio::spawn(async move { attached.attach(browser_side).await });
        wait_connected(&browser).await;

        for _ in 0..2 {
            host::write_message(
                &mut ext_side,
                &serde_json::to_vec(&json!({ "type": "session_killed" })).unwrap(),
            )
            .await
            .unwrap();
        }
        for _ in 0..200 {
            if count.load(std::sync::atomic::Ordering::SeqCst) > 0 {
                break;
            }
            sleep(Duration::from_millis(5)).await;
        }
        // Give a possible (incorrect) second invocation a moment to land before asserting.
        sleep(Duration::from_millis(50)).await;
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    /// H5 (ADR-0030 Decision 3): `GRACE_WINDOW` is pinned strictly less than `TOOL_TIMEOUT`
    /// (docs/tasks/hub/PINS.md SS4). Not one of the task's named tests; a direct transcription
    /// check on the two pinned constants themselves, no derived value.
    #[test]
    fn grace_window_is_pinned_and_strictly_less_than_tool_timeout() {
        assert_eq!(GRACE_WINDOW, Duration::from_secs(10));
        assert!(GRACE_WINDOW < TOOL_TIMEOUT);
    }

    /// H5 (ADR-0030 Decision 3): a reconnect within the grace window must NOT fail a pending
    /// call. Drives `spawn_grace_drain` directly (a private fn in this same module) with a short
    /// window so the test stays fast; the real `GRACE_WINDOW` constant (10s) is exercised
    /// separately by `grace_window_is_pinned_and_strictly_less_than_tool_timeout` above and by
    /// `attach` calling it verbatim.
    #[tokio::test]
    async fn a_reconnect_within_the_grace_window_does_not_fail_a_pending_call() {
        let browser = Browser::new();
        let (tx, rx) = oneshot::channel();
        browser
            .pending
            .lock()
            .unwrap()
            .insert("held".to_string(), tx);

        let drain_err = ToolError::extension("Browser extension disconnected before responding");
        browser.spawn_grace_drain(Duration::from_millis(200), drain_err);

        // Reconnect well within the window.
        sleep(Duration::from_millis(20)).await;
        browser.connected.send_replace(true);

        // Give the grace task time to observe the reconnect and skip draining.
        sleep(Duration::from_millis(300)).await;
        assert!(
            browser.pending.lock().unwrap().contains_key("held"),
            "a reconnect within the grace window must not fail the pending call"
        );
        drop(rx);
    }

    /// H5 (ADR-0030 Decision 3): once the grace window elapses with NO reconnect (a real drop),
    /// pending calls fail with the exact, unchanged disconnect error text -- the grace window
    /// changes WHEN pending fail, never the error TEXT.
    #[tokio::test]
    async fn grace_window_elapsing_with_no_reconnect_drains_pending_with_the_pinned_disconnect_text(
    ) {
        let browser = Browser::new();
        let (tx, rx) = oneshot::channel();
        browser
            .pending
            .lock()
            .unwrap()
            .insert("held".to_string(), tx);

        browser.spawn_grace_drain(
            Duration::from_millis(50),
            ToolError::extension("Browser extension disconnected before responding"),
        );

        let result = tokio::time::timeout(Duration::from_secs(2), rx)
            .await
            .expect("the grace window must elapse and drain within the bound")
            .expect("the sender must have sent a result, not been dropped silently");
        let err = result.unwrap_err();
        assert!(
            err.to_string()
                .contains("Browser extension disconnected before responding"),
            "{err}"
        );
    }
}
