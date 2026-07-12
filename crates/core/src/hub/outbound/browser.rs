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
//!
//! On-screen notification ([`Browser::notify`], SAPS PRES-HIGH-01): the mcp-server sends
//! `{ "type": "notification", "tabId", "class", "icon"?, "title", "description"?, "ref"? }`.
//! Same posture as `request_group` -- fire-and-forget, no `id`, no reply. `class` and `icon` are
//! the standard severity taxonomy this codebase's own tracing already uses --
//! `"info"`/`"debug"`/`"warn"`/`"error"` -- so the primitive stays general-purpose rather than
//! denial-specific (today: `class: "error"` for a sacred-domain denial, `"warn"` for a policy
//! denial) the extension renders without judging; `title` is the
//! always-shown headline, `description` an optional supporting line; `ref` is an opaque
//! cross-reference (today: the denial_id) a viewer can correlate back to the structured audit
//! record. First caller is a denial, fired from [`crate::mcp::pipeline::run_tool_call`] at each of
//! the three points a call is denied -- the ONE place today where governance decides something and the
//! extension is never otherwise contacted, so nothing on screen shows a block happened without
//! this. Deliberately general so a future notification need (a policy hot-reload landing, for
//! example) is a new `class`/`icon` value at an existing call site, not a new message type.

use super::diagnostics::Diagnostic;
use crate::ToolError;
use ghostlight_transport::host;
use ghostlight_transport::observability::DebugSink;
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
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

/// A screenshot cached per session for later `upload_image` reference (ADR-0050 Decision 4). Holds
/// the base64 bytes and the media type exactly as the extension's `computer` screenshot result
/// carried them, so `upload_image` can forward them to a file input or drag-drop target.
#[derive(Clone)]
pub(crate) struct CachedImage {
    pub(crate) base64: String,
    pub(crate) media_type: String,
}

/// Per-guid bounded screenshot cache (ADR-0050 D4): each session's last
/// [`SCREENSHOT_CACHE_BOUND`] screenshots, newest last, keyed by minted `img_...` id.
type ScreenshotCache = Arc<Mutex<HashMap<String, VecDeque<(String, CachedImage)>>>>;

/// The per-guid screenshot-cache bound (ADR-0050 D4): pushing a 9th screenshot evicts the oldest.
const SCREENSHOT_CACHE_BOUND: usize = 8;

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

/// Clone `args` with its `tabId` field overwritten to `native` (ADR-0058). Used to build the
/// extension-bound request from a caller's still-composite `args` without mutating the caller's
/// own value (a `browser_batch`/`script` sub-step re-entering the chokepoint must see its
/// original, untouched composite tabId).
fn merge_tab_id(args: &Value, native: i64) -> Value {
    let mut owned = args.clone();
    if let Some(obj) = owned.as_object_mut() {
        obj.insert("tabId".to_string(), json!(native));
    }
    owned
}

/// Recursively rewrite every plain-number `"tabId"` key in `v` to its composite form (ADR-0058),
/// walking both real JSON structure (`structuredContent`, nested objects/arrays) and any
/// `content[].text` block whose text happens to parse as JSON (`tabs_context_mcp`/
/// `tabs_create_mcp` report their tab list this way, as a JSON-stringified text block AND as
/// `structuredContent`). A text block that is not valid JSON is left untouched. Generic on
/// purpose: covers every current tabId-reporting tool and any future one without a matching
/// manual edit here.
fn encode_tab_ids_in_value(v: &mut Value, target: u32) {
    match v {
        Value::Object(map) => {
            if let Some(Value::Number(n)) = map.get("tabId") {
                if let Some(native) = n.as_i64() {
                    map.insert(
                        "tabId".to_string(),
                        json!(crate::constants::tab_id::encode(target, native)),
                    );
                }
            }
            if let Some(Value::String(text)) = map.get("text") {
                if let Ok(mut parsed) = serde_json::from_str::<Value>(text) {
                    encode_tab_ids_in_value(&mut parsed, target);
                    if let Ok(restr) = serde_json::to_string(&parsed) {
                        map.insert("text".to_string(), Value::String(restr));
                    }
                }
            }
            for value in map.values_mut() {
                encode_tab_ids_in_value(value, target);
            }
        }
        Value::Array(items) => {
            for item in items {
                encode_tab_ids_in_value(item, target);
            }
        }
        _ => {}
    }
}

/// Parse the extension's `rescale_coords` reply: a text content block holding
/// `{"points": [[x, y], ...]}`. None on any shape mismatch (the caller keeps raw coordinates).
fn parse_rescaled_points(reply: &Value) -> Option<Vec<(f64, f64)>> {
    let text = reply
        .get("content")?
        .as_array()?
        .first()?
        .get("text")?
        .as_str()?;
    let parsed: Value = serde_json::from_str(text).ok()?;
    let points = parsed.get("points")?.as_array()?;
    let mut out = Vec::with_capacity(points.len());
    for p in points {
        let pair = p.as_array()?;
        out.push((pair.first()?.as_f64()?, pair.get(1)?.as_f64()?));
    }
    Some(out)
}

/// One attached browser's live send half plus enough identity to detect a stale self-removal
/// race (ADR-0058): `generation` is this [`Browser::attach`] call's own monotonic id, so a
/// reader loop that is about to remove its session on disconnect can tell whether a LATER
/// attach (a reconnect from the same browser) has already replaced it -- and if so, leave the
/// newer entry alone.
struct BrowserSession {
    sender: mpsc::UnboundedSender<Vec<u8>>,
    generation: u64,
}

