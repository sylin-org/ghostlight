// SPDX-License-Identifier: Apache-2.0 OR MIT
//! In-process test fixture (ADR-0051 Phase 4): drive the REAL MCP session chokepoint
//! (`transport::mcp::server::serve_session`) over an in-memory `tokio::io::duplex` pipe, wired to a
//! REAL `Browser` (optionally attached to a drivable fake extension over a second duplex) and a
//! REAL `ServiceContext` built by the same `ServiceContext::from_startup` a spawned service uses --
//! with NO spawned OS process, no stdio, no exe-lock contention.
//!
//! This is the seam the incidentally-end-to-end wiring tests (tool_enforcement, tool_advertisement,
//! shadow_mode, most of mcp_protocol, ...) are migrated onto in P4.2: the SAME code path
//! (`serve_session` -> governance `decide` -> dispatch), the SAME JSON-RPC-over-a-stream wire, none
//! of the process/stdio/exe-lock flakiness that made the spawn tier fragile on a live dev machine.
//!
//! The two seams this generalizes previously lived inline: a `Browser` over `tokio::io::duplex`
//! plus a fake extension (`tests/hub_multiplex.rs`), and an all-open `Governance` reached through
//! the server loop (`tests/all_open_golden.rs`). Here they are one reusable [`Harness`].
//!
//! RUNTIME FLAVOR: a test that drives a tool which ORCHESTRATES internal sub-calls -- `script`, and
//! a non-denied `form_fill` -- must use `#[tokio::test(flavor = "multi_thread", worker_threads =
//! 2)]`. Those tools re-enter the runtime via `tokio::task::block_in_place` +
//! `Handle::block_on` (`crates/core/src/mcp/script.rs`), which panics on the default current-thread
//! test runtime; the panic surfaces inside the spawned `tools/call` task, so the only visible
//! symptom is that [`Harness::drive`] hangs waiting for a reply that never comes. Plain
//! (non-orchestrating) tool calls and denied-before-dispatch cases run fine on the default runtime.

#![allow(dead_code)]

use ghostlight::browser::pattern::is_valid_pattern;
use ghostlight::governance::manifest::document::{parse_manifest, Manifest};
use ghostlight::governance::manifest::source::{LoadedPolicy, ManifestOrigin};
use ghostlight::hub::outbound::browser::Browser;
use ghostlight::hub::role::{set_role, Role};
use ghostlight::hub::session::SessionGuid;
use ghostlight::hub::ServiceContext;
use ghostlight::mcp::server::serve_session;
use ghostlight::native::host;
use ghostlight::observability::DebugSink;
use serde_json::{json, Value};
use std::sync::Once;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// The governance chokepoint asserts this process's role is `Service` (ADR-0030 Decision 1
/// addendum; `ghostlight_transport::role`). A spawned service records its role during startup; an
/// in-process harness must record it once per test binary, before the first `serve_session`.
/// `set_role` panics if called twice, so a `Once` guards it. Only the fixture ever calls this, and
/// nothing an in-process session reaches asserts the `Adapter` role, so recording `Service` here is
/// inert for any binary that also spawns real subprocesses (those run in their own processes).
static ROLE_ONCE: Once = Once::new();
fn ensure_service_role() {
    ROLE_ONCE.call_once(|| set_role(Role::Service));
}

/// Parse a JSON `Value` into a validated schema-3 [`Manifest`], the way a `--manifest file://`
/// source would, so a governed [`Harness`] can be built from the exact manifest shape the
/// spawn-based tests already author. Panics if the manifest is invalid (a test bug).
pub fn manifest_from_value(value: &Value) -> Manifest {
    parse_manifest(
        &value.to_string(),
        "in-proc-test-manifest",
        is_valid_pattern,
    )
    .expect("the in-process test manifest parses and validates")
}

/// A real, in-process service session substrate: one [`ServiceContext`] (built once via
/// `from_startup`, exactly as a spawned service builds it) that [`Harness::drive`] clones per
/// session. Construct inside a `#[tokio::test]` -- `from_startup` spawns background tasks and so
/// requires an active tokio runtime.
pub struct Harness {
    ctx: ServiceContext,
}

impl Harness {
    /// All-open (no manifest): behavior is byte-identical to a spawned all-open service.
    pub fn all_open() -> Self {
        Self::build(LoadedPolicy {
            manifest: None,
            origin: None,
            user_manifest_ignored: false,
        })
    }

    /// Governed by `manifest` at the user-file layer: grants are enforced, and the manifest's own
    /// `config` entries (e.g. `audit.*`) apply at the user config layer exactly as a `--manifest
    /// file://` spawn resolves them, so an audit-asserting test can point `audit.file.path` at a
    /// temp file and read it back -- still with no spawned process.
    pub fn governed(manifest: Manifest) -> Self {
        Self::build(LoadedPolicy {
            manifest: Some(manifest),
            origin: Some(ManifestOrigin::UserFile),
            user_manifest_ignored: false,
        })
    }

    fn build(policy: LoadedPolicy) -> Self {
        ensure_service_role();
        let browser = Browser::new();
        let ctx = ServiceContext::from_startup(
            browser,
            DebugSink::disabled(),
            policy,
            ghostlight::governance::config::reload::PolicySource::SourceString { user_source: None },
            None,
        )
        .expect("build the in-process ServiceContext");
        Self { ctx }
    }

