# Ghostlight governance language

Status: Working product primer

Related browser language: [Ghostlight ubiquitous language](ubiquitous-language.md)

Short version: [Ghostlight governance language summary](governance-language-summary.md)

This document defines how Ghostlight talks about limits, settings, user control, decisions,
evidence, and recovery. It is the governance companion to the browser ubiquitous-language
primer. The browser primer starts with useful browser work. This primer starts with the human
intent behind a limit.

The goal is not to make governance less strict. The goal is to make strict behavior easy to
understand, configure, test, and recover from. A person should not need to understand internal
ports, policy tiers, browser mechanisms, or vendor tool names to answer these questions:

1. What browser work is allowed?
2. Which sites does that answer apply to?
3. Is Ghostlight observing or blocking?
4. Why did this call stop?
5. What is the safest useful next move?

This is a working design source for the rebuilt action pipeline. Accepted ADRs and live code remain
production authority until a new ADR or marked amendment accepts the definitions here. Existing
policy and settings formats remain compatibility inputs until a deliberate migration replaces
them.

## The short version

Ghostlight keeps five concerns separate:

- A **policy** limits browser operations by host and capability.
- **Settings** control instance behavior such as audit output, privacy, and protected hosts.
- **Runtime safety controls** let a user pause or end work and let Ghostlight stop a confused
  client loop.
- A **canonical decision** records what governance decided without model-specific prose.
- The **Ghostlight renderer** explains that decision through Ghostlight's one exact model-facing
  result contract.

No policy means policy does not restrict browser operations. Protected hosts, ownership, user
safety controls, request restrictions, and browser availability still apply. Auditing and privacy
also have their own settings and do not disappear with policy.

Governance is automatic. A model does not need to call a policy tool before ordinary browser work.

## Choose the right governance path

| What you want | Use | Do not use |
| --- | --- | --- |
| Ordinary unrestricted personal browser work | No policy | An "unrestricted" policy file |
| Keep Ghostlight away from personal sites under every policy mode | Protected hosts setting | A policy rule in observe mode |
| Limit your own browser work to named sites and capabilities | Personal policy | Organization settings levels |
| Require one policy on a managed machine | Fixed organization policy | A relocatable user policy path |
| Distribute one signed policy to a fleet | Managed organization policy | A Ghostlight-hosted control plane |
| Suggest organization defaults that a person may change | Organization default settings | Required settings |
| Lock an organization setting | Organization required settings | A personal policy field |
| Narrow one request further than the service policy | Request restriction | A second competing service policy |
| Pause work because the person took control | User hold | A policy denial |
| Stop one confused client after repeated denials | Attention pause | A global hold or a new policy rule |
| Understand what is currently effective | Effective-policy explanation | An authored-file explanation alone |
| Test a draft without blocking work | Observe, explain, and simulate | Deploy-and-guess enforcement |

## Product objective

Governance delight means:

- all-open remains a first-class zero-configuration posture;
- a policy author starts from one small, truthful example;
- one concept has one name and one preferred authoring location;
- omitted values receive defaults only when omission cannot weaken the author's stated intent;
- a typo or contradiction fails once with a precise correction;
- the effective policy is easier to inspect than the raw source layers;
- observe mode is visibly not protection;
- a blocked model receives a concise reason and a safe recovery, not an internal stack trace;
- the user can always distinguish policy, protected-host safety, takeover, attention, browser
  availability, and an uncertain browser effect;
- audit records governance and execution as separate facts; and
- every MCP client receives the same concise Ghostlight outcome without changing the underlying
  decision.

Delight never means:

- silently permitting a typo;
- guessing a host, capability, user decision, or policy mode;
- weakening a protected-host rule;
- converting a block into a transport error or a success;
- inviting a workaround around policy;
- replaying an uncertain browser effect; or
- moving policy logic into the extension or a model-specific adapter.

## Concern boundaries

### Policy

A policy answers: **Which capabilities may run on which hosts?**

It contains ordered rules and one explicit default enforcement posture. A personal policy does not
also act as a general settings file. Organization distribution may package policy and settings
together, but they remain distinct normalized sections.

### Settings

Settings answer: **How should this Ghostlight instance behave?**

Audit output, privacy transformations, protected hosts, runtime budgets, and component enablement
are settings. They are grouped by their actual concern. They do not all become "governance" merely
because the same resolver loads them.

### Runtime safety controls

Runtime safety answers: **Should work enter the browser right now?**

A user hold, end-session control, attention pause, browser disconnect, cancellation, and final
admission check are not policy decisions. They remain visible as their own outcome classes.

### Canonical decision

The service decides from canonical operation identity, required capabilities, a normalized
resource, one immutable authority snapshot, and any tighten-only request restriction. The decision
contains typed facts. It does not contain an MCP lifecycle field or browser mechanism name.

### Ghostlight rendering

The Ghostlight renderer owns the external response. It translates typed operation facts into the one
Ghostlight result schema, concise content blocks, and schema-valid suggested calls. MCP revisions may
wrap that result differently, but they cannot alter the canonical decision trace or browser-effect
truth.

### Browser mechanism

The extension receives policy-free browser mechanisms and bounded presentation state. It may show
a decision already made by the service. It never evaluates policy, chooses a recovery, counts
policy rules, or grants authority.

## Ubiquitous terms

These are the preferred canonical terms. Historical policy and settings parsers may accept old
spellings, but new code and product copy use one meaning consistently.

| Term | Meaning |
| --- | --- |
| Policy | The one active service ruleset that limits browser work by host and capability. |
| Rule | One ordered host-coverage entry in a policy. Historical formats call it a grant. |
| Capability | One independent proof class required by a canonical operation: read, interact, write, or execute. |
| Read | Ghostlight can prove the operation only observes or retrieves. |
| Interact | Ghostlight sends user-interface input whose downstream effect is decided by the page. Historical policy files call this `action`. |
| Write | The operation declares that its purpose is to change data or state, such as setting a form value. Downstream consequences may still be page-defined. |
| Execute | The operation runs arbitrary code. No other capability implies it. |
| Host | A normalized site host such as `app.example.com`. Policy never matches a full URL string. |
| Protected host | A user or organization never-touch host. It is blocked under every policy mode, including with no policy. Historical copy calls this a sacred domain. |
| Enforcement | Whether a policy blocks (`enforce`) or only records what it would block (`observe`). |
| Allowed | Policy permits the operation. It says nothing yet about browser availability or execution success. |
| Would block | Observe mode found the same policy reason that enforce mode would block, but execution may continue. Historical audit calls this `shadow_deny`. |
| Blocked | A governance decision refused dispatch or refused a committed navigation landing. In the operation result, a pre-dispatch refusal has status `blocked`; a refused committed landing has status `partial`. Effect and decision phase say which occurred. |
| Decision id | A stable bounded identifier that correlates a block or would-block outcome with audit evidence. |
| Policy source | The one authority source from which the active service policy was selected. |
| Setting source | The layer that supplied one effective setting value. This is separate from policy selection. |
| Required setting | An organization value the user cannot override. Historical formats call this mandatory. |
| Default setting | An organization suggestion the user may override. Historical formats call this recommended. |
| Request restriction | An immutable per-request constraint that may narrow service authority and can never widen it. |
| Authority snapshot | The immutable current policy and settings snapshot plus its epoch. It does not freeze live tab ownership in place of final admission. |
| Work context | One selected operation's authority snapshot plus its immutable request restriction, scheduling lease, and correlation facts. |
| User hold | A global person-controlled take-the-wheel pause. It is not a denial. |
| Attention pause | A session-scoped pause after a bounded burst of enforced denials. It is not a policy rule and does not affect other sessions. |
| End session | A person-controlled stop that invalidates the session. Historical code may call it panic or kill. |
| Effective policy | The selected and normalized rules, enforcement, organization voice, and relevant effective settings actually in force. |
| Authored policy | One source file as written. It may not describe the full live result after source selection and settings resolution. |
| Last-known-good | The most recent valid managed policy that remains in force when a refresh is unavailable, invalid, or a rollback. |

