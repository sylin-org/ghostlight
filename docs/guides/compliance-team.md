# Ghostlight for the compliance team

How to take a policy from a blank page to organization-wide enforcement, with evidence
at every step. The whole journey below runs on the free tiers; production deployment by
an organization of more than five people is what requires a license
([PRICING.md](../../PRICING.md)). Evaluation never does.

## What you are governing

Ghostlight gives AI agents (any MCP client) controlled access to employees' real,
authenticated browser sessions. Every one of its actions carries an intrinsic
capability classification:

- `read` -- provably observation only (screenshots, page reads, console/network reads).
- `action` -- UI input whose effect the page decides (clicks, typing, keys). An action
  can cause a write; that is exactly why it is its own class.
- `write` -- a declared mutation (form field setting).
- `execute` -- arbitrary code in the page (`javascript_tool`).

Policy grants capabilities on hosts; it never has to name tools. The agent-facing
`explain` tool (and the table in the [README](../../README.md)) is the authoritative
directory of which action requires what.

## The policy document

A manifest is a small JSON file (schema 3). A grant names hosts (`allow` patterns, with
optional `deny` carve-outs) and the capabilities it permits there:

    {
      "schema": 3,
      "name": "support-team-crm",
      "version": "2026.07.1",
      "mode": "observe",
      "identity": { "resolved_by": "local_file", "principal": "support-team" },
      "grants": [
        {
          "id": "crm-read-write",
          "hosts": { "allow": ["*.crm.example.com"], "deny": ["admin.crm.example.com"] },
          "allowed": ["read", "action", "write"]
        },
        {
          "id": "docs-read-only",
          "hosts": { "allow": ["docs.example.com"] },
          "allowed": ["read"]
        }
      ],
      "config": [
        { "key": "audit.enabled", "value": true, "level": "mandatory" },
        { "key": "audit.destination", "value": "syslog", "level": "recommended" }
      ]
    }

Everything not granted is denied. `deny` carve-outs let you say "everywhere on the CRM
except the admin console" directly. The `config` block carries organization settings;
`"level": "mandatory"` locks a key so users cannot override it.

## Step 1: prototype on your desk

Start from an embedded template and read it back in plain sentences:

    ghostlight policy init --template enterprise-healthcare --out policy.json
    ghostlight policy explain policy.json

`policy explain` renders exactly what the file permits and denies, in prose, before
anything runs. The `examples/` directory has five ready-to-adapt manifests
(`enterprise-healthcare`, `qa-staging`, `research-read-only`, `developer-observe`,
`developer-unrestricted`).

## Step 2: replay reality against the draft (no browser needed)

Have a pilot user run their normal agent work all-open with the audit recorder on
(`ghostlight config set audit.enabled true`). Then replay that recorded activity
through your draft:

    ghostlight policy simulate policy.json --replay audit.jsonl

The output is the would-have-been decision for every recorded call: what your policy
would have allowed, what it would have denied, and under which grant. Iterate on the
draft until the denials are the ones you intend. No browser, no agent, no risk.

## Step 3: observe mode, live

Set `"mode": "observe"` in the manifest and run it with a pilot group
(`GHOSTLIGHT_MANIFEST=file://...` per user, or the org path from step 5). In observe
mode every call still dispatches, and every call that enforce WOULD have blocked is
recorded as a `shadow_deny` audit record with the same stable denial id it would carry
in production. You are collecting enforcement evidence with zero user impact.

There are exactly two modes: `observe` and `enforce`. Mode resolves with clear
precedence: a grant's own `mode` overrides the manifest's, which overrides the
`governance.mode` config default. Sacred domains and user-authored protections always
enforce, in every mode.

## Step 4: enforce

Flip `"mode": "enforce"` (or remove it and set the config default). Blocked calls now
return a denial the agent can read and adapt to: the capability, the host, and a stable
`D-xxxxxxxx` denial id that also appears in the audit record, so a user report and a
log line are trivially matched. Tool advertisement is filtered too: a governed client
only sees the tools its grants could ever permit, plus `explain`.

## Step 5: deploy organization-wide

Place the manifest at the machine org-policy path:

- Windows: `%ProgramData%\ghostlight\policy.json`
- macOS: `/Library/Application Support/ghostlight/policy.json`
- Linux: `/etc/ghostlight/policy.json`

The path is fixed: no flag, environment variable, or config key relocates or bypasses
it. When an org policy is present, a user-supplied manifest is ignored (and that
displacement is itself recorded as a `user_manifest_ignored` audit event).
`"level": "mandatory"` config entries lock those keys against user override; users see
the lock in `ghostlight config list`.

Edits hot-reload: the running session re-resolves with no restart, an advertised-set
change re-advertises the tools, and an invalid edit keeps the last-good policy (fail
closed) rather than falling open. `ghostlight doctor` shows the active manifest's name,
version, and hash on any machine.

For a fleet, you can distribute one signed policy centrally instead of placing a file on
every machine. Sign it with `ghostlight policy publish` and point each endpoint's
`managed.json` bootstrap at a source you control (an HTTPS URL, an object store, a file
share, or a USB path for an air-gapped install). Every endpoint verifies the signature
locally against your public key, caches the last good copy, and keeps enforcing it when
the source is unreachable; it never falls open, and it refuses a rolled-back version.
`ghostlight doctor` then reports the managed sequence, freshness, and source on each
machine, so you can confirm a publish reached the fleet. The mechanics, including the
signing commands and the bootstrap fields, are in the
[governance configuration guide](governance-configuration.md).

## Step 6: evidence

Every call -- permitted, denied, and shadow-denied alike -- produces one JSON-Lines
audit record: identity, tool, capability, host, decision, grant id, denial id, duration,
and the manifest hash that was in force, so any decision is attributable to the exact
policy version that made it. Under managed policy each tool-call record also carries
`policy_seq`, the org-signed publish sequence, so evidence ties a decision to the exact
published version your fleet was running, not only its hash. Session events (the panic
kill switch, manifest reloads, user-manifest displacement) land in the same stream.

Send it to your SIEM over RFC 5424 syslog or collect the JSONL file directly: see the
[SIEM integration guide](siem-integration.md).

## Always-on protections (independent of your policy)

Sacred never-touch domains (user- or org-authored) deny every tool on a matching tab
regardless of grants or mode. The take-the-wheel pause and the panic kill switch belong
to the human at the browser, always. Secret-field redaction strips password/OTP/payment
values from page reads when enabled. None of these are ever gated by licensing
(ADR-0028: license state never affects behavior).

## Licensing, in one paragraph

Everything above is free to evaluate at any scale, with no key and no registration.
Production use with organization-configured governance requires a commercial license;
the first ten organizations get it free for a year ([PRICING.md](../../PRICING.md), the
founding program). The Continuity Promise applies regardless: license state never
interrupts enforcement, audit, or your workflows. Questions: hello@sylin.org.
