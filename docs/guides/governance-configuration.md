# Configure Ghostlight 1.0 governance

Ghostlight needs no policy for personal use. With no policy configured, ordinary HTTP(S) browser
work is open. Runtime controls, credential handoff, stale-handle checks, browser-local interlocks,
and protected loopback or link-local destinations still apply.

A policy can only narrow that baseline. Managed policy, local policy, and per-request restrictions
intersect. No lower layer can restore authority removed above it.

## Write a schema-3 policy

Read, Action, Write, and Execute are independent capabilities. A compound operation needs every
capability in its set; Execute does not imply the other three. The exact operation map lives in
[`../1.0/LANGUAGE.md`](../1.0/LANGUAGE.md).

```json
{
  "schema": 3,
  "name": "Support workspace",
  "version": "2026-08-14",
  "mode": "enforce",
  "identity": {
    "principal": "support-agent",
    "groups": ["support"]
  },
  "grants": [
    {
      "id": "support-sites",
      "hosts": {
        "allow": ["support.example.com", "*.support.example.com"],
        "deny": ["admin.support.example.com"]
      },
      "allowed": ["read", "action", "write"],
      "description": "Ordinary support work"
    }
  ],
  "config": [
    {"key": "browser.tabs.allow_close", "value": false, "level": "mandatory"},
    {"key": "privacy.preserve_target_names", "value": false, "level": "mandatory"},
    {"key": "channels.cli.enabled", "value": false, "level": "mandatory"},
    {"key": "content.security.sacred_domains", "value": ["vault.example.com"], "level": "mandatory"}
  ]
}
```

The document is typo-closed. Unknown fields, settings, capabilities, modes, and malformed host
patterns invalidate it. A configured source with no valid initial policy fails closed.

Host patterns are `*`, an exact hostname, or one leading suffix wildcard such as
`*.example.com`. Exact matches outrank longer suffix matches, which outrank `*`; an exact tie
denies. A grant's deny patterns shrink only that grant. Grants are checked in written order, and
the first grant admitting the complete capability set wins.

`mode` is `enforce` or `observe`. Observe records what would have been denied while allowing
ordinary work to continue. Protected destinations always enforce. A per-grant mode may override
the manifest mode; the strictest effective layer wins.

Supported settings are:

| Key | Value | Effect |
| --- | --- | --- |
| `browser.tabs.allow_close` | boolean | `false` removes model-driven close. |
| `privacy.preserve_target_names` | boolean | `false` keeps page-authored target names out of results and audit. |
| `channels.mcp.enabled` | boolean | `false` refuses MCP session admission. |
| `channels.cli.enabled` | boolean | `false` refuses `ghostlight call` admission. |
| `content.security.sacred_domains` | hostname array | Adds never-touch destinations. |
| `policy.user.enabled` | boolean | `false` stops this machine's user from authoring a local policy. |

`policy.user.enabled` is honored only from an organization layer, and it gates authoring rather than
enforcement. A user policy that already exists keeps applying when it is switched off, because a
user layer can only subtract authority and ignoring it would hand authority back. It is not a
security control: a user layer could never widen anything. Use it when a fleet needs to stay
predictable, and supply an `organization.statement` so the person reads a reason rather than a
missing button.

## Say who wrote the policy

A policy may name its author. The block is optional, informational, and never participates in a
decision. It exists so the person being governed can see who is restricting them and where to ask.

```json
{
  "organization": {
    "name": "Example Organization",
    "statement": "Keeps browser work inside approved support sites.",
    "url": "https://example.com/browser-policy",
    "contacts": [
      {"kind": "email", "value": "security@example.com", "label": "Security team"}
    ]
  }
}
```

`name` is required when the block is present, `url` must be HTTPS, and at most 8 contacts are
allowed. The workbench shows the URL as text: destinations it can open come from a closed
vocabulary, never from an authored address.

Manifests are typo-closed, so a policy carrying this block is rejected by a Ghostlight older than
its introduction. Keep it out of documents you publish to a mixed fleet until the fleet has moved.

When a signed bundle carries the separate presentation block, that presentation wins on conflict.
Both are covered by the signature; the presentation block is the outer published statement, so
bundles already deployed keep behaving exactly as they do.

## Use a local policy

There is exactly one user layer. Its document comes from one of two places, in this order:

1. `GHOSTLIGHT_POLICY_FILE`, when set to the absolute path of a schema-3 JSON file before starting
   Ghostlight. Ghostlight reads that file and never writes to it. The workbench shows it read-only
   and says why.
2. Otherwise the file Ghostlight owns, beside the managed cache in the per-user state directory:
   `%LOCALAPPDATA%\Ghostlight\user-policy.json` on Windows,
   `$XDG_STATE_HOME/ghostlight/user-policy.json` on Linux. This is the file the workbench writes,
   and it is optional: a machine that has never authored one is all-open, not failing closed.

Valid replacements apply atomically to future invocations. A malformed replacement keeps the last
valid policy; a malformed cold start fails closed.