### Words to avoid in new product language

| Avoid | Prefer | Why |
| --- | --- | --- |
| manifest | policy | Manifest is a compatibility file term, not the user's job. |
| grant | rule | A rule is easier to read as ordered host coverage plus capabilities. |
| sacred domain | protected host | The behavior is a never-touch safety boundary over normalized hosts. |
| shadow deny | would block | It states the observe-mode outcome directly. |
| action capability | interact capability | It distinguishes uncertain UI effects from declared writes. |
| mutate mode | enforce | Enforcement posture and operation effect are different axes. |
| identity block | audit labels | Authored labels are not verified identity or authority. |
| org mandatory | organization required | It says both the owner and override behavior. |
| org recommended | organization default | It says the user may override it. |
| kill switch | end session | Use kill only in internal historical references. |

## Canonical policy language

### Shortest complete policy

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

This policy says:

- observe what this rule would block before enforcing it;
- cover subdomains of `crm.example.com` except the admin host; and
- permit observation, UI interaction, and declared data writes on the first matching rule.

It does not enable arbitrary code. It does not globally block `admin.crm.example.com`; this rule
simply does not cover that host. A later rule may cover it. Put a host in protected settings when
no rule or mode may ever touch it.

### Policy fields

| Field | Required | Meaning |
| --- | --- | --- |
| `schema` | yes | Canonical policy format version. |
| `name` | yes | Short human policy name. |
| `revision` | yes | Human release label stamped into effective-policy and audit views. It need not be semver. |
| `enforcement` | yes | `observe` or `enforce`. This intent is never guessed. |
| `rules` | yes | Ordered rules. An empty list is an explicit block-all policy for capability-bearing work under enforce. |
| `rules[].id` | yes | Unique bounded audit and explanation label. |
| `rules[].hosts` | yes | Non-empty host patterns this rule may cover. |
| `rules[].except_hosts` | no | Local carve-outs from this rule. Default: empty. |
| `rules[].capabilities` | yes | Non-empty independent capability set. |
| `rules[].description` | no | Short administrator rationale. It does not affect decisions. |

### Rule evaluation

Rules are evaluated in authored order.

1. Normalize the governing host before policy matching.
2. Find the first rule whose `hosts` includes the host and whose `except_hosts` does not exclude
   it.
3. Check whether that one rule contains every required capability.
4. If it does, allow. If it does not, block or would-block under that rule.
5. If no rule covers the host, block or would-block because the host is not covered.

`except_hosts` is local to one rule. It does not create a global site block. This distinction is
shown in every explanation and linted when later rules re-open an excluded host.

An empty enforce policy blocks every capability-bearing operation. An empty observe policy records
that every capability-bearing operation would block, then allows dispatch. Capability-free
operations remain not applicable to policy in either posture.

Capabilities are independent, not a ladder. `write` does not imply `interact`; a button click can
cause a page-defined purchase or deletion even though Ghostlight did not declare a write. Nothing
but `execute` permits arbitrary code.

Operations that require no governed capability do not need a rule. Policy allowance is still not
proof that the browser is connected, the tab is owned, a user hold is clear, or execution
succeeded.

### Host patterns and matching

Authority-sensitive host behavior is exact:

- `example.com` is an exact host pattern.
- `*.example.com` matches one or more subdomain labels, including
  `a.b.example.com`. It never matches the apex `example.com`.
- `*` is the catch-all token. It is accepted only in policy rule `hosts` and
  `except_hosts`. It is rejected in protected-host settings.
- A wildcard never matches an IPv4 or IPv6 literal.
- Exact IPv4 and IPv6 literals are accepted and stored in canonical form. IPv6 patterns use the
  bare RFC 5952 form without URL brackets; brackets belong only to URL syntax.
- `localhost` is accepted as an exact host.
- Schemes, ports, paths, queries, fragments, userinfo, embedded whitespace, empty labels, and a
  wildcard anywhere except one leading `*.` are invalid patterns.
- DNS labels are lowercase ASCII, 1 to 63 bytes, and cannot begin or end with `-`. International
  names are authored in their IDNA A-label form. The complete canonical host is at most 253
  bytes.
- Duplicate patterns are rejected after canonicalization. Overlapping patterns are allowed but
  explained and linted.

The governing URL is parsed with one WHATWG-compatible URL parser before matching. Only HTTP and
HTTPS produce an ordinary governed host. The parser removes userinfo before selecting the real
host, lowercases names, converts international names to A-labels, canonicalizes IPv4 and IPv6,
ignores the port for host matching, and removes at most one trailing dot. An invalid URL,
unsupported scheme, absent host, or ambiguous normalization fails closed whenever policy or an
always-on boundary needs a host.

Within one rule, Ghostlight finds the most specific matching `hosts` pattern and the most specific
matching `except_hosts` pattern. Exact beats wildcard, a longer wildcard suffix beats a shorter
one, and `*` is least specific. The more specific side wins; an equal-specificity tie goes to
`except_hosts`. A rule with no matching `hosts` pattern does not cover the host. Rules themselves
are then considered in authored order, and the first covering rule resolves the policy decision.

### Exact working policy schema

