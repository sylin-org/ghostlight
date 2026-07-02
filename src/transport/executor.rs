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

use crate::debug::DebugSink;
use crate::transport::native::host;
use crate::ToolError;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot, watch};

/// How long to wait for the extension to answer a single tool call before giving up.
const TOOL_TIMEOUT: Duration = Duration::from_secs(60);

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
        }
    }

    /// The observability sink (used by the mcp-server to record the MCP boundary).
    pub fn debug(&self) -> &DebugSink {
        &self.debug
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
        let id = self.next_id.fetch_add(1, Ordering::Relaxed).to_string();
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id.clone(), tx);
        self.debug.tool_begin(&id, tool);

        let request = json!({ "id": id, "type": "tool_request", "tool": tool, "args": args });
        let framed = match serde_json::to_vec(&request)
            .map_err(|e| e.to_string())
            .and_then(|bytes| host::encode(&bytes).map_err(|e| e.to_string()))
        {
            Ok(framed) => framed,
            Err(e) => {
                self.pending.lock().unwrap().remove(&id);
                let err = ToolError::binary(format!("failed to encode the tool request: {e}"));
                self.debug.tool_end(&id, false, &err.to_string());
                return Err(err);
            }
        };

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
        for (_, tx) in self.pending.lock().unwrap().drain() {
            let _ = tx.send(Err(drain_err.clone()));
        }
        AttachOutcome::Detached
    }

    /// Route one framed reply to its waiting caller (by id). Replies without an id are events.
    fn route_reply(&self, payload: &[u8]) {
        let Ok(reply) = serde_json::from_slice::<Value>(payload) else {
            tracing::warn!("dropping unparseable extension reply");
            return;
        };
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
}
