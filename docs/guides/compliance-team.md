# Ghostlight 1.0 for a compliance team

Ghostlight applies local, monotonic authority to visible browser work. The 1.0 runtime does not
fetch policy, operate a hosted control plane, or transmit page content. An organization provisions
its managed authority file through its existing endpoint-management channel.

## 1. Define the boundary

Start with the smallest capabilities and hosts the user job needs. The flat version-1 schema is
documented in [`governance-configuration.md`](governance-configuration.md).

```json
{
  "version": 1,
  "managed": true,
  "expires_unix_ms": 1893456000000,
  "allow_capabilities": ["read", "action", "write"],
  "deny_capabilities": ["execute"],
  "allow_tab_close": false,
  "allow_hosts": ["support.example.com", "*.support.example.com"],
  "deny_hosts": ["admin.support.example.com"]
}
```

`read`, `action`, `write`, and `execute` are the complete capability vocabulary. Host allow-lists
from local, managed, and request layers must all match. Any deny wins. `allow_tab_close: false` is
monotonic and remains independent of the extension's local preserve-tabs interlock.

## 2. Validate before provisioning

Run the exact candidate with `GHOSTLIGHT_MANAGED_AUTHORITY_FILE` pointing to a temporary copy.
Open **Checkup** and confirm managed authority is configured and valid. Exercise:

- one permitted read;
- one permitted action if the policy includes action;
- one host denial;
- one capability denial; and
- one model-driven close denial.

The MCP terminal result, browser receipt, workbench History item, and JSONL audit reason must agree.
An invalid or expired configured managed file must fail closed.

## 3. Provision locally

Use the organization's authenticated endpoint-management tooling to place the policy at an
administrator-controlled path and set `GHOSTLIGHT_MANAGED_AUTHORITY_FILE` for the Ghostlight
process. The path is explicit rather than magical so packaging and fleet management can use native
OS conventions without teaching the model or extension about them.

Every managed file requires `managed: true` and a future Unix-millisecond expiry. Rotation is an
external deployment transaction: validate the complete replacement, update atomically through the
endpoint manager, and verify Checkup again. Ghostlight snapshots authority once per started
invocation, so an in-flight unit of work does not change policy halfway through.

The 1.0 runtime deliberately does not implement remote retrieval, signing, last-known-good fetch,
observe mode, config locks, or tool-catalog filtering. Those historical 0.8 designs are not 1.0
claims.

## 4. Collect payload-free evidence

Set `GHOSTLIGHT_AUDIT_FILE` to an organization-collected local path. The append-only JSONL records
contain timestamp, opaque ids, tool, capability, authority id, allow/deny decision, stable reason,
terminal status, and effect class. They contain no URL, hostname, page content, selector, form
value, file path, script, screenshot, or dialog text.

Use the endpoint's existing file collector. Ghostlight 1.0 does not open a syslog socket or send
audit over the network. The exact record contract and safe collection pattern are in
[`siem-integration.md`](siem-integration.md).

## 5. Preserve the human controls

Compliance policy must not replace user visibility. Keep the extension toolbar, blocked receipt,
dedicated tab group, workbench history, pause, attention, and end-session paths enabled. The
browser-local preserve-tabs setting is a second protective gate, not a policy editor.

## Release evidence

Before organizational rollout, require the signed platform package, matching extension, clean
install/upgrade/uninstall evidence, visible-browser policy-denial journey, and native notification
smoke test from [`../1.0/ACCEPTANCE.md`](../1.0/ACCEPTANCE.md).