The JSON Schema below is the intended canonical authoring shape. Host-pattern grammar, duplicate
checks, normalization, and cross-rule lint remain semantic validation so the schema stays flat.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://ghostlight.dev/schemas/governance-policy-v1.json",
  "title": "Ghostlight governance policy",
  "type": "object",
  "properties": {
    "schema": {"type": "integer", "const": 1},
    "name": {"type": "string", "minLength": 1, "maxLength": 128},
    "revision": {"type": "string", "minLength": 1, "maxLength": 128},
    "enforcement": {
      "type": "string",
      "enum": ["observe", "enforce"]
    },
    "rules": {
      "type": "array",
      "maxItems": 256,
      "items": {"$ref": "#/$defs/rule"}
    }
  },
  "required": ["schema", "name", "revision", "enforcement", "rules"],
  "additionalProperties": false,
  "$defs": {
    "host_pattern": {
      "type": "string",
      "minLength": 1,
      "maxLength": 253,
      "description": "Canonical exact host, one leading-wildcard host, or the policy-only * catch-all token."
    },
    "capability": {
      "type": "string",
      "enum": ["read", "interact", "write", "execute"]
    },
    "rule": {
      "type": "object",
      "properties": {
        "id": {
          "type": "string",
          "minLength": 1,
          "maxLength": 64,
          "pattern": "^[a-z][a-z0-9_-]{0,63}$"
        },
        "hosts": {
          "type": "array",
          "minItems": 1,
          "maxItems": 128,
          "items": {"$ref": "#/$defs/host_pattern"}
        },
        "except_hosts": {
          "type": "array",
          "maxItems": 128,
          "items": {"$ref": "#/$defs/host_pattern"},
          "default": []
        },
        "capabilities": {
          "type": "array",
          "minItems": 1,
          "maxItems": 4,
          "items": {"$ref": "#/$defs/capability"}
        },
        "description": {
          "type": "string",
          "minLength": 1,
          "maxLength": 240
        }
      },
      "required": ["id", "hosts", "capabilities"],
      "additionalProperties": false
    }
  }
}
```

## Protected hosts

Protected hosts are an always-on safety ceiling, not a policy rule.

- They apply with no policy.
- They apply in observe mode.
- They apply to a current page, a navigation target, and every committed navigation landing.
- A request restriction may add protected hosts but cannot remove them.
- An organization may add protected hosts but cannot remove the user's list.
- Lists from independent authority sources are unioned after normalization. Ordinary setting
  precedence must never replace a stricter list with a narrower one.
- The normalized effective union contains at most 512 unique patterns. Startup overflow fails
  loud. Reload overflow keeps the last-known-good settings snapshot. A request restriction that
  would exceed the bound is rejected before browser dispatch.
- A bare catch-all is rejected because it would make Ghostlight unusable through a settings
  accident. A deliberate block-all policy remains possible and visible through policy.

Canonical setting name: `safety.protected_hosts`.

Historical `content.security.sacred_domains` inputs normalize into this list. The current runtime
does not yet union every settings layer correctly; that is a safety root fix required before this
language can become accepted production authority.

## Settings language

The canonical settings surface is grouped by purpose. It does not call engine timing, privacy,
browser availability, audit routing, and policy mode one undifferentiated governance registry.

### Governance and safety settings

| Setting | Type | Product default | Meaning |
| --- | --- | --- | --- |
| `safety.protected_hosts` | list of host patterns | `[]` | Always-on never-touch hosts; effective value is the union of every authorized source. |
| `privacy.redact_sensitive_fields` | boolean | `true` | Remove structurally sensitive field values from model-facing observations. |
| `audit.output` | `off`, `file`, `stderr`, or `syslog` | `file` | The one switch for audit recording and destination. |
| `audit.file.path` | absolute path or empty | empty | File destination; empty uses the platform default. Relevant only when output is `file`. |
| `audit.syslog.address` | host and port | `127.0.0.1:514` | Syslog destination. Relevant only when output is `syslog`. |
| `runtime.browser_connect_timeout_ms` | integer from 0 to 60000 | `5000` | Maximum wait for the browser adapter on the first browser call. Zero means do not wait. |
| `runtime.sequence_max_duration_ms` | integer from 1000 to 480000 | `120000` | Service ceiling for one canonical browser sequence. Models do not need to supply it. |
| `service.local_bridge_enabled` | boolean | `true` | Whether Ghostlight accepts owner-only local service connections. |
| `service.management_ui_enabled` | boolean | `true` | Whether the local management UI is available. It does not enable or disable browser work. |

Policy enforcement is authored once in policy. Canonical v1 has no rule override and no second
settings location for it. An import adapter may preserve a historical per-rule or fallback mode as
an internal compatibility fact, but new authors do not see that branch.

Runtime budgets belong under `runtime.*`. Component switches belong under `service.*`. They
remain settings, not policy vocabulary. Canonical v1 exposes only settings with a live reader;
reserved or inert compatibility keys are rejected rather than carried forward as false controls.

### Exact working settings schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://ghostlight.dev/schemas/governance-settings-v1.json",
  "title": "Ghostlight instance settings",
  "type": "object",
  "properties": {
    "safety.protected_hosts": {
      "type": "array",
      "maxItems": 256,
      "items": {"type": "string", "minLength": 1, "maxLength": 253},
      "default": []
    },
    "privacy.redact_sensitive_fields": {
      "type": "boolean",
      "default": true
    },
    "audit.output": {
      "type": "string",
      "enum": ["off", "file", "stderr", "syslog"],
      "default": "file"
    },
    "audit.file.path": {
      "type": "string",
      "default": ""
    },
    "audit.syslog.address": {
      "type": "string",
      "minLength": 1,
      "maxLength": 512,
      "default": "127.0.0.1:514"
    },
    "runtime.browser_connect_timeout_ms": {
      "type": "integer",
      "minimum": 0,
      "maximum": 60000,
      "default": 5000
    },
    "runtime.sequence_max_duration_ms": {
      "type": "integer",
      "minimum": 1000,
      "maximum": 480000,
      "default": 120000
    },
    "service.local_bridge_enabled": {
      "type": "boolean",
      "default": true
    },
    "service.management_ui_enabled": {
      "type": "boolean",
      "default": true
    }
  },
  "additionalProperties": false
}
```

Destination-specific settings are checked semantically. A UI or generated form hides irrelevant
fields. A file may contain them, but the effective explanation states which value is active.

### Setting layers

Most settings resolve from highest precedence to lowest:

1. managed-organization required;
2. fixed machine-organization required;
3. the user's setting;
4. managed-organization default;
5. fixed machine-organization default; and
6. product default.

A managed package and a fixed machine package may both contribute setting layers even when policy
comes from only one of them or from a lower source. Cross-package duplicates resolve by the order
above. Within one package, placing the same key in both required and default maps is invalid. The
effective view names both the layer and organization source, so a value never appears merely as
the vague label `organization`.

The effective view always shows the value, source, and whether it is locked.

`safety.protected_hosts` is the deliberate exception: authorized lists accumulate by union. A
higher source may add protection but cannot erase protection from a lower source.

