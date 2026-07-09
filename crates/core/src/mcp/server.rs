// SPDX-License-Identifier: Apache-2.0 OR MIT
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
//!
//! ADR-0030 Decision 3 (H5, "MANDATORY screenshot chunking"): this session's own writer task
//! relays every reply through [`write_chunked`], which chunks a reply at or above
//! [`SCREENSHOT_CHUNK_THRESHOLD`] bytes with an explicit yield between chunks -- see the module
//! doc on `src/hub/mod.rs` for the accepted-bottleneck framing this closes a gap in.

use crate::browser::{advertise, directory, polarity};
use crate::governance::config::reload::ConfigStore;
use crate::governance::dispatch::Governance;
use crate::governance::enforcement::LocalPdp;
use crate::governance::manifest::identity::ManifestIdentity;
use crate::governance::manifest::source::LoadedPolicy;
use crate::governance::ports::{AuditSink, Denial};
use crate::hub::outbound::browser::Browser;
use crate::hub::session::SessionGuid;
use crate::hub::ServiceContext;
use crate::mcp::pipeline;
use crate::mcp::tools::{advertised_tools_json, agent_guide_text};
use crate::mcp::types::{text_content, JsonRpcResponse};
use crate::Result;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

/// The exact `notifications/tools/list_changed` wire line (ADR-0025 Decision 4), pinned:
/// standard MCP, no `id`, no `params`.
const TOOLS_LIST_CHANGED_LINE: &str =
    r#"{"jsonrpc":"2.0","method":"notifications/tools/list_changed"}"#;

/// Oversize threshold (ADR-0030 Decision 3, "MANDATORY screenshot chunking"; PINNED in
/// `docs/tasks/hub/PINS.md` SS4): a reply serialized at or above this many bytes is written to
/// this session's own stream in fixed-size chunks (below [`write_chunked`]) instead of one
/// `write_all` call, so a large payload cannot head-of-line-block a shared single-threaded
/// runtime and starve another session's small reply. Well under `native::host::MAX_MESSAGE_LEN`
/// (128 MiB); this is a hub-internal relay/scheduling property on the service<->adapter/web hop
/// ONLY, never a change to the frozen extension `host.rs` wire.
pub const SCREENSHOT_CHUNK_THRESHOLD: usize = 8 * 1024 * 1024;

/// Chunk size for [`write_chunked`]'s fixed-size relay writes. Not itself a pinned oracle (only
/// [`SCREENSHOT_CHUNK_THRESHOLD`] and the yield-between-chunks behavior are pinned in PINS.md
/// SS4); 1 MiB keeps any one chunk's write time small relative to `TOOL_TIMEOUT`.
const CHUNK_SIZE: usize = 1024 * 1024;

/// Write `buf` to `out`, chunked (ADR-0030 Decision 3; PINS.md SS4) once `buf.len()` is `>=`
/// [`SCREENSHOT_CHUNK_THRESHOLD`]: fixed [`CHUNK_SIZE`] `write_all` calls with an explicit
/// `tokio::task::yield_now().await` between them, so the runtime gets a scheduling point another
/// session's small reply can use instead of waiting out the whole write. Below the threshold:
/// ONE `write_all` call, byte-identical to the pre-H5 behavior (a lone all-open session's
/// ordinary replies never cross this threshold, so this stays a pass-through no-op for it).
/// Writes the SAME bytes either way -- chunking changes only the NUMBER of write calls and
/// inserts yield points, never the content or the JSON-RPC framing.
async fn write_chunked<W: AsyncWrite + Unpin>(out: &mut W, buf: &[u8]) -> std::io::Result<()> {
    if buf.len() < SCREENSHOT_CHUNK_THRESHOLD {
        return out.write_all(buf).await;
    }
    for chunk in buf.chunks(CHUNK_SIZE) {
        out.write_all(chunk).await?;
        tokio::task::yield_now().await;
    }
    Ok(())
}

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

/// RAII guard incrementing `ServiceContext::live_sessions` on construction and decrementing on
/// drop (ADR-0030 Decision 8; PINS.md SS5.4): every session -- adapter today, web at H8 -- is
/// counted at this ONE chokepoint so the service's idle-grace watcher (`hub::idle_grace_watch`)
/// can tell whether any session is live. Adds no output; a byte-identical no-op for a lone
/// all-open session's wire bytes.
struct LiveSessionGuard(Arc<AtomicUsize>);

