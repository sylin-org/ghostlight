# Configure Ghostlight governance

Ghostlight needs no policy for personal use. With no policy configured, ordinary HTTP(S) browser
work is open, including localhost, loopback, and link-local destinations. Runtime controls,
explicit credential-input acknowledgement, stale-handle checks, browser-local interlocks, and the
HTTP(S)-only boundary still apply. Host restrictions belong to policy.

A policy can only narrow that baseline. Managed policy and local policy intersect. No lower layer
can restore authority removed above it. A request cannot narrow its own authority:
`restrict_hosts` and `restrict_capabilities` are refused before dispatch, and a decision records no
caller restriction.

## Write a schema-4 policy

Read, Action, Write, and Execute are independent capabilities. A compound operation needs every
capability in its set; Execute does not imply the other three. The exact operation map lives in
[`../1.0/LANGUAGE.md`](../1.0/LANGUAGE.md).

```json
{
  "schema": 4,
  "name": "Support workspace",
  "version": "2026-08-14",
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
    {"key": "browser.startup", "value": "manual", "level": "mandatory"},
    {"key": "content.security.sacred_domains", "value": ["vault.example.com"], "level": "mandatory"}
  ]
}
```

The document is typo-closed. Unknown fields, settings, capabilities, and malformed host
patterns invalidate it. A configured source with no valid initial policy fails closed.

Host patterns are `*`, an exact hostname, or one leading suffix wildcard such as
`*.example.com`. Exact matches outrank longer suffix matches, which outrank `*`; an exact tie
denies. A grant's deny patterns shrink only that grant. Grants are checked in written order, and
the first grant admitting the complete capability set wins.

Every denial is enforced. Observe mode was removed in 2.0; `ghostlight policy simulate` replays a
candidate against recorded audit instead, and lists what it would have refused.

Schema 4 is schema 3 without `mode`, the channel switches (`channels.mcp.enabled`,
`channels.cli.enabled`), `policy.user.enabled`, and the frame options (`content.frames.handling`,
`content.frames.notice`). Pages with embedded documents behave one way: Ghostlight reads the
documents policy permits, leaves the others out, and reports what it left out. A file in the retired
schema 3 fails with `unsupported policy schema 3; run ghostlight policy migrate <file>`, and the
next section moves it.

Supported settings are:

Screenshot permission applies to the complete rendered image, including visible embedded content.
Frame host exclusions continue to restrict semantic text, targets, and actions; they do not redact
screenshots. Set `browser.screenshots.enabled` to `false` to prevent that visual disclosure.
An organization refusal cannot be overridden by a local allowance. Recording source restrictions
remain separate.

| Key | Value | Effect |
| --- | --- | --- |
| `browser.tabs.allow_close` | boolean | `false` removes model-driven close. |
| `browser.screenshots.enabled` | boolean | `false` refuses every screenshot; enabled by default. |
| `privacy.preserve_target_names` | boolean | `false` keeps page-authored target names out of results and audit. |
| `audit.availability` | `keep_working` or `require_audit` | Defaults to continuing with visible history degradation. Require audit stops subsequent browser work during a known saving failure. Either layer can require it. |
| `browser.startup` | `on_demand` or `manual` | Controls whether admitted work may request one bounded browser-startup attempt. Windows defaults to `on_demand`; Linux defaults to `manual`. |
| `content.security.sacred_domains` | hostname array | Adds never-touch destinations. |

`browser.startup` is an operational control, not a security boundary. `on_demand` permits one
bounded recovery attempt when admitted work finds no connected browser; `manual` returns a
diagnosis and leaves startup to the person. An organization-authored `manual` value is a ceiling a
user layer cannot relax.

## Move a schema-3 policy

```sh
ghostlight policy migrate policy.json
```

The command rewrites a schema-3 file as schema 4 only where the meaning stays the same. It drops
`mode` when it is `enforce`, a channel switch set to `true`, `policy.user.enabled` (a user layer can
only tighten, so dropping it never widens anything), and a frame option at its default
(`permitted_content` and `when_affected`). It copies the original to `policy.json.schema3.bak`,
writes the result in one step, and prints each part it removed.

It refuses anything else, because moving it would change what the policy does: `mode: "observe"`,
a channel set to `false`, a frame option set to anything but its default, and any content schema 4
would also refuse. The refusal prints one sentence per part and leaves the file untouched. Edit the
file to say what you mean in schema 4, then run the command again.