The canonical format has no settings presets until two profiles produce meaningfully different,
truthful results. A name such as `fully_open` must never imply that settings can remove an active
policy. A name such as `restricted` must never duplicate `safe`.

## Policy source selection

Policy selection and setting resolution are different operations. Do not present them as one tier
stack.

The current service selects at most one service policy in this order:

1. an admin-provisioned managed-policy bootstrap selects a verified managed policy;
2. otherwise, the fixed machine organization policy;
3. otherwise, an explicit user source supplied by `--manifest`;
4. otherwise, the user source string in `GHOSTLIGHT_MANIFEST`; and
5. otherwise, no policy, which means all governed operations are permitted.

Canonical policy selection tests for a present `policy`, not merely for a present organization
package. A settings-only managed or fixed organization package contributes its setting layers,
then policy selection continues at the next eligible source. A present valid policy selects the
slot and stops fallback. This includes a policy with `rules: []`, which is an intentional
block-all policy under enforce. Omitting `policy` never silently means block all and never hides a
lower policy source.

An optional request restriction is intersected after service policy selection. It may only narrow
the result. It is not a second service policy and cannot grant an operation the service policy did
not allow.

Both user-source locations use the same compatibility grammar. A bare string is a filesystem
path. `file://<path>` is an explicit filesystem path. `env://NAME` means the named environment
variable contains policy JSON. `managed://` is never user-activatable, and every other URI scheme
is rejected. `--manifest` wins over `GHOSTLIGHT_MANIFEST` when both are present.

### Request restrictions

A request restriction is always enforce-only and tighten-only. It has no observe mode. A
historical session-scoped overlay is captured immutably beside the current authority snapshot for
each selected operation; storing its source at session scope does not turn it into a second live
policy or let it widen later work.

Combination order is exact:

1. A protected-host block wins first.
2. An enforced service-policy block wins next.
3. If service policy allows dispatch or only would-block under observe, an enforced request
   restriction may still block.
4. If the restriction allows and service policy would-block, the final model-visible governance
   outcome is would-block from policy.
5. Otherwise the final outcome is allowed or not applicable.

When a request restriction blocks, `source` and `reason` are `request_restriction`, it receives its
own `decision_id`, content-derived `restriction_id`, and the matched `restriction_rule_id` when one
rule resolved. It never borrows a policy `rule_id`. If an observe policy would also have blocked,
audit retains that policy observation as a secondary would-block fact while the model-facing final
outcome reports the enforced restriction block. A restriction that is invalid, stale,
unclassifiable, or too large is rejected before browser dispatch.

The canonical request-restriction document is deliberately smaller than a policy. It has no
name, revision, enforcement switch, settings, or presentation identity:

```json
{
  "schema": 1,
  "rules": [
    {
      "id": "crm-read-only",
      "hosts": ["*.crm.example.com"],
      "capabilities": ["read"]
    }
  ],
  "protected_hosts": ["admin.crm.example.com"]
}
```

Rules use the canonical policy rule vocabulary and matching semantics. They are always enforced.
An empty `rules` array blocks every capability-bearing operation that reaches the restriction.
`protected_hosts` is optional and joins the always-on protected-host union; it never permits a
host. A document may contain at most 64 rules and 128 protected hosts. Rule ids are unique after
validation.

Normalization canonicalizes hosts, rejects duplicates, sorts host sets, materializes absent
`except_hosts` and `protected_hosts` as empty arrays, orders capability sets as `read`,
`interact`, `write`, `execute`, and preserves rule order. Ghostlight then serializes the normalized
document with RFC 8785 JSON Canonicalization Scheme and computes SHA-256. The full audit identity
is `sha256:<64 lowercase hex digits>`. The bounded result correlation id is `R-` followed by the
first 32 hex digits. This derived id is `restriction_id`; it is never accepted from the caller.
Both values are stored once in the immutable work context. The audit row carries them in its
top-level `request_restriction` axis. A blocking canonical decision and model-facing result carry
the bounded `restriction_id`, plus `restriction_rule_id` when one rule resolved; they do not repeat
the full digest.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://ghostlight.dev/schemas/governance-request-restriction-v1.json",
  "title": "Ghostlight request restriction",
  "type": "object",
  "properties": {
    "schema": {"type": "integer", "const": 1},
    "rules": {
      "type": "array",
      "maxItems": 64,
      "items": {
        "$ref": "https://ghostlight.dev/schemas/governance-policy-v1.json#/$defs/rule"
      }
    },
    "protected_hosts": {
      "type": "array",
      "maxItems": 128,
      "items": {"type": "string", "minLength": 1, "maxLength": 253},
      "default": []
    }
  },
  "required": ["schema", "rules"],
  "additionalProperties": false
}
```

Schema-3 session overlays normalize one way into this document. Grants become rules;
`hosts.allow` becomes `hosts`; `hosts.deny` becomes rule-local `except_hosts`; capability
`action` becomes `interact`; and overlay sacred domains become `protected_hosts`. Top-level and
grant modes, name, version, authored identity, and config entries other than sacred domains are
discarded with a compatibility warning because they never narrow canonical authority. A
caller-supplied decision id, restriction id, or content digest is rejected as an identity-spoofing
attempt. The restriction is then validated under the canonical bounds. The normalized document,
not the historical bytes, supplies the content identity and `restriction_id`.

### Managed organization policy

Managed distribution changes how an organization policy arrives, not what a policy means.

- An administrator provisions the source and trusted public keys.
- The organization signs the policy package.
- Ghostlight verifies locally and rejects rollback.
- The last-known-good verified policy remains active when refresh is unavailable or invalid.
- First boot with required managed policy but no valid policy fails closed.
- No managed-policy failure may fall back to all-open.
- The effective view shows organization name, policy revision, publish sequence, freshness,
  last-known-good reason, and bounded human contact when supplied.

An organization package may carry three normalized sections:

- `policy`;
- `required_settings`; and
- `default_settings`.

Within one organization package, the same setting key cannot appear in both maps. That is a
validation error rather than an implicit precedence choice. A managed and fixed package may name
the same key; the canonical cross-package order in Setting layers resolves it. Protected hosts
from every map still join the effective union with user and request sources after each map
validates. Organization presentation fields are bounded context for humans; they are never
authority or model-authored instructions.

`policy` is optional so an organization may require or suggest settings while leaving browser
authority to lower policy-source selection. If no lower source contains policy, authority is
all-open. At least one of `policy`, `required_settings`, or `default_settings` must be present. The
two settings maps are partial layers: JSON Schema `default` annotations from the referenced
settings vocabulary are never materialized into an organization layer. Product defaults enter
only at final setting resolution.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://ghostlight.dev/schemas/governance-organization-package-v1.json",
  "title": "Ghostlight organization governance package",
  "type": "object",
  "properties": {
    "schema": {"type": "integer", "const": 1},
    "policy": {
      "$ref": "https://ghostlight.dev/schemas/governance-policy-v1.json"
    },
    "required_settings": {
      "$ref": "https://ghostlight.dev/schemas/governance-settings-v1.json"
    },
    "default_settings": {
      "$ref": "https://ghostlight.dev/schemas/governance-settings-v1.json"
    },
    "organization": {
      "type": "object",
      "properties": {
        "name": {"type": "string", "minLength": 1, "maxLength": 120},
        "rationale": {"type": "string", "minLength": 1, "maxLength": 500},
        "contact": {"type": "string", "minLength": 1, "maxLength": 240}
      },
      "required": ["name"],
      "additionalProperties": false
    }
  },
  "required": ["schema"],
  "anyOf": [
    {"required": ["policy"]},
    {"required": ["required_settings"]},
    {"required": ["default_settings"]}
  ],
  "additionalProperties": false
}
```

