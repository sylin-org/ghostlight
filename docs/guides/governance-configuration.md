# Configuring governance

Ghostlight starts wide open. With no policy in place, your agent can use every tool on every site
you are logged into, and nothing is recorded. That default is deliberate: governance is an overlay
you reach for when you have a reason, not a tax you pay to get started.

This guide is the mechanics of that overlay: how a policy is shaped, why it is shaped that way, and
how to put one into force. For the ten-minute personal setup see [solo-developer.md](solo-developer.md);
for rolling a policy across an organization see [compliance-team.md](compliance-team.md). Both cite
this page for the how.

## Two things you configure

Governance has two moving parts, and they answer different questions:

- A **policy manifest** answers "who may do what, and where." It is a small JSON file of grants.
- The **layered configuration** answers "how does this instance behave" (audit on or off,
  redaction, sacred domains, the enforcement default). It is a set of dotted keys.

A manifest can carry configuration entries too, which is how an organization ships both at once. We
will get there. Start with capabilities, because every grant is written in their vocabulary.

## Capabilities: the vocabulary of a grant

Ghostlight sorts every action an agent can take into one of four capabilities. This is the part
worth slowing down on, because a policy never names tools. It names capabilities on hosts, and lets
the classification map back to tools for you.

- **`read`** is provable observation: a screenshot, a page read, a console or network dump. Nothing
  changes.
- **`action`** is UI input: a click, a keystroke, a drag. It gets its own class for a reason that
  is not obvious at first. A single click can trigger a purchase or a delete. The effect lives in
  the page, not in the keystroke, so Ghostlight cannot honestly file it under a harmless `read` or a
  predictable `write`. Granting `action` means trusting the agent with consequences the page
  decides, which is why you grant it deliberately.
- **`write`** is a declared mutation, like setting a form field: the intent is explicit and the
  effect is bounded.
- **`execute`** is arbitrary code in the page (`javascript_tool`). It is never implied by any of the
  others, because "run anything" is a different kind of trust from "click this button."

These are independent primitives, not a ladder. Granting `write` does not grant `action`, and
nothing grants `execute` but `execute`. When you need to see exactly which action maps to which
capability in a live session, ask the agent to call `explain`. It is the authoritative directory,
generated from the same registry enforcement uses, so it never drifts from reality.

The `computer` tool is worth a closer look, because it carries thirteen actions at two levels. Its
read-only actions (`screenshot`, `scroll`, `zoom`, `scroll_to`, `hover`) require `read`; its input
actions (`left_click`, `right_click`, `type`, `key`, `left_click_drag`, `double_click`,
`triple_click`) require `action`; and `wait` requires nothing. So a grant of `read` alone lets an
agent look at a page and scroll it but not click anything. That is often exactly the read-only
posture you want for a first policy.

## The manifest

A policy manifest is a small JSON file. Here is a complete one:

    {
      "schema": 3,
      "name": "support-crm",
      "version": "2026.07.1",
      "mode": "observe",
      "identity": { "resolved_by": "local_file", "principal": "support-team" },
      "grants": [
        {
          "id": "crm",
          "hosts": { "allow": ["*.crm.example.com"], "deny": ["admin.crm.example.com"] },
          "allowed": ["read", "action", "write"]
        },
        {
          "id": "docs",
          "hosts": { "allow": ["docs.example.com"] },
          "allowed": ["read"]
        }
      ],
      "config": [
        { "key": "audit.enabled", "value": true, "level": "mandatory" }
      ]
    }

The fields:

- **`schema`** must be `3`. The parser is strict and rejects any key it does not recognize, so a
  typo fails loudly at load rather than silently doing nothing. This is on purpose: a policy that
  quietly ignores a misspelled field is worse than one that refuses to load.
- **`name`** and **`version`** are free-form labels. `version` is not semver; it is whatever string
  helps you tell one revision from the next. It matters because it rides into every audit record, so
  a decision is always attributable to the exact policy that made it.
- **`mode`** is optional and covered below.
- **`identity`** is optional and descriptive. `resolved_by` and `principal` are free strings that
  say who this policy is about. They label the audit trail; they are not an authentication check.
  Ghostlight governs an already-authenticated browser session, so identity here is for evidence, not
  for login.