impl LiveSessionGuard {
    fn new(counter: Arc<AtomicUsize>) -> Self {
        counter.fetch_add(1, Ordering::Relaxed);
        Self(counter)
    }
}

impl Drop for LiveSessionGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Marks this session's guid live for the ownership gate (ADR-0047 D5): a tab owned by a guid
/// with NO live session is adoptable by another session. Counted (not boolean) because a
/// reconnect's new connection can briefly overlap the old one's teardown.
struct LiveGuidGuard {
    live_guids: Arc<Mutex<HashMap<String, usize>>>,
    guid: String,
}

impl LiveGuidGuard {
    fn new(live_guids: Arc<Mutex<HashMap<String, usize>>>, guid: String) -> Self {
        *live_guids
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(guid.clone())
            .or_insert(0) += 1;
        Self { live_guids, guid }
    }
}

impl Drop for LiveGuidGuard {
    fn drop(&mut self) {
        let mut map = self
            .live_guids
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(count) = map.get_mut(&self.guid) {
            *count -= 1;
            if *count == 0 {
                map.remove(&self.guid);
            }
        }
    }
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

/// MCP revisions this server implements, oldest first (ADR-0041 D5; latest bumped to 2025-11-25 by
/// ADR-0049). The advertised surface uses only features present in ALL of them beyond
/// capability-gated additions (structuredContent / outputSchema entered 2025-06-18); optional
/// features are declared via `capabilities`, so claiming a revision never claims its optional
/// features.
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] =
    &["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"];

/// The newest supported revision (ADR-0049): offered when the client requests nothing or something
/// unknown, per the spec's version-negotiation rule.
pub const LATEST_PROTOCOL_VERSION: &str = "2025-11-25";

