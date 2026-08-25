# ADR-0077: Local-only ingress and removal of inbound.web

Date: 2026-07-14
Status: Accepted
Supersedes: ADR-0030 web-ingestion decisions, ADR-0033 Decisions 3-8 as they apply to
`inbound.web`, ADR-0034's web transport, ADR-0076

## Context

Ghostlight controls the user's ordinary authenticated browser session. That user context is the
product boundary, not a browser compute target to expose as a service. The HTTP/1.1 WebSocket
`inbound.web` adapter created a second MCP session source over TCP. Even while disabled by default,
its implementation retained a non-loopback bind path, source-policy grammar, a remote-enable
management action, WebSocket framing, and tests that preserved the possibility of anonymous remote
browser control.

ADR-0076 proposed a standards-based managed remote transport as the condition for bringing native
remote access back. The owner has instead chosen to remove the capability until the surface risks
are understood well enough to justify a new design. Keeping dormant transport and policy
scaffolding would preserve risk and maintenance cost without serving the local product.

The loopback management Console is a different bounded context. It observes local service state and
does not ingest MCP tool calls. It remains useful and can own its small HTTP listener directly.

## Decision

### 1. Ghostlight accepts browser-control sessions only through local OS IPC

The MCP client path is the owner-only named pipe on Windows or Unix domain socket on macOS and
Linux. The Chrome extension path remains Chromium native messaging through the local relay. There
is no TCP, WebSocket, Streamable HTTP, cloud-browser, or non-loopback browser-control listener.

The product does not advertise SSH, VPN, port-forward, reverse-proxy, or remote-desktop tunneling
as a Ghostlight transport. Operating-system administrators can always expose local resources
outside the application, but that is not an implemented or supported Ghostlight capability.

### 2. inbound.web and its policy surface delete

Delete the complete `inbound.web` adapter and every artifact that exists only for it:

- its TCP listener and WebSocket handshake/framing implementation;
- `inbound.web.enabled` and `inbound.web.from`;
- the connecting-source PDP and `DecisionRequest.inbound_source`;
- the remote-enable Console route, UI, copy, and CSRF header;
- its transport registration, tests, Lightbox scenarios, and generated config documentation;
- proposed native remote authentication in ADR-0076.

There is no disabled feature flag or compatibility shim. A configuration containing a retired key
fails as an unknown key, consistent with the pre-1.0 clean-break policy.

### 3. manage.web owns a standalone loopback listener

The management Console keeps its HTTP UI on explicit `127.0.0.1`. It binds only when
`manage.web.enabled` resolves true. Its router independently enforces:

- a loopback peer address;
- a loopback `Host` header to prevent DNS rebinding;
- bounded HTTP request headers;
- no WebSocket upgrade.

The redundant `manage.web.from` configuration key deletes because the listener and router are
loopback-only by construction. Policy may disable the Console; no configuration can widen it.

The port remains test-isolatable through a management-specific environment override. Debug state
reports the actual management port after a successful bind.

### 4. Future remote access starts from zero

Remote browser control may return only after a new threat model, user need, client evidence, and
accepted ADR justify it. That work starts as a new transport and cannot restore deleted
`inbound.web` code, configuration names, or protocol behavior by default.

Headless, isolated-profile, and cloud-browser execution remain outside Ghostlight's product
direction. Ghostlight acts in the visible browser context of the local user.

## Consequences

- The anonymous plaintext remote-ingress path and its dormant reactivation surface are gone.
- The local architecture becomes one browser-control ingress: owner-only OS IPC.
- The Console is simpler, read-only, and physically incapable of admitting MCP sessions.
- Existing configurations that mention retired web-ingestion keys must remove them before upgrade.
- Organizations cannot use Ghostlight as a remotely hosted browser-control service.
- A future remote design bears the full burden of a fresh security and product decision.

## Amendment (2026-08-25)

This record stands as written; the text above is the decision as it was made. The 1.0 candidate
narrows the supported operating-system matrix to Windows and Linux
([1.0/ACCEPTANCE.md](../1.0/ACCEPTANCE.md)). macOS mentions above describe the scope considered at
decision time; macOS remains a later row of the platform table, deferred for want of test hardware,
not removed from the product's future.
