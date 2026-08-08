// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The binary <-> extension wire protocol (reference documentation).
//!
//! Both directions carry UTF-8 JSON, one object per native message (Chrome frames each with a
//! 4-byte little-endian length prefix; see [`super::host`]). The browser-only relay carries these
//! objects verbatim; only the persistent service (in [`crate::hub::outbound::browser`]) constructs
//! and parses them. Service callers issue typed
//! [`crate::browser::mechanism::MechanismRequest`] values. One isolated compatibility serializer
//! translates those physical identities to the covered adapter wire below; the legacy `tool` and
//! `args` members are never service dispatch authority or model-facing declarations. Adapter wire
//! envelopes remain documented here rather than becoming a second domain type system.
//!
//! ## binary -> extension
//! ```json
//! { "id": "<string>", "type": "tool_request", "tool": "<legacy adapter alias>", "args": { ... }, "guid": "<workspace id>", "resultFeatures": ["tabDeltaV1"], "execution": { ... }, "workspace"?: { "groupTitle": "<presentation title>" } }
//! ```
//!
//! ## extension -> binary
//! ```json
//! { "id": "<string>", "type": "tool_response", "result": { "content": [ ... ] } }
//! { "id": "<string>", "type": "tool_error",    "error":  "<message>", "hop": "<cdp|page>", "detail": "<string>", "code": "<string>" }
//! ```
//!
//! `result` is the browser execution result later normalized by the service. Replies without an
//! `id` are browser events; [`crate::hub::outbound::browser::Browser`] handles the recognized event
//! vocabulary without involving MCP lifecycle.
//!
//! `hop`, `detail`, and `code` on a `tool_error` reply are optional. `hop` is only ever `"cdp"` or
//! `"page"` -- the extension tags mechanism (which layer threw), never policy; an absent `hop`
//! means the binary attributes the failure to the extension itself (see
//! [`crate::ToolError::from_extension_wire`]). `detail` is debug-log-only material (logged with
//! `tracing::debug!` in [`crate::hub::outbound::browser`]) and must never appear in a tool result
//! surfaced to the MCP client.
//! `code` carries a small machine-readable extension state only where the service has a safe,
//! explicit recovery path; unknown codes are ignored.
//!
//! ## Browser tab transition result (ADR-0099)
//!
//! `resultFeatures` is an additive per-request compatibility opt-in. When it contains
//! `"tabDeltaV1"`, the extension may add this bounded observation to `structuredContent`:
//! ```json
//! { "tabDelta": {
//!   "opened": [{ "tabId": 42, "active": true }],
//!   "closed": [],
//!   "activeTabId": 42,
//!   "more": false
//! } }
//! ```
//! The extension emits only transitions correlated with the exact managed opener tab and opaque
//! workspace while the request ran. It reports observation, not causality. The service validates
//! and atomically adopts every still-open `opened` tab before converting native ids to composite
//! ids and returning the result. An older extension ignores the request member; a newer extension
//! never exposes the result to an older service that did not opt in.
//!
//! ## Take-the-wheel hold (g10, ADR-0018 step 2)
//!
//! A separate, minimal request/reply vocabulary on the SAME channel, for the extension's popup
//! and keyboard-shortcut controls. It only shares the envelope style with `tool_request` /
//! `tool_response` / `tool_error` above and with the (not-yet-implemented) shared format doc
//! section 9 settings protocol (`get_status` / `get_config` / `set_config_key`); it is not part
//! of that protocol.
//!
//! ## extension -> binary (requests; `id` is a caller-chosen string, unique per request)
//! ```json
//! { "id": "<string>", "type": "get_hold" }
//! { "id": "<string>", "type": "set_hold", "held": true }
//! { "id": "<string>", "type": "toggle_hold" }
//! ```
//!
//! ## binary -> extension (responses; `id` is echoed)
//! ```json
//! { "id": "<echoed>", "type": "hold_state", "result": { "held": true } }
//! { "id": "<echoed>", "type": "hold_error", "error": "set_hold requires a boolean 'held'" }
//! ```
//!
//! All three request types receive a `hold_state` reply carrying the state AFTER the request
//! was applied (`get_hold` reports without changing it; `set_hold` sets it; `toggle_hold` flips
//! it atomically in the binary). A `set_hold` whose `held` member is missing or not a JSON
//! boolean gets the `hold_error` reply above and changes nothing. Request/reply only: the
//! binary never pushes an unsolicited `hold_state` or `hold_error`. The native-host relays these
//! messages verbatim, exactly like every other frame; only the service
//! ([`crate::hub::outbound::browser::Browser`]) interprets them.
//!
//! ## Panic kill switch (g11, ADR-0018 step 2)
//!
//! ## extension -> binary (event; no `id` -- it is an event, not a reply)
//! ```json
//! { "type": "session_killed" }
//! ```
//!
//! Sent once the extension has detached its own debugger attachments (or begun to; the marker
//! that guarantees the detach completes lives in the extension's own storage, not on the wire)
//! and is tearing down the native port. The service marks the browser connection killed, fails
//! every in-flight and subsequent browser call with a truthful hop-attributed error until a fresh
//! native-host connection attaches, and writes one audit session-event record. No framing
//! change; the native-host relays this event verbatim like any other frame.
//!
//! ## Tab-URL query (g13, grant enforcement)
//!
//! ## binary -> extension
//! ```json
//! { "id": "<string>", "type": "tab_url_request", "tabId": <number> }
//! ```
//!
//! ## extension -> binary
//! ```json
//! { "id": "<string>", "type": "tab_url_response", "result": { "url": "<string or null>" } }
//! ```
//!
//! Mechanism only: the extension reports `chrome.tabs.get(tabId).url` verbatim (`null` for an
//! unknown/closed tab or a lookup failure) and makes no policy decision about it. The
//! service's neutral dispatch pipeline ([`crate::hub::outbound::browser::Browser::tab_url`]) uses the
//! reported URL to resolve the governing domain for a tab-scoped tool call; it is never trusted
//! from tool call parameters. This reply routes through the same generic (non-`tool_error`)
//! reply path as a `tool_response` -- no new routing logic, only a new `type` value.
//!
//! ## Browser wire `guid` member (ADR-0047, amended by ADR-0096)
//!
//! Browser `tool_request` frames retain the field spelling `guid` for browser adapter version
//! skew. Its value is now the service-minted
//! [`ghostlight_transport::workspace_id::WorkspaceId`] for the
//! work's browser workspace. It is not an MCP request id, protocol session, process identity, or
//! authentication credential. Domain code and the typed MCP-edge bridge use the honest
//! `WorkspaceId` name. This compatibility field must never be written raw to logs or audit.
//!
//! ## On-screen notification (SAPS PRES-HIGH-01)
//!
//! ## binary -> extension
//! ```json
//! { "type": "notification", "tabId": <number>, "class": "<string>", "icon": "<string>",
//!   "title": "<string>", "description": "<string>", "ref": "<string>" }
//! ```
//!
//! `icon`, `description`, and `ref` are optional; `title` is always present. Additive; ONE new
//! `type` value on the SAME channel with a fire-and-forget presentation posture (no `id`, no
//! reply, no policy decision on the extension side -- it renders exactly what it is told).
//! `title` is deliberately NOT the extension's `caption()`
//! mechanism (optional decorative flavor text, off by default) -- a notification is substantive
//! and must always render. `class` is the standard severity taxonomy this codebase's own tracing
//! already uses -- `"info"`/`"debug"`/`"warn"`/`"error"` -- so the primitive stays general-purpose
//! rather than denial-specific (today: `"error"` for a sacred-domain denial, `"warn"` for a policy
//! denial); `ref` is an opaque cross-reference
//! (today: a denial_id) a viewer can correlate back to
//! the structured audit record later. The canonical operation pipeline emits it at
//! each of the three points a call is denied.
//!
//! ## Extension debug events (ADR-0059)
//!
//! ## extension -> binary (event; no `id` -- fire-and-forget, same posture as `focus`)
//! ```json
//! { "type": "debug_event", "event": "<string>", "detail": <any>? }
//! ```
//!
//! Sent ONLY when the extension's own `chrome.storage.local` debug flag is on (default off,
//! toggled from the options page); never sent otherwise, so a normal install produces zero
//! extra traffic. `event` is a short name (`"connect_attempt"`, `"connect_disconnect"`, or a
//! `"managed_tab_*"` browser-topology lifecycle name);
//! `detail` is optional, freeform, and never policy-bearing -- purely a developer breadcrumb.
//! The binary appends it verbatim into [`crate::hub::outbound::browser::Browser`]'s existing
//! debug-state event ring (the SAME file `ghostlight doctor`/a raw `debug-state-<pid>.json`
//! read already surfaces every other lifecycle note from), so the extension's own view of a
//! connection interleaves with the service's, ordered by arrival -- one file, not two to
//! correlate by hand.