## Read and write policy in the workbench

The workbench has a Policy destination, reached from the tab row or from the state chip beside it.
It shows the compiled result rather than the configuration: what agents may do right now, which
layer decided each line, the rules behind those lines, the boundaries no policy can lift, and the
exact document and path for every layer in force.

When Ghostlight owns the user file and no organization has switched authoring off, the same page
edits it. Rules read as sentences, host patterns are read back in plain words as they are typed,
capabilities an organization refuses are shown refused on the control itself, and a rule that can
never fire says so in place. Before applying, the page replays the candidate through the production
decision engine against this machine's recorded audit and reports what would have been refused.

The same page authors the registered settings, presented as toggles grouped by what they are about
-- where agents may connect, in the browser, privacy -- each starting on, since every one of them
is permissive by default. Turning one off is the only thing the switch does: the permissive value
is never written, because a user layer cannot hand authority back, and a switch already forced off
by an organization renders disabled and names who set it. `policy.user.enabled` stays
organization-only and is refused if a user document tries to author it. Sacred destinations are
edited as a list with the same plain-words readback host patterns get.

Applying validates before it replaces anything and writes atomically, so no action in the window
can leave Ghostlight configured with a policy it cannot read. Removing the rules is one action and
returns authority to whatever remains above them.

Validate and inspect a candidate with the production parser and capability directory:

```sh
ghostlight policy validate policy.json
ghostlight policy explain policy.json
ghostlight policy simulate policy.json audit.jsonl
```

Simulation is audit-free. It reports which existing audit records the candidate would deny and
which rule supplied the decision.

## Publish signed managed policy

Managed delivery is opt-in. Without an administrator-provisioned bootstrap, Ghostlight performs no
policy network I/O. The organization owns its signing keys and the file or HTTPS source.

Create keys offline. The default creates required Ed25519 and additive ML-DSA-65 keys:

```sh
ghostlight policy keygen policy-keys
ghostlight policy pubkey policy-keys/policy-ed25519.seed --mldsa-seed policy-keys/policy-mldsa65.seed
```

Add optional signed presentation in a separate JSON file:

```json
{
  "org_name": "Example Organization",
  "rationale": "Keeps browser work inside approved support sites.",
  "contacts": [
    {"kind": "email", "value": "security@example.com", "label": "Security team"}
  ]
}
```

Publish the next monotonic sequence and print a ready bootstrap:

```sh
ghostlight policy publish policy.json \
  --ed25519-seed policy-keys/policy-ed25519.seed \
  --mldsa-seed policy-keys/policy-mldsa65.seed \
  --source https://policy.example.com/ghostlight.bundle \
  --out ghostlight.bundle \
  --presentation presentation.json
```

Deploy the bundle to the named source. Provision the printed `managed.json` at:

- Windows: `%PROGRAMDATA%\Ghostlight\managed.json`
- Linux: `/etc/ghostlight/managed.json`

The strict bootstrap accepts `source`, `pubkey_ed25519`, optional `pubkey_mldsa`, optional
`bearer_token`, optional `ca_cert_pem`, and optional `poll_seconds`. Production sources are a
local file or HTTPS. Redirects are refused. HTTPS uses conditional ETag requests, capped retry
backoff, and bounded deterministic jitter.

Every bundle is verified before activation and again when read from cache. Lower sequences and a
different bundle reusing the same sequence are refused. A valid replacement applies to future
invocations. A malformed, unreachable, or unsigned update keeps the verified last-known-good
policy. A configured cold start without a valid source or cache fails closed. Signed policy does
not expire automatically; staleness remains visible without erasing protection.

The workbench Policy Passport shows organization, verification, sequence, freshness, source
class, last verification, rationale, and contact channels. The local managed-status sidecar holds
the same content-minimized operational facts without policy rules, source addresses, or
credentials.

## Denials and attention

An enforced denial carries a deterministic `D-` id, the deciding tier, grant, rule, complete RAWX
set, effective authority identity, mode, and managed sequence in audit. Three matching denials in
60 seconds, or five enforced denials in 120 seconds, pause that workspace for attention. Resume,
resume quietly, keep paused, and end session use the existing browser and workbench controls.

## Audit collection

Ghostlight appends one content-minimized JSONL record per terminal invocation. Set
`GHOSTLIGHT_AUDIT_FILE` to choose its absolute path; otherwise it sits beside runtime discovery.
Use the endpoint's existing file collector for SIEM delivery. Ghostlight does not upload audit or
open a direct syslog or HTTP delivery channel. See [`siem-integration.md`](siem-integration.md).

## Permanent ceilings

Policy never grants non-HTTP(S) schemes, localhost or its subdomains, loopback addresses, or
link-local addresses. Committed landings are checked again, so redirects cannot turn an allowed
request into access to a protected destination.

The extension's **Preserve controlled tabs** setting is an independent physical interlock. Both
orchestrator policy and that browser-local choice must allow model-driven close. Manual browser
closure always remains the user's action.
