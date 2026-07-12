// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The binary <-> extension wire protocol (reference documentation).
//!
//! Both directions carry UTF-8 JSON, one object per native message (Chrome frames each with a
//! 4-byte little-endian length prefix; see [`super::host`]). The native-host relays these objects
//! verbatim; only the mcp-server (in [`crate::hub::outbound::browser`]) constructs and parses them, so
//! they are documented here rather than modeled as types.
//!
//! ## binary -> extension
//! ```json
//! { "id": "<string>", "type": "tool_request", "tool": "<tool name>", "args": { ... } }
//! ```
//!
//! ## extension -> binary
//! ```json
//! { "id": "<string>", "type": "tool_response", "result": { "content": [ ... ] } }
//! { "id": "<string>", "type": "tool_error",    "error":  "<message>", "hop": "<cdp|page>", "detail": "<string>" }
//! ```
//!
//! `result` is an MCP tool result object. Replies without an `id` (events, heartbeats) are ignored
//! by the mcp-server in v1.0; Phase 3 will buffer console/network events pushed this way.
//!
//! `hop` and `detail` on a `tool_error` reply are both optional. `hop` is only ever `"cdp"` or
//! `"page"` -- the extension tags mechanism (which layer threw), never policy; an absent `hop`
//! means the binary attributes the failure to the extension itself (see
//! [`crate::ToolError::from_extension_wire`]). `detail` is debug-log-only material (logged with
//! `tracing::debug!` in [`crate::hub::outbound::browser`]) and must never appear in a tool result
//! surfaced to the MCP client.
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
//! messages verbatim, exactly like every other frame; only the mcp-server
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
//! and is tearing down the native port. The mcp-server marks the session killed, fails every
//! in-flight and subsequent tool call with a truthful hop-attributed error until a fresh
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
//! mcp-server's dispatch chokepoint ([`crate::hub::outbound::browser::Browser::tab_url`]) uses the
//! reported URL to resolve the governing domain for a tab-scoped tool call; it is never trusted
//! from tool call parameters. This reply routes through the same generic (non-`tool_error`)
//! reply path as a `tool_response` -- no new routing logic, only a new `type` value.
//!
//! ## Adapter/control session-hello's `guid` member (H3, ADR-0030 Decision 4)
//!
//! This section documents the wire vocabulary between the binary and the extension; the
//! adapter/control session-hello below is a DIFFERENT connection (thin ADAPTER <-> persistent
//! SERVICE, never the extension link) that rides the SAME 4-byte-LE `host.rs` framing. H2
//! (`src/hub/handshake.rs`) defines the hello's `hub`/`role` members and its
//! `ROLE_ADAPTER`/`ROLE_CONTROL` constants; this is not a second or separate handshake frame, only
//! the documentation of one more member on that existing hello:
//!
//! ```json
//! { "hub": 1, "role": "adapter", "guid": "<uuid-v4>" }
//! ```
//!
//! `guid` is present only for `role == "adapter"` (absent for the reserved `"control"` role): the
//! adapter-minted session identity (`crate::hub::session::SessionGuid`), a canonical lowercase
//! hyphenated UUIDv4. The thin ADAPTER mints it once per process (`ipc::relay_adapter`) and reuses
//! it for the process's lifetime; the SERVICE parses it (`SessionGuid::parse`), binds it to the
//! presenting OS peer (`crate::hub::session::SessionRegistry::admit`), and threads it into
//! `transport::mcp::server::serve_session` as that session's opaque identity. The EXTENSION link
//! uses NO hello at all (its own endpoint, server-speaks-first; PINS.md SS1 as amended
//! 2026-07-04), so there is no `ext` role and nothing about the extension link to document here.
//!
//! ## Tab-group-per-session request (H7, ADR-0030 Decision 6/7)
//!
//! ## binary -> extension
//! ```json
//! { "type": "group_request", "guid": "<session guid>", "tabIds": [<number>...], "title": "<string>" }
//! ```
//!
//! ## extension -> binary
//! ```json
//! { "type": "group_response", "guid": "<echoed>", "ok": <bool> }
//! ```
//!
//! Additive; ONE new `type` value on the SAME channel, out of band from tool dispatch exactly
//! like the tab-URL query above. Mechanism only; the extension groups the named tabIds and makes
//! no policy decision (never inspects a tab's url/host/domain/grant to decide membership --
//! ADR-0030 Decision 6: "The extension's per-group checks remain defense-in-depth only"). No
//! `id` member on either side: this is fire-and-forget presentation, never correlated back to a
//! waiting caller the way a `tool_request`/`tab_url_request` reply is (an incoming
//! `group_response` is simply an id-less event to [`crate::hub::outbound::browser::Browser`], same
//! as `session_killed`). The service tracks which tabIds belong to which session
//! (`crate::hub::session`, ADR-0030 Decision 6); this message is how it tells the extension to
//! reflect that in a visible Chrome tab group. Same GUID reuses its existing group; a different
//! GUID gets a distinct one (ADR-0030 Decision 7: "two adapters in one editor -> two GUIDs -> two
//! groups"). The GUID here is the SAME secret material as the session-hello's `guid` member above
//! and MUST NOT be written to any log/audit sink from this path.
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
//! `type` value on the SAME channel, the same fire-and-forget-presentation posture as
//! `group_request` above (no `id`, no reply, no policy decision on the extension side -- it
//! renders exactly what it is told). `title` is deliberately NOT the extension's `caption()`
//! mechanism (optional decorative flavor text, off by default) -- a notification is substantive
//! and must always render. `class` is the standard severity taxonomy this codebase's own tracing
//! already uses -- `"info"`/`"debug"`/`"warn"`/`"error"` -- so the primitive stays general-purpose
//! rather than denial-specific (today: `"error"` for a sacred-domain denial, `"warn"` for a policy
//! denial); `ref` is an opaque cross-reference
//! (today: a denial_id) a viewer can correlate back to
//! the structured audit record later. First caller: [`crate::mcp::pipeline::run_tool_call`], at
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
//! extra traffic. `event` is a short name (`"connect_attempt"`, `"connect_disconnect"` today);
//! `detail` is optional, freeform, and never policy-bearing -- purely a developer breadcrumb.
//! The binary appends it verbatim into [`crate::hub::outbound::browser::Browser`]'s existing
//! debug-state event ring (the SAME file `ghostlight doctor`/a raw `debug-state-<pid>.json`
//! read already surfaces every other lifecycle note from), so the extension's own view of a
//! connection interleaves with the service's, ordered by arrival -- one file, not two to
//! correlate by hand.
