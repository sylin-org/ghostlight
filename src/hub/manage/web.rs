// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The manage.web adapter -- the loopback HTTP UI for observing runtime state and administering
//! policy. Served from the SAME loopback listener the inbound.web adapter binds (one listener,
//! two gated routing contexts, ADR-0033 Decision 7): [`crate::hub::inbound::web::handle_connection`]
//! classifies each accepted request and delegates the non-WS, in-scope ones here.
//!
//! The plane runs its OWN capability decision (`manage.web.from`, permanently loopback) on every
//! request, separate from the inbound.web adapter's `inbound.web.from` gate. The two planes are
//! independently enableable and independently authorized.
//!
//! Routes (PINS.md CS1/CS2/CS3/CS4/CS5/CS10, `docs/tasks/console`):
//! - `GET /` -- the embedded HTML shell.
//! - `GET /manage.css`, `GET /manage.js` -- the shell's static assets.
//! - `GET /api/v1/config` -- the provenance-aware config view (read of `layers::Resolution`).
//! - `GET /api/v1/sessions` -- the live-sessions/groups view.
//! - `POST /api/v1/config/inbound-web-enable-remote` -- the ONE write action (writes the user-layer
//!   `inbound.web.from` key to `["*"]`; refused under an org-mandatory lock with a 409).
//!
//! Every response writer flushes and shuts down the write half of the stream before returning, so
//! the full body drains cleanly to the client regardless of OS-specific socket-close timing.

use crate::governance::ports::{AuditSink, Decision, SessionEventRecord};
use crate::hub::inbound::web::{decide_inbound_web_from, write_http_error};
use crate::hub::manage::assets;
use crate::hub::ServiceContext;
use std::net::SocketAddr;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

/// Every path this router recognizes, regardless of method -- distinguishes a 404 ("no such
/// path") from a 405 ("wrong method on a path that exists"). Grows as new management actions
/// land.
pub(crate) fn is_known_path(stripped_path: &str) -> bool {
    matches!(
        stripped_path,
        "/" | "/manage.css"
            | "/manage.js"
            | "/api/v1/config"
            | "/api/v1/sessions"
            | "/api/v1/config/inbound-web-enable-remote"
    )
}

/// The management-plane router. Authorizes the connecting source against `inbound.web.from`
/// (the SAME decision the WS-upgrade path uses -- the management plane rides the inbound.web
/// listener's allowlist today), then serves a known static asset, a JSON view, the one write
/// action, or answers 404/405. Reached only for a request the listener classified as NOT a
/// WS-upgrade attempt AND in the management route scope.
pub(crate) async fn route(
    stream: &mut TcpStream,
    method: &str,
    stripped_path: &str,
    headers: &[(String, String)],
    ctx: &ServiceContext,
    peer_addr: SocketAddr,
) -> crate::Result<()> {
    // Permanent loopback hard-lock (defense-in-depth on top of the validator): even if the policy
    // allowlist were somehow widened, the management plane refuses any non-loopback peer before
    // routing.
    if !peer_addr.ip().is_loopback() {
        write_http_error(stream, 403, "Forbidden").await?;
        return Ok(());
    }

    let allowlist = live_inbound_web_from(&ctx.store);
    let (decision, source) = decide_inbound_web_from(headers, peer_addr, &allowlist, ctx);
    if !matches!(decision, Decision::Allow { .. }) {
        tracing::info!(source = %source, decision = ?decision, "manage.web request refused by inbound.web.from");
        write_http_error(stream, 403, "Forbidden").await?;
        return Ok(());
    }

    let result = match (method, stripped_path) {
        ("GET", "/") => write_asset(stream, "text/html; charset=utf-8", assets::INDEX_HTML).await,
        ("GET", "/manage.css") => {
            write_asset(stream, "text/css; charset=utf-8", assets::MANAGE_CSS).await
        }
        ("GET", "/manage.js") => {
            write_asset(
                stream,
                "application/javascript; charset=utf-8",
                assets::MANAGE_JS,
            )
            .await
        }
        ("GET", "/api/v1/config") => write_config_response(stream, ctx).await,
        ("GET", "/api/v1/sessions") => write_sessions_response(stream, ctx).await,
        ("POST", "/api/v1/config/inbound-web-enable-remote") => {
            write_enable_remote_response(stream, ctx).await
        }
        _ if is_known_path(stripped_path) => {
            write_plain_error(stream, 405, "Method Not Allowed", "method not allowed").await
        }
        _ => write_plain_error(stream, 404, "Not Found", "not found").await,
    };
    result?;
    // Flush the response, then shut down the write half so the full body reaches the client
    // before the socket closes.
    stream.flush().await.ok();
    stream.shutdown().await.ok();
    Ok(())
}

