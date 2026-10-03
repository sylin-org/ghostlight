# Configure Ghostlight 1.0 governance

Ghostlight needs no policy for personal use. With no policy configured, ordinary HTTP(S) browser
work is open, including localhost, loopback, and link-local destinations. Runtime controls,
explicit credential-input acknowledgement, stale-handle checks, browser-local interlocks, and the
HTTP(S)-only boundary still apply. Host restrictions belong to policy.

A policy can only narrow browser capability authority. Managed and local policy intersect; tool
calls do not author another policy layer. No lower layer can restore authority removed above it.
Browser attention is an operational preference: background by default, with an operator-selected
foreground opt-out and a mandatory organization ceiling.

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
    {"key": "browser.startup", "value": "manual", "level": "mandatory"},
    {"key": "browser.attention", "value": "background", "level": "mandatory"},
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
| `channels.mcp.enabled` | boolean | `false` refuses MCP session admission. |
| `channels.cli.enabled` | boolean | `false` refuses `ghostlight call` admission. |
| `audit.availability` | `keep_working` or `require_audit` | Defaults to continuing with visible history degradation. Require audit stops subsequent browser work during a known saving failure. Either layer can require it; observe mode does not relax it. |
| `browser.startup` | `on_demand` or `manual` | Controls whether admitted work may request one bounded browser-startup attempt. Windows defaults to `on_demand`; Linux defaults to `manual`. |
| `browser.attention` | `background` or `foreground` | Background is the default and protects the person's active tab, unowned pages, and placement from direct agent mechanisms. Foreground restores direct activation and unbound same-host adoption. |
| `content.security.sacred_domains` | hostname array | Adds never-touch destinations. |
| `policy.user.enabled` | boolean | `false` stops this machine's user from authoring a local policy. |

`policy.user.enabled` is honored only from an organization layer, and it gates authoring rather than
enforcement. A user policy that already exists keeps applying when it is switched off, because a
user layer can only subtract authority and ignoring it would hand authority back. It is not a
security control: a user layer could never widen anything. Use it when a fleet needs to stay
predictable, and supply an `organization.statement` so the person reads a reason rather than a
missing button.

`browser.startup` is an operational control, not a security boundary. With foreground attention,
`on_demand` permits one bounded recovery attempt when admitted work finds no connected browser;
background attention overrides launch with human handoff. `manual` returns a
diagnosis and leaves startup to the person. An organization-authored `manual` value is a ceiling a
user layer cannot relax.

`browser.attention` is authored by the person or organization, never by a model tool call.
A mandatory organization background value pins the choice. A recommended value is a default
that an explicit user value may override. User background can always tighten foreground.
The effective view states the choice and its deciding layer; it is persisted in the same user
policy file as other settings. No extension switch or separate attention service is added.

Background work does not adopt an unowned same-host tab, foreground a window, select another
tab in the person's focused or shared window, merge duplicate groups, or move existing tabs.
It opens in an unfocused window containing only owned tabs, creating one if needed. For trusted
native keyboard or pointer input, it may select an owned tab inside that unfocused work window.
Any native key/mouse surface in a focused or shared window returns `native_input` protection
before input, including an already active tab. Native work coordinates per window and rechecks current
target, placement, ownership, and focus before packets. Human focus, tab addition, or movement
interrupts that scope. If an effect may have landed, the result retains partial or unknown
effects and never replays input. Show tab permits review; return focus elsewhere or use a new
background tab and reobserve before further native work. Focused/shared-window resize and active
close in a focused window, with an unowned neighbor, or in a last-tab window are refused.
Ordinary admitted navigation in a controlled tab remains available. Attention is not a content
freeze; use Pause when taking over. If no browser is connected,
background recovery asks the person to open it even when `browser.startup` is `on_demand`.
An older adapter lacking the negotiated mechanism returns a visible refusal before effect.
That adapter receives local Ended cleanup and is disconnected until updated, without physical
tab closure or Stop on another adapter. Cleanup delivery does not assert acknowledged detachment.
Whole-value text edits do not select a tab. They can edit an inactive owned target in a shared
or focused window, but refuse the active target in a focused window. The same event fences and
input checks apply; cleanup may release already held input after takeover without replay.

Use Workbench `Show tab` to reveal an exact currently owned tab. Showing it grants no permission,
resumes no work, and repeats no action. Pause and Stop keep their existing meanings. This protects
Ghostlight's direct mechanisms, not focus or popups caused by explicitly run page code or page
actions. A new unfocused work window can still appear. See [ADR-0186](../adr/0186-quiet-browser-coexistence.md).

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

The same page authors the registered settings, grouped by what they are about -- where agents may
connect, in the browser, privacy. Boolean permissions start on. Turning one off is the only thing
its switch does: the permissive value is never written, because a user layer cannot hand authority
back. Browser startup and attention are closed choices instead of free-form fields. A control pinned
by an organization renders disabled and names who set it. `policy.user.enabled` stays
organization-only and is refused if a user document tries to author it. Sacred destinations are
edited as a list with the same plain-words readback host patterns get.

With no existing user policy, changing the first preference in an untouched empty draft shows
one all-sites grant preserving the current permissions under organization rules. You can save
the choice without writing grant boilerplate. Adding, editing, or removing rules makes that
draft explicit; the editor never puts a removed grant back for you.

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
set, effective authority identity, mode, and managed sequence in audit. Repeated refusals do not
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
`GHOSTLIGHT_AUDIT_FILE` to choose its absolute path; otherwise it sits beside runtime discovery.
Use the endpoint's existing file collector for SIEM delivery. Ghostlight does not upload audit or
open a direct syslog or HTTP delivery channel. See [`siem-integration.md`](siem-integration.md).

## Permanent ceilings

Policy never grants non-HTTP(S) schemes. Localhost, its subdomains, loopback, and link-local
addresses use the ordinary host rules. An exact host such as `localhost` or `127.0.0.1` can be
allowed or denied by a grant, and `content.security.sacred_domains` can mark a host never-touch
even in observe mode. No special local-access setting or exception is needed.

Committed landings are checked again before their content is accepted. These checks govern
browser operations and observed destinations; they do not filter every network request a page
makes or classify hostnames by their resolved IP address.

The extension's **Preserve controlled tabs** setting is an independent physical interlock. Both
orchestrator policy and that browser-local choice must allow model-driven close. Manual browser
closure always remains the user's action.

## When history cannot be saved

The H7 source supports `audit.availability` in the existing schema-3 `config` array:

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
