//! JSON-RPC 2.0 server over stdio (the mcp-server role).
//!
//! Reads newline-delimited JSON-RPC from stdin, handles `initialize` / `tools/list` / `tools/call`,
//! and writes responses to stdout (one compact JSON object per line). `tools/call` is forwarded to
//! `pipeline::handle_tools_call` (ADR-0024 Decision 2: the generic ingest pipeline, the dispatch
//! chokepoint, moved to its own module); this module keeps only the JSON-RPC protocol loop,
//! `tools/list`, and the composition root. stdout is reserved for the protocol stream; operational
//! logs go to stderr.
//!
//! `tools/call` runs concurrently: each call is spawned on its own task (so a slow or waiting call
//! never blocks `initialize`, `ping`, or later requests) and every response -- inline or from a
//! spawned call -- funnels through a single writer task that owns stdout, so lines are never
//! interleaved mid-write.
//!
//! ADR-0025 (manifest hot-reload): the live `Governance` facade is held behind
//! `Arc<Mutex<Arc<Governance>>>`, the same swap idiom `ConfigStore` already uses for `Config`.
//! Every `tools/list`/`tools/call` clones the current `Arc<Governance>` ONCE, at the top of the
//! main read loop, and uses that one snapshot for the whole call (torn never, ADR-0025 Decision
//! 6). A policy-subscription task, spawned once at startup, watches `ConfigStore::policy()` for
//! a published manifest change, rebuilds `Governance` via [`build_governance`], carries the
//! retained client identity forward, swaps the new instance in, records the `manifest_reload`
//! session event, and -- iff the swap changed the ADVERTISED tool set -- sends
//! `notifications/tools/list_changed` through the SAME single-writer stdout task every other
//! outbound message uses (the writer channel is now [`Outbound`], not a bare `JsonRpcResponse`).

use crate::browser::{advertise, pattern, polarity};
use crate::governance::audit::Recorder;
use crate::governance::config::reload::ConfigStore;
use crate::governance::dispatch::Governance;
use crate::governance::enforcement::LocalPdp;
use crate::governance::manifest::identity::ManifestIdentity;
use crate::governance::manifest::source::LoadedPolicy;
use crate::governance::ports::AuditSink;
use crate::transport::executor::Browser;
use crate::transport::mcp::pipeline;
use crate::transport::mcp::tools::TOOLS_JSON;
use crate::transport::mcp::types::JsonRpcResponse;
use crate::Result;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

/// The exact `notifications/tools/list_changed` wire line (ADR-0025 Decision 4), pinned:
/// standard MCP, no `id`, no `params`.
const TOOLS_LIST_CHANGED_LINE: &str =
    r#"{"jsonrpc":"2.0","method":"notifications/tools/list_changed"}"#;

/// The single-writer stdout channel's message type (ADR-0025 Decision 4): an ordinary JSON-RPC
/// response, or the `list_changed` notification. Widened from a bare `JsonRpcResponse` so both
/// share one writer task and can never interleave mid-write. The pinned shape is the two bare
/// variants exactly as written (not `Box<JsonRpcResponse>`); `large_enum_variant` is allowed
/// rather than boxing, per BOOTSTRAP rule 14 (a byte-pinned shape moves by transcription, not
/// re-derivation) -- this channel is not a hot inner loop, so the size difference is immaterial.
#[allow(clippy::large_enum_variant)]
pub(super) enum Outbound {
    Response(JsonRpcResponse),
    ToolsListChanged,
}

/// Build the live `Governance` facade from a resolved policy (ADR-0025 Decision 2): all-open
/// when no manifest is active, a governed overlay otherwise. Exactly the wiring `run` performed
/// inline at startup before this task, extracted so the policy-subscription task (below) can
/// rebuild an equivalent instance on every hot-reloaded manifest swap.
fn build_governance(policy: &LoadedPolicy, recorder: Arc<dyn AuditSink>) -> Governance {
    match &policy.manifest {
        Some(manifest) => Governance::governed(
            Box::new(LocalPdp::new(polarity::evaluate_host)),
            recorder,
            manifest.grants.clone(),
            manifest.hash.clone(),
            manifest.mode,
        ),
        None => Governance::all_open(recorder),
    }
}