/// The manifest resolved at startup (G12, shared format doc sections 1.2-1.3): `None` manifest
/// means all-open. G12 itself only feeds a user-supplied manifest's `config` entries into the
/// layer resolver (below) and holds the rest at this scope for later stage-2 tasks (G13 grant
/// enforcement, G14 tool-advertisement filtering) to read grants from; loading it does not
/// change which calls execute.
///
/// The transport-generic session chokepoint (ADR-0030 Decision 2: "HubCore / ServiceContext vs
/// per-session state"): every transport calls this ONE function over its own
/// `S: AsyncRead + AsyncWrite` stream. `ctx` carries the SHARED-per-service state (the `Browser`
/// handle, the `ConfigStore`, the audit `Recorder`, and the startup `LoadedPolicy`); everything
/// built in this function's body is PER-SESSION (the swappable `Governance`, the writer task,
/// the policy-subscription task) and is dropped when the session ends. `guid` (H3, ADR-0030
/// Decision 4; PINS.md SS9) is this session's opaque identity -- EVERY session carries a real
/// one, including the SERVICE's own directly-served stdio session, never `Option<SessionGuid>`.
/// H4 (ADR-0030 Decision 6) is the first task to consume it: the pre-dispatch cross-session
/// tab-ownership gate in the read loop below keys the shared `ctx.owned_tabs` map on it. A lone
/// session's own guid still first-touch-adopts every tab it names, so this stays a
/// byte-identical pass-through for a single live session (ADR-0030 "Preserved invariants":
/// all-open byte-identity).
pub async fn serve_session<S>(
    stream: S,
    ctx: ServiceContext,
    guid: crate::hub::session::SessionGuid,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Send + 'static,
{
    // The governance chokepoint (ADR-0030 Decision 1 addendum; PINS.md SS8): every transport
    // enters here first, and this process must only ever be the SERVICE. A no-op fail-loud
    // backstop when the role is already correct (as it always is by construction); see
    // `ghostlight_transport::role`.
    ghostlight_transport::role::assert_service_role("serve_session");

    let ServiceContext {
        browser,
        capabilities,
        store,
        recorder,
        initial_policy: loaded_policy,
        session_registry: _,
        owned_tabs,
        session_titles,
        mint_quota: _,
        live_sessions,
        live_guids,
        debug_sink: _,
    } = ctx;

    // H6 (ADR-0030 Decision 8; PINS.md SS5.4): count this session for the idle-grace watcher's
    // whole duration -- the ONE chokepoint every transport (adapter today, web at H8) passes
    // through. Adds no output; a no-op for the all-open byte-identity invariant.
    let _live_guard = LiveSessionGuard::new(live_sessions);

    // ADR-0047 D5: mark this session's guid live for the ownership gate, so a DIFFERENT session can
    // adopt this guid's tabs only once this connection is gone (dead-owner reassignment, no timers).
    let _live_guid_guard = LiveGuidGuard::new(Arc::clone(&live_guids), guid.as_str().to_string());

    let (read_half, write_half) = tokio::io::split(stream);
    // Hot-reload substrate (ADR-0019, extended by ADR-0025 to the manifest): the resolved
    // Config is held behind an atomic swap; the watcher re-resolves on a config/org/manifest
    // change with no restart. With no files present this resolves to the built-in defaults, so
    // all-open behavior is byte-identical to stage 1. `loaded_policy` was already parsed once by
    // `source::load_policy` above (ADR-0023 Decision 1: `parse_manifest` is the sole
    // reader/parser/validator of the policy file); the store derives both the org layers (an
    // org-sourced manifest's config entries) and the user layer (a user-supplied manifest's
    // config entries, G12) from it directly, with no second read of the org file.
    let mut lines = BufReader::new(read_half).lines();

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
    // has severed its own debugger attachments; this session writes exactly one audit
    // session-event record per kill (`tracing::info!` fires regardless of `audit.enabled`, so
    // the operational log always has the event). Reads the CURRENT snapshot at kill time
    // (ADR-0025 Decision 6: holds and the kill switch are orthogonal to the manifest, but the
    // client identity every rebuilt `Governance` carries forward is easiest to prove correct by
    // always reading the live slot rather than a startup-time capture).
    //
    // H2 (ADR-0030 Decision 7): registered via the SESSION-SCOPED, removable
    // `register_session_kill_hook` (not the permanent `on_session_killed`), so this session's
    // hook fires on every kill (global: `hold`/`killed`/`connected` all latch on the ONE shared
    // `Browser`, never per-session) but deregisters when this session ends -- a dead session
    // records nothing on a later kill. `_kill_handle` is held for the whole function body so the
    // registration outlives every kill this session may observe.
    let _kill_handle = {
        let governance_slot = Arc::clone(&governance_slot);
        browser.register_session_kill_hook(move || {
            current_governance(&governance_slot).record_session_killed();
            tracing::info!("session killed by the user");
        })
    };

    let (tx, mut rx) = mpsc::unbounded_channel::<Outbound>();

    // A single writer owns the session's write half so responses -- including those from
    // spawned `tools/call` tasks and the ADR-0025 `list_changed` notification -- never
    // interleave mid-write. `debug` is cloned before the spawn so both the writer and the read
    // loop below can record the MCP boundary.
    let debug = browser.debug().clone();
    let writer = tokio::spawn(async move {
        let mut out = write_half;
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
            if write_chunked(&mut out, buf.as_bytes()).await.is_err() || out.flush().await.is_err()
            {
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
        let fixture = advertised_tools_json();
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

    // ADR-0047 D3: one seat carrying this session's identity + owned-tab handle, threaded into the
    // dispatch arm so a spawned tools/call can claim a tab the session creates (tabs_create_mcp).
    // Holds its own clones (a cheap Arc clone + guid clone) so the loop's own `&owned_tabs`/`&guid`
    // borrows for `check_tab_ownership` stay valid alongside it.
    let seat = SessionSeat {
        guid: guid.clone(),
        owned_tabs: Arc::clone(&owned_tabs),
        session_titles: Arc::clone(&session_titles),
        live_guids: Arc::clone(&live_guids),
    };

    while let Some(line) = lines.next_line().await? {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let governance = current_governance(&governance_slot);
        // H4 (ADR-0030 Decision 6; PINS.md SS3): the cross-session tab-ownership gate runs
        // BEFORE any dispatch into `handle_line`'s "tools/call" arm -- and therefore before
        // `pipeline::handle_tools_call`'s own `LazyTabUrl` probe can ever fire for this line's
        // tabId. A pass-through `None` for every other line (not a `tools/call`, no numeric
        // `tabId`, or a `tabId` this session already owns/first-touch-adopts): a lone all-open
        // session owns everything it touches, so this never disturbs that path (ADR-0030
        // "Preserved invariants": all-open byte-identity). H7 (ADR-0030 Decision 6/7; PINS.md
        // SS6) piggybacks on this SAME gate: a NEWLY adopted tabId (never an already-owned one)
        // also fires the out-of-band group-request presentation through `browser`.
        if let Some(resp) = check_tab_ownership(
            line,
            &owned_tabs,
            &live_guids,
            &session_titles,
            &guid,
            &governance,
            &browser,
        ) {
            let _ = tx.send(Outbound::Response(resp));
            continue;
        }
        if let Some(resp) = handle_line(
            &browser,
            &capabilities,
            &store,
            &governance,
            &seat,
            line,
            &tx,
        )
        .await
        {
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

/// H4 (ADR-0030 Decision 6; PINS.md SS3): the pre-dispatch cross-session tab-ownership gate.
/// Re-parses `line` itself (a separate, cheap `Value` parse from `handle_line`'s own, so this
/// stays a standalone check ahead of it rather than widening `handle_line`'s signature or
/// touching `pipeline.rs`'s frozen stage order) purely to read `method`/`params.name`/
/// `params.arguments.tabId` -- the SAME fields `handle_tools_call` would read, just far enough
/// ahead that a refusal never reaches it. Returns `None` (a pure pass-through: not a
/// `tools/call`, unparseable, no numeric `tabId`, or a `tabId` this session already
/// owns/first-touch-adopts) for every line that must fall through to the unchanged
/// `handle_line`/`handle_tools_call` path; a lone all-open session's own guid first-touch-adopts
/// every tab it ever names, so this is a byte-identical no-op for a single live session
/// (ADR-0030 "Preserved invariants": all-open byte-identity). Returns
/// `Some(response)` -- the uniform, leak-free `"unknown tab"` result (PINS.md SS3), recorded as a
/// deny with `domain: null` (the host is NEVER resolved for an unowned tab) -- ONLY for a
/// `tools/call` naming a numeric `tabId` a DIFFERENT live session already owns, and only BEFORE
/// any dispatch, hence before `pipeline::LazyTabUrl`'s own probe could ever fire for it (Decision
/// 6: "BEFORE any `tab_url` probe").
///
/// H7 (ADR-0030 Decision 6/7; PINS.md SS6): a NEWLY adopted tabId (never an already-owned one,
/// and never a refusal) also emits the out-of-band group-request presentation through `browser`
/// before returning -- "when a session's owned-tab set changes ... emit the group request".
fn check_tab_ownership(
    line: &str,
    owned_tabs: &Arc<Mutex<HashMap<i64, SessionGuid>>>,
    live_guids: &Mutex<HashMap<String, usize>>,
    titles: &Mutex<HashMap<String, String>>,
    guid: &SessionGuid,
    governance: &Governance,
    browser: &Browser,
) -> Option<JsonRpcResponse> {
    let raw: Value = serde_json::from_str(line).ok()?;
    if raw.get("method").and_then(Value::as_str) != Some("tools/call") {
        return None;
    }
    let id = raw.get("id").cloned();
    let params = raw.get("params")?;
    let name = params.get("name").and_then(Value::as_str)?;
    let args = params.get("arguments");
    let tab_id = args.and_then(|a| a.get("tabId")).and_then(Value::as_i64)?;

    match crate::hub::session::claim_tab_live(owned_tabs, live_guids, guid, tab_id) {
        crate::hub::session::TabClaim::Owned => None,
        crate::hub::session::TabClaim::Adopted => {
            emit_group_request(browser, owned_tabs, titles, governance, guid);
            None
        }
        crate::hub::session::TabClaim::Refused => {
            record_unowned_tab_denial(governance, name, args);
            Some(JsonRpcResponse::success(id, text_content("unknown tab")))
        }
    }
}

/// H7 (ADR-0030 Decision 6/7; PINS.md SS6): tell the extension to (re)group this session's
/// CURRENT, complete owned-tab set (not just the tabId that just triggered the call) into its
/// Chrome tab group. Fire-and-forget through the shared `Browser` seam (H2's existing plumbing --
/// this function builds no new native-send transport); a missing extension link is a harmless
/// no-op, same as any other out-of-band presentation call. The GUID is passed only as the wire
/// argument `Browser::request_group` needs; it is never logged here. The title is the client-name
/// title (ADR-0047 D4) resolved from the service-lifetime `titles` registry and this session's
/// captured `clientInfo.name`, deduped and cached per guid so it stays stable across reconnects.
fn emit_group_request(
    browser: &Browser,
    owned_tabs: &Arc<Mutex<HashMap<i64, SessionGuid>>>,
    titles: &Mutex<HashMap<String, String>>,
    governance: &Governance,
    guid: &SessionGuid,
) {
    let tab_ids = crate::hub::session::owned_tab_ids(owned_tabs, guid);
    let title = crate::hub::session::session_title(
        titles,
        guid,
        governance
            .current_client()
            .as_ref()
            .map(|c| c.name.as_str()),
    );
    browser.request_group(guid.as_str(), &tab_ids, &title);
}

/// Record the H4 unowned-tab refusal as a deny (PINS.md SS3, transcribed): `decision: "deny"`,
/// `domain: null` (never resolved -- resolving it is the very leak being closed), `held: false`,
/// `duration_ms: 0`. Reuses `Governance::begin`/`CallAudit::sacred_deny`'s existing zero-duration,
/// no-domain deny shape (the same public API `pipeline::sacred_check`'s own denial already uses)
/// rather than a new recording path; the denial id follows the existing `denial::denial_id`
/// `"D-"` + 8-lowercase-hex scheme, ruled `cross_session/unowned_tab`.
fn record_unowned_tab_denial(governance: &Governance, name: &str, args: Option<&Value>) {
    let action = directory::descriptor(name)
        .and_then(|d| d.action_key)
        .and_then(|key| args.and_then(|a| a.get(key)))
        .and_then(Value::as_str);
    let lookup = directory::requires(name, action);
    let audit = governance.begin(name, action, lookup);
    let rule = "cross_session/unowned_tab";
    let denial = Denial {
        rule: rule.to_string(),
        grant_id: None,
        denial_id: crate::governance::denial::denial_id("", "", rule),
        domain: String::new(),
        message: "unknown tab".to_string(),
    };
    audit.sacred_deny(&denial, None);
}

/// One session's identity + shared-state handles (ADR-0047 D3), threaded from `serve_session`
/// into the dispatch arms so a spawned tools/call can claim tabs the session creates.
pub(super) struct SessionSeat {
    pub(super) guid: SessionGuid,
    pub(super) owned_tabs: Arc<Mutex<HashMap<i64, SessionGuid>>>,
    pub(super) session_titles: Arc<Mutex<HashMap<String, String>>>,
    pub(super) live_guids: Arc<Mutex<HashMap<String, usize>>>,
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
    capabilities: &crate::hub::outbound::Registry,
    store: &Arc<ConfigStore>,
    governance: &Arc<Governance>,
    seat: &SessionSeat,
    line: &str,
    tx: &mpsc::UnboundedSender<Outbound>,
) -> Option<JsonRpcResponse> {
    let raw: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => {
            // A blank / whitespace-only line is a benign keepalive, never a request: stay silent.
            if line.trim().is_empty() {
                return None;
            }
            // ADR-0049: a malformed but non-empty frame gets an addressable JSON-RPC parse error
            // (`id: null`, per the spec, since a broken frame carries no recoverable id) instead
            // of a silent drop, so a broken client fails fast rather than hanging on a reply that
            // never arrives.
            tracing::warn!(error = %e, "replying -32700 to an unparseable JSON-RPC line");
            return Some(JsonRpcResponse::error(
                Some(Value::Null),
                -32700,
                "Parse error: the line is not valid JSON-RPC 2.0",
            ));
        }
    };

    // ADR-0049: JSON-RPC batching (a top-level array of messages) was removed from MCP in the
    // 2025-06-18 revision. Reject a batch frame loudly with a teaching message -- one that points
    // the model at the two supported ways to do several things -- instead of dropping it silently
    // (having no id or method, it would otherwise look like a malformed notification and hang the
    // client waiting for responses that never come).
    if raw.is_array() {
        return Some(JsonRpcResponse::error(
            Some(Value::Null),
            -32600,
            "Batching (a JSON-RPC array of requests) is not supported -- MCP removed it in the \
             2025-06-18 revision. Send one JSON-RPC message per line. To run several browser \
             actions in a single call, use the `script` tool.",
        ));
    }

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
            // ADR-0041 D5 / ADR-0049: negotiate the protocol revision from the client's request.
            let requested = raw
                .get("params")
                .and_then(|p| p.get("protocolVersion"))
                .and_then(Value::as_str);
            Some(JsonRpcResponse::success(
                id,
                initialize_result(requested, capabilities),
            ))
        }
        "tools/list" => Some(JsonRpcResponse::success(id, tools_list_result(governance))),
        "tools/call" => {
            let browser = browser.clone();
            let store = Arc::clone(store);
            let governance = Arc::clone(governance);
            let tx = tx.clone();
            let params = raw.get("params").cloned();
            // ADR-0047 D3: the session's guid rides the tool envelope, and a tab this session
            // CREATES via tabs_create_mcp is claimed from the response so no other session can
            // first-touch-steal it; a newly adopted tab fires the group-request presentation.
            let guid = seat.guid.clone();
            let owned_tabs = Arc::clone(&seat.owned_tabs);
            let session_titles = Arc::clone(&seat.session_titles);
            let live_guids = Arc::clone(&seat.live_guids);
            let tool_name = params
                .as_ref()
                .and_then(|p| p.get("name"))
                .and_then(Value::as_str)
                .map(str::to_string);
            tokio::spawn(async move {
                let resp = pipeline::handle_tools_call(
                    &browser,
                    &store,
                    &governance,
                    guid.as_str(),
                    id,
                    params.as_ref(),
                )
                .await;
                if tool_name.as_deref() == Some("tabs_create_mcp") {
                    if let Some(tab_id) = resp
                        .result
                        .as_ref()
                        .and_then(|r| r.get("structuredContent"))
                        .and_then(|s| s.get("tabId"))
                        .and_then(Value::as_i64)
                    {
                        if let crate::hub::session::TabClaim::Adopted =
                            crate::hub::session::claim_tab_live(
                                &owned_tabs,
                                &live_guids,
                                &guid,
                                tab_id,
                            )
                        {
                            emit_group_request(
                                &browser,
                                &owned_tabs,
                                &session_titles,
                                &governance,
                                &guid,
                            );
                        }
                    }
                }
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

/// Negotiate the MCP protocol revision (the spec's version-negotiation rule): echo the client's
/// requested revision when this server supports it, else offer the latest supported one and let the
/// client decide whether to proceed. Pure, so it is the unit-tested seam; `initialize_result` stays
/// a thin renderer.
fn negotiate_protocol_version(requested: Option<&str>) -> &'static str {
    SUPPORTED_PROTOCOL_VERSIONS
        .iter()
        .find(|v| Some(**v) == requested)
        .copied()
        .unwrap_or(LATEST_PROTOCOL_VERSION)
}

fn initialize_result(
    requested: Option<&str>,
    capabilities: &crate::hub::outbound::Registry,
) -> Value {
    // The capability manifest (ADR-0034 Decision 6): a per-capability section so the model
    // learns the landscape at handshake -- what capabilities exist, what each is for, which
    // tools each owns, and the per-capability guidance. Additive alongside the existing
    // `instructions`; MCP clients that ignore unknown fields still see the flat `tools/list`
    // array and work perfectly.
    let manifest: Vec<Value> = capabilities
        .capabilities()
        .iter()
        .map(|cap| {
            let tools: Vec<&str> = cap.directory().iter().map(|d| d.tool).collect();
            let guide = cap.agent_guide();
            json!({
                "code": cap.code(),
                "descriptor": cap.descriptor(),
                "tools": tools,
                "guidance": {
                    "summary": guide.summary,
                    "workflow": guide.workflow,
                    "flow": guide.flow,
                    "denials": guide.denials,
                },
            })
        })
        .collect();

    json!({
        "protocolVersion": negotiate_protocol_version(requested),
        // ADR-0049: advertise tools.listChanged -- the server DOES emit
        // notifications/tools/list_changed on manifest hot-reload (ADR-0025), so a
        // capability-strict client must know to expect the notification it will receive.
        "capabilities": { "tools": { "listChanged": true } },
        "serverInfo": { "name": ghostlight_transport::instance::Instance::resolve().mcp_server_name(), "version": env!("CARGO_PKG_VERSION") },
        // The agent onboarding guide (ADR-0031 Decision 1), composed from each capability's
        // guide (ADR-0034 Decision 6). Today only the browser capability contributes; future
        // capabilities enrich this additively.
        "instructions": agent_guide_text(),
        // The capability manifest (ADR-0034 Decision 6): per-capability metadata.
        "ghostlight:capabilities": manifest,
    })
}

/// The advertised surface (g14): the embedded sacred fixture verbatim under all-open, or
/// filtered to the union over the active manifest's grants (`browser::advertise::advertised_tools`)
/// once one is active. Schema text is never altered; only which tools appear in the array
/// changes.
fn tools_list_result(governance: &Governance) -> Value {
    let fixture = advertised_tools_json();
    advertise::advertised_tools(&fixture, governance.grants())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::manifest::document::{Grant, HostRules};
    use crate::governance::ports::Capability;

    /// ADR-0041 D5 / ADR-0049: a supported requested revision is echoed back verbatim.
    #[test]
    fn protocol_version_negotiation_echoes_supported() {
        assert_eq!(negotiate_protocol_version(Some("2024-11-05")), "2024-11-05");
        assert_eq!(negotiate_protocol_version(Some("2025-03-26")), "2025-03-26");
        assert_eq!(negotiate_protocol_version(Some("2025-06-18")), "2025-06-18");
        assert_eq!(negotiate_protocol_version(Some("2025-11-25")), "2025-11-25");
    }

    /// ADR-0049: an unknown/future revision falls back to the latest supported one.
    #[test]
    fn protocol_version_negotiation_offers_latest_for_unknown() {
        assert_eq!(negotiate_protocol_version(Some("9999-01-01")), "2025-11-25");
    }

    /// ADR-0049: an absent requested revision falls back to the latest supported one.
    #[test]
    fn protocol_version_negotiation_offers_latest_when_absent() {
        assert_eq!(negotiate_protocol_version(None), "2025-11-25");
    }

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
    /// whole read-only surface). Splitting a read+write union across two grants versus one
    /// combined grant does NOT change the set for any tool whose variants require at most ONE
    /// capability each (satisfied identically either way, since `requires.is_empty()` or a
    /// subset of a SINGLE grant is unaffected by how the union is split across grants) --
    /// EXCEPT `form_fill` (C10, ADR-0036 Decision 4), the first tool whose `action: None`
    /// variant requires TWO capabilities (`[read, write]`) TOGETHER: reachability is a
    /// single-grant subset check (`browser::advertise::tool_has_a_reachable_variant`), so two
    /// SEPARATE single-capability grants never satisfy it (no one grant carries both), while one
    /// grant carrying the union does. This is the mechanically-forced consequence of C10 adding
    /// the batch's first multi-capability-requiring variant, surfaced by this pre-existing test;
    /// the assertion below is corrected to name it explicitly rather than silently going stale.
    #[test]
    fn advertised_set_diff_gates_the_notification() {
        let fixture = advertised_tools_json();

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

        fn names(v: &Value) -> Vec<String> {
            v["tools"]
                .as_array()
                .expect("tools array")
                .iter()
                .map(|t| t["name"].as_str().expect("name").to_string())
                .collect()
        }
        let split_names = names(&split_set);
        let combined_names = names(&combined_set);

        assert!(
            !split_names.contains(&"form_fill".to_string()),
            "form_fill needs read+write from a SINGLE grant; two separate single-capability \
             grants never satisfy it: {split_names:?}"
        );
        assert!(
            combined_names.contains(&"form_fill".to_string()),
            "one grant carrying both read and write does satisfy form_fill's variant: \
             {combined_names:?}"
        );

        let split_rest: Vec<&String> = split_names.iter().filter(|n| *n != "form_fill").collect();
        let combined_rest: Vec<&String> = combined_names
            .iter()
            .filter(|n| *n != "form_fill")
            .collect();
        assert_eq!(
            split_rest, combined_rest,
            "every OTHER tool's variants require at most one capability each, satisfied \
             identically either way two grants collapse to the same capability union"
        );
    }
}