Ghostlight moves the user policy it owns the same way when it loads it, and keeps the same backup.
When that policy cannot move, the user layer stays invalid and Ghostlight fails closed until you
run the command. Files an administrator owns are never rewritten: the managed file, the signed
bundles, and a file named by `GHOSTLIGHT_POLICY_FILE` fail closed in schema 3 until their owner moves
them.

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

1. `GHOSTLIGHT_POLICY_FILE`, when set to the absolute path of a schema-4 JSON file before starting
   Ghostlight. Ghostlight reads that file and never writes to it. The workbench shows it read-only
   and says why.
2. Otherwise the file Ghostlight owns, beside the managed cache in the policy directory of the
   Ghostlight home: `~/.ghostlight/policy/user-policy.json` on both systems. This is the file the
   workbench writes, and it is optional: a machine that has never authored one is all-open, not
   failing closed.

Valid replacements apply atomically to future invocations. A malformed replacement keeps the last
valid policy; a malformed cold start fails closed.

## Read and write policy in the workbench

The workbench has a Policy destination, reached from the tab row or from the state chip beside it.
It shows the compiled result rather than the configuration: what agents may do right now, which
layer decided each line, the rules behind those lines, the boundaries no policy can lift, and the
exact document and path for every layer in force.

When Ghostlight owns the user file, the same page edits it. Rules read as sentences, host patterns are read back in plain words as they are typed,
capabilities an organization refuses are shown refused on the control itself, and a rule that can
never fire says so in place. Before applying, the page replays the candidate through the production
decision engine against this machine's recorded audit and reports what would have been refused.

The same page authors the registered settings, grouped by what they are about -- history, in the
browser, privacy. Boolean permissions start on. Turning one off is the only thing
its switch does: the permissive value is never written, because a user layer cannot hand authority
back. Browser startup is a closed two-choice select instead of a free-form field. A control pinned
by an organization renders disabled and names who set it. Sacred destinations are
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

## Denials and explicit user control

An enforced denial carries a deterministic `D-` id, the deciding tier, grant, rule, complete RAWX
set, effective authority identity, and managed sequence in audit. Repeated refusals do not
pause the workspace or create a review/resume requirement. Each subsequent request follows the
same configured authority and explicit human controls.

Fill and typing accept `user_authorized_credentials`, false by default. Set it only when the user
explicitly instructed credential entry; existing conversation authorization is sufficient. It is
a per-request caller acknowledgement, not independently verified proof or a policy grant. Without
it, credential detection returns guidance for that request and creates no session hold. With it,
configured policy and human Pause/Stop still apply. Browser feedback contains no buttons or
input-intercepting overlays; interactive runtime control belongs in the workbench.

## Audit collection

Ghostlight appends one content-minimized JSONL record per terminal invocation. Set
`GHOSTLIGHT_AUDIT_FILE` to choose its absolute path; otherwise it is `~/.ghostlight/state/audit.jsonl`.
Use the endpoint's existing file collector for SIEM delivery. Ghostlight does not upload audit or
open a direct syslog or HTTP delivery channel. See [`siem-integration.md`](siem-integration.md).

## Permanent ceilings

Policy never grants non-HTTP(S) schemes. Localhost, its subdomains, loopback, and link-local
addresses use the ordinary host rules. An exact host such as `localhost` or `127.0.0.1` can be
allowed or denied by a grant, and `content.security.sacred_domains` can mark a host never-touch.
No special local-access setting or exception is needed.

Committed landings are checked again before their content is accepted. These checks govern
browser operations and observed destinations; they do not filter every network request a page
makes or classify hostnames by their resolved IP address.

The extension's **Preserve controlled tabs** setting is an independent physical interlock. Both
orchestrator policy and that browser-local choice must allow model-driven close. Manual browser
closure always remains the user's action.

## When history cannot be saved

The H7 source supports `audit.availability` in the schema-4 `config` array:

```json
{"key":"audit.availability","value":"require_audit","level":"mandatory"}
```

Keep working is the default. Require audit refuses subsequent browser work during a known
storage failure. It preserves previous effects and keeps policy explanation, diagnostics, and
human controls available. The Policy destination offers both choices; an organization requirement
cannot be relaxed by the user layer. Each invocation retains its original policy snapshot.

Ghostlight checks a failed destination automatically at most once per five seconds. Recovery
allows new requests; it never repeats browser actions or backfills missing receipts. At a glance
and Status show saving health and explicit gaps. See [H7 verification](../tasks/security-hardening/h7-audit-health.md)
for source, installed-version, and platform evidence before relying on this setting in a fleet.