/// The live `inbound.web.from` allowlist, read from the store's current resolution. Re-read on
/// every management request so a policy edit is honored without a service restart. (The
/// management plane rides the inbound.web allowlist today; a dedicated `manage.web.from` key
/// exists and is permanently loopback, but the authz decision currently uses the shared allowlist
/// for parity with the WS path.)
fn live_inbound_web_from(store: &crate::governance::config::reload::ConfigStore) -> Vec<String> {
    let resolution = store.current_resolution();
    let resolved = resolution
        .get(crate::governance::config::INBOUND_WEB_FROM)
        .expect("registered key resolves");
    resolved
        .value
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_else(|| vec!["localhost".to_string()])
}

/// `GET /api/v1/config` (PINS.md CS2): the provenance-aware config view -- per registered key,
/// its resolved value, source layer, lock state, and description, in `KEYS` registry order. A
/// READ of `layers::Resolution` only; never a manifest document.
async fn write_config_response(stream: &mut TcpStream, ctx: &ServiceContext) -> crate::Result<()> {
    let resolution = ctx.store.current_resolution();
    let payload = config_payload(&resolution).to_string();
    write_json(stream, 200, "OK", &payload).await
}

/// The pure `Resolution` -> JSON transform behind `GET /api/v1/config`: per registered key, its
/// resolved value, source layer, lock state, and description, in `KEYS` registry order. Split out
/// from the socket-writing handler so the JSON contract is unit-testable against a hand-built
/// `Resolution` with no spawned service and no policy file.
fn config_payload(resolution: &crate::governance::config::layers::Resolution) -> serde_json::Value {
    let keys: Vec<serde_json::Value> = resolution
        .iter()
        .map(|(key, resolved)| {
            let description = crate::governance::config::key_def(key)
                .map(|def| def.description)
                .unwrap_or_default();
            serde_json::json!({
                "key": key,
                "value": resolved.value,
                "source": resolved.source.as_str(),
                "locked": resolved.locked,
                "description": description,
            })
        })
        .collect();
    serde_json::json!({ "keys": keys })
}

/// `GET /api/v1/sessions` (PINS.md CS3): the live-sessions/groups view -- the current
/// live-session COUNT (every source, adapter or web) plus, for adapter sessions admitted since
/// the service started, a TRUNCATED (never full) guid, pid, and owned tabs.
async fn write_sessions_response(
    stream: &mut TcpStream,
    ctx: &ServiceContext,
) -> crate::Result<()> {
    let live_session_count = ctx.live_sessions.load(std::sync::atomic::Ordering::Relaxed);
    let summaries =
        crate::hub::session::live_session_summaries(&ctx.session_registry, &ctx.owned_tabs);
    let payload = sessions_payload(&summaries, live_session_count).to_string();
    write_json(stream, 200, "OK", &payload).await
}

/// The pure summaries -> JSON transform behind `GET /api/v1/sessions`: the live-session count
/// plus, per adapter binding, a TRUNCATED guid, pid, and owned tabs. Split out from the
/// socket-writing handler so the JSON contract is unit-testable against hand-built summaries with
/// no spawned adapter/service.
fn sessions_payload(
    summaries: &[crate::hub::session::SessionSummary],
    live_session_count: usize,
) -> serde_json::Value {
    let adapter_bindings: Vec<serde_json::Value> = summaries
        .iter()
        .map(|s| {
            serde_json::json!({
                "guid": s.guid,
                "pid": s.pid,
                "owned_tab_ids": s.owned_tab_ids,
            })
        })
        .collect();
    serde_json::json!({
        "live_session_count": live_session_count,
        "adapter_bindings": adapter_bindings,
        "note": "adapter_bindings lists sessions admitted since the service started; a listed \
                 binding may no longer be currently connected. Web/Console HTTP sessions are not \
                 yet individually tracked.",
    })
}

/// `POST /api/v1/config/inbound-web-enable-remote` (PINS.md CS4/CS5): the management plane's ONE write
/// action. The request body is NEVER read -- the written value is the ONE pinned literal below,
/// never caller-supplied. Writes the single user-layer `inbound.web.from` key via K1's
/// `set_user_value` (the SAME path `ghostlight config set` uses), refusing cleanly under an
/// org-mandatory lock (or any other failure) with a uniform 409, and records exactly one
/// `config_changed` session-event audit record on success.
async fn write_enable_remote_response(
    stream: &mut TcpStream,
    ctx: &ServiceContext,
) -> crate::Result<()> {
    let key = crate::governance::config::INBOUND_WEB_FROM;
    let value = serde_json::json!(["*"]);
    let outcome = crate::governance::config::cli::set_user_value(
        key,
        value.clone(),
        crate::browser::pattern::is_valid_pattern,
    );
    match outcome {
        Ok(path) => {
            record_config_changed(ctx);
            let payload = serde_json::json!({
                "key": key,
                "value": value,
                "written_to": path.display().to_string(),
                "note": "takes effect the next time the Ghostlight service restarts",
            })
            .to_string();
            write_json(stream, 200, "OK", &payload).await
        }
        Err(e) => {
            let payload = serde_json::json!({ "error": e.to_string() }).to_string();
            write_json(stream, 409, "Conflict", &payload).await
        }
    }
}