Cryptographic envelope, source transport, and cache metadata remain outside the policy language.

## Canonical decision seam

The rebuilt action pipeline should make three types explicit.

### Governance snapshot

One immutable snapshot contains:

- selected policy source and identity;
- normalized ordered rules;
- policy enforcement;
- effective relevant settings and their sources;
- organization presentation facts;
- the authority epoch needed for audit and final admission.

It does not contain a model profile, external tool name, MCP lifecycle state, page payload, or
browser mechanism. The immutable request restriction is carried beside this snapshot in the work
context. Live tab ownership is rechecked at final admission rather than frozen into authority.

### Governance request

| Field | Meaning |
| --- | --- |
| `operation` | Canonical operation family. |
| `intent` | Canonical concrete operation intent. |
| `required_capabilities` | Complete independent capability set from the canonical descriptor. |
| `resource` | Normalized governed resource: host, exempt resource, no resource, out of scope, or indeterminate. |
| `subject` | Bound local authority facts required by the selected policy, never self-authored presentation labels. |

Policy state does not repeat inside every request. The request is evaluated against one snapshot.

### Governance decision

| Field | Required | Meaning |
| --- | --- | --- |
| `outcome` | yes | `not_applicable`, `allowed`, `would_block`, or `blocked`. |
| `source` | yes | `none`, `policy`, `protected_host`, or `request_restriction`. |
| `phase` | yes | `pre_dispatch` or `landing`; a landing decision occurs only after navigation committed. |
| `reason` | yes | Stable reason code for every outcome. |
| `decision_id` | on block or would-block | Stable audit correlation id. |
| `rule_id` | when one rule resolved | Bounded rule attribution. |
| `restriction_id` | when source is `request_restriction` | Content-derived `R-<32 lowercase hex>` request-restriction identity. |
| `restriction_rule_id` | when one restriction rule resolved | Bounded authored rule attribution; never a policy `rule_id`. |
| `resource` | when resolved | Normalized host or bounded resource label, never a full sensitive URL. |
| `required_capabilities` | yes | Complete set used in the decision. |
| `policy` | when active | Name, revision, content identity, source, and managed sequence when relevant. |

Initial reason codes:

| Code | Meaning |
| --- | --- |
| `no_policy` | No service policy applied. |
| `capability_free` | The operation needs no governed capability. |
| `rule_allowed` | The first covering rule contains every required capability. |
| `resource_exempt` | An active policy explicitly exempts this resource kind from host coverage. |
| `capability_union_allowed` | A hostless operation is covered by the first rule containing every required capability. |
| `capability_union_not_allowed` | No rule contains every capability required by a hostless operation. |
| `protected_host` | An always-on protected-host rule blocked the resource. |
| `host_not_covered` | No policy rule covers the resource. |
| `capability_not_allowed` | The covering rule lacks one or more required capabilities. |
| `unsupported_resource` | The resource kind is outside policy scope. |
| `resource_unknown` | The governed resource could not be proven. |
| `unclassified_operation` | No canonical capability classification exists. |
| `request_restriction` | The tighten-only request restriction refused work. |

These codes are canonical facts. Surfaces translate them into model-appropriate prose. They do not
parse historical denial sentences to recover meaning.

Decision evaluation has three exact stages. Protected-host safety runs first:

| Condition | Outcome | Source | Reason | Decision id | Rule id |
| --- | --- | --- | --- | --- | --- |
| A governed current, target, or landing host matches a protected host | `blocked` | `protected_host` | `protected_host` | present | absent |
| Protected hosts are active, a page-scoped host is required for safety, and that host cannot be proven | `blocked` | `protected_host` | `resource_unknown` | present | absent |
| No protected-host boundary blocks | continue to service policy | -- | -- | -- | -- |

When protected-host safety continues, service policy produces one provisional decision:

| Condition | Outcome | Source | Reason | Decision id | Rule id |
| --- | --- | --- | --- | --- | --- |
| No protected-host boundary blocks and no capabilities are required | `not_applicable` | `none` | `capability_free` | absent | absent |
| Required capabilities and no service policy | `not_applicable` | `none` | `no_policy` | absent | absent |
| Active policy and an explicitly exempt resource | `allowed` | `policy` | `resource_exempt` | absent | absent |
| Hostless operation and the first capability-covering rule contains every requirement | `allowed` | `policy` | `capability_union_allowed` | absent | present |
| Hostless operation, no rule covers every capability, observe | `would_block` | `policy` | `capability_union_not_allowed` | present | first rule when present |
| Hostless operation, no rule covers every capability, enforce | `blocked` | `policy` | `capability_union_not_allowed` | present | first rule when present |
| Resolved host and first covering policy rule contains every capability | `allowed` | `policy` | `rule_allowed` | absent | present |
| No rule covers, observe | `would_block` | `policy` | `host_not_covered` | present | absent |
| Covering rule lacks a capability, observe | `would_block` | `policy` | `capability_not_allowed` | present | present |
| Unsupported or unknown governed resource, observe | `would_block` | `policy` | matching resource reason | present | absent |
| Any of the previous three policy mismatches, enforce | `blocked` | `policy` | matching policy reason | present | as applicable |

Finally, the request restriction combines with that provisional result:

| Safety or provisional service result | Restriction result | Final result |
| --- | --- | --- |
| Protected-host block | not evaluated | Keep the protected-host block. |
| Enforced service-policy block | not evaluated | Keep the service-policy block. |
| `allowed`, `not_applicable`, or `would_block` | block | Return `blocked`, source and reason `request_restriction`, with its own decision, restriction id, and matched restriction rule id when one resolved. |
| `would_block` | allow or absent | Keep the policy would-block; an allowing restriction cannot erase it. |
| `allowed` | allow or absent | Keep the policy allow. |
| `not_applicable` | allow or absent | Keep the no-policy or capability-free not-applicable result. |

