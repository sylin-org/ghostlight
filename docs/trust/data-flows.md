# Ghostlight Data Flows

This page states, plainly and exhaustively, where data moves when you run Ghostlight. The
short version: it moves between local processes on your endpoint and to destinations you
configure, and nowhere else.

## What runs where

The Ghostlight binary runs on the endpoint. The thin extension runs inside the user's own
Chrome on the same endpoint. There is no Ghostlight service, cloud backend, or vendor-hosted
component anywhere in the path. Your MCP client and the model behind it are yours, running
where you run them.

## Flows that exist

| Flow | Transport | Where it goes |
| --- | --- | --- |
| MCP client to binary | stdio | Local, same machine. |
| Binary to extension | Chromium native messaging | Local, same machine. |
| Extension to pages | DevTools protocol | The user's own authenticated browser session. |
| Audit records | file (JSON Lines), syslog (RFC 5424 over UDP), stderr, or none | The destination you configure; default is a local file. |
| Managed policy fetch | conditional HTTPS GET | Your own policy endpoint, and only when your organization configures central policy. |

Every one of these is either local to the endpoint or directed at a destination you own and
choose. The managed policy fetch is the only outbound network flow, it is optional, and it
targets an endpoint your organization hosts.

## Flows that do not exist

The following flows are absent by design, not merely unused, and each is foreclosed by
ADR-0028 Decision 9 (never phone home, normative and permanent):

- Vendor telemetry: none. Ghostlight sends no usage, diagnostic, or analytics data to the
  vendor.
- Licensing callbacks: none. License state is evaluated locally and never validated against a
  vendor server.
- Update phone-home: none. The binary does not call out to check for or pull updates.
- Model-provider calls: none. Ghostlight calls no LLM; the model belongs to your MCP client.

There is zero vendor-bound traffic. Ghostlight has no channel over which your data could reach
the vendor, because no such channel is built.

## Local artifacts

Ghostlight writes a small set of artifacts, all on your endpoint and all owned and retained by
you:

- Audit records: JSON Lines files (or a syslog stream, or nothing) at the destination you set.
- Policy cache and status sidecar: present only when you use central policy. The cache is
  signed and its signature is verified on load, so a tampered on-disk policy is rejected rather
  than enforced; the sidecar records the current policy status.
- Configuration files: the local settings and policy configuration you author.

None of these is transmitted to the vendor. Deletion and retention are entirely under your
control.

See [security-overview.md](security-overview.md) and [sub-processors.md](sub-processors.md).

Last reviewed: 2026-07-10 against v0.5.4 | Contact: support@sylin.org