/// A cloneable handle the mcp-server uses to call tools on the extension.
#[derive(Clone)]
pub struct Browser {
    next_id: Arc<AtomicU64>,
    pending: Pending,
    /// Every currently-attached browser, keyed by its OS pid (ADR-0058; replaces the pre-0058
    /// single `outgoing: Option<Sender>` slot). A hello for a pid already present REPLACES the
    /// entry (a reconnect/relaunch from the SAME browser); a hello for a new pid is ADDED
    /// alongside any others.
    sessions: Arc<Mutex<HashMap<u32, BrowserSession>>>,
    /// Focus recency (ADR-0058): front = the browser that most recently reported
    /// `chrome.windows.onFocusChanged` gaining focus. Only entries also present in `sessions`
    /// are ever consulted; a disconnected browser's entry is pruned on its own detach. Used ONLY
    /// to pick a target when a call names no tab at all (tab-creation bootstrap); a call that
    /// names a tab is always routed by that tab's OWN encoded browser pid, never by focus.
    focus_chain: Arc<Mutex<Vec<u32>>>,
    /// Monotonic source for [`BrowserSession::generation`] (ADR-0058).
    next_session_generation: Arc<AtomicU64>,
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
    /// Per-session screenshot cache (ADR-0050 Decision 4): a `computer` screenshot result is cached
    /// here under a minted `img_...` id so `upload_image` can later place it into a file input or a
    /// drag-drop target. Bounded per guid ([`SCREENSHOT_CACHE_BOUND`]), evicting the oldest.
    screenshot_cache: ScreenshotCache,
    /// gif_creator recording sessions (ADR-0053 D3/D4): per-tab state + disk-backed frames, fed by
    /// the extension's unsolicited `gif_frame` events and read back at export.
    recordings: Arc<super::recording::RecordingStore>,
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
            sessions: Arc::new(Mutex::new(HashMap::new())),
            focus_chain: Arc::new(Mutex::new(Vec::new())),
            next_session_generation: Arc::new(AtomicU64::new(1)),
            // Dropping the initial receiver is fine: updates use `send_replace`, which does not
            // require a live receiver (unlike `send`, which would fail and skip the update).
            connected: Arc::new(watch::channel(false).0),
            debug,
            held: Arc::new(Mutex::new(None)),
            killed: Arc::new(AtomicBool::new(false)),
            kill_hooks: Arc::new(Mutex::new(Vec::new())),
            next_hook_id: Arc::new(AtomicU64::new(1)),
            screenshot_cache: Arc::new(Mutex::new(HashMap::new())),
            recordings: Arc::new(super::recording::RecordingStore::new()),
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

    /// True while at least one native-host / extension is connected (ADR-0058).
    pub fn is_connected(&self) -> bool {
        !self.sessions.lock().unwrap().is_empty()
    }

    /// A snapshot of every attached browser for `ghostlight doctor` (ADR-0058, CAP-MED-01):
    /// pid and whether it is the current focus-chain front, most-recently-focused first, falling
    /// back to ascending pid order for browsers that have never reported focus (a stable,
    /// deterministic order beats an arbitrary hash-map one for a diagnostic listing).
    pub fn browser_snapshot(&self) -> Vec<ghostlight_transport::ipc::BrowserInfo> {
        let sessions = self.sessions.lock().unwrap();
        let chain = self.focus_chain.lock().unwrap();
        let mut pids: Vec<u32> = sessions.keys().copied().collect();
        pids.sort_unstable();
        let mut ordered: Vec<u32> = chain
            .iter()
            .copied()
            .filter(|p| sessions.contains_key(p))
            .collect();
        for pid in &pids {
            if !ordered.contains(pid) {
                ordered.push(*pid);
            }
        }
        ordered
            .into_iter()
            .enumerate()
            .map(|(i, pid)| ghostlight_transport::ipc::BrowserInfo {
                pid,
                focused: i == 0,
            })
            .collect()
    }

    /// Resolve which browser a call targets, from an optional COMPOSITE tab id (ADR-0058):
    /// decoded from `args.tabId` when the call names a tab, else the focus-chain front (falling
    /// back to the sole attached browser, then to the smallest pid, when nothing has ever
    /// reported focus). Returns `(target_pid, native_tab_id)`; `target_pid` is `None` only when
    /// NO browser is attached at all. `native_tab_id` is `Some` only when a tab was named --
    /// the value to put on the wire to the extension, which never learns the composite encoding
    /// exists.
    fn resolve_target(&self, composite_tab_id: Option<i64>) -> (Option<u32>, Option<i64>) {
        if let Some(composite) = composite_tab_id {
            let (pid, native) = crate::constants::tab_id::decode(composite);
            return (Some(pid), Some(native));
        }
        let sessions = self.sessions.lock().unwrap();
        if sessions.is_empty() {
            return (None, None);
        }
        let chain = self.focus_chain.lock().unwrap();
        if let Some(pid) = chain.iter().find(|p| sessions.contains_key(p)) {
            return (Some(*pid), None);
        }
        (sessions.keys().min().copied(), None)
    }