/// The identity to stamp on a `manifest_reload` session event (ADR-0025 Decision 5): `None` for
/// a swap to all-open, else the active manifest's `name`/`version`/`hash` (already computed by
/// `parse_manifest`, never a second read).
fn manifest_identity_of(policy: &LoadedPolicy) -> Option<ManifestIdentity> {
    policy.manifest.as_ref().map(|m| ManifestIdentity {
        name: m.name.clone(),
        version: m.version.clone(),
        hash: m.hash.clone(),
    })
}

/// Snapshot the current `Governance` out of the swap slot: one `Arc` clone, released
/// immediately (ADR-0025 Decision 2, the same per-call-snapshot idiom `ConfigStore::current`
/// already follows for `Config`).
fn current_governance(slot: &Arc<Mutex<Arc<Governance>>>) -> Arc<Governance> {
    slot.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

/// MCP protocol version this server speaks.
pub const PROTOCOL_VERSION: &str = "2024-11-05";

/// Run the stdio MCP server loop until stdin closes. `browser` is the (shared) handle to the
/// extension; tool calls are forwarded through it. `loaded_policy` is the manifest resolved at
/// startup (G12, shared format doc sections 1.2-1.3): `None` manifest means all-open. G12
/// itself only feeds a user-supplied manifest's `config` entries into the layer resolver
/// (below) and holds the rest at this scope for later stage-2 tasks (G13 grant enforcement,
/// G14 tool-advertisement filtering) to read grants from; loading it does not change which
/// calls execute.
pub async fn run(
    browser: Browser,
    loaded_policy: LoadedPolicy,
    user_source: Option<String>,
) -> Result<()> {
    if let Some(manifest) = &loaded_policy.manifest {
        tracing::debug!(
            name = %manifest.name,
            version = %manifest.version,
            hash = %manifest.hash,
            "active manifest held for later governance tasks"
        );
    }

    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    // Hot-reload substrate (ADR-0019, extended by ADR-0025 to the manifest): the resolved
    // Config is held behind an atomic swap; the watcher re-resolves on a config/org/manifest
    // change with no restart. With no files present this resolves to the built-in defaults, so
    // all-open behavior is byte-identical to stage 1. `loaded_policy` was already parsed once by
    // `source::load_policy` above (ADR-0023 Decision 1: `parse_manifest` is the sole
    // reader/parser/validator of the policy file); the store derives both the org layers (an
    // org-sourced manifest's config entries) and the user layer (a user-supplied manifest's
    // config entries, G12) from it directly, with no second read of the org file. `user_source`
    // is retained so the store's own re-selection on reload (`ConfigStore::reresolve`) covers
    // the watched user `file://` manifest source too, not just the org file.
    let store = ConfigStore::load_initial_with_policy(
        pattern::is_valid_pattern,
        &loaded_policy,
        user_source,
    )?;
    store.clone().spawn_watcher();

    // The audit flight recorder (ADR-0018 step 1) is orthogonal to the governance mode: it
    // records under all-open too, gated only by audit.enabled (shared format doc section 4.5).
    // Its destination is live (RECONCILIATION.md section 3): a config-change watcher re-opens
    // the sink whenever audit.enabled / audit.destination / audit.file.path changes.
    let recorder = Arc::new(Recorder::from_config(&store.current()));
    tokio::spawn({
        let recorder = Arc::clone(&recorder);
        let mut changes = store.subscribe();
        async move {
            while changes.changed().await.is_ok() {
                let config = changes.borrow().clone();
                recorder.reload(&config);
            }
        }
    });

    // Grant enforcement (g13): governed once a manifest is active (org or user-sourced;
    // `loaded_policy` already resolved which one wins), all-open otherwise. `governance_slot`
    // (ADR-0025 Decision 2) is the swappable snapshot slot: every call clones the CURRENT
    // `Arc<Governance>` once, at the top of the main loop below, and uses it for the whole call.
    let governance = Arc::new(build_governance(
        &loaded_policy,
        recorder.clone() as Arc<dyn AuditSink>,
    ));
    if loaded_policy.user_manifest_ignored {
        // ADR-0025 Decision 5: the promised startup audit note (implements the note
        // `source.rs`'s doc comment used to defer to "a future audit task").
        governance.record_user_manifest_ignored();
    }
    let governance_slot: Arc<Mutex<Arc<Governance>>> = Arc::new(Mutex::new(governance));

    // Panic kill switch (g11, ADR-0018 step 2): the extension signals `session_killed` once it
    // has severed its own debugger attachments; the binary writes exactly one audit
    // session-event record per kill (`tracing::info!` fires regardless of `audit.enabled`, so
    // the operational log always has the event). Reads the CURRENT snapshot at kill time
    // (ADR-0025 Decision 6: holds and the kill switch are orthogonal to the manifest, but the
    // client identity every rebuilt `Governance` carries forward is easiest to prove correct by
    // always reading the live slot rather than a startup-time capture).
    browser.on_session_killed({
        let governance_slot = Arc::clone(&governance_slot);
        move || {
            current_governance(&governance_slot).record_session_killed();
            tracing::info!("session killed by the user");
        }
    });

    let (tx, mut rx) = mpsc::unbounded_channel::<Outbound>();

    // A single writer owns stdout so responses -- including those from spawned `tools/call`
    // tasks and the ADR-0025 `list_changed` notification -- never interleave mid-write. `debug`
    // is cloned before the spawn so both the writer and the read loop below can record the MCP
    // boundary.
    let debug = browser.debug().clone();
    let writer = tokio::spawn(async move {
        let mut stdout = tokio::io::stdout();
        while let Some(msg) = rx.recv().await {
            let mut buf = match msg {
                Outbound::Response(resp) => {
                    let buf = match serde_json::to_string(&resp) {
                        Ok(buf) => buf,
                        Err(e) => {
                            tracing::warn!(error = %e, "dropping unserializable response");
                            continue;
                        }
                    };
                    if debug.is_enabled() {
                        // Use the already-typed id (do not re-parse the whole -- possibly large
                        // -- body).
                        let id = resp.id.as_ref().map(Value::to_string).unwrap_or_default();
                        debug.mcp_response(&id, &buf);
                    }
                    buf
                }
                Outbound::ToolsListChanged => TOOLS_LIST_CHANGED_LINE.to_string(),
            };
            buf.push('\n');
            if stdout.write_all(buf.as_bytes()).await.is_err() || stdout.flush().await.is_err() {
                break;
            }
        }
    });

    // ADR-0025 Decision 2/4/5: the policy-subscription task. On every published `LoadedPolicy`
    // (a settled, identity-changing reload of the org policy file or a watched user file://
    // source): rebuild `Governance`, carry the retained client identity forward, swap it in,
    // record `manifest_reload`, and -- iff the ADVERTISED set changed -- send the notification;
    // finally, iff `user_manifest_ignored` newly transitioned to true, record that event too.
    // Under all-open with no watched manifest sources the policy channel never publishes, so
    // this task simply parks forever and emits nothing (the all-open goldens stay untouched).
    // Its own JoinHandle is retained (`policy_subscription`, below) and explicitly aborted once
    // the main loop ends: this task's `while ... .changed().await` loop has no natural end (the
    // policy watcher never signals "no more publishes are coming"), so its captured `Outbound`
    // sender clone would otherwise never drop, and the writer task's shutdown-on-drop below
    // would then wait forever for a sender that is never released -- the process would never
    // exit on stdin close.
    let policy_subscription = tokio::spawn({
        let governance_slot = Arc::clone(&governance_slot);
        let recorder = recorder.clone() as Arc<dyn AuditSink>;
        let mut policy_changes = store.policy();
        let tx = tx.clone();
        let fixture: Value =
            serde_json::from_str(TOOLS_JSON).expect("embedded tools.json is valid");
        async move {
            let mut ignored_in_force = policy_changes.borrow().user_manifest_ignored;
            while policy_changes.changed().await.is_ok() {
                let loaded_policy = policy_changes.borrow_and_update().clone();

                let outgoing = current_governance(&governance_slot);
                let before = advertise::advertised_tools(&fixture, outgoing.grants());
                let client = outgoing.current_client();
                drop(outgoing);

                let new_governance = build_governance(&loaded_policy, recorder.clone());
                if let Some(client) = client {
                    new_governance.set_client(&client.name, &client.version);
                }
                let after = advertise::advertised_tools(&fixture, new_governance.grants());
                let new_governance = Arc::new(new_governance);

                {
                    let mut guard = governance_slot
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner);
                    *guard = Arc::clone(&new_governance);
                }

                new_governance.record_manifest_reload(manifest_identity_of(&loaded_policy));

                if before != after {
                    let _ = tx.send(Outbound::ToolsListChanged);
                }

                if crate::governance::ports::user_manifest_ignored_transitioned(
                    ignored_in_force,
                    loaded_policy.user_manifest_ignored,
                ) {
                    new_governance.record_user_manifest_ignored();
                }
                ignored_in_force = loaded_policy.user_manifest_ignored;
            }
        }
    });

    while let Some(line) = lines.next_line().await? {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let governance = current_governance(&governance_slot);
        if let Some(resp) = handle_line(&browser, &store, &governance, line, &tx).await {
            let _ = tx.send(Outbound::Response(resp));
        }
    }
    // Stop the policy-subscription task FIRST (and wait for the cancellation to actually take
    // effect) so its own `Outbound` sender clone is released before the writer's shutdown check
    // below relies on every sender being dropped.
    policy_subscription.abort();
    let _ = policy_subscription.await;
    drop(tx);
    let _ = writer.await;
    Ok(())
}