- **`grants`** is the heart of it, below.
- **`config`** carries configuration keys, covered under layering.

### Grants and host polarity

A grant says: on these hosts, these capabilities are allowed. The shape is an `id` (unique within
the file), a `hosts` block, and an `allowed` list of capabilities.

The host model is **default-deny**, and this is the single most important thing to internalize. A
host that no grant's `allow` list matches is not covered, and an uncovered host permits nothing. An
empty `allow` list covers nothing at all, whatever is in `deny`. You are not blocking sites; you are
opening the specific ones you name, and everything else stays closed. Write policies from that
direction and they will surprise you far less.

`deny` carves holes in what `allow` opened. "Everywhere on the CRM except the admin console" is
`allow: ["*.crm.example.com"], deny: ["admin.crm.example.com"]`, said directly.

Two properties of host patterns are worth understanding rather than memorizing:

- **Leading wildcard only, and it never swallows the apex.** `*.acme.com` matches `app.acme.com` and
  `mail.acme.com`, but never the bare `acme.com`. That is not a quirk to route around; it is a
  boundary. A wildcard that reached the apex would let a grant cover more than you wrote. If you mean
  the apex too, you name it: `["acme.com", "*.acme.com"]`.
- **The most specific pattern wins, and a tie goes to deny.** If `app.acme.com` is matched by an
  exact `deny` and a wildcard `allow`, the exact deny wins because it is more specific. When two
  patterns are equally specific, deny wins. Both readings that could keep you safe are the ones that
  hold.

You do not have to worry about the usual URL-smuggling tricks. Hosts are normalized before matching
(userinfo stripped, IP addresses canonicalized, international domains punycoded), so
`allowed.com@evil.com` and its cousins resolve to what they really are.

## Modes: observe before you enforce

There are exactly two enforcement modes, and the distance between them is where most of the value
lives.

- **`observe`** dispatches every call, then records the ones enforce *would* have blocked as a
  `shadow_deny` in the audit trail, carrying the same stable denial id they would get in production.
- **`enforce`** actually blocks. A denied call returns a message the agent can read and adapt to:
  the capability, the host, and a `D-xxxxxxxx` id that also lands in the audit record.

The reason `observe` exists is that a policy is a hypothesis about how your agents actually work,
and hypotheses are wrong in ways you cannot predict from a desk. Observe mode lets you run a real
policy against real usage and collect the list of everything it would have broken, with zero impact
on anyone. You tune until the shadow denials are the ones you meant, then flip to enforce. Skipping
this step is how a policy that looked right on paper takes down a team's Monday.

Mode resolves with a clear precedence, most specific first: a grant's own `mode`, then the
manifest's top-level `mode`, then the `governance.mode` configuration key. There is no command-line
flag or environment variable for mode; it lives in the policy and the config, where it is auditable.

## Sacred domains: the rule that ignores your mode

Some sites you never want touched, full stop, however the rest of your policy is tuned. That is the
sacred list:

    ghostlight config set content.security.sacred_domains '["*.mybank.com","brokerage.example"]'

Sacred domains are the one rule that ignores your mode setting. Everything else in governance is a
dial you can turn down to `observe` while you tune; this is not a dial. It rides a separate path at
the dispatch point, so "never touch my bank" holds even while the rest of your policy is still in
shadow, and even when there is no manifest at all. The sites you most want protected are exactly the
ones you do not want depending on a correctly-written grant. (Sacred patterns follow the same host
rules as grants, except the catch-all bare `*` is refused here: a sacred list that matched
everything would be a footgun, not a protection.)

## The layered configuration

Beyond the manifest, Ghostlight's behavior is a set of dotted keys (`audit.enabled`,
`content.security.secrets.redact`, `governance.mode`, and about a dozen more). You rarely touch most
of them, but understanding how they resolve matters the moment an organization is involved.

Values come from five layers. From highest precedence to lowest:

1. **`org_mandatory`**: organization keys marked `mandatory`. These lock.
2. **`user`**: what you set with `ghostlight config set`.
3. **`org_recommended`**: organization keys marked `recommended`. Defaults you may override.
4. **`preset`**: a named bundle (`fully_open`, `safe`, `restricted`).
5. **`builtin`**: the shipped defaults. Always complete, so resolution never fails.