An exempt resource is the deliberate `about:blank`-class bypass accepted by the canonical resource
resolver. A no-resource operation still has capabilities but no host, such as listing controlled
tab metadata; it uses the ordered any-rule capability union and never invents a host. Out-of-scope
and indeterminate resources use `unsupported_resource` and `resource_unknown`, respectively.

One operation may produce more than one governance decision. The canonical bound is 32 decisions
per operation: pre-dispatch first, then committed landings in navigation order. Audit stores that
ordered trace in one correlated operation record. The model-facing result projects the ordered
`would_block` and `blocked` decisions and omits ordinary allowed or not-applicable boilerplate.

Navigation combination rules are deterministic:

- target would-block followed by an allowed landing retains the target would-block;
- allowed target followed by a landing would-block retains the landing would-block;
- would-block at both phases retains both in order, including distinct rules and reasons;
- an enforced target block stops before dispatch;
- an enforced landing block retains any earlier would-block decisions, appends the landing block,
  stops readiness observation, and triggers the typed safety-park result; and
- a decision-journal overflow fails closed with problem `decision_trace_overflow`; a lost exact
  document or landing identity fails closed with problem `landing_identity_lost`. They are not
  reported as the same fault.

Both landing-integrity failures stop readiness and require a typed safety park. If at least one
requested navigation commit is proven, the operation is `partial`, effect `committed`, repeat
`do_not_repeat`, and readiness `unavailable`. With no proven commit it is `outcome_unknown`, effect
`unknown`, repeat `do_not_repeat`, and readiness `unavailable`. The first 32 decisions remain in
audit. Model-facing governance contains exactly the evaluated non-normal decisions within that
trace; it never invents a 33rd decision. When landing identity is lost before the bound, any
applicable `resource_unknown` policy or protected-host decision is appended normally. Direct and
nested sequence results use the same terminal tuple and common safety-park receipt. No denied,
unknown, or stale page URL or title is presented as final.

An unclassified capability-bearing operation fails closed as `blocked` with source `policy` when a
policy is active, or source `request_restriction` when only the restriction requires
classification. With neither authority active, canonical operation validation rejects it before
governance rather than inventing an all-open classification.

## Decision and execution trace

The pipeline keeps policy truth and runtime truth distinct:

1. Decode the Ghostlight call into one typed operation.
2. Validate canonical arguments and identify the typed scheduling resource without probing the
   current tab URL or governing host.
3. Admit the request to its resource queue and record the current authority epoch.
4. Select it fairly and acquire the surface execution lease. Retire it before work if the
   admission epoch changed while queued.
5. Capture the current authority snapshot and immutable request restriction into the work context.
   Once execution starts, keep that snapshot through landing checks and audit.
6. Stop for an ended session, active user hold, attention pause, or lost live ownership when
   applicable.
7. Classify the operation into its complete capability set.
8. Resolve the governed resource under the lease without inspecting page meaning.
9. Apply protected-host safety.
10. Evaluate the selected policy and then the tighten-only request restriction.
11. Perform final end-session, session-liveness, hold, attention, live ownership,
    browser-generation, and authority admission at the physical send boundary.
12. Dispatch browser mechanisms.
13. Re-check every committed navigation landing under the same snapshot before readiness or page
    presentation.
14. Produce one canonical operation outcome with status, effect, repeat, readiness, governance,
    and problem truth.
15. Record the governance decision trace and execution outcome as separate audit axes.
16. Let the Ghostlight renderer produce the exact model-facing result.

The exact implementation may combine local steps, but it cannot remove their semantic boundaries.
An allow is not a successful dispatch. A block is not a transport failure. A hold is not a policy
block. An unknown browser effect is not a denial.

Topology-only calls use their typed topology lane and do not invent a page-resource probe merely to
fit this trace. Queue selection and the execution lease still precede any live authority or browser
resource observation owed by that operation.

## One Ghostlight output contract

Every MCP client receives the same Ghostlight operation and result semantics. Protocol revision affects
MCP lifecycle and envelope details only. Client identity never selects a different browser
dictionary, governance decision, effect classification, or recovery language.

The service produces typed Ghostlight facts. The MCP edge then performs one mechanical
projection:

- operation names become the exact Ghostlight tool names;
- typed results become the matching Ghostlight result payload;
- summaries, problems, and suggested next steps remain service-authored;
- text and image parts use the MCP revision's standard content envelope; and
- workspace authority is added only where the request-stateless MCP revision requires it.

There is no Claude, Codex, Legacy, or other vendor renderer. A future model-specific adapter needs
a new ADR and measured evidence that it reduces invalid calls or turns enough to justify a second
contract.

### Output invariants

Every Ghostlight result must preserve these meanings:

- a pre-dispatch block has effect none;
- a governance-refused committed landing has operation status partial, reports the navigation
  effect as committed, names the landing phase, reports any safety-park outcome separately, and
  never invites replay;
- a would-block call was allowed only because policy is in observe mode;
- `effect: committed` is never presented as no effect;
- `effect: unknown` is never presented as safe to replay;
- a user hold, attention pause, policy block, browser outage, cancellation, and tool failure remain
  distinguishable enough for the model to choose the correct next move;
- a protected-host block never suggests a workaround;
- a policy block may suggest asking the user, contacting the named administrator, waiting for an
  authority change, choosing an already-authorized alternative, or stopping;
- page content, adapter errors, and organization-authored prose cannot become trusted model
  instructions; and
- a suggested call validates against the exact Ghostlight schema for the active MCP revision.

If the Ghostlight schema cannot express a material governance, effect, uncertainty, or recovery fact,
the schema is incomplete and must be corrected before the operation ships.

### Model-facing block example

The Ghostlight projection of a pre-dispatch policy block should be short and actionable:

```json
{
  "status": "blocked",
  "summary": "Ghostlight did not run this call because the active policy does not cover admin.crm.example.com.",
  "effect": "none",
  "repeat": "check_state_first",
  "governance": [
    {
      "outcome": "blocked",
      "source": "policy",
      "phase": "pre_dispatch",
      "reason": "host_not_covered",
      "decision_id": "D-4c5a910e"
    }
  ],
  "problem": {
    "code": "policy_blocked",
    "message": "No policy rule covers this host. Decision D-4c5a910e."
  },
  "suggested_next_steps": [
    {
      "kind": "ask_user",
      "question": "Would you like me to stop or continue with an already-authorized part of the task?",
      "reason": "The requested browser action is outside the active policy."
    },
    {
      "kind": "stop",
      "reason": "Do not retry or work around an enforced policy block."
    }
  ]
}
```

## Explanation and recovery delight

### Effective view first

The primary explanation answers what is in force now:

- policy source, name, revision, and enforcement;
- observe mode clearly labeled as not blocking;
- ordered rules in plain language;
- effective protected hosts without exposing unrelated private patterns to a model;
- effective settings, source, and lock state;
- managed freshness and last-known-good state;
- ignored or displaced user sources;
- organization rationale and contact when trusted and bounded; and
- warnings that materially change the reader's understanding.

An authored-file explanation remains available for review before deployment, but it is labeled
"authored policy" and never presented as the effective live result.

### Plain rule rendering

Prefer:

> Rule `crm` covers subdomains of crm.example.com except admin.crm.example.com. It permits read,
> interaction, and declared writes. The policy is in observe mode, so a mismatch is recorded but
> not blocked.

Avoid:

> Grant crm resolves action/write on the first matching domain under shadow enforcement.

### Recovery rules

| Situation | Helpful response | Never suggest |
| --- | --- | --- |
| Protected host | State that Ghostlight will not operate there; offer user handoff or stop | Removing the host, another tool, or another path around it |
| Enforced policy block before dispatch | Name the missing capability or uncovered host and decision id; offer trusted organization contact, authorized alternative, ask-user, or stop | Immediate retry or evasion |
| Enforced landing block after navigation committed | State that navigation committed but the landing was refused, report the safety-park result separately, and prohibit replay | Claiming no effect or repeating navigation |
| Would block | Say observe mode allowed the call and the same policy would block it under enforce | Claiming the policy protected the action |
| User hold | Say the person has browser control and Ghostlight is waiting | Editing policy or reconnecting the browser |
| Attention pause | Say repeated denials paused this one session and a person must choose a disposition | Retrying the denied action |
| Browser unavailable | Offer the correct browser reconnection step | Calling an administrator about policy |
| Unknown effect | Ask the model to observe or ask the user; prohibit replay | Treating uncertainty as a policy denial |
| Managed last-known-good | State that the last verified policy remains active and why refresh did not replace it | Claiming governance is absent |

Ordinary allowed success does not repeat policy boilerplate. Governance copy appears when it changes
the model's decision, when observe mode needs honest labeling, or when the user explicitly asks for
status or explanation.

## Authoring delight

### Start from intent, not a blank schema

The authoring flow should ask:

1. Which hosts should this rule cover?
2. Which of read, interact, write, or execute should it permit?
3. Should the policy observe or enforce?
4. Are any hosts protected under every policy?
5. Is this personal, machine organization, or managed fleet policy?

It then emits canonical policy and settings separately.

### Lint before deployment

Explain and validate report, in plain language:

- duplicate rule ids, host patterns, or capabilities;
- empty coverage or empty capability rules;
- a later rule that re-opens a host excluded by an earlier rule;
- a rule that can never resolve because an earlier rule covers all of its hosts;
- `interact`, `write`, or `execute` without `read`, as a warning rather than an invented
  implication;
- observe-mode policies described as protection;
- an empty enforce policy, with an explicit block-all explanation;
- settings that are shadowed by an organization-required value;
- destination settings that do not apply to the selected audit output;
- duplicate or non-normalized protected hosts;
- a managed policy older than the held publish sequence;
- untrusted authored labels that resemble verified identity; and
- compatibility fields that normalize away or cannot affect behavior.

Every diagnostic names the source and field, explains the effective behavior, and gives one valid
correction. Unknown fields fail rather than disappear.

### Simulate the same decision

Simulation uses the same normalized policy, capability descriptors, resource normalization, rule
evaluation, and reason codes as live governance. It reports allowed, would-block, and blocked
counts by rule and reason. It never claims that recorded traffic covers future workflows.

## Audit language

Audit separates governance from execution.

| Axis | Values | Purpose |
| --- | --- | --- |
| `governance_evaluation` | `not_evaluated` or `evaluated` | Say whether governance ran before a runtime stop. |
| `governance_decisions` | ordered array of 0 to 32 canonical decisions | Preserve pre-dispatch and landing outcomes without flattening redirects. Each entry carries outcome, source, reason, phase, ids, resource, capabilities, and policy attribution from the canonical decision type. |
| `request_restriction` | absent, or `{restriction_id, content_identity}` | Identify the immutable restriction once per operation. The id matches `^R-[0-9a-f]{32}$`; content identity matches `^sha256:[0-9a-f]{64}$`. |
| `execution_status` | Canonical operation status | What happened after governance. |
| `effect` | `none`, `committed`, `unknown` | What browser effect Ghostlight can prove. |
| `repeat` | `safe`, `check_state_first`, `do_not_repeat` | Whether replay is safe. |
| `operation` | Canonical operation id | Replayable semantic identity. |
| `intent` | Canonical concrete intent | Closed content-free variant such as `navigate.url` or `act.click`; never operation arguments, URLs, queries, text, code, or form values. |
| `required_capabilities` | Complete ordered set when classified; absent when an early runtime stop prevented classification | No lossy first-capability summary and no invented classification. |
| `resource` | Normalized bounded resource when resolved | No full URL or page payload; absent when evaluation stopped earlier. |
| `runtime_control` | hold, attention, end-session, or none | Explain non-policy admission state without falsifying policy. |

An audit row never presents an allowed decision as the only explanation for a call that was held
or attention-paused. It records the decision trace and runtime result on separate axes.

When end-session, session liveness, hold, attention, or ownership conclusively stops work before
classification and governance, there is no `GovernanceDecision`. Audit records
`governance_evaluation: not_evaluated`, an empty `governance_decisions` array, and the exact
runtime control or admission result. If a live control changes only at final admission after
governance ran, audit keeps the real ordered decisions and records the later runtime stop
separately.

Audit is owner-configured, never Ghostlight telemetry, and remains content-free. It does not store
page text, form values, screenshots, queries, full URLs, opaque workspace authority, or
model-authored suggested guidance. Audit output is configured independently from whether a policy
exists.

## Current compatibility mapping

Existing formats remain readable through a deterministic normalizer during migration.