    /// Attach a drivable fake extension to this harness's `Browser`. Every framed `tool_request`
    /// the service dispatches is handed to `responder` (the parsed request `Value`); `responder`'s
    /// return `Value` becomes the `result` of a framed `tool_response` echoed back by the request's
    /// `id`. Blocks until the `Browser` reports connected. Without this, a `tools/call` reaches
    /// dispatch and returns the familiar `not connected` execution error -- which is exactly the
    /// signal most enforcement/advertisement wiring tests assert on, so most callers never attach.
    pub async fn attach_fake_extension<F>(&self, responder: F)
    where
        F: Fn(&Value) -> Value + Send + 'static,
    {
        let (browser_side, mut ext_side) = tokio::io::duplex(64 * 1024);
        let attached = self.ctx.browser.clone();
        tokio::spawn(async move {
            let _ = attached.attach(browser_side).await;
        });
        tokio::spawn(async move {
            while let Ok(Some(req)) = host::read_message(&mut ext_side).await {
                let v: Value = match serde_json::from_slice(&req) {
                    Ok(v) => v,
                    Err(_) => break,
                };
                let reply = json!({
                    "id": v["id"],
                    "type": "tool_response",
                    "result": responder(&v),
                });
                if host::write_message(&mut ext_side, &serde_json::to_vec(&reply).unwrap())
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });

        for _ in 0..400 {
            if self.ctx.browser.is_connected() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("the fake extension never reported connected");
    }

    /// Drive `requests` (each serialized as one newline-delimited JSON-RPC line) through a FRESH
    /// in-process session and return the responses to the `id`-bearing requests, in arrival order.
    /// Look responses up by `id`: `tools/call` runs concurrently (each spawns its own task), so
    /// arrival order does not track request order -- identical to the spawn-based `drive` helpers.
    pub async fn drive(&self, requests: &[Value]) -> Vec<Value> {
        let (client, server) = tokio::io::duplex(256 * 1024);
        let ctx = self.ctx.clone();
        let guid = SessionGuid::mint();
        let session = tokio::spawn(async move {
            let _ = serve_session(server, ctx, guid).await;
        });

        let (read_half, mut write_half) = tokio::io::split(client);
        for req in requests {
            let mut line = serde_json::to_vec(req).expect("serialize request");
            line.push(b'\n');
            write_half
                .write_all(&line)
                .await
                .expect("write request line");
        }
        write_half.flush().await.ok();

        // Read the id-bearing replies BEFORE closing the write half: closing it signals EOF, which
        // ends the session -- so keep it open until every expected reply is in hand (the same order
        // the spawn-based `drive` helpers use when they drop the adapter's stdin last).
        let expected = requests.iter().filter(|r| r.get("id").is_some()).count();
        let mut lines = BufReader::new(read_half).lines();
        let mut responses = Vec::with_capacity(expected);
        for _ in 0..expected {
            let line = lines
                .next_line()
                .await
                .expect("read a response line")
                .expect("the session closed before every expected reply arrived");
            responses.push(serde_json::from_str(&line).expect("each response line is JSON"));
        }

        drop(write_half);
        drop(lines);
        let _ = session.await;
        responses
    }

    /// Like [`Harness::drive`], but writes each `&str` line VERBATIM (so a malformed frame or a
    /// JSON-RPC batch array can be exercised) and reads EXACTLY `expected` responses. The ADR-0049
    /// parse-error / batch rejects reply with `id: null`, so they cannot be counted by id-presence
    /// the way [`Harness::drive`] does; the caller states `expected` directly.
    pub async fn drive_raw(&self, lines: &[&str], expected: usize) -> Vec<Value> {
        let (client, server) = tokio::io::duplex(256 * 1024);
        let ctx = self.ctx.clone();
        let guid = SessionGuid::mint();
        let session = tokio::spawn(async move {
            let _ = serve_session(server, ctx, guid).await;
        });

        let (read_half, mut write_half) = tokio::io::split(client);
        for line in lines {
            write_half
                .write_all(line.as_bytes())
                .await
                .expect("write raw line");
            write_half.write_all(b"\n").await.expect("write newline");
        }
        write_half.flush().await.ok();

        let mut reader = BufReader::new(read_half).lines();
        let mut responses = Vec::with_capacity(expected);
        for _ in 0..expected {
            let line = reader
                .next_line()
                .await
                .expect("read a response line")
                .expect("the session closed before every expected reply arrived");
            responses.push(serde_json::from_str(&line).expect("each response line is JSON"));
        }

        drop(write_half);
        drop(reader);
        let _ = session.await;
        responses
    }
}

/// The `[initialize, tools/call name(arguments)]` request pair every call-driving test opens with
/// (mirrors `tests/tool_enforcement.rs::init_and_call`).
pub fn init_and_call(name: &str, arguments: Value) -> Vec<Value> {
    vec![
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":name,"arguments":arguments}}),
    ]
}

/// Find the response to request `id` (never rely on position; see [`Harness::drive`]).
pub fn by_id(responses: &[Value], id: i64) -> &Value {
    responses
        .iter()
        .find(|r| r["id"] == id)
        .unwrap_or_else(|| panic!("no response with id {id} in {responses:?}"))
}

/// The first text content block of a tool result (panics if absent).
pub fn text_of(resp: &Value) -> &str {
    resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("no text content block in {resp:?}"))
}