The split around the `user` layer is the point. An organization can hand down two kinds of setting.
A `recommended` one sits *below* you: a sensible default you are free to change. A `mandatory` one
sits *above* you and cannot be overridden. `ghostlight config set` refuses a locked key and tells
you it is locked, rather than pretending to succeed. Locking only happens when the settings arrive
through the machine org-policy file, not when they ride in a manifest you supplied yourself, so you
cannot accidentally lock a key against yourself.

To see the whole resolved picture, `ghostlight config list` shows every key, its effective value,
which layer won, and whether it is locked. For the authoritative catalog of keys and their meanings,
run `ghostlight config docs` (generated from the binary, so it is never out of date); for the JSON
Schema of the user config file, `ghostlight config schema`.

## Putting a policy into force

Two ways, depending on who the policy is for.

**For yourself,** point the server at a file:

    GHOSTLIGHT_MANIFEST=file:///absolute/path/to/policy.json

Set that in the MCP server's environment. No manifest means all-open; removing the variable removes
all policy. That is the whole personal loop.

**For an organization,** place the manifest at the fixed machine path:

- Windows: `%ProgramData%\ghostlight\policy.json`
- macOS: `/Library/Application Support/ghostlight/policy.json`
- Linux: `/etc/ghostlight/policy.json`

This path cannot be moved by a flag, an environment variable, or a config key. That rigidity is the
feature, not a limitation: a policy an employee could relocate or point elsewhere is not a policy.
When an org policy is present it takes precedence, and any user-supplied manifest is ignored (that
displacement is itself recorded, as a `user_manifest_ignored` event, so it is visible rather than
silent).

For an organization that would rather distribute one signed policy to a whole fleet than place a
file on every machine, there is a third path, covered next.

## Central distribution: managed policy

The org-policy file above has to be present on each machine. For a fleet you can instead have every
endpoint pull one signed policy from a source you control, verify it locally, and keep enforcing the
last good copy when that source is unreachable. This is the managed policy path. It changes how a
policy travels, not what a policy says: the manifest inside is the same schema-3 document from the
rest of this guide.

### How trust works

Your organization signs its own policy with its own key. Ghostlight embeds no policy key of its own;
it verifies against the public key you provision. Authenticity therefore lives in the signature, not
in the transport, so the same signed bytes verify identically whether they arrive over HTTPS, from an
object store or file share, or on a USB stick carried into an air-gapped network.

You sign with the org authoring commands (they ship in every build):

    # print your public key(s) for the bootstrap
    ghostlight policy pubkey --seed org.seed

    # sign a manifest into a policy bundle at publish sequence 1
    ghostlight policy sign --seed org.seed --seq 1 policy.json

    # or do both at once and get a ready-to-paste bootstrap snippet
    ghostlight policy publish --seed org.seed --seq 1 policy.json

The seed is a 32-byte private signing seed you generate and guard (for example `openssl rand 32`).
For a production, post-quantum-ready key add a second `--mldsa-seed` file; the bundle then carries
two signatures and both must verify. `policy publish` is the one-command path: it signs the bundle
and prints the `managed.json` you hand your fleet.

### The bootstrap

Each machine reads an admin-only `managed.json` that sits beside the org-policy file, delivered by
your MDM exactly like the policy file:

- Windows: `%ProgramData%\ghostlight\managed.json`
- macOS: `/Library/Application Support/ghostlight/managed.json`
- Linux: `/etc/ghostlight/managed.json`

It names the source and the public key to trust:

    {
      "source": "https://policy.example.com/ghostlight.bundle",
      "pubkey_ed25519": "b3f1...",
      "poll_seconds": 300
    }

`source` is any location the fleet can reach: an HTTPS URL, an object store, a file share, or a
local or USB path for an air-gapped install. Optional fields tighten the fetch: `pubkey_mldsa` (the
second key for a composite bundle), `bearer_token` (sent as `Authorization: Bearer` on the request),
`ca_cert_pem` (pin the source's certificate authority), and `poll_seconds` (how often to re-check
for a new publish). Only the admin bootstrap can turn managed policy on; a user cannot self-activate
it through `GHOSTLIGHT_MANIFEST`.

### Continuity: last-known-good, never fail open