| Current input | Canonical meaning |
| --- | --- |
| policy manifest | policy |
| manifest `schema: 3` | compatibility input normalized into canonical schema 1; never re-labeled without validation |
| manifest `name` | policy `name` |
| manifest `version` | policy `revision` |
| grant | rule |
| grant `description` | rule `description` |
| `hosts.allow` | `hosts` |
| `hosts.deny` | `except_hosts` for that rule only, preserving most-specific allow/deny and tie-to-except behavior |
| `allowed` | `capabilities` |
| capability `action` | capability `interact` |
| manifest `mode` | policy `enforcement` |
| missing manifest mode plus `governance.mode` | materialized policy enforcement fallback during import |
| per-grant `mode` | internal compatibility-only effective enforcement on that normalized rule; not authorable in canonical v1 |
| `identity` | historical untrusted audit presentation accepted by the compatibility parser; discarded from canonical authority and new authoring |
| `content.security.sacred_domains` | `safety.protected_hosts` |
| `content.security.secrets.redact` | `privacy.redact_sensitive_fields` |
| `audit.enabled=false` or `audit.destination=none` | `audit.output=off` |
| `audit.enabled=true`, destination `file` | `audit.output=file` |
| `audit.enabled=true`, destination `stderr` | `audit.output=stderr` |
| `audit.enabled=true`, destination `syslog` | `audit.output=syslog` |
| `audit.file.path` | `audit.file.path` |
| `audit.syslog.address` | `audit.syslog.address` |
| `org_mandatory` | organization required setting |
| `org_recommended` | organization default setting |
| personal policy `config[].level` | ignored as authority; every entry becomes a user setting and a compatibility warning explains the discarded level |
| `fully_open`, `safe`, or `restricted` preset | materialized legacy setting values; no canonical preset identity survives |
| `engine.connection.first_call_wait_ms` | `runtime.browser_connect_timeout_ms` |
| `engine.script.budget_ms` | `runtime.sequence_max_duration_ms` |
| `inbound.pipe.enabled` | `service.local_bridge_enabled` |
| `manage.web.enabled` | `service.management_ui_enabled` |
| `outbound.browser.enabled` | no canonical destination; migration rejects it because the current service has no live reader |
| session overlay | immutable tighten-only request restriction captured into each work context |
| org policy file origin | fixed organization policy source |
| managed origin | verified managed organization policy source with sequence and freshness |
| `--manifest` user file or `env://` source | explicit user policy source |
| `GHOSTLIGHT_MANIFEST` user file or `env://` source | environment-supplied user policy source |
| `shadow_deny` | would block |

Compatibility normalization is one-way into the canonical model. Canonical code does not carry
both names or branch on a source format. A frozen format renderer may preserve its promised bytes.

For exact legacy behavior, an imported rule may carry one internal
`compatibility_enforcement: observe | enforce | null` fact. A non-null value fully overrides the
imported policy enforcement for that rule, matching the historical evaluator; it is not
strictest-wins. Effective explanations label this branch as compatibility-only. A policy with any
such override cannot be losslessly exported as canonical v1: it remains a compatibility input or
requires an explicit author decision during migration.

## Current truth gaps found by this pass

These are implementation or documentation defects to resolve before accepting this primer. They
are not intended features.

The governance guide's false claim that all-open records nothing was corrected during this pass.
The remaining gaps are:

1. The `fully_open` settings preset is not all-open authorization. An active policy still governs.
2. `safe`, `restricted`, and the built-in Minimal settings are currently identical.
3. The `developer-unrestricted` template has no rules and therefore appears to block all
   capability-bearing work under the enforce fallback.
4. A personal policy setting requires a `level` field even though that level cannot take effect.
5. Personal-policy settings displaced by organization policy have contradictory documented and
   implemented precedence.
6. Audit can be disabled in two ways, and status may say audit is on with destination `none`.
7. The user settings schema is typo-closed, but the loader warns and ignores unknown or invalid
   entries. That is especially unsafe for misspelled protected hosts.
8. Protected-host lists are selected by ordinary setting precedence instead of unioned across
   authorized sources.
9. Current protected-host probing treats an unresolvable page host as no match. The intended
   always-on boundary fails closed when a page-scoped safety host cannot be proven.
10. Managed required/default settings are not reflected truthfully by every config CLI view and
    write path.
11. `outbound.browser.enabled` is advertised but has no live behavior reader.
12. Authored `identity` is informational, yet some copy implies identity-bound policy and audit.
13. Current denial objects own final caller prose too early for model-specific rendering.
14. Current audit names canonical operation and intent as `tool` and `action`, stores only a lossy
    capability summary, and uses extra booleans to explain holds and attention after recording an
    allowed policy decision.

Each item needs a root fix, a deliberate compatibility mapping, or removal. Documentation must not
paper over it.

## Governance delight acceptance rubric

### Language test

- A reader can distinguish policy, settings, protected hosts, user hold, attention pause, browser
  availability, and execution uncertainty from their first sentence.
- New product copy uses policy, rule, host, capability, allowed, would block, and blocked
  consistently.
- Authored audit labels are never called verified identity.

### Authoring test

- A personal scoped policy needs only name, revision, enforcement, and ordered rules.
- Personal settings do not ask for organization level.
- Unknown fields and contradictory values fail once with one valid correction.
- No two controls disable the same feature.
- No preset names two identical results or implies it can override policy authority.
- Explain renders both the authored source and the effective live result, clearly labeled.

### Decision test

- Both supported MCP revisions expose identical canonical operations and governance decision
  traces.
- Protected hosts apply with no policy, in observe mode, and at every navigation landing.
- Request restrictions only narrow.
- Policy allowance never bypasses final user-control, ownership, attention, or browser-generation
  admission.
- No page content participates in policy classification.

### Output test

- Every result validates against the exact Ghostlight schema for its operation and MCP revision.
- A policy block, protected-host block, hold, attention pause, browser outage, and unknown effect
  remain distinct.
- Suggested calls use real Ghostlight names and complete validating arguments.
- No MCP envelope weakens status, effect, repeat, readiness, problem, or uncertainty.

### Audit test

- Governance decision, runtime control, execution status, effect, and repeat are distinct fields.
- When classification ran, the complete capability set is preserved; an early stop never invents
  one.
- Policy source and exact revision are attributable.
- Content payloads and opaque workspace authority remain absent.
- Simulation and live decisions share the same canonical reason codes.

### Managed-policy test

- First boot without required valid managed policy fails closed.
- Invalid, unavailable, or rolled-back refresh retains last-known-good.
- Effective config views and lock behavior include managed required/default settings.
- Organization contact and rationale are bounded presentation facts, never authority or model
  instructions.

## Implementation order

This pass defines language and target semantics. It does not change production governance by
itself.

1. Accept this language through a new ADR or marked amendment.
2. Add canonical governance snapshot, request, decision, reason, and audit vocabularies without
   changing the external policy formats yet.
3. Split typed decision facts from surface prose.
4. Normalize existing policy and settings inputs into the canonical model.
5. Fix protected-host union, managed settings visibility, audit controls, presets, template truth,
   and typo handling before advertising the new authoring format.
6. Rebuild the action pipeline around canonical operation plus immutable governance snapshot.
7. Implement the Ghostlight renderer first.
8. Remove the Legacy surface, surface selectors, and vendor-adapter scaffolding.
9. Add both-revision Ghostlight schema, outcome, governance, and recovery gates.
10. Keep only external policy/settings parsers and the independently versioned browser-mechanism
    compatibility wire.

The governing principle is simple: one canonical decision trace, one truthful execution outcome,
and one delightful browser language for every model.
