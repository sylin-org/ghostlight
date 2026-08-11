# Ghostlight 1.0 governance configuration

Ghostlight is useful without a policy. With no configured authority file, ordinary remote
HTTP(S) browser work is permitted while protected hosts, credentials, stale handles, runtime
holds, and browser-local interlocks remain enforced.

Policy is a monotonic restriction layer. It can remove capabilities, hosts, or model-driven tab
close; it cannot add a browser mechanism or override another denying layer.

## Capabilities

The closed 1.0 capability vocabulary is:

- `read` -- observe browser facts;
- `action` -- cause ordinary browser interaction;
- `write` -- enter non-credential user data; and
- `execute` -- run an explicit script or commit a consequential submission.

Each catalog tool has one highest required capability. The complete mapping is maintained with the
schemas in [`../1.0/LANGUAGE.md`](../1.0/LANGUAGE.md).

## Local policy

Set `GHOSTLIGHT_POLICY_FILE` to the absolute path of a JSON policy owned by the current user. The
schema is flat and typo-closed:

```json
{
  "version": 1,
  "allow_capabilities": ["read", "action", "write"],
  "deny_capabilities": ["execute"],
  "allow_tab_close": false,
  "allow_hosts": ["example.com", "*.example.com"],
  "deny_hosts": ["admin.example.com"]
}
```

Fields:

| Field | Required | Meaning |
| --- | --- | --- |
| `version` | yes | Must be `1`. |
| `managed` | no | Must be `true` only for a managed authority file. |
| `expires_unix_ms` | no | Required future Unix-millisecond expiry for managed authority. |
| `allow_capabilities` | no | Intersects the current capability set. Omission leaves it unchanged. |
| `deny_capabilities` | no | Removes named capabilities after the allow intersection. |
| `allow_tab_close` | no | `false` removes model-driven close. `true` cannot restore another layer's denial. |
| `allow_hosts` | no | Adds a required host allow-list layer. Every configured layer must match. |
| `deny_hosts` | no | Denies matching hosts regardless of an allow-list match. |

Unknown fields, unknown capabilities, unsupported versions, invalid host patterns, and non-JSON
input invalidate the layer. A configured invalid layer fails closed; Ghostlight does not silently
fall back to unrestricted work.

Host patterns are exact hostnames or a leading wildcard such as `*.example.com`. A wildcard does
not match the apex, so include `example.com` separately when both are intended. Patterns cannot
contain schemes, ports, paths, or embedded wildcards.

## Managed authority

Set `GHOSTLIGHT_MANAGED_AUTHORITY_FILE` to a separately provisioned JSON file:

```json
{
  "version": 1,
  "managed": true,
  "expires_unix_ms": 1893456000000,
  "allow_capabilities": ["read", "action"],
  "allow_tab_close": false,
  "allow_hosts": ["support.example.com"]
}
```

A managed file must carry `managed: true` and an unexpired `expires_unix_ms`. Local policy,
managed authority, and per-request restrictions intersect. The most restrictive outcome wins;
order never grants authority.

The 1.0 orchestrator consumes a locally provisioned managed file. It does not fetch policy, poll a
vendor service, accept remote activation, or implement a hidden management channel. External
provisioning, signing, and fleet distribution are outside this runtime contract.

## Protected host ceiling

Policy never grants:

- schemes other than `http` and `https`;
- `localhost` or its subdomains;
- loopback IP addresses; or
- link-local addresses, including cloud metadata endpoints.

Committed navigation landings are checked again before Ghostlight accepts content or readiness.
A redirect or page script therefore cannot use an initially allowed URL to smuggle in a denied
landing.

## Per-request restrictions

Every tool may carry `restrict_capabilities` and `restrict_hosts`. They intersect configured
authority for that invocation only:

```json
{
  "url": "https://example.com",
  "restrict_capabilities": ["read", "action"],
  "restrict_hosts": ["example.com"]
}
```

Restrictions are useful when a client wants a narrower unit of work. They cannot add a capability
or host absent from another layer. One immutable effective snapshot remains attached to started
work even if policy files change during the invocation.

## Runtime controls

The extension toolbar and desktop workbench send semantic intents to the orchestrator's one
runtime-control owner:

- pause or hold stops later effects;
- resume permits work again unless the session was ended;
- attention stops effects until the user resolves the condition; and
- end-session is terminal until an explicit start-session intent.

For managed local testing, `GHOSTLIGHT_RUNTIME_CONTROL_FILE` may name a file whose trimmed content
is `active`, `hold`, `attention`, or `end_session`. An unreadable or unrecognized configured value
holds rather than opening work.

## Tab-close protection

Model-driven tab close requires both:

1. effective orchestrator authority, including `allow_tab_close`; and
2. the extension's local **Preserve controlled tabs** setting.

Either layer may deny. Neither can expand the other. A refusal keeps the tab available as visual
evidence, shows a fixed browser receipt, and returns a blocked no-effect result. Manual browser
closure remains the user's action.

## Payload-free audit and history

The orchestrator appends one JSONL record per terminal invocation. By default the file is
`audit.jsonl` beside the runtime discovery file; `GHOSTLIGHT_AUDIT_FILE` selects an explicit path.

Records contain only timestamp, opaque invocation/workspace/authority ids, tool, capability,
allowed decision, stable reason, terminal status, and effect class. They never contain URLs, page
text, selectors, target labels, form values, file paths, scripts, screenshots, or dialog text.

The workbench reconstructs at most 500 newest terminal facts from this audit for History. Search
and notifications operate on the same content-free projection. Deleting or rotating the audit is
an external retention decision; it does not alter authority.

## Validate a policy

Before release, policy validation is performed by the same strict decoder used to start work.
Open **Status** to see whether configured local and managed sources are present and valid. Then
exercise both an allowed journey and a deliberately blocked journey against a non-sensitive test
site. The browser receipt, MCP result, audit reason, and workbench history must agree.