/// Parse and route one JSON-RPC line.
///
/// Returns `Some(response)` for requests (an `id` member is present, even if `null`) and `None` for
/// notifications (no `id` member) and for lines we cannot parse at all. Fields are read from a raw
/// [`Value`] so a structurally invalid but id-bearing request still gets an addressable `-32600`.
///
/// `pub(super)` (ADR-0024 Decision 2): the pipeline module's own moved test
/// `tools_call_produces_one_audit_record_with_client_identity` drives this directly, alongside
/// `pipeline::handle_tools_call` -- a compile-necessary visibility widening from the pre-move
/// private fn, since the two functions now live in sibling modules.
pub(super) async fn handle_line(
    browser: &Browser,
    store: &Arc<ConfigStore>,
    governance: &Arc<Governance>,
    line: &str,
    tx: &mpsc::UnboundedSender<Outbound>,
) -> Option<JsonRpcResponse> {
    let raw: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "dropping unparseable JSON-RPC line");
            return None;
        }
    };

    let is_notification = raw.get("id").is_none();
    let id = raw.get("id").cloned();

    let Some(method) = raw.get("method").and_then(Value::as_str) else {
        return if is_notification {
            tracing::debug!("dropping malformed notification (no method)");
            None
        } else {
            Some(JsonRpcResponse::error(
                id,
                -32600,
                "Invalid Request: missing or non-string 'method'",
            ))
        };
    };

    if browser.debug().is_enabled() {
        let id_str = id.as_ref().map(Value::to_string).unwrap_or_default();
        browser.debug().mcp_request(method, &id_str, line);
    }

    match method {
        "initialize" => {
            // Record the MCP client's self-reported identity (clientInfo.name [+ version]), if it
            // sent one, for `ghostlight doctor`/`status` to display. Missing params/clientInfo, or
            // non-string fields, are silently fine: this is best-effort observability, not part of
            // the protocol contract, and the response below never depends on it.
            if let Some(client_info) = raw.get("params").and_then(|p| p.get("clientInfo")) {
                if let Some(name) = client_info.get("name").and_then(Value::as_str) {
                    let ident = match client_info.get("version").and_then(Value::as_str) {
                        Some(version) => format!("{name} {version}"),
                        None => name.to_string(),
                    };
                    browser.debug().set_client(&ident);
                }
            }
            // Capture the same clientInfo into the audit recorder's client field (shared
            // format doc section 6.1), first-wins for the whole session.
            capture_client_info(governance, raw.get("params"));
            // Warm the extension channel while the client finishes its handshake. The extension
            // side initiates the connection (Chrome spawns the native-host, which dials the
            // endpoint this process has served since startup), so there is nothing to dial from
            // here; this watcher verifies readiness and records the outcome.
            let wait_ms = store.current().first_call_wait_ms();
            tokio::spawn({
                let browser = browser.clone();
                async move {
                    let started = Instant::now();
                    if browser.wait_connected(Duration::from_millis(wait_ms)).await {
                        tracing::info!(
                            elapsed_ms = started.elapsed().as_millis() as u64,
                            "extension channel ready"
                        );
                    } else {
                        tracing::info!(
                            "extension channel not ready within the warmup window; \
                             the first tools/call will wait for it"
                        );
                    }
                }
            });
            Some(JsonRpcResponse::success(id, initialize_result()))
        }
        "tools/list" => Some(JsonRpcResponse::success(id, tools_list_result(governance))),
        "tools/call" => {
            let browser = browser.clone();
            let store = Arc::clone(store);
            let governance = Arc::clone(governance);
            let tx = tx.clone();
            let params = raw.get("params").cloned();
            tokio::spawn(async move {
                let resp =
                    pipeline::handle_tools_call(&browser, &store, &governance, id, params.as_ref())
                        .await;
                let _ = tx.send(Outbound::Response(resp));
            });
            None
        }
        "ping" => Some(JsonRpcResponse::success(id, json!({}))),
        _ if is_notification => {
            tracing::debug!(method, "ignoring unknown notification");
            None
        }
        other => Some(JsonRpcResponse::error(
            id,
            -32601,
            format!("Method not found: {other}"),
        )),
    }
}