/// Record ONE `config_changed` session-event audit record on a SUCCESSFUL write (mirroring
/// `Governance::record_manifest_reload`'s own "callers only invoke this on a successful swap"
/// rule). The management-plane POST handler has no per-session `Governance` (it is a plain HTTP
/// action on the shared service, never a tool-call dispatch through `serve_session`), so it calls
/// the underlying `AuditSink` directly.
fn record_config_changed(ctx: &ServiceContext) {
    let record = SessionEventRecord {
        event_id: uuid::Uuid::new_v4().to_string(),
        ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        identity: None,
        client: None,
        event: "config_changed",
        manifest: None,
    };
    ctx.recorder.record_session_event(&record);
}

/// Serve one embedded static asset verbatim, with a `Content-Length` computed from its actual
/// UTF-8 byte length.
async fn write_asset(stream: &mut TcpStream, content_type: &str, body: &str) -> crate::Result<()> {
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    Ok(())
}

/// Write a JSON response with the given status and pre-serialized payload.
async fn write_json(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    payload: &str,
) -> crate::Result<()> {
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{payload}",
        payload.len()
    );
    stream.write_all(response.as_bytes()).await?;
    Ok(())
}

/// A plain-text error response: the exact literal ASCII body, no trailing newline.
async fn write_plain_error(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    body: &str,
) -> crate::Result<()> {
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nConnection: close\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `config_payload` emits every registered key, in registry order, each with the five
    /// contracted fields. A pure test over the builtin-only resolution -- no service, no file,
    /// every platform.
    #[test]
    fn config_payload_emits_every_registered_key_in_registry_order() {
        let resolution = resolve(&LayerInputs::default());
        let payload = config_payload(&resolution);
        let keys = payload["keys"].as_array().expect("keys array");

        let expected: Vec<&str> = crate::governance::config::KEYS
            .iter()
            .map(|d| d.key)
            .collect();
        assert_eq!(keys.len(), expected.len());
        for (entry, key) in keys.iter().zip(expected.iter()) {
            assert_eq!(entry["key"], *key);
            assert!(entry.get("value").is_some(), "{key}: value present");
            assert!(entry["source"].is_string(), "{key}: source string");
            assert!(entry["locked"].is_boolean(), "{key}: locked bool");
            assert!(
                entry["description"].is_string(),
                "{key}: description string"
            );
        }
    }

    /// An org-mandatory entry serialises as `source: "org_mandatory"`, `locked: true`.
    #[test]
    fn config_payload_reflects_an_org_mandatory_key_as_locked() {
        let mut inputs = LayerInputs::default();
        inputs
            .org_mandatory
            .insert("audit.enabled".to_string(), serde_json::json!(true));
        let resolution = resolve(&inputs);

        let payload = config_payload(&resolution);
        let entry = payload["keys"]
            .as_array()
            .unwrap()
            .iter()
            .find(|k| k["key"] == "audit.enabled")
            .expect("audit.enabled is registered");
        assert_eq!(entry["source"], "org_mandatory");
        assert_eq!(entry["locked"], true);
        assert_eq!(entry["value"], true);
    }

    /// `sessions_payload` serialises the live count, each binding's truncated guid/pid/owned
    /// tabs, and the tracking-scope note -- a pure test over hand-built summaries.
    #[test]
    fn sessions_payload_serialises_count_bindings_and_note() {
        let summaries = vec![crate::hub::session::SessionSummary {
            guid: "abcd1234".to_string(),
            pid: 4242,
            owned_tab_ids: vec![7, 9],
        }];
        let payload = sessions_payload(&summaries, 3);

        assert_eq!(payload["live_session_count"], 3);
        let bindings = payload["adapter_bindings"].as_array().unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0]["guid"], "abcd1234");
        assert_eq!(bindings[0]["pid"], 4242);
        assert_eq!(bindings[0]["owned_tab_ids"], serde_json::json!([7, 9]));
        assert!(payload["note"]
            .as_str()
            .unwrap()
            .contains("admitted since the service started"));
    }

    use crate::governance::config::layers::{resolve, LayerInputs};
}
