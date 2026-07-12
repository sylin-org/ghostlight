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
/// relay's own pid (diagnostic only); `browserPid`/`browserCreated` are the relay's PARENT
/// process identity (the browser that spawned it), captured the same way
/// [`crate::proc::parent`] already captures an agent's MCP-client parent for its watchdog -- a
/// [`crate::proc::ProcId`], not a bare pid, so a dead browser's reused pid is never mistaken for
/// a live one. Admits the connection as an independent session keyed by `browserPid`, replacing
/// any existing session under the same identity (a reconnect/relaunch from the SAME browser),
/// never rejecting a DIFFERENT browser's hello the way the old single-slot attach did.
pub const ROLE_BROWSER: &str = "browser";

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
