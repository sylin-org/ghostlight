// SPDX-License-Identifier: Apache-2.0 OR MIT
//! In-process test fixture for the ADR-0096 protocol-neutral service boundary.
//!
//! The fixture builds the same [`ServiceContext`] as the persistent service, projects its canonical
//! catalog, and executes immutable [`ghostlight::work::WorkContext`] values through
//! [`ghostlight::tool::pipeline::run_work`]. It optionally attaches a drivable fake extension over
//! an in-memory duplex. There is no MCP lifecycle, JSON-RPC parser, stdio, or spawned process here;
//! exact protocol behavior belongs to the date-named handlers in `crates/mcp-connector`.
//!
//! Integration tests supply request-shaped JSON values to [`Harness::drive`] as a compact test
//! instruction format. Calls carry a serialized canonical [`BrowserOperation`], and catalog
//! requests return a canonical service projection. This fixture deliberately does not emulate a
//! model-facing surface or MCP wire implementation.
//!
//! RUNTIME FLAVOR: a test that drives a tool which ORCHESTRATES internal sub-calls -- `script`, and
//! a non-denied `form_fill` -- must use `#[tokio::test(flavor = "multi_thread", worker_threads =
//! 2)]`. Those tools re-enter the runtime via `tokio::task::block_in_place` +
//! `Handle::block_on` (`crates/core/src/tool/script.rs`), which panics on the default current-thread
//! test runtime; the panic surfaces inside the spawned call task, so the only visible
//! symptom is that [`Harness::drive`] hangs waiting for a result that never comes. Plain
//! (non-orchestrating) tool calls and denied-before-dispatch cases run fine on the default runtime.

#![allow(dead_code)]

use ghostlight::browser::pattern::is_valid_pattern;
use ghostlight::governance::manifest::document::{parse_manifest, Manifest};
use ghostlight::governance::manifest::source::{LoadedPolicy, ManifestOrigin};
use ghostlight::governance::ports::ClientInfo;
use ghostlight::hub::outbound::browser::Browser;
use ghostlight::hub::peer::PeerUser;
use ghostlight::hub::ServiceContext;
use ghostlight::native::host;
use ghostlight::observability::DebugSink;
use ghostlight::tool::outcome::CallOutcome;
use ghostlight::work::{CancellationToken, WorkContext};
use ghostlight_transport::bridge::WorkspaceUse;
use ghostlight_transport::operation::{BrowserOperation, IntentId, OperationId};
use serde_json::{json, Value};
use std::time::Duration;

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

/// A real in-process service substrate built once through [`ServiceContext::from_startup`].
/// Construct inside a `#[tokio::test]`; startup spawns background tasks and requires an active
/// tokio runtime.
pub struct Harness {
    ctx: ServiceContext,
}

