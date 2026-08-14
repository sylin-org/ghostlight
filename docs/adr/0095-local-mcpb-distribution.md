# ADR-0095: Local MCPB distribution

Status: Accepted

Date: 2026-08-04

Builds on: ADR-0015, ADR-0028, ADR-0065, ADR-0070, ADR-0077, ADR-0091, and ADR-0093

## Amendment by ADR-0096 (2026-08-04)

The bundle now carries all three current executables for each packaged platform:
`ghostlight-mcp`, `ghostlight`, and `ghostlight-relay`. The launcher still performs the same
browser-side `ghostlight install --no-clients --no-open` handoff, then starts `ghostlight-mcp` on
inherited stdio. The removed `ghostlight-relay --role agent` path is not a fallback.

## Context

Claude Desktop's connector directory accepts local MCP servers packaged as an MCP Bundle (MCPB).
Ghostlight already has the required local stdio MCP transport, native browser host, persistent
service, and Chrome Web Store adapter. Its normal installer also registers detected MCP clients.
An MCPB host owns its own client entry, so running the default installer from an MCPB would create
a second Claude configuration and blur ownership.

Ghostlight cannot replace that local architecture with a hosted MCP endpoint for directory reach.
Browser control remains local-only under ADR-0077, and ADR-0028 forbids runtime download or
phone-home behavior. The Chrome adapter also remains a Chrome Web Store install under ADR-0091.

## Decision

### 1. Ship a self-contained MCPB as a service release asset

The MCPB carries the Windows x86_64 `ghostlight-mcp-connector`, `ghostlight`, and
`ghostlight-browser-connector` binaries. A small Node launcher selects the packaged target at
runtime. The package does not fetch binaries or dependencies after installation.

The tracked MCPB manifest versions with the service. It publishes functional listing copy, the
canonical privacy policy, and runtime-generated tools. It does not claim that the open-core bundle
has one SPDX license.

### 2. Give package managers a client-registration boundary

`ghostlight install --no-clients` registers the browser native host and service but never reads or
writes an MCP-client configuration. The MCPB launcher runs that idempotent command with
`--no-open`, captures installer stdout away from the MCP protocol, and then starts
`ghostlight-relay --role agent` with inherited stdio.

This is a narrow selection mode, not a second installer. Normal installs keep detecting and
registering clients as before.

### 3. Keep the browser adapter separate and store-only

The MCPB listing and included README state that Ghostlight in Browser must be installed from the
Chrome Web Store. The package never embeds an unpacked extension and does not change adapter
versioning.

### 4. Do not add remote ingress for directory compatibility

The MCPB is a local distribution surface. It does not add TCP, WebSocket, Streamable HTTP, a
hosted browser proxy, telemetry, activation, or an update ping. A directory that requires a public
HTTPS MCP URL is not compatible with Ghostlight's current trust boundary. That incompatibility is
documented, not routed around.

### 5. Accept the MCPB uninstall limitation

The MCPB format has no package uninstall hook. Removing the bundle can therefore leave an OS
native-host registration whose executable path no longer exists. Chromium treats that as an
unavailable host; it grants no access and creates no network activity. A later Ghostlight install
refreshes the path, and `ghostlight uninstall` remains the explicit full cleanup path.

## Consequences

- Claude Desktop can install and own Ghostlight as one local connector without a duplicate client
  entry.
- Release assembly must include the supported Windows platform pair before it can build the MCPB.
- First launch performs local, idempotent browser-side registration and may start the service.
- Users still install the Chrome Web Store adapter separately.
- OpenAI's hosted-URL directory remains out of scope unless it accepts local stdio packages; a
  remote transport will not be built only to satisfy a listing form.