    /// Record that `browser_pid` just reported gaining window focus (ADR-0058): move-to-front,
    /// no duplicate entries. Only "gained focus" is ever reported or tracked -- losing focus to
    /// something else (another app, or nothing) carries no actionable signal, so the chain's
    /// recency order alone already answers "who was focused most recently, among those still
    /// attached" without a separate blurred/focused boolean per entry.
    fn note_focus(&self, browser_pid: u32) {
        let mut chain = self.focus_chain.lock().unwrap();
        chain.retain(|p| *p != browser_pid);
        chain.insert(0, browser_pid);
        drop(chain);
        self.debug
            .ipc_note(&Diagnostic::FocusReported { browser_pid }.describe());
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
    /// `guid` is the calling session's [`SessionGuid`] string (ADR-0047 D3), written verbatim into
    /// the additive `tool_request` envelope field. Trained tool schemas are untouched; the extension
    /// consumes `guid` only for session-scoped tab operations (`tabs_create_mcp`/`tabs_context_mcp`)
    /// and ignores it for every other tool.
    ///
    /// Every failure is a hop-attributed [`ToolError`]: no extension connected, an encoding
    /// failure before the request left the process, the extension reporting a tool error (tagged
    /// `cdp`, `page`, or untagged and attributed to the `extension` hop), a mid-call disconnect,
    /// or a timeout.
    pub async fn call(
        &self,
        guid: &str,
        tool: &str,
        args: &Value,
    ) -> std::result::Result<Value, ToolError> {
        // The killed check precedes everything else, including the pending-map insert and the
        // not-connected check (g11 constraint 12): after a kill the port drops and every
        // session is gone, so the generic not-connected error would otherwise win by accident.
        // The binary knows the real cause; the engine is truthful. No debug tool_begin/tool_end
        // pairing here: the call never began in any trackable sense.
        if self.killed.load(Ordering::SeqCst) {
            return Err(kill_error());
        }

        // ADR-0058: resolve which browser this call targets from its (possibly composite) tabId,
        // and -- when it named one -- rewrite a LOCAL copy of `args` carrying the plain native
        // tab id the extension actually understands. `args` itself is never mutated: a caller
        // that recurses back through the governance chokepoint with its OWN, still-composite
        // copy (`browser_batch`/`script` sub-steps) must see the untouched original.
        let composite = args.get("tabId").and_then(Value::as_i64);
        let (target, native_tab) = self.resolve_target(composite);
        let Some(target) = target else {
            let err = ToolError::extension("Browser extension not connected");
            self.debug.tool_begin("-", tool);
            self.debug.tool_end("-", false, &err.to_string());
            return Err(err);
        };
        let owned_args;
        let call_args = match native_tab {
            Some(native) => {
                owned_args = merge_tab_id(args, native);
                &owned_args
            }
            None => args,
        };

        // gif_creator (ADR-0053 D4): while this tab records, note the action BEFORE it runs so
        // the screencast frame its paint produces is the frame that carries its ring/label.
        self.note_gif_action(guid, tool, call_args, target).await;
        let result = self.raw_call(guid, tool, call_args, target).await?;
        let result = self.encode_tab_ids(result, target);
        Ok(self.cache_and_inject_screenshot(guid, tool, result))
    }

    /// The bare envelope + dispatch of [`Browser::call`], shared with internal sends that must not
    /// re-enter the gif action-noting or screenshot-cache layers. `target` is the already-resolved
    /// browser pid (ADR-0058) this specific request is sent to.
    async fn raw_call(
        &self,
        guid: &str,
        tool: &str,
        args: &Value,
        target: u32,
    ) -> std::result::Result<Value, ToolError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed).to_string();
        let request =
            json!({ "id": id, "type": "tool_request", "tool": tool, "args": args, "guid": guid });
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
        self.send_and_await(id, framed, tool, target).await
    }

    /// The gif_creator recording sessions (ADR-0053), for the orchestrator handler.
    pub(crate) fn recordings(&self) -> &super::recording::RecordingStore {
        &self.recordings
    }

    /// While `tool`'s tab is actively recording, describe the action for overlay tagging
    /// (ADR-0052 D4 semantics, service-side per ADR-0053 D4). Model-space coordinates rescale to
    /// CSS viewport px by ASKING the extension (`rescale_coords`, an internal op over its live
    /// ScreenshotContext -- the mechanism data stays where Chrome produces it; querying beats
    /// mirroring). Best-effort: on any failure the raw coordinates stand (identical in the common
    /// unzoomed case).
    async fn note_gif_action(&self, guid: &str, tool: &str, args: &Value, target: u32) {
        if tool != "computer" && tool != "navigate" {
            return;
        }
        // `args` here is ALREADY the native (post-rewrite) form (ADR-0058: `Browser::call` passes
        // its `call_args`, not the caller's original), so this matches `handle_gif_frame`'s own
        // native-tabId keying of `self.recordings` -- both sides agree on the extension's own id.
        let Some(tab) = args.get("tabId").and_then(Value::as_i64) else {
            return;
        };
        if !self.recordings.is_active(tab) {
            return;
        }
        let Some(mut meta) = crate::gif::describe_action(tool, args) else {
            return;
        };
        meta.ts_ms = chrono::Utc::now().timestamp_millis();
        let mut points: Vec<Value> = Vec::new();
        if let Some((x, y)) = meta.coordinate {
            points.push(json!([x, y]));
        }
        if let Some((x, y)) = meta.start_coordinate {
            points.push(json!([x, y]));
        }
        if !points.is_empty() {
            let rescale_args = json!({ "tabId": tab, "points": points });
            if let Ok(reply) = self
                .raw_call(guid, "rescale_coords", &rescale_args, target)
                .await
            {
                if let Some(rescaled) = parse_rescaled_points(&reply) {
                    let mut it = rescaled.into_iter();
                    if meta.coordinate.is_some() {
                        meta.coordinate = it.next();
                    }
                    if meta.start_coordinate.is_some() {
                        meta.start_coordinate = it.next();
                    }
                }
            }
        }
        self.recordings.note_action(tab, meta);
    }

    /// One unsolicited `gif_frame` event from the extension's screencast relay (ADR-0053 D2):
    /// hand the base64 JPEG to the recording store (which drops it unless the tab is actively
    /// recording).
    fn handle_gif_frame(&self, event: &Value) {
        let Some(tab) = event.get("tabId").and_then(Value::as_i64) else {
            return;
        };
        let Some(data) = event.get("data").and_then(Value::as_str) else {
            return;
        };
        let ts = event
            .get("ts")
            .and_then(Value::as_i64)
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
        let device_width = event.get("deviceWidth").and_then(Value::as_f64);
        self.recordings.on_frame(tab, data, ts, device_width);
    }

    /// Re-encode every native tabId the extension reported in `result` back to composite form
    /// (ADR-0058), using the browser this call was actually routed to. See
    /// [`encode_tab_ids_in_value`] for the walk itself; this is just the `Browser::call` hook.
    fn encode_tab_ids(&self, mut result: Value, target: u32) -> Value {
        encode_tab_ids_in_value(&mut result, target);
        result
    }

    /// ADR-0050 Decision 4 -- the ONE sanctioned additive change to a trained tool's OUTPUT: after a
    /// `computer` result carrying a screenshot `image` content block, cache the image under `guid`
    /// and append a text block naming the minted imageId, so the model can later reference it with
    /// `upload_image`. Every other tool and every image-less `computer` result passes through
    /// untouched (the `computer` INPUT schema and its descriptor row are unchanged).
    fn cache_and_inject_screenshot(&self, guid: &str, tool: &str, mut result: Value) -> Value {
        if tool != "computer" {
            return result;
        }
        let image = result
            .get("content")
            .and_then(Value::as_array)
            .and_then(|blocks| {
                blocks
                    .iter()
                    .find(|b| b.get("type").and_then(Value::as_str) == Some("image"))
            });
        let Some(image) = image else {
            return result;
        };
        let Some(base64) = image
            .get("data")
            .and_then(Value::as_str)
            .map(str::to_string)
        else {
            return result;
        };
        let media_type = image
            .get("mimeType")
            .and_then(Value::as_str)
            .unwrap_or("image/jpeg")
            .to_string();
        let image_id = self.cache_screenshot(guid, base64, media_type);
        if let Some(content) = result.get_mut("content").and_then(Value::as_array_mut) {
            content.push(json!({
                "type": "text",
                "text": format!(
                    "[imageId: {image_id}] Reference this id with upload_image to place this \
                     screenshot into a file input or drag-drop target."
                ),
            }));
        }
        result
    }

    /// Cache a screenshot for `guid` and return its minted `img_...` imageId (ADR-0050 D4). Bounds
    /// the guid's deque to the last [`SCREENSHOT_CACHE_BOUND`] entries -- pushing a 9th evicts the
    /// oldest.
    pub(crate) fn cache_screenshot(
        &self,
        guid: &str,
        base64: String,
        media_type: String,
    ) -> String {
        let image_id = format!("img_{}", uuid::Uuid::new_v4().simple());
        let mut cache = self.screenshot_cache.lock().unwrap();
        let deque = cache.entry(guid.to_string()).or_default();
        deque.push_back((image_id.clone(), CachedImage { base64, media_type }));
        while deque.len() > SCREENSHOT_CACHE_BOUND {
            deque.pop_front();
        }
        image_id
    }

    /// Resolve a previously cached screenshot for `guid` by imageId (ADR-0050 D4), or None on a miss.
    pub(crate) fn resolve_cached_image(&self, guid: &str, image_id: &str) -> Option<CachedImage> {
        let cache = self.screenshot_cache.lock().unwrap();
        cache
            .get(guid)?
            .iter()
            .find(|(id, _)| id == image_id)
            .map(|(_, img)| img.clone())
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
        // ADR-0058: `tab_id` here is the composite MCP-facing id; decode to route to the owning
        // browser and query it by its own native id.
        let (target, native_tab) = self.resolve_target(Some(tab_id));
        let Some(target) = target else {
            let err = ToolError::extension("Browser extension not connected");
            self.debug.tool_begin("-", "tab_url_request");
            self.debug.tool_end("-", false, &err.to_string());
            return Err(err);
        };
        let native_tab = native_tab.unwrap_or(tab_id);

        let id = self.next_id.fetch_add(1, Ordering::Relaxed).to_string();
        let request = json!({ "id": id, "type": "tab_url_request", "tabId": native_tab });
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
        let result = self
            .send_and_await(id, framed, "tab_url_request", target)
            .await?;
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
        // ADR-0058: `tab_ids` are the composite, MCP-facing ids this session owns (mirrored
        // from `args.tabId` at claim time -- `hub::session::claim_tab_live`). A session's tabs
        // all belong to the SAME browser, so the first id's encoded pid picks the target; every
        // id is decoded to its native form for the extension, which never learns the encoding.
        let Some(&first) = tab_ids.first() else {
            return; // nothing to group
        };
        let (target, _) = self.resolve_target(Some(first));
        let Some(target) = target else {
            return;
        };
        let native_ids: Vec<i64> = tab_ids
            .iter()
            .map(|&t| crate::constants::tab_id::decode(t).1)
            .collect();
        let request = json!({
            "type": "group_request",
            "guid": guid,
            "tabIds": native_ids,
            "title": title,
        });
        let Ok(bytes) = serde_json::to_vec(&request) else {
            return;
        };
        let Ok(framed) = host::encode(&bytes) else {
            return;
        };
        self.send_fire_and_forget(target, framed);
    }

    /// Enqueue an already-framed, fire-and-forget message onto `target`'s session, dropping it
    /// silently if that browser is not (or no longer) attached -- the shared tail of
    /// [`Browser::request_group`], [`Browser::notify`], and [`Browser::send_hold_reply`].
    fn send_fire_and_forget(&self, target: u32, framed: Vec<u8>) {
        if let Some(session) = self.sessions.lock().unwrap().get(&target) {
            let _ = session.sender.send(framed);
        }
    }

    /// Push an on-screen notification to the extension: the SAME fire-and-forget,
    /// out-of-band-presentation posture as [`Browser::request_group`] above, just a general
    /// vocabulary instead of one narrow purpose. No `id`, no reply awaited, no policy decision
    /// made on the extension side -- the binary has ALREADY decided everything (`class`, `icon`,
    /// `title`, `description`); the extension only renders it. `title` is the always-shown
    /// headline (e.g. "Blocked - example.com"); `description` is an optional supporting line
    /// (e.g. "access is denied (sacred domain)"). This is deliberately NOT the extension's
    /// `caption()` mechanism: a caption is optional decorative flavor text, off by default; a
    /// notification is substantive and must never be silently gated behind that preference.
    /// First caller: a denial (SAPS PRES-HIGH-01) -- governance blocks a call before the
    /// extension is ever contacted for the call itself, so today nothing on screen shows a
    /// block happened. `tab_id: None` renders nothing (there is no always-visible "every tab"
    /// surface today; a future global-notification need can extend this, not narrow it). A
    /// missing/dead connection or an encoding failure is a harmless no-op, same reasoning as
    /// `request_group`: this is presentation, never a tool call.
    pub fn notify(
        &self,
        tab_id: Option<i64>,
        class: &str,
        icon: Option<&str>,
        title: &str,
        description: Option<&str>,
        reference: Option<&str>,
    ) {
        let Some(tab_id) = tab_id else {
            return;
        };
        // ADR-0058: `tab_id` is composite; decode to route to the owning browser and render on
        // its own native tab.
        let (target, native_tab) = self.resolve_target(Some(tab_id));
        let (Some(target), Some(native_tab)) = (target, native_tab) else {
            return;
        };
        let mut notification = json!({
            "type": "notification",
            "tabId": native_tab,
            "class": class,
            "title": title,
        });
        if let Some(icon) = icon {
            notification["icon"] = json!(icon);
        }
        if let Some(description) = description {
            notification["description"] = json!(description);
        }
        if let Some(reference) = reference {
            notification["ref"] = json!(reference);
        }
        let Ok(bytes) = serde_json::to_vec(&notification) else {
            return;
        };
        let Ok(framed) = host::encode(&bytes) else {
            return;
        };
        self.send_fire_and_forget(target, framed);
    }

    /// Shared send-and-await core behind [`Browser::call`] and [`Browser::tab_url`] (g13):
    /// register the pending reply slot, enqueue the already-framed bytes on `target`'s session if
    /// still attached (fail fast otherwise), and await the correlated reply up to
    /// [`TOOL_TIMEOUT`]. Each caller frames its own request first, since their encode-failure
    /// messages differ. `target` is resolved (ADR-0058) before this is called; a `target` that
    /// named a specific browser (via a decoded tabId) but is no longer attached gets a more
    /// specific message than the generic "not connected" the zero-browsers case gets.
    async fn send_and_await(
        &self,
        id: String,
        framed: Vec<u8>,
        debug_label: &str,
        target: u32,
    ) -> CallResult {
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id.clone(), tx);
        self.debug.tool_begin(&id, debug_label);

        // Enqueue only if `target`'s session is still attached; otherwise fail fast. The lock is
        // scoped so it is never held across the await below.
        let sent = {
            let sessions = self.sessions.lock().unwrap();
            match sessions.get(&target) {
                Some(session) => session.sender.send(framed).is_ok(),
                None => false,
            }
        };
        if !sent {
            self.pending.lock().unwrap().remove(&id);
            let err = if self.is_connected() {
                ToolError::extension("The browser that owns this tab is no longer connected")
                    .next_step("re-check tabs_context_mcp; this tab's browser may have closed")
            } else {
                ToolError::extension("Browser extension not connected")
            };
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

    /// Attach a connected native-host stream: read its `ROLE_BROWSER` session-hello (ADR-0058),
    /// admit it as an independent session keyed by the hello's `browserPid`, then spawn a writer
    /// draining outgoing frames to it and run a reader routing replies back to waiting callers.
    ///
    /// UNLIKE the pre-0058 single-slot design, a well-formed hello is ALWAYS admitted: a
    /// `browserPid` already present REPLACES the existing entry (a service-worker relaunch or
    /// reconnect from the SAME browser), and a NEW `browserPid` is ADDED alongside any others --
    /// this is the actual multi-browser support. [`AttachOutcome::AlreadyAttached`] is returned
    /// ONLY for a connection that never presents a valid hello at all: a bare probe (`doctor`'s
    /// harmless connect-and-close, which sends nothing) or a malformed/wrong-role frame. Either
    /// way nothing here is touched; the peer's stream is dropped and it observes EOF. This is
    /// what lets [`crate::transport::native::ipc::serve`] accept connections ahead of time
    /// (spawning `attach` per connection) so the pipe always has a spare instance ready, instead
    /// of parking the accept loop for the whole service lifetime.
    ///
    /// On [`AttachOutcome::Detached`] this ONE session's entry is removed (guarded against a
    /// same-pid reconnect race by comparing [`BrowserSession::generation`], never blindly
    /// cleared) and this session's own pending calls are failed via the grace-drain below;
    /// `is_connected()`/`wait_connected` reflect "at least one browser remains," recomputed after
    /// the removal.
    pub async fn attach<S>(&self, stream: S) -> AttachOutcome
    where
        S: AsyncRead + AsyncWrite + Send + 'static,
    {
        let (mut read_half, mut write_half) = tokio::io::split(stream);

        let hello_bytes = match host::read_message(&mut read_half).await {
            Ok(Some(bytes)) => bytes,
            _ => {
                // ADR-0059: distinguishable from a malformed hello below -- this is the ordinary
                // `doctor` probe shape (connect, read nothing, disconnect), expected traffic.
                self.debug.ipc_note(&Diagnostic::BareProbe.describe());
                return AttachOutcome::AlreadyAttached;
            }
        };
        let hello: Value = match serde_json::from_slice(&hello_bytes) {
            Ok(v) => v,
            Err(e) => {
                self.debug.ipc_note(
                    &Diagnostic::MalformedHello {
                        parse_error: &e.to_string(),
                    }
                    .describe(),
                );
                return AttachOutcome::AlreadyAttached;
            }
        };
        if hello.get("role").and_then(Value::as_str)
            != Some(ghostlight_transport::handshake::ROLE_BROWSER)
        {
            self.debug.ipc_note(
                &Diagnostic::WrongRole {
                    role: hello.get("role").and_then(Value::as_str),
                }
                .describe(),
            );
            return AttachOutcome::AlreadyAttached;
        }
        let browser_pid = hello.get("browserPid").and_then(Value::as_u64).unwrap_or(0) as u32;

        let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let generation = self.next_session_generation.fetch_add(1, Ordering::Relaxed);
        let replaced = {
            let mut sessions = self.sessions.lock().unwrap();
            let replaced = sessions.contains_key(&browser_pid);
            sessions.insert(
                browser_pid,
                BrowserSession {
                    sender: tx,
                    generation,
                },
            );
            replaced
        };
        self.debug.ipc_note(
            &Diagnostic::Attached {
                browser_pid,
                replaced_existing: replaced,
            }
            .describe(),
        );
        // A native-host stream attaching means the extension (re)connected -- which, because of
        // the extension's own storage-marker gate, only happens after the user's explicit
        // reconnect or a full browser restart (g11). `killed` stays GLOBAL (ADR-0058 scope): any
        // browser attaching clears it, matching the pre-0058 single-browser behavior.
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
                    self.route_reply(browser_pid, &payload);
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

        // Compare-before-remove (ADR-0058): only clear OUR OWN entry, never a newer one that has
        // already replaced it (a reconnect from the same browser that raced ahead of this reader
        // loop noticing its own stream died).
        {
            let mut sessions = self.sessions.lock().unwrap();
            if matches!(sessions.get(&browser_pid), Some(s) if s.generation == generation) {
                sessions.remove(&browser_pid);
                self.debug
                    .ipc_note(&Diagnostic::Detached { browser_pid }.describe());
            } else {
                self.debug
                    .ipc_note(&Diagnostic::DetachedStale { browser_pid }.describe());
            }
            self.focus_chain
                .lock()
                .unwrap()
                .retain(|p| *p != browser_pid);
        }
        let still_connected = self.is_connected();
        self.debug.set_connected(still_connected);
        self.connected.send_replace(still_connected);
        writer.abort();

        // ADR-0030 Decision 3 (H5): hold pending calls for a bounded grace window awaiting
        // reconnect instead of failing them the instant the stream closes. Spawned so `attach`
        // itself still returns `Detached` promptly regardless of the window's length -- neither
        // `ipc::serve`'s per-connection task nor any other caller here blocks on it. ADR-0058
        // simplification: this still waits on the GLOBAL `wait_connected` signal (any browser),
        // not specifically THIS browser reconnecting -- with more than one browser attached, a
        // pending call whose OWN browser stays gone can silently ride to its own `TOOL_TIMEOUT`
        // instead of the earlier, clearer grace-window message. Disclosed, accepted scope
        // boundary (see ADR-0058 "Explicitly out of scope"), not a silent gap.
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
    /// `session_killed`, no `id`), a focus event (ADR-0058: `{"type":"focus"}`, no `id`), a hold
    /// request (g10: `get_hold` / `set_hold` / `toggle_hold`, answered here and returned early),
    /// or a reply to a waiting tool caller (by id). Messages without an id are otherwise events.
    /// `browser_pid` is the SENDING session's identity (from its own `attach()` hello), needed to
    /// move the focus chain and to address a hold reply back to the right connection.
    fn route_reply(&self, browser_pid: u32, payload: &[u8]) {
        let Ok(reply) = serde_json::from_slice::<Value>(payload) else {
            tracing::warn!("dropping unparseable extension reply");
            return;
        };

        let msg_type = reply.get("type").and_then(Value::as_str);

        if reply.get("id").is_none() && msg_type == Some("session_killed") {
            self.handle_session_killed();
            return;
        }

        if reply.get("id").is_none() && msg_type == Some("focus") {
            self.note_focus(browser_pid);
            return;
        }

        // ADR-0059: the extension's own debug-mode lifecycle notes (connect attempts,
        // onDisconnect + chrome.runtime.lastError), forwarded ONLY when its local debug flag is
        // on. Appended into the SAME structured ring `ipc_note` already writes to, so one file
        // shows the extension's view interleaved with the service's own, by arrival order.
        if reply.get("id").is_none() && msg_type == Some("debug_event") {
            let event = reply.get("event").and_then(Value::as_str).unwrap_or("?");
            let detail = reply.get("detail").cloned().unwrap_or(Value::Null);
            self.debug.ipc_note(
                &Diagnostic::FromExtension {
                    browser_pid,
                    event,
                    detail: &detail,
                }
                .describe(),
            );
            return;
        }

        if reply.get("id").is_none() && msg_type == Some("gif_frame") {
            // gif_creator capture (ADR-0053 D2): the first unsolicited extension event beyond the
            // handshake. Unknown id-less event types keep falling through to the generic
            // event/heartbeat return below, so protocol skew stays harmless.
            self.handle_gif_frame(&reply);
            return;
        }

        if let (Some(id), Some(kind @ ("get_hold" | "set_hold" | "toggle_hold"))) =
            (reply.get("id").and_then(Value::as_str), msg_type)
        {
            self.handle_hold_request(browser_pid, id, kind, &reply);
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
    /// over the SAME connection it arrived on (`browser_pid`). `get_hold` reports without
    /// changing state; `set_hold` requires a boolean `held` member (a missing or non-boolean
    /// value is a `hold_error` that changes nothing); `toggle_hold` flips atomically. Every
    /// request receives the state AFTER the request was applied. `held`/`killed` stay GLOBAL
    /// (ADR-0058 scope), so the state itself does not depend on which browser asked.
    fn handle_hold_request(&self, browser_pid: u32, id: &str, kind: &str, request: &Value) {
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
        self.send_hold_reply(browser_pid, &reply);
    }

    /// Frame and enqueue a hold reply on `browser_pid`'s connection (the one it arrived on),
    /// dropping it silently if that session is already gone (the same fire-and-forget posture as
    /// every other best-effort send in this module).
    fn send_hold_reply(&self, browser_pid: u32, reply: &Value) {
        let Ok(bytes) = serde_json::to_vec(reply) else {
            tracing::warn!("failed to serialize a hold reply");
            return;
        };
        let Ok(framed) = host::encode(&bytes) else {
            tracing::warn!("failed to frame a hold reply");
            return;
        };
        self.send_fire_and_forget(browser_pid, framed);
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

/// The browser capability (ADR-0034): the outbound executor for the user's own authenticated
/// Chromium session. Implements [`super::ICapability`] by exposing the browser's tool directory
/// and agent guide; holds the [`Browser`] handle that the pipeline dispatches tool-calls through.
///
/// Constructed once at startup and registered in the composition root's [`super::Registry`].
#[derive(Clone)]
pub struct BrowserCapability {
    browser: Browser,
}

impl BrowserCapability {
    pub fn new(browser: Browser) -> Self {
        Self { browser }
    }

    /// The underlying [`Browser`] handle (the pipeline dispatches tool-calls through this).
    pub fn browser(&self) -> &Browser {
        &self.browser
    }
}

impl super::ICapability for BrowserCapability {
    fn code(&self) -> &'static str {
        "browser"
    }

    fn descriptor(&self) -> &'static str {
        "Drives the user's own authenticated Chromium session over the extension link."
    }

    fn directory(&self) -> &'static [crate::browser::directory::ToolDescriptor] {
        crate::browser::directory::REGISTRY
    }

    fn agent_guide(&self) -> crate::browser::directory::AgentGuide {
        crate::browser::directory::AGENT_GUIDE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    /// The default fake browser pid most tests attach as (ADR-0058): arbitrary, just needs to be
    /// nonzero and distinct from any REAL pid the test process itself might carry.
    const TEST_PID: u32 = 4242;

    /// A valid `ROLE_BROWSER` session-hello for `pid`, framed and ready to write.
    fn test_hello(pid: u32) -> Vec<u8> {
        ghostlight_transport::handshake::browser_hello_bytes(
            1,
            Some(ghostlight_transport::proc::ProcId { pid, created: 0 }),
        )
    }

    /// Attach a fake extension as `pid` (ADR-0058): create the duplex, write its hello, spawn
    /// `attach()`, and wait for `browser` to report connected. Returns the extension-side half
    /// for the test to drive. The common case for every test below that does not care about a
    /// SPECIFIC pid value.
    async fn attach_fake_extension_as(browser: &Browser, pid: u32) -> tokio::io::DuplexStream {
        let (browser_side, mut ext_side) = tokio::io::duplex(64 * 1024);
        host::write_message(&mut ext_side, &test_hello(pid))
            .await
            .unwrap();
        let attached = browser.clone();
        tokio::spawn(async move { attached.attach(browser_side).await });
        // Poll for THIS pid specifically (not `wait_connected`'s global is_connected(), which a
        // SECOND attach -- a different pid, or the same one reconnecting -- would see as already
        // true from an EARLIER session and return without ever giving the just-spawned task a
        // chance to run). Sleep-then-check (not check-then-sleep) so at least one scheduling
        // tick always happens before the first check, even on a single-threaded test runtime.
        for _ in 0..200 {
            sleep(Duration::from_millis(5)).await;
            if browser.browser_snapshot().iter().any(|b| b.pid == pid) {
                return ext_side;
            }
        }
        panic!("browser never registered pid {pid} as connected");
    }

    async fn attach_fake_extension(browser: &Browser) -> tokio::io::DuplexStream {
        attach_fake_extension_as(browser, TEST_PID).await
    }

    #[tokio::test]
    async fn call_round_trips_a_tool_response() {
        let browser = Browser::new();
        let mut ext_side = attach_fake_extension(&browser).await;

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

        let result = browser
            .call(
                "test-guid",
                "navigate",
                &json!({ "url": "https://example.com" }),
            )
            .await
            .unwrap();
        assert_eq!(result, json!({ "echoed": "navigate" }));
        fake_ext.await.unwrap();
    }

    #[tokio::test]
    async fn call_surfaces_a_tool_error() {
        let browser = Browser::new();
        let mut ext_side = attach_fake_extension(&browser).await;

        tokio::spawn(async move {
            let req = host::read_message(&mut ext_side).await.unwrap().unwrap();
            let v: Value = serde_json::from_slice(&req).unwrap();
            let reply = json!({ "id": v["id"], "type": "tool_error", "error": "boom" });
            host::write_message(&mut ext_side, &serde_json::to_vec(&reply).unwrap())
                .await
                .unwrap();
        });

        let err = browser
            .call("test-guid", "javascript_tool", &json!({}))
            .await
            .unwrap_err();
        let text = err.to_string();
        assert!(text.starts_with("[hop: extension]"), "{text}");
        assert!(text.contains("boom"), "{text}");
    }

    #[tokio::test]
    async fn call_without_a_connection_fails_fast() {
        let browser = Browser::new();
        let err = browser
            .call("test-guid", "navigate", &json!({}))
            .await
            .unwrap_err();
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
        let browser = Browser::new();
        let mut ext_side = attach_fake_extension(&browser).await;

        // ADR-0058: tab_ids are composite (encoding TEST_PID); the wire shows the DECODED
        // native ids the extension actually understands.
        let composite_ids = [
            crate::constants::tab_id::encode(TEST_PID, 101),
            crate::constants::tab_id::encode(TEST_PID, 202),
        ];
        browser.request_group(
            "11111111-1111-4111-8111-111111111111",
            &composite_ids,
            "title",
        );

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
        let result = browser
            .call("test-guid", "navigate", &json!({}))
            .await
            .unwrap();
        assert_eq!(result, json!({ "ok": true }));
        fake_ext.await.unwrap();
    }

    #[tokio::test]
    async fn call_surfaces_a_cdp_tagged_tool_error_without_leaking_detail() {
        let browser = Browser::new();
        let mut ext_side = attach_fake_extension(&browser).await;

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

        let err = browser
            .call("test-guid", "computer", &json!({}))
            .await
            .unwrap_err();
        let text = err.to_string();
        assert!(text.starts_with("[hop: cdp]"), "{text}");
        assert!(text.contains("Input.dispatchMouseEvent failed"), "{text}");
        assert!(!text.contains("verbose internals"), "{text}");
    }

    #[tokio::test]
    async fn call_surfaces_a_page_tagged_tool_error() {
        let browser = Browser::new();
        let mut ext_side = attach_fake_extension(&browser).await;

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

        let err = browser
            .call("test-guid", "form_input", &json!({}))
            .await
            .unwrap_err();
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
        let (browser_side, mut ext_side) = tokio::io::duplex(64 * 1024);
        let browser = Browser::new();
        // Written before attach() ever starts reading; the duplex buffers it, so the delayed
        // attach() below finds its hello waiting immediately once it runs.
        host::write_message(&mut ext_side, &test_hello(TEST_PID))
            .await
            .unwrap();

        let attached = browser.clone();
        tokio::spawn(async move {
            sleep(Duration::from_millis(50)).await;
            let _ = attached.attach(browser_side).await;
        });

        let ready = browser.wait_connected(Duration::from_secs(2)).await;
        assert!(ready, "wait_connected must wake once attach() connects");
    }

    /// A bare probe connection (no hello at all -- `doctor`'s harmless connect-and-close) is
    /// rejected without touching any live session's state, exactly as a stray was pre-ADR-0058.
    #[tokio::test]
    async fn a_hello_less_probe_is_rejected_without_disturbing_the_live_session() {
        let browser = Browser::new();
        let mut ext_side = attach_fake_extension(&browser).await;

        let (probe_side, probe_ext) = tokio::io::duplex(64 * 1024);
        drop(probe_ext); // closes immediately, sending no hello -- exactly a bare probe
        let outcome = browser.attach(probe_side).await;
        assert_eq!(outcome, AttachOutcome::AlreadyAttached);
        assert!(
            browser.is_connected(),
            "the live session must stay connected after a hello-less probe"
        );

        // ...and the live session still round-trips a call.
        let ext = tokio::spawn(async move {
            let req = host::read_message(&mut ext_side).await.unwrap().unwrap();
            let v: Value = serde_json::from_slice(&req).unwrap();
            let reply = json!({ "id": v["id"], "type": "tool_response", "result": { "ok": true } });
            host::write_message(&mut ext_side, &serde_json::to_vec(&reply).unwrap())
                .await
                .unwrap();
        });
        let result = browser
            .call("test-guid", "navigate", &json!({}))
            .await
            .unwrap();
        assert_eq!(result, json!({ "ok": true }));
        ext.await.unwrap();
    }

    /// ADR-0058: two DIFFERENT browsers (distinct pids) are BOTH admitted as independent, live
    /// sessions -- the actual multi-browser support this whole change exists for. Each one's
    /// own tab (encoded with its own pid) routes a call to that SPECIFIC session, never the
    /// other.
    #[tokio::test]
    async fn two_different_browsers_are_both_admitted_and_route_independently() {
        let browser = Browser::new();
        let mut first_ext = attach_fake_extension_as(&browser, 1001).await;
        let mut second_ext = attach_fake_extension_as(&browser, 2002).await;
        assert!(browser.is_connected());
        assert_eq!(browser.browser_snapshot().len(), 2);

        let first_task = tokio::spawn(async move {
            let req = host::read_message(&mut first_ext).await.unwrap().unwrap();
            let v: Value = serde_json::from_slice(&req).unwrap();
            assert_eq!(
                v["args"]["tabId"], 5,
                "browser 1's request carries its own native tabId"
            );
            let reply =
                json!({ "id": v["id"], "type": "tool_response", "result": { "from": "first" } });
            host::write_message(&mut first_ext, &serde_json::to_vec(&reply).unwrap())
                .await
                .unwrap();
        });
        let result = browser
            .call(
                "test-guid",
                "navigate",
                &json!({ "tabId": crate::constants::tab_id::encode(1001, 5) }),
            )
            .await
            .unwrap();
        assert_eq!(result, json!({ "from": "first" }));
        first_task.await.unwrap();

        let second_task = tokio::spawn(async move {
            let req = host::read_message(&mut second_ext).await.unwrap().unwrap();
            let v: Value = serde_json::from_slice(&req).unwrap();
            assert_eq!(
                v["args"]["tabId"], 9,
                "browser 2's request carries its own native tabId"
            );
            let reply =
                json!({ "id": v["id"], "type": "tool_response", "result": { "from": "second" } });
            host::write_message(&mut second_ext, &serde_json::to_vec(&reply).unwrap())
                .await
                .unwrap();
        });
        let result = browser
            .call(
                "test-guid",
                "navigate",
                &json!({ "tabId": crate::constants::tab_id::encode(2002, 9) }),
            )
            .await
            .unwrap();
        assert_eq!(result, json!({ "from": "second" }));
        second_task.await.unwrap();
    }

    /// ADR-0058: a fresh hello for a pid ALREADY attached REPLACES the old session (a
    /// service-worker relaunch or reconnect from the SAME browser), rather than being rejected.
    #[tokio::test]
    async fn a_reconnect_from_the_same_pid_replaces_the_old_session() {
        let browser = Browser::new();
        let _first_ext = attach_fake_extension_as(&browser, TEST_PID).await;
        assert_eq!(browser.browser_snapshot().len(), 1);

        let mut second_ext = attach_fake_extension_as(&browser, TEST_PID).await;
        assert_eq!(
            browser.browser_snapshot().len(),
            1,
            "the same pid reconnecting replaces, never adds a second entry"
        );

        // The NEW connection serves calls; the old one is simply not read from again.
        let ext = tokio::spawn(async move {
            let req = host::read_message(&mut second_ext).await.unwrap().unwrap();
            let v: Value = serde_json::from_slice(&req).unwrap();
            let reply = json!({ "id": v["id"], "type": "tool_response", "result": { "ok": true } });
            host::write_message(&mut second_ext, &serde_json::to_vec(&reply).unwrap())
                .await
                .unwrap();
        });
        let result = browser
            .call("test-guid", "navigate", &json!({}))
            .await
            .unwrap();
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
        let browser = Browser::new();
        let mut ext_side = attach_fake_extension(&browser).await;

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
        let browser = Browser::new();
        browser.set_held(true);
        let ext_side = attach_fake_extension(&browser).await;

        drop(ext_side);
        // Let the reader loop observe the close and remove its session before asserting.
        for _ in 0..200 {
            if !browser.is_connected() {
                break;
            }
            sleep(Duration::from_millis(5)).await;
        }

        assert!(
            browser.held_for().is_some(),
            "the hold must survive the extension disconnecting"
        );
    }

    /// Test 1a (g11 spec section 9): the kill event fails an in-flight call with the exact
    /// section-7 error, and the extension never sees a reply.
    #[tokio::test]
    async fn kill_fails_in_flight_calls() {
        let browser = Browser::new();
        let mut ext_side = attach_fake_extension(&browser).await;

        let caller = browser.clone();
        let call_task =
            tokio::spawn(async move { caller.call("test-guid", "navigate", &json!({})).await });

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
        let browser = Browser::new();
        let mut ext_side = attach_fake_extension(&browser).await;

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

        let result = tokio::time::timeout(
            Duration::from_secs(1),
            browser.call("test-guid", "navigate", &json!({})),
        )
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
        let browser = Browser::new();
        let mut ext_side = attach_fake_extension(&browser).await;

        host::write_message(
            &mut ext_side,
            &serde_json::to_vec(&json!({ "type": "session_killed" })).unwrap(),
        )
        .await
        .unwrap();
        drop(ext_side);
        for _ in 0..200 {
            if !browser.is_connected() {
                break;
            }
            sleep(Duration::from_millis(5)).await;
        }

        let err = browser
            .call("test-guid", "navigate", &json!({}))
            .await
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("The user ended the browser session (kill switch)"),
            "{err}"
        );
    }

    /// Test 1d: a fresh attach clears the kill; a call round-trips normally afterward.
    #[tokio::test]
    async fn fresh_attach_clears_the_kill() {
        let browser = Browser::new();
        let mut first_ext = attach_fake_extension_as(&browser, TEST_PID).await;

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

        // Tear down the first connection (a real "session ended") and wait for it to be
        // removed before attaching a fresh one -- reconnecting from the SAME pid replaces the
        // entry outright (ADR-0058), but this exercises the ordinary disconnect-then-reconnect
        // path too.
        drop(first_ext);
        for _ in 0..200 {
            if !browser.is_connected() {
                break;
            }
            sleep(Duration::from_millis(5)).await;
        }

        let mut second_ext = attach_fake_extension_as(&browser, TEST_PID).await;
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
        let result = browser
            .call("test-guid", "navigate", &json!({}))
            .await
            .unwrap();
        assert_eq!(result, json!({ "ok": true }));
        fake_ext.await.unwrap();
    }

    /// Test 1e: the hook fires exactly once even if two kill frames arrive on the same
    /// connection.
    #[tokio::test]
    async fn kill_hook_fires_exactly_once_per_transition() {
        let browser = Browser::new();
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let hook_count = Arc::clone(&count);
        browser.on_session_killed(move || {
            hook_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
        let mut ext_side = attach_fake_extension(&browser).await;

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

    /// ADR-0050 D4: the per-session screenshot cache mints an `img_`-prefixed id, round-trips the
    /// bytes/media-type, misses on an unknown id/guid, is bounded to 8 (a 9th insert evicts the
    /// 1st), and `cache_and_inject_screenshot` appends exactly one imageId text block to a
    /// `computer` screenshot result while passing every other result through unchanged. (Named with
    /// a snake_case tail rather than the prompt's `imageId` to satisfy `-D warnings`.)
    #[test]
    fn screenshot_cache_round_trips_and_injects_image_id() {
        let browser = Browser::new();

        // Cache + resolve round trip.
        let id = browser.cache_screenshot("g1", "AAAA".to_string(), "image/jpeg".to_string());
        assert!(id.starts_with("img_"), "minted id is img_-prefixed: {id}");
        let got = browser
            .resolve_cached_image("g1", &id)
            .expect("cached image resolves");
        assert_eq!(got.base64, "AAAA");
        assert_eq!(got.media_type, "image/jpeg");

        // Unknown id, and a different guid, both miss.
        assert!(browser.resolve_cached_image("g1", "img_nope").is_none());
        assert!(browser.resolve_cached_image("other", &id).is_none());

        // Bound N=8: after a 9th insert into one guid, the first id is evicted.
        let first = browser.cache_screenshot("g2", "0".to_string(), "image/jpeg".to_string());
        for i in 1..8 {
            browser.cache_screenshot("g2", i.to_string(), "image/jpeg".to_string());
        }
        assert!(
            browser.resolve_cached_image("g2", &first).is_some(),
            "the 8th entry keeps the first cached"
        );
        let ninth = browser.cache_screenshot("g2", "9".to_string(), "image/jpeg".to_string());
        assert!(
            browser.resolve_cached_image("g2", &first).is_none(),
            "the 9th insert evicts the 1st"
        );
        assert!(
            browser.resolve_cached_image("g2", &ninth).is_some(),
            "the newest entry stays"
        );

        // Injection: a computer screenshot result gains exactly one trailing imageId text block;
        // the leading text and the image block are preserved, and the id resolves in g3's cache.
        let result = json!({
            "content": [
                { "type": "text", "text": "screenshot taken" },
                { "type": "image", "data": "BBBB", "mimeType": "image/png" }
            ]
        });
        let injected = browser.cache_and_inject_screenshot("g3", "computer", result);
        let content = injected["content"].as_array().unwrap();
        assert_eq!(content.len(), 3, "exactly one trailing text block appended");
        assert_eq!(content[1]["type"], "image", "the image block is preserved");
        let text = content[2]["text"].as_str().unwrap();
        assert!(
            text.starts_with("[imageId: img_"),
            "the trailing block names the minted imageId: {text}"
        );
        let injected_id = text
            .strip_prefix("[imageId: ")
            .and_then(|s| s.split(']').next())
            .unwrap();
        let cached = browser
            .resolve_cached_image("g3", injected_id)
            .expect("the injected id resolves in the cache");
        assert_eq!(cached.base64, "BBBB");
        assert_eq!(cached.media_type, "image/png");

        // A non-computer result, and a computer result with no image, pass through byte-unchanged.
        let navigate = json!({"content":[{"type":"text","text":"ok"}]});
        assert_eq!(
            browser.cache_and_inject_screenshot("g3", "navigate", navigate.clone()),
            navigate
        );
        let click = json!({"content":[{"type":"text","text":"clicked"}]});
        assert_eq!(
            browser.cache_and_inject_screenshot("g3", "computer", click.clone()),
            click
        );
    }
}