/// Capture `clientInfo` from the MCP `initialize` params into the audit recorder (shared
/// format doc section 6.1 `client` field). Both `name` and `version` must be strings;
/// otherwise the session's records carry `client: null`.
fn capture_client_info(governance: &Governance, params: Option<&Value>) {
    let info = params.and_then(|p| p.get("clientInfo"));
    let name = info.and_then(|i| i.get("name")).and_then(Value::as_str);
    let version = info.and_then(|i| i.get("version")).and_then(Value::as_str);
    if let (Some(name), Some(version)) = (name, version) {
        governance.set_client(name, version);
    }
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "ghostlight", "version": env!("CARGO_PKG_VERSION") },
    })
}

/// The advertised surface (g14): the embedded sacred fixture verbatim under all-open, or
/// filtered to the union over the active manifest's grants (`browser::advertise::advertised_tools`)
/// once one is active. Schema text is never altered; only which tools appear in the array
/// changes.
fn tools_list_result(governance: &Governance) -> Value {
    let fixture: Value = serde_json::from_str(TOOLS_JSON).expect("embedded tools.json is valid");
    advertise::advertised_tools(&fixture, governance.grants())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::manifest::document::{Grant, HostRules};
    use crate::governance::ports::Capability;

    fn grant(allowed: &[Capability]) -> Grant {
        Grant {
            id: "g".to_string(),
            hosts: HostRules {
                allow: vec!["example.com".to_string()],
                deny: Vec::new(),
            },
            allowed: allowed.to_vec(),
            description: None,
            mode: None,
        }
    }

    /// ADR-0025 Decision 4: the notification fires only when the ADVERTISED SET actually
    /// changes. Adding a read-only grant where there were none changes the set (unlocks the
    /// whole read-only surface); splitting a read+write union across two grants versus one
    /// combined grant does NOT change the set (every variant here requires at most one
    /// capability, satisfied identically either way) -- the pinned "two grants collapsing to
    /// the same capability union" no-op case.
    #[test]
    fn advertised_set_diff_gates_the_notification() {
        let fixture: Value = serde_json::from_str(TOOLS_JSON).expect("fixture parses");

        let none: Vec<Grant> = Vec::new();
        let read_only = vec![grant(&[Capability::Read])];
        let none_set = advertise::advertised_tools(&fixture, Some(&none));
        let read_set = advertise::advertised_tools(&fixture, Some(&read_only));
        assert_ne!(
            none_set, read_set,
            "adding a read-only grant where there were none changes the advertised set"
        );

        let split = vec![grant(&[Capability::Read]), grant(&[Capability::Write])];
        let combined = vec![grant(&[Capability::Read, Capability::Write])];
        let split_set = advertise::advertised_tools(&fixture, Some(&split));
        let combined_set = advertise::advertised_tools(&fixture, Some(&combined));
        assert_eq!(
            split_set, combined_set,
            "two grants collapsing to the same capability union advertise the identical set"
        );
    }
}
