# Ghostlight Data Flows

This page states where data moves when you run Ghostlight: between local processes on your
endpoint and to destinations you configure, and nowhere else.

## What runs where

The Ghostlight service and thin relay run on the endpoint. The extension runs inside the user's own
Chromium browser on the same endpoint. There is no vendor-hosted Ghostlight service or cloud backend
anywhere in the path. Your MCP client and the model behind it are yours, running where you run them.

## Flows that exist

| Flow | Transport | Where it goes |
| --- | --- | --- |
| MCP client to agent relay | stdio | Local, same machine. |
| Agent relay to service | Named pipe or Unix-domain socket | Local, owner-scoped IPC. |
| Service to browser relay | Named pipe or Unix-domain socket | Local, owner-scoped IPC. |
| Browser relay to extension | Chromium native messaging | Local, same machine. |
| Extension to pages | DevTools protocol | The user's own authenticated browser session. |
| Audit records | file (JSON Lines), syslog (RFC 5424 over UDP), stderr, or none | The destination you configure; default is a local file. |
| Managed policy fetch | conditional HTTP(S) GET | Your own policy endpoint, and only when your organization configures central policy. The bundle signature, not the transport, is the trust anchor. |

Every one of these is either local to the endpoint or directed at a destination you own and
choose. Only two flows can leave the endpoint, and both are yours: audit delivery to the
destination you configure, and the optional managed policy fetch from an endpoint your
organization hosts.

## Flows that do not exist

The following flows are absent by design, not merely unused, and each is foreclosed by
ADR-0028 Decision 9 (never phone home, normative and permanent):

- Vendor telemetry: none. Ghostlight sends no usage, diagnostic, or analytics data to the
  vendor.
- Licensing callbacks: none. License state is evaluated locally and never validated against a
  vendor server.
- Update phone-home: none. The binary does not call out to check for or pull updates. (The
  extension, once installed from the Chrome Web Store, follows Chrome's own store update
  mechanism; self-hosted and load-unpacked installs update only when you update them.)
- Model-provider calls: none. Ghostlight calls no LLM; the model belongs to your MCP client.

There is zero vendor-bound traffic. Ghostlight has no channel over which your data could reach
the vendor, because no such channel is built.

## Local artifacts

Ghostlight writes a small set of artifacts, all on your endpoint and all owned and retained by
you:

- Audit records: JSON Lines files (or a syslog stream, or nothing) at the destination you
  set. Records are decision metadata only, never page content, typed values, or screenshots;
  the records are not themselves signed, so integrity of the log store is a customer-side
  control.
- Policy cache and status sidecar: present only when you use central policy. The cache is
  signed and its signature is verified on load, so a tampered on-disk policy is rejected rather
  than enforced; the sidecar records the current policy status.
- Configuration files: the local settings and policy configuration you author.

None of these is transmitted to the vendor. Deletion and retention are entirely under your
control.

See [security-overview.md](security-overview.md) and [sub-processors.md](sub-processors.md).

Last reviewed: 2026-07-10 against v0.5.6 | Contact: support@sylin.org
