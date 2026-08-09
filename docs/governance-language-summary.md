# Ghostlight governance language: quick reference

Status: Non-normative summary of the accepted ADR-0102 contract

For exact semantics, schemas, compatibility mappings, and migration gates, use the
[full governance-language primer](governance-language.md). ADR-0102 is the production authority
for implementation.

## The idea

- A **policy** limits browser work by host and capability.
- **Settings** control safety, privacy, audit, runtime, and service behavior.
- **User hold**, **attention pause**, **end session**, browser availability, and final admission
  are runtime state, not policy rules.
- Ghostlight makes one decision trace and renders it through the one Ghostlight result
  contract on every supported MCP revision.

No policy means policy does not restrict browser operations. Protected hosts, ownership, request
restrictions, user controls, and browser availability still apply. Audit and privacy are configured
separately.

## Choose what to configure

| Need | Use |
| --- | --- |
| Unrestricted personal work | No policy |
| Never let Ghostlight touch personal sites | `safety.protected_hosts` |
| Limit your own browser work | Personal policy |
| Require one policy on a managed machine | Fixed organization policy |
| Distribute signed policy to a fleet | Managed organization policy |
| Suggest a setting | Organization default setting |
| Lock a setting | Organization required setting |
| Narrow one request | Tighten-only request restriction |
| Test before blocking | Policy enforcement `observe` |
| Block policy mismatches | Policy enforcement `enforce` |

## Shortest policy

```json
{
  "schema": 1,
  "name": "support-crm",
  "revision": "2026.08.1",
  "enforcement": "observe",
  "rules": [
    {
      "id": "crm",
      "hosts": ["*.crm.example.com"],
      "except_hosts": ["admin.crm.example.com"],
      "capabilities": ["read", "interact", "write"]
    }
  ]
}
```

Rules are checked in order. The first rule that covers the normalized host decides whether its
capabilities are enough. `except_hosts` removes coverage from that rule only; a later rule may
cover the host. Use protected hosts for a global never-touch boundary.

Policy enforcement is required because omitting it could silently weaken or strengthen the
author's intent. Canonical v1 has no rule-level override.

## Capability chooser

Capabilities are independent, not a ladder.

| Capability | Grant it when |
| --- | --- |
| `read` | Ghostlight may perform operations proven to observe or retrieve only. |
| `interact` | Ghostlight may send UI input whose consequence is decided by the page, such as a click. |
| `write` | Ghostlight may perform an operation whose declared purpose is to change data or state. |
| `execute` | Ghostlight may run arbitrary code. No other capability implies it. |

Historical policy capability `action` normalizes to canonical `interact`.

## Host patterns

- `example.com` matches only that exact host.
- `*.example.com` matches subdomains, not the apex `example.com`.
- `*` is the policy-only catch-all. Protected hosts reject it.
- Patterns contain no scheme, port, path, query, fragment, userinfo, or whitespace.
- International domains use IDNA A-label form.
- Exact IPv4 and IPv6 are allowed; wildcards never match IP literals.
- Within one rule, the most specific include or exception wins; a tie goes to the exception.
- Invalid or unresolvable hosts fail closed when governance needs a host.

## Shortest settings file

```json
{
  "safety.protected_hosts": ["*.mybank.example"],
  "privacy.redact_sensitive_fields": true,
  "audit.output": "file"
}
```

Most settings resolve in this order:

1. managed-organization required;
2. fixed machine-organization required;
3. user;
4. managed-organization default;
5. fixed machine-organization default;
6. product default.

Protected hosts are different: authorized sources union after normalization. No organization or
request may remove another source's protected host.

A settings-only organization package does not hide a personal or machine policy. It contributes
settings, then policy selection continues. Only a present policy stops that search; an explicit
empty enforce policy is how an administrator chooses block-all.

`audit.output` is the one canonical audit switch: `off`, `file`, `stderr`, or `syslog`. No policy
is required for audit.

## Decision meanings

| Outcome | Meaning |
| --- | --- |
| `not_applicable` | No policy decision was needed, such as no policy or no required capability. |
| `allowed` | The first covering policy rule contains every required capability. |
| `would_block` | Observe mode found the reason enforce mode would block, but dispatch may continue. |
| `blocked` | Policy, protected hosts, or a request restriction refused the operation or landing. |

Governance outcome and operation outcome are separate. A block before dispatch becomes operation
`status: blocked`, `effect: none`. A policy-refused landing after navigation committed becomes
operation `status: partial`, `effect: committed`, and `repeat: do_not_repeat`.

## Safe recovery

| Situation | Safe next move | Never do |
| --- | --- | --- |
| Protected host | Hand control to the user or stop | Suggest a workaround |
| Enforced policy block | Ask the user, use trusted organization contact, choose an already-authorized alternative, or stop | Retry immediately or evade the rule |
| Would block | Continue only because observe mode permits it; tell the user that enforce would block | Claim the policy protected the call |
| User hold | Wait for the person | Edit policy or reconnect the browser |
| Attention pause | Wait for the person to resume, quiet, or end this session | Retry the denied call |
| Browser unavailable | Reconnect the correct browser side | Treat it as a policy issue |
| Unknown browser effect | Observe state or ask the user | Replay the action |
| Managed last-known-good | State which verified policy remains active and why | Claim governance disappeared |

## One outcome contract

Navigation may produce more than one governance decision. Ghostlight reports its non-normal decisions
in order: target first, then each committed landing. Every supported MCP revision preserves
governance outcome, operation status, effect, repeat safety, readiness, and uncertainty. Suggested
calls always use the exact Ghostlight names and valid Ghostlight arguments.

Ghostlight can carry observe-mode truth without turning success into a problem:

```json
{
  "status": "ok",
  "summary": "The requested page read completed. The active policy would block this call under enforce mode.",
  "effect": "none",
  "repeat": "safe",
  "governance": [
    {
      "outcome": "would_block",
      "source": "policy",
      "phase": "pre_dispatch",
      "reason": "capability_not_allowed",
      "decision_id": "D-4c5a910e",
      "rule_id": "crm"
    }
  ]
}
```

## What stays separate

- Policy is not audit configuration.
- Protected hosts are not observe-mode policy rules.
- A user hold is not a denial.
- An attention pause is not a global hold.
- Policy allowance is not proof of dispatch or success.
- Browser failure is not a policy block.
- Outcome uncertainty is not permission to retry.
- Authored labels are not verified identity.
- MCP lifecycle and envelopes are not canonical authority.

## Migration

Existing schema-3 policies and dotted settings remain accepted inputs while they normalize into
the canonical model. The full primer records the required root fixes, including misleading
presets, duplicate audit controls, protected-host union, managed-settings visibility, template
truth, typo handling, denial prose ownership, and audit-axis clarity.