impl Harness {
    /// Build an all-open service with no manifest.
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
        let browser = Browser::new();
        let ctx = ServiceContext::from_startup(
            browser,
            DebugSink::disabled(),
            policy,
            ghostlight::governance::config::reload::PolicySource::SourceString {
                user_source: None,
            },
            None,
        )
        .expect("build the in-process ServiceContext");
        Self { ctx }
    }

    /// Attach a drivable fake extension to this harness's `Browser`. Every framed `tool_request`
    /// the service dispatches is handed to `responder` (the parsed request `Value`); `responder`'s
    /// return `Value` becomes the `result` of a framed `tool_response` echoed back by the request's
    /// `id`. Blocks until the `Browser` reports connected. Without this, a canonical call reaches
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
            // ADR-0058/0061: send the relay hello then the extension's opening identity frame. The
            // service assigns this fake browser a slot; the plain, un-encoded small tabIds every
            // caller of this fixture uses decode to slot 0, which `resolve_target` treats as
            // "unrouted" and resolves to this sole focus-front browser -- so no caller needs to know
            // about composite encoding.
            let hello = ghostlight_transport::handshake::browser_hello_bytes(1, None);
            if host::write_message(&mut ext_side, &hello).await.is_err() {
                return;
            }
            let identity = serde_json::to_vec(&json!({
                "type": ghostlight_transport::handshake::EXTENSION_IDENTITY_TYPE,
                ghostlight_transport::handshake::BROWSER_ID_FIELD: "inproc-fixture",
            }))
            .unwrap();
            if host::write_message(&mut ext_side, &identity).await.is_err() {
                return;
            }
            while let Ok(Some(req)) = host::read_message(&mut ext_side).await {
                let v: Value = match serde_json::from_slice(&req) {
                    Ok(v) => v,
                    Err(_) => break,
                };
                let response_type = if v["type"] == "tab_url_request" {
                    "tab_url_response"
                } else {
                    "tool_response"
                };
                let reply = json!({
                    "id": v["id"],
                    "type": response_type,
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

    /// Translate request-shaped test instructions through one fresh service-owned workspace.
    ///
    /// This convenience surface preserves the result-shaped JSON consumed by older governance and
    /// dispatch tests. It is not an MCP state machine: framing, lifecycle validation, request
    /// correlation, and revision envelopes are tested by `crates/mcp-connector`.
    pub async fn drive(&self, requests: &[Value]) -> Vec<Value> {
        let owner = PeerUser("inproc-fixture".to_string());
        let workspace = self
            .ctx
            .workspaces
            .mint(&owner, true)
            .expect("mint the fixture workspace");
        let mut client = None;
        let mut responses = Vec::new();

        for request in requests {
            let Some(id) = request.get("id").cloned() else {
                continue;
            };
            let method = request
                .get("method")
                .and_then(Value::as_str)
                .expect("fixture instruction has a method");
            let result = match method {
                "initialize" => {
                    client = request_client(request);
                    json!({})
                }
                "operations/list" => self.catalog_result(),
                "operations/call" => {
                    let params = request
                        .get("params")
                        .expect("canonical call fixture instruction has params");
                    let operation = serde_json::from_value::<BrowserOperation>(
                        params
                            .get("operation")
                            .cloned()
                            .expect("canonical call fixture instruction has an operation"),
                    )
                    .expect("canonical call fixture instruction contains a valid wire shape");
                    // This convenience fixture represents a trusted fake-browser inventory. Seed
                    // caller-supplied tab handles explicitly so ordinary tool tests do not need a
                    // synthetic tabs_context_mcp prelude. Workspace-authority tests bypass this
                    // helper and exercise verification-only request admission directly.
                    let mut fixture_tabs = Vec::new();
                    collect_fixture_tab_ids(&operation.arguments, &mut fixture_tabs);
                    fixture_tabs.sort_unstable();
                    fixture_tabs.dedup();
                    if !fixture_tabs.is_empty() {
                        let claim = self.ctx.workspaces.claim_tabs(&workspace, &fixture_tabs);
                        assert_ne!(
                            claim,
                            ghostlight::hub::workspace::TabClaim::Refused,
                            "fixture tab inventory crossed a workspace"
                        );
                    }
                    self.execute(operation, &workspace, &owner, client.clone())
                        .await
                }
                other => panic!("unsupported neutral fixture instruction: {other}"),
            };
            responses.push(json!({"id": id, "result": result}));
        }

        self.ctx
            .workspaces
            .release(&workspace, &owner)
            .expect("release the fixture workspace");
        responses
    }

    fn catalog_result(&self) -> Value {
        let authority = self.ctx.authority.current();
        serde_json::to_value(ghostlight::tool::catalog::project_catalog(
            &authority.governance,
            None,
            *self.ctx.catalog_generation.borrow(),
        ))
        .expect("canonical catalog projection serializes")
    }

    async fn execute(
        &self,
        canonical: BrowserOperation,
        workspace: &ghostlight_transport::workspace_id::WorkspaceId,
        owner: &PeerUser,
        client: Option<ClientInfo>,
    ) -> Value {
        let descriptor = ghostlight::operation::registry::descriptor(canonical.key())
            .expect("fixture call maps to an implemented operation");
        let workspace = match descriptor.workspace_use {
            WorkspaceUse::Independent => None,
            _ => Some(workspace.clone()),
        };
        let lease = workspace.as_ref().map(|workspace| {
            self.ctx
                .workspaces
                .lease(workspace, owner)
                .expect("lease the fixture workspace")
        });
        let work = WorkContext::new(workspace, canonical, None, client, None);
        let cancellation = CancellationToken::new();
        let outcome = ghostlight::tool::pipeline::run_work(
            &self.ctx.browser,
            &self.ctx.store,
            &self.ctx.authority,
            &self.ctx.workspaces,
            &work,
            &cancellation,
        )
        .await;
        drop(lease);
        render_outcome(outcome)
    }

    /// Execute a canonical operation without a workspace for pre-admission service-boundary tests.
    ///
    /// This seam is intentionally narrow: callers use it to prove that an invalid canonical pair
    /// fails before workspace or browser dispatch. Ordinary fixture calls must continue through
    /// [`Harness::drive`], which mints and verifies a real workspace.
    pub async fn execute_unscoped_canonical(&self, operation: BrowserOperation) -> Value {
        let work = WorkContext::new(None, operation, None, None, None);
        let cancellation = CancellationToken::new();
        let outcome = ghostlight::tool::pipeline::run_work(
            &self.ctx.browser,
            &self.ctx.store,
            &self.ctx.authority,
            &self.ctx.workspaces,
            &work,
            &cancellation,
        )
        .await;
        render_outcome(outcome)
    }
}

fn collect_fixture_tab_ids(value: &Value, tab_ids: &mut Vec<i64>) {
    match value {
        Value::Object(object) => {
            if let Some(tab_id) = object.get("tab").and_then(Value::as_i64) {
                tab_ids.push(tab_id);
            }
            for child in object.values() {
                collect_fixture_tab_ids(child, tab_ids);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_fixture_tab_ids(item, tab_ids);
            }
        }
        _ => {}
    }
}

fn request_client(request: &Value) -> Option<ClientInfo> {
    let client = request.pointer("/params/clientInfo")?;
    Some(ClientInfo {
        name: client.get("name")?.as_str()?.to_string(),
        version: client.get("version")?.as_str()?.to_string(),
    })
}

fn render_outcome(outcome: CallOutcome) -> Value {
    match outcome {
        CallOutcome::Success { result } => result,
        CallOutcome::Failure { error } => error_result(error.to_string()),
        CallOutcome::NotDispatched { message } => execution_result("not_dispatched", true, message),
        CallOutcome::OutcomeUnknown { message } => {
            execution_result("outcome_unknown", false, message)
        }
        CallOutcome::Denied { message, .. } | CallOutcome::AttentionRequired { message } => {
            text_result(message)
        }
        CallOutcome::Held { prolonged } => json!({
            "content": [{"type": "text", "text": "browser session held by user"}],
            "structuredContent": {"held": {"prolonged": prolonged}}
        }),
        CallOutcome::Cancelled { message, .. } => execution_result("cancelled", false, message),
    }
}

fn text_result(message: String) -> Value {
    json!({"content": [{"type": "text", "text": message}]})
}

fn error_result(message: String) -> Value {
    json!({
        "content": [{"type": "text", "text": message}],
        "isError": true,
    })
}

fn execution_result(status: &str, retry_safe: bool, message: String) -> Value {
    json!({
        "content": [{"type": "text", "text": message}],
        "structuredContent": {
            "execution": {"status": status, "retrySafe": retry_safe}
        },
        "isError": true,
    })
}

/// One canonical call instruction for [`Harness::drive`].
pub fn operation_call(id: i64, operation: BrowserOperation) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "operations/call",
        "params": {"operation": operation},
    })
}

/// Construct one canonical operation for an in-process test instruction.
pub fn operation(id: OperationId, intent: IntentId, arguments: Value) -> BrowserOperation {
    BrowserOperation::new(id, intent, arguments)
}

/// The `[initialize, canonical operation]` instruction pair every call-driving test opens with.
pub fn init_and_call(operation: BrowserOperation) -> Vec<Value> {
    vec![
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        operation_call(2, operation),
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
