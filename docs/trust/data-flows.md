# Ghostlight data flows

Ghostlight has no vendor-hosted runtime or cloud backend. Browser work, policy decisions, and
audit run on the endpoint.

## Flows that exist

| Flow | Transport | Destination |
| --- | --- | --- |
| MCP client to MCP connector | stdio | Same endpoint. |
| MCP connector to service | Authenticated loopback TCP with typed framing | Same endpoint. |
| Service to browser connector | Authenticated loopback TCP with typed framing | Same endpoint. |
| Browser connector to extension | Chromium native messaging | Same endpoint. |
| Extension to the controlled page | Chrome DevTools Protocol | The user's existing browser profile. |
| Browser page traffic | Browser networking | Sites the user or MCP client directs the browser to. |
| Audit records | Append-only JSON Lines file | Local path selected by the endpoint owner. |
| Managed policy fetch | Local file read or conditional HTTPS GET | Customer-configured source, only when an administrator provisions `managed.json`. |

The managed HTTPS source may use a bearer token and organization CA pin. The signed bundle is the
authority: Ed25519 is required, optional ML-DSA-65 makes both legs mandatory, and rollback is
refused by monotonic sequence. No bootstrap means no policy network work.

## Flows that do not exist

- No telemetry, analytics, crash upload, activation callback, or update ping.
- No audit upload, direct syslog delivery, HTTP collector, or inbound management listener.
- No Ghostlight model-provider call. The MCP client owns its model relationship.
- No vendor policy endpoint or embedded vendor policy key.

There is zero vendor-bound traffic. The only Ghostlight-initiated network flow that may leave the
endpoint is the explicitly configured managed HTTPS fetch to the customer's own source. Ordinary
site traffic remains browser traffic.

## Local artifacts

- `audit.jsonl`, or the path in `GHOSTLIGHT_AUDIT_FILE`, contains content-minimized terminal
  records. Use the endpoint's existing file collector for SIEM delivery.
- The runtime discovery file contains loopback ports, protocol majors, and a local authentication
  token.
- Managed deployments keep a verified signed bundle cache and a content-minimized status sidecar.
  The sidecar includes verification, sequence, freshness, source class, timing, organization, and
  contacts, but no source address, bearer token, verification key, or policy rules.
- Browser adapter settings and MCP-client registrations remain in their platform-native local
  stores.

The governed host and optional bounded target name are the only page-derived audit text. Audit
does not contain paths, queries, fragments, arbitrary page text, selectors, target handles, typed
values, scripts, screenshots, recordings, or credentials. Retention and deletion are customer
controls.

See [security-overview.md](security-overview.md),
[the SIEM guide](../guides/siem-integration.md), and
[the governance guide](../guides/governance-configuration.md).

Last reviewed: 2026-08-14 against the 1.0 source candidate | Contact: support@sylin.org
