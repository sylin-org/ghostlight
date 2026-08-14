# Ghostlight 1.0 for a compliance team

Ghostlight applies signed, host-scoped authority to visible browser work. It has no vendor policy
service. Your organization owns the policy, keys, endpoint bootstrap, and optional HTTPS or file
distribution source.

## Roll out safely

1. Write the smallest schema-3 grants that admit the required hosts and independent RAWX sets.
2. Start in `mode: "observe"` and collect content-minimized JSONL from a pilot group.
3. Run `ghostlight policy validate`, `policy explain`, and `policy simulate` against real audit.
4. Generate organization-owned signing keys offline and publish a monotonically sequenced bundle.
5. Provision `managed.json` through your endpoint-management tooling.
6. Confirm the workbench Policy Passport, then move the policy to `mode: "enforce"`.

The complete schema, signing commands, bootstrap paths, and failure behavior are in
[`governance-configuration.md`](governance-configuration.md).

## What to verify

Exercise one permitted read, one permitted action, a host denial, a capability denial, and a
model-driven close denial. The MCP result, browser receipt, workbench History, Policy Passport,
and JSONL audit must agree. An enforced denial must carry the same denial id and grant attribution
as audit.

Test update failure too. A bad signature, rollback, malformed source, or unreachable source must
retain the active verified bundle. A configured cold start without a valid source or cache must
fail closed. Three matching denials in 60 seconds must enter the visible attention state until a
person chooses what happens next.

## Collect evidence

Set `GHOSTLIGHT_AUDIT_FILE` to an organization-collected local path and use the endpoint's existing
file collector. Each record carries opaque correlation ids, the complete RAWX requirement set,
decision attribution, managed `policy_seq`, terminal outcome, governed host, and bounded
measurements. It excludes paths, queries, fragments, arbitrary page text, selectors, form values,
file contents, scripts, screenshots, dialog text, policy rules, and credentials.

Ghostlight does not upload audit or send direct syslog/HTTP. The exact record contract and safe
collection pattern are in [`siem-integration.md`](siem-integration.md).

## Keep the human controls

Managed policy does not replace visibility. Keep the toolbar, denial receipts, dedicated tab
group, workbench history, pause, attention, resume quietly, and end-session paths available. The
browser-local preserve-tabs setting remains a second gate beneath policy.

Before rollout, require the checksum-bound package, matching extension, provenance attestation,
clean install/upgrade/uninstall evidence, visible-browser policy journey, and native notification
smoke test in [`../1.0/ACCEPTANCE.md`](../1.0/ACCEPTANCE.md).