The verified bundle is cached to disk, and at boot Ghostlight loads and re-verifies that cache before
it ever touches the network. From there, two situations that would leave a lesser system unprotected
instead keep the last good policy:

- **The source is unreachable.** The cached policy keeps enforcing. Nothing falls open.
- **The source returns a bad bundle** (wrong signature, invalid schema). The bad update is rejected
  and the last valid policy stands.

Only one case refuses to run at all: a first boot with no cache and an unreachable source. Ghostlight
fails closed there rather than starting wide open on a policy it was told to have but cannot fetch.

A third protection is the reason to bump `--seq` on every release. The publish sequence is monotonic,
and a validly-signed bundle whose sequence is below the one already held is treated as a rollback and
refused, so a stale mirror or a replayed old bundle cannot quietly downgrade your fleet. The cache
stands. When `poll_seconds` is set the source is re-checked on that interval and a newer publish is
picked up live, with no restart.

### Seeing that it landed

`ghostlight doctor` reports the managed state on any machine, with no live agent session, so an
administrator can answer "did my policy reach this endpoint?" directly:

    Governance:
      mode  enforce (denied calls are blocked)
      managed  seq 7 (fresh), fetched 2026-07-11T09:14:02+00:00
      source   https://policy.example.com/ghostlight.bundle

When the cache is standing in for an unreachable or refused update, the managed line says so
(`last_known_good: source_unreachable`, `update_rejected`, or `rollback_refused`), which is exactly
the guardian moment you want visible rather than silent.

### What the governed session is told

Under managed governance the session speaks for the policy it carries. Ask the agent to call
`explain` and its answer gains a short Policy Passport: that managed governance is active, the policy
version and whether it is current or standing on last-known-good, a reminder that sacred domains stay
off-limits under any policy including this one, and, when the bundle names them, who governs the
session and how to reach them. A denial under managed governance can likewise carry one extra line
pointing the person to their organization's contact, so a blocked action becomes a door to a human
rather than a dead end. These org-voice details (a name, a rationale, a contact) are part of the
signed bundle and are held to strict display limits when it is verified, so an organization can add
its voice but never spoof or crowd out what Ghostlight has to tell the user.

## The authoring loop

You do not write a manifest from a blank page. The tooling gives you a loop:

1. **Start from a template.** `ghostlight policy init --template developer-unrestricted --out
   policy.json` writes an embedded example. The three templates are `enterprise-healthcare`,
   `developer-unrestricted`, and `qa-staging`; the [examples/](../../examples/) directory has more
   to copy.
2. **Read it back in plain language.** `ghostlight policy explain policy.json` renders exactly what
   the file permits and denies, in sentences, before anything runs. If the prose does not match your
   intent, the JSON will not either.
3. **Replay reality against it.** Have a pilot user work all-open with the audit recorder on, then
   `ghostlight policy simulate policy.json --replay audit.jsonl`. It reports what your draft would
   have allowed and denied, per grant, with no browser and no risk. Iterate until the denials are
   the ones you want.
4. **Observe, then enforce,** as above.

## When policies change: hot-reload

An in-force policy re-reads itself when the file changes, with no restart. The mechanism is
validate-then-swap: a candidate is fully parsed and resolved before it replaces the live one, so a
half-saved or invalid file never becomes active. A bad edit keeps the last-good policy rather than
falling open, which is the safe direction to fail. Startup is stricter still: a broken org policy
there refuses to boot, because booting open on a broken policy is precisely the failure you cannot
afford.

## Evidence

Every call, permitted or denied or shadow-denied, produces one JSON-Lines audit record: identity,
tool, capability, host, decision, grant id, denial id, duration, and the manifest hash in force.
Under managed policy each tool-call record additionally carries `policy_seq`, the org-signed publish
sequence in force, so a decision is attributable not only to a manifest hash but to the exact
published version your fleet was running. For streaming it to a SIEM, see
[siem-integration.md](siem-integration.md).

## Reference, not restated

The authoritative sources, all generated from the binary or published as specs, so this guide never
has to go stale repeating them:

- `ghostlight config docs`: every configuration key and its meaning.
- `ghostlight config schema`: JSON Schema for the user config file.
- [open-spec/](../../open-spec/): the RAWX capability model, vendor-neutral.
- [examples/](../../examples/): ready-to-adapt manifests.
