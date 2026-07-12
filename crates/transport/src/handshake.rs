// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Session-hellos for both local endpoints.
//!
//! The ADAPTER/CONTROL endpoint's hello (ADR-0030 Decision 1, the 2026-07-04 two-endpoint
//! amendment; PINS.md SS1) is carried ON TOP OF the existing 4-byte-LE `transport::native::host`
//! framing (never a change to that framing) as one JSON object:
//! `{ "hub": 1, "role": "<role>", "guid": "<uuid>"? }`.
//!
//! ADR-0058 amends the EXTENSION endpoint's part of this: it is no longer assumed to have exactly
//! one possible peer, so it now carries its own hello too (`ROLE_BROWSER` below), read first by
//! [`crate::hub`]'s browser attach path -- the SAME "peer speaks first" pattern this endpoint's
//! ADAPTER/CONTROL sibling already used. The extension is still identified by the endpoint it
//! arrives at (never a shared demux), and `host.rs` framing is still unchanged; only the
//! singleton assumption is repealed.

/// The session-hello protocol major version (PINS.md SS1).
pub const HUB_PROTO: u32 = 1;

/// An MCP stdio adapter session (PINS.md SS1): the role `hub::run_mcp_server` (ALWAYS the thin
/// ADAPTER as of ADR-0030 Decision 8's always-ready-service amendment; PINS.md SS5.1) sends via
/// `ipc::relay_adapter`, and the role dispatched to
/// [`crate::transport::mcp::server::serve_session`] on the service side.
pub const ROLE_ADAPTER: &str = "adapter";

/// The control-plane role (doctor/console): a non-session, read-only request/reply over the
/// ADAPTER/CONTROL endpoint. The hello is `{ hub, role: "control", request: "<name>" }`; the
/// service answers one framed reply and closes, admitting no session (no guid, no anti-squat
/// proof, no `serve_session`). Access is bounded by the endpoint's owner-only transport ACL
/// (same OS user only), and replies carry only non-sensitive liveness. The first request is
/// [`CONTROL_REQUEST_STATUS`], which `ghostlight doctor` uses to render a real extension
/// connected/disconnected verdict without requiring `--debug` instrumentation (CAP-MED-01).
pub const ROLE_CONTROL: &str = "control";

/// The `control` request that returns a liveness snapshot ([`crate::ipc::StatusReply`]): whether
/// the browser extension is currently attached, and how many tool sessions are live.
pub const CONTROL_REQUEST_STATUS: &str = "status";

/// The SERVICE's anti-squat proof, sent AFTER admitting the adapter's hello and BEFORE
/// `serve_session` (ADR-0030 Decision 8 amendment; PINS.md SS5.3): `{"hub":1,"role":"service-proof",
/// "mac":"<hex>"}`, the lowercase-hex HMAC-SHA256 of the adapter's exact hello bytes, keyed by this
/// install's per-user `hub-key` (`src/hub/antisquat.rs`).
pub const ROLE_SERVICE_PROOF: &str = "service-proof";

/// The EXTENSION endpoint's session-hello role (ADR-0058): `{"hub":1,"role":"browser",
/// "relayPid":<u32>,"browserPid":<u32>,"browserCreated":<u64>}`, sent by the browser-role relay
/// immediately after connecting, before the generic byte-relay loop starts. `relayPid` is the
/// relay's own pid; `browserPid`/`browserCreated` are the relay's PARENT process identity (the
/// browser that spawned it).
///
/// ADR-0061 amends what this hello is FOR: `browserPid` is no longer the session identity (it
/// degraded to a colliding `0` when [`crate::proc::parent`] could not resolve the browser, which
/// mis-routed freshly minted tabs). Identity now comes from the EXTENSION's own persistent UUID,
/// carried in the [`EXTENSION_IDENTITY_TYPE`] frame the extension sends right after this hello (see
/// below). This hello's pids stay purely diagnostic; the relay is otherwise unchanged (a pure byte
/// pipe that never parses the extension's frames).
pub const ROLE_BROWSER: &str = "browser";

/// The extension's opening identity frame (ADR-0061): `{"type":"browser_hello","browserId":"<uuid>"}`,
/// the FIRST native message the extension posts on every connect. The relay forwards it verbatim, so
/// the service reads it as the second frame on a `ROLE_BROWSER` connection (right after the relay's
/// hello above) and keys this browser's session -- and its composite tab ids -- by the UUID. The
/// extension owns and persists the UUID (`chrome.storage.local`), so it survives relay reconnects and
/// service-worker relaunches and is never blank or colliding, unlike the guessed `browserPid`.
pub const EXTENSION_IDENTITY_TYPE: &str = "browser_hello";

/// The field carrying the extension's persistent browser UUID in an [`EXTENSION_IDENTITY_TYPE`]
/// frame (ADR-0061).
pub const BROWSER_ID_FIELD: &str = "browserId";

/// Parse an extension identity frame (ADR-0061): returns the non-empty `browserId` from a
/// `{"type":"browser_hello","browserId":"<uuid>"}` message, or `None` for any other shape, a
/// missing/blank id, or non-JSON bytes. The service admits a browser session only on `Some(_)`
/// (fail closed: no identity, no admission -- there is no `browserPid` fallback anymore).
pub fn parse_extension_identity(bytes: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    if value.get("type").and_then(serde_json::Value::as_str) != Some(EXTENSION_IDENTITY_TYPE) {
        return None;
    }
    let id = value.get(BROWSER_ID_FIELD)?.as_str()?.trim();
    (!id.is_empty()).then(|| id.to_string())
}

/// Build the [`ROLE_BROWSER`] session-hello's JSON bytes (unframed -- the caller frames it with
/// `host::write_message`, same as every other hello). `browser` is `None` when this process's
/// parent could not be determined (rare); the hello still carries `browserPid: 0`,
/// `browserCreated: 0` in that case, a degraded-but-valid identity distinct from any real pid
/// (pid 0 never names a real process).
pub fn browser_hello_bytes(relay_pid: u32, browser: Option<crate::proc::ProcId>) -> Vec<u8> {
    let (browser_pid, browser_created) = browser.map_or((0, 0), |p| (p.pid, p.created));
    let hello = serde_json::json!({
        "hub": HUB_PROTO,
        "role": ROLE_BROWSER,
        "relayPid": relay_pid,
        "browserPid": browser_pid,
        "browserCreated": browser_created,
    });
    // A json! literal of primitive fields never fails to serialize.
    serde_json::to_vec(&hello).expect("browser hello serializes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_well_formed_extension_identity_frame() {
        let bytes = br#"{"type":"browser_hello","browserId":"abc-123"}"#;
        assert_eq!(parse_extension_identity(bytes), Some("abc-123".to_string()));
    }

    #[test]
    fn rejects_a_frame_of_the_wrong_type() {
        let bytes = br#"{"type":"focus","browserId":"abc-123"}"#;
        assert_eq!(parse_extension_identity(bytes), None);
    }

    #[test]
    fn rejects_a_missing_or_blank_browser_id() {
        assert_eq!(
            parse_extension_identity(br#"{"type":"browser_hello"}"#),
            None
        );
        assert_eq!(
            parse_extension_identity(br#"{"type":"browser_hello","browserId":""}"#),
            None
        );
        assert_eq!(
            parse_extension_identity(br#"{"type":"browser_hello","browserId":"   "}"#),
            None,
            "a whitespace-only id is blank once trimmed"
        );
    }

    #[test]
    fn rejects_non_json_bytes() {
        assert_eq!(parse_extension_identity(b"not json at all"), None);
    }
}
