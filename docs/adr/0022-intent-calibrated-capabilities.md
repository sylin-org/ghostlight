# 0022. Intent-calibrated capabilities: epistemic classification, per-action requirements, host polarity

- Status: Accepted
- Date: 2026-07
- Supersedes: the read/write classification model of the stage-2 shared format doc
  (`docs/tasks/stage-2/00-shared-format.md` sections 4.3, 6.1 `rw`, and 8) and the
  `access` / `tools` / `exclude_tools` grant fields of manifest schema 2. ADR-0018,
  ADR-0019, and ADR-0020 remain in force; this ADR changes WHAT a grant expresses,
  not when enforcement ships, how config layers resolve, or the org policy
  experience commitments.

## Context

Live verification of stage 2 (2026-07-02, real Chrome + Claude Code) exposed a
defect that turned out to be a category error, not a bug: `navigate` was
classified `mutate`, so a grant with `access: "read"` could not navigate to the
very domains it granted. The shipped example manifests ("Read-only external
research resources") were unusable as described, and the project's own
acceptance script (BROWSER-TESTS.md g13-1) assumed the opposite behavior.

Root-cause analysis found the read/write axis conflating four things it cannot
distinguish:

1. **Retrieval** whose effect is knowable (a `navigate` is a GET; safe by HTTP
   semantics).
2. **Input dispatch** whose effect is fundamentally unknowable (a click's
   consequence is decided by the page's own code; classifying it "by effect"
   would require knowing the intent of every page on the web).
3. **Declared mutation** (a structured form submit exists to change state).
4. **Arbitrary code** (`javascript_tool` can do any of the above and bypass the
   UI entirely).

The design insight (user, in review): **action is not intent.** A navigate is a
read; a navigate to a logout URL is an intent problem, and intent at a
destination is governed by scoping WHERE the agent may act (host allow/deny
lists, the sacred list), never by misclassifying the action itself. The
governor must classify by what it can PROVE about an operation, and control
scope for what it cannot.

## Decision

### 1. Capability taxonomy: classify by epistemic status

Four capability primitives, defined by what the governor can prove about an
operation, not by its (unknowable) downstream effect:

| Capability | Definition (what the governor knows) | HTTP analogy |
|---|---|---|
| `read` | Provably retrieval/observation only. Commits no activation and no data to the page. | GET / no request |
| `action` | Dispatches UI input whose effect is page-determined and unknowable. Commits an activation or data to page handlers. | GET or POST, undecidable |
| `write` | A declared mutation: the operation's purpose is to change state. | POST/PUT/DELETE |
| `execute` | Unbounded: arbitrary code; any method, any target, UI bypass. | any / none |

The line between `read` and `action` is COMMITMENT: an operation that moves the
viewport or pointer without committing an activation or data (`scroll`,
`hover`, `scroll_to`) is `read`, even though it dispatches DOM events. An
operation that commits an activation (`left_click`) or data (`type`) is
`action`. This retains and generalizes the stage-2 rationale ("a read-only
grant that cannot scroll cannot read below the fold").

`action` is NOT a weaker `write`. It encompasses the ability to CAUSE writes (a
click can submit a form). Documentation must state this loudly wherever the
vocabulary is rendered (explain, schema descriptions, key reference).

`execute` is never implied by any other capability. A grant that wants
`javascript_tool` says `execute` explicitly.

Vocabulary (wire/file names, lowercase): `"read"`, `"action"`, `"write"`,
`"execute"`. This vocabulary is the product language; presets and docs build on
it.

### 2. The action directory: per-action bound requirement sets

Every action on the tool surface carries a declared requirement set and a
curt, agent-targeted description, compiled into the binary as a static table
(the browser plugin, successor to `browser/classify.rs`):

    { action, requires: [capability, ...], description }

Population rule: an action requires the capability matching the STRONGEST
effect the governor can prove it has. Actions that provably touch no page and
no server require nothing (`requires: []`).

The authoritative table (12 tools + 13 computer actions; tool names and the
computer action enum are the sacred surface of ADR-0007):

| Action | requires | Rationale |
|---|---|---|
| `navigate` | `["read"]` | Provably a GET (top-level document load). Also the host-coverage enforcement point (pre-dispatch target check + post-navigate landing check, both retained). |
| `read_page` | `["read"]` | DOM read. |
| `get_page_text` | `["read"]` | DOM read. |
| `find` | `["read"]` | DOM read. |
| `read_console_messages` | `["read"]` | Buffer read. Can carry secrets; accepted as `read` (see Consequences). |
| `read_network_requests` | `["read"]` | Buffer read. Can carry tokens/PII; accepted as `read` (see Consequences). |
| `tabs_context_mcp` | `["read"]` | Reveals URLs/titles of the MCP tab group (information disclosure); domain-less, governed by the no-host union rule. |
| `update_plan` | `[]` | Informational pass-through; touches no page, no server. |
| `tabs_create_mcp` | `[]` | Creates an empty tab; browser state only, no page, no server. |
| `resize_window` | `[]` | Browser state only. |
| `form_input` | `["write"]` | Declared mutation (structured fill/submit). |
| `javascript_tool` | `["execute"]` | Unbounded. |
| computer `screenshot` | `["read"]` | Capture. |
| computer `zoom` | `["read"]` | Capture (region). |
| computer `scroll` | `["read"]` | Viewport movement; commits nothing. |
| computer `scroll_to` | `["read"]` | Viewport movement. |
| computer `hover` | `["read"]` | Pointer movement; commits no activation. |
| computer `wait` | `[]` | Pure timer; touches nothing. |
| computer `left_click` | `["action"]` | Commits an activation; effect page-determined. |
| computer `right_click` | `["action"]` | Commits an activation. |
| computer `double_click` | `["action"]` | Commits an activation. |
| computer `triple_click` | `["action"]` | Commits an activation. |
| computer `type` | `["action"]` | Commits data to page handlers. |
| computer `key` | `["action"]` | Commits input. |
| computer `left_click_drag` | `["action"]` | Commits input. |

(The `explain` directory tool of Decision 7 adds one more row: `requires: []`.)

Two invariants, enforced in the type system and pinned by tests:

- **Absent means DENY.** An action with no directory entry is a classification
  miss and is denied (fail closed). "No entry" and "no requirements" are
  distinct states and must be unconfusable in code (`Option<&[Capability]>`
  style: `None` = deny, `Some(&[])` = allow).
- **Empty means ALLOW, unconditionally.** `requires: []` short-circuits to
  allow BEFORE resource resolution: no host lookup, no grant scan. The bar for
  assigning `[]` is "provably affects no page and no server"; it is a reviewed,
  deliberate judgment, never a default.

The sacred-domains check (g08) is unaffected: it runs at the dispatch
chokepoint ahead of all of this, keyed on tab/target hosts, and `requires: []`
actions carry no tab or target so it never applies to them.

`requires` is a SET (subset containment below), though every current entry is a
singleton or empty. Sets keep the algebra honest: `action` and `write` are
independent capabilities, not ordered tiers, and a future multi-capability
action needs no schema change.

Descriptions are governance metadata, published through the Decision 7 tool,
`policy explain`, and generated docs. They are NEVER merged into the 13
trained tool schemas (ADR-0007 fidelity).

### 3. Grants carry capability sets; capabilities replace tool lists

A schema-3 grant declares `allowed: [capability, ...]`. The per-grant `tools`
and `exclude_tools` fields are REMOVED, along with `access`.

Enforcement is subset containment: a call is permitted by a grant iff
`requires(action)` is a subset of `allowed(grant)`.

Why replacement is safe and honest:

- The flagship exclusion pattern (`exclude_tools: ["javascript_tool"]`) maps
  exactly to omitting `execute`.
- Excluding `form_input` while allowing clicks was security theater: a click
  can submit the same form. Capabilities cannot express that false comfort,
  which is a feature.
- The real loss of granularity is accepted: e.g. a grant can no longer allow
  `read_page` but exclude `read_network_requests` (both are `read`). See
  Consequences; a future "capability qualifier" mechanism is the escape hatch
  if a real deployment needs it.

`allowed: []` is valid (a grant that scopes hosts but permits nothing beyond
`requires: []` actions, which need no grant anyway); `policy explain` renders
it plainly rather than the validator rejecting it. A validator warning (not an
error) notes that `action`/`write`/`execute` without `read` is almost
certainly a mistake (the agent cannot see what it is acting on).

### 4. Host polarity: per-grant allow/deny with a pinned default

A schema-3 grant scopes hosts with explicit polarity:

    "hosts": { "allow": [pattern, ...], "deny": [pattern, ...] }

Both members optional; each defaults to `[]`.

Semantics (all four rules are load-bearing):

1. **The default is always DENY.** A host matched by neither list is not
   covered by the grant. `allow: ["*"]` is the explicit everything token; the
   only way to get a permissive grant is to type it. `deny` only carves holes
   out of `allow`; a grant with only `deny` entries covers nothing. There is
   no lint or warning on `allow: ["*"]`: it is an explicit, unambiguous
   declaration of intent and the author owns it.
2. **Most-specific match wins; exact tie goes to deny.** Specificity order:
   exact host pattern > `*.suffix` wildcard (between two wildcards, the longer
   suffix is more specific) > `*`. If the identical pattern appears in both
   lists, deny wins.
3. **Pattern grammar is exactly:** `*` (universal), `*.domain.tld` (subdomain
   wildcard; matches subdomains only, never the apex, per the existing G07
   matcher), or an exact host. No partial globs (`site*`, `*bank*`), no
   scheme, no port, no path. The field is named `hosts`, not `urls`, because
   that is what it matches; path-level rules are explicitly out of scope (see
   Future work).
4. **Per-grant scope only.** A grant's `deny` shrinks THAT grant's coverage
   and nothing else; a host denied by grant A may still be covered by grant B.
   The absolute, cross-cutting never-touch tier remains
   `content.security.sacred_domains` (g08), which wins over every grant in
   every mode. Grant composition across the manifest stays first-match-wins in
   manifest order, where "match" means the grant's polarity evaluates to
   allowed for the host.

Canonical postures:

    { "allow": ["site1.com", "site2.com"] }          allowlist: those two, nothing else
    { "allow": ["*"], "deny": ["site1.com"] }        denylist: everything except site1
    { "allow": ["*"] }                                everything
    { }  or  { "allow": [] }                          nothing
    { "deny": ["site1.com"] }                         nothing (deny carves from allow; no allow, nothing to carve)

Note the denylist posture fails open by nature (an unlisted bad host is
allowed). That is the author's explicit choice via `allow: ["*"]`; presets and
docs steer sensitive contexts toward allowlist without nagging.

### 5. The evaluation algorithm

Two static inputs, one runtime evaluator. The directory (code) says what each
action costs; the policy (manifest) says what is allowed where; only the
evaluator sees a concrete request and joins them.

For one call, in order:

1. **Hold/kill/sacred checks** run first, unchanged (g10, g11, g08). Sacred
   denials remain a separate always-on path outside the mode switch.
2. **Directory lookup.** No entry for the action: DENY (rule
   `unknown_action`). `requires: []`: ALLOW immediately (skip everything
   below; no resource resolution, no audit grant attribution; the call is
   still audited).
3. **Resource resolution** (unchanged machinery): the call resolves to a host,
   `AlwaysAllow` (about:blank), `OutOfScope(scheme)`, `Indeterminate` (fail
   closed), or `None` (domain-less call with non-empty requires). The
   pre-navigate target check and the post-navigate landing re-check (with
   about:blank parking on a real deny only, never on a shadow deny) are
   retained exactly.
4. **Coverage.** Walk grants in manifest order; the first grant whose host
   polarity (Decision 4) evaluates to allowed for the resolved host is the
   resolving grant. A grant whose deny matches simply does not cover the host
   (evaluation continues to the next grant). If no grant covers it: DENY with
   rule `denied_domain` attributed to the first grant whose deny pattern
   matched, if any, else rule `unmatched_domain` with no grant.
5. **Capability check.** `requires(action)` subset of `allowed(resolving
   grant)`: ALLOW attributed to that grant. Otherwise DENY, rule `capability`,
   attributed to that grant, naming the missing capability.
6. **No-host union rule** (calls with `GoverningResource::None` and non-empty
   requires, e.g. `tabs_context_mcp`): allow iff ANY grant's `allowed` covers
   `requires`; attribute the first such grant; else deny rule `capability`
   attributed to the first grant (or `unmatched_domain` semantics when there
   are no grants), mirroring the current g13 union rule shape.
7. **Mode switch** (g15) applies unchanged: per-grant `mode` > manifest `mode`
   > `governance.mode`; observe turns the deny into `shadow_deny` and the call
   runs.

Denial rule strings (shared-format section 7 style; the stable denial-id
scheme, `D-` + 8 hex over manifest hash + grant id + rule, is unchanged):

| Rule | Replaces | Meaning |
|---|---|---|
| `unmatched_domain` | (kept) | No grant's allow covers the host. |
| `denied_domain` | (new) | A grant's deny pattern matched the host. |
| `capability` | `access` | Covering grant lacks a required capability. |
| `unknown_action` | (was an unconditional deny in dispatch) | Directory miss. |
| `scheme/<scheme>` | (kept) | Non-http(s) target. |
| `sacred` | (kept) | Never-touch list. |
| (removed) | `tool/<name>` | Tool lists no longer exist. |

Denial messages keep the section-7.2 voice: name the action label
(`computer (<action>)` form), the host, the grant id, the missing capability,
and what remains available.

### 6. Manifest schema 3

The grant shape changes; everything else (name/version/mode/identity/config,
the 4.2 content hash mechanics, strict JSON, `deny_unknown_fields`) carries
over:

    {
      "schema": 3,
      "name": "...", "version": "...",
      "mode": "observe" | "enforce",          // optional, unchanged
      "identity": { ... },                     // optional, unchanged
      "grants": [
        {
          "id": "research",
          "hosts": { "allow": ["*.arxiv.org", "scholar.google.com"] },
          "allowed": ["read"],
          "description": "Read-only research sources.",
          "mode": "enforce"                    // optional per-grant override, unchanged
        }
      ],
      "config": [ ... ]                        // unchanged (ADR-0019 entries)
    }

- `schema` must be exactly 3. A schema-2 document is rejected with a precise
  error naming this ADR (schema 2 never shipped; stage 2 is unmerged, so no
  dual-version support is built).
- `hosts` replaces `domains`; `allowed` replaces `access`; `tools` and
  `exclude_tools` are gone (unknown fields, rejected by `deny_unknown_fields`).
- Authoring guidance for translating old examples: `access: "read"` becomes
  `allowed: ["read"]`; `access: "write"` becomes `allowed: ["action",
  "write"]`; `access: "all"` becomes `allowed: ["read", "action", "write"]`.
  `execute` is NEVER part of a mechanical translation; it is added explicitly
  where the old grant genuinely relied on `javascript_tool` (a deliberate
  tightening).

### 7. The directory is self-describing: the `explain` tool

One new tool named `explain` is ADDED to the advertised tool surface. It takes
no arguments, requires `[]` (always allowed, always advertised under any
manifest), and returns the action directory: every action, its required
capabilities, and its agent-targeted description, plus the capability
vocabulary definitions. Trained Claude ignores it (it knows the 13); untrained
or non-Anthropic models call it and learn the surface. It is the map.

This DELIBERATELY relaxes ADR-0007 from "the advertised surface is
byte-identical to the official extension" to: **the 13 trained tool schemas
are byte-identical; exactly one additive, argument-less governance tool is
sanctioned on top.** `tests/tool_schema_fidelity.rs` is amended once to pin
the new invariant (13 trained entries byte-identical AND exactly one sanctioned
addition with a pinned schema); `CLAUDE.md`'s sacred-surface wording is updated
to match. No other addition, removal, or edit is sanctioned by this ADR.

Known risk, accepted: a model may spuriously call a tool named `explain` for
generic "explain this page" requests. The description is written to make the
tool self-disqualifying for page content ("Returns this server's action
directory and capability requirements; does not read or explain web pages").
If spurious invocation shows up in practice, renaming (e.g.
`tool_capabilities`) is a one-line follow-up decision; the name `explain` is
the decided starting point. (Namespace note: the `browser-mcp policy explain`
CLI subcommand is a different surface; no technical collision.)

### 8. Consumers: advertisement, audit, simulate, explain, presets

- **Advertisement (g14):** a tool is advertised iff it has at least one
  variant (for `computer`: at least one action) whose `requires` is a subset
  of the union of all grants' `allowed`, or whose `requires` is `[]`.
  Consequence: `requires: []` tools (`tabs_create_mcp`, `resize_window`,
  `update_plan`, `explain`) are advertised under every manifest, including an
  empty-grants one; the g14 "empty grants advertises nothing" test becomes
  "advertises exactly the requires-empty set".
- **Audit (g06):** the record's `rw` field is replaced by `capability`: the
  action's required capability set rendered as its single element for
  singletons, `"none"` for `[]` (JSON: a string; `"read" | "action" |
  "write" | "execute" | "none"`). Everything else in the record is unchanged.
- **Simulate (g17):** replays through the same evaluator; the bucket table's
  classification step consults the directory; a recorded `rw` value is
  ignored (already true by design). Old (`rw`-era) audit files remain
  replayable: simulate never trusted the recorded class.
- **`policy explain` (g16):** renders capability sets and host polarity in
  plain language, including the `action`-can-cause-writes warning sentence and
  the denylist-posture (`allow: ["*"]`) statement of fact.
- **Presets (ADR-0019/g18) are untouched** (they govern config keys, not
  grants), but the capability vocabulary invites a future preset-like ladder
  for grant authoring (see Future work).

## Consequences

- Positive: read grants work. `navigate` is `read`; the canonical "let the
  agent go read these sites" grant is expressible and true to its description.
- Positive: the model is honest. The unknowable (input dispatch) is named
  `action` and scoped by host, instead of being mislabeled as a knowable
  write. Nothing pretends to classify by effect.
- Positive: one table drives classification, enforcement, advertisement,
  explain, generated docs, and the `explain` tool. The scattered
  classify/advertise/docs surfaces cannot drift apart.
- Positive: `execute` is never implied; arbitrary code becomes an explicit,
  visible grant decision.
- Positive: schema 3 lands before anything ships (stage 2 unmerged): zero
  migration debt, no dual-version support.
- Negative: capability sets are coarser than per-tool lists; "allow read_page
  but not read_network_requests" is no longer expressible. Accepted; the
  escape hatch (capability qualifiers) is deliberately NOT built until a real
  deployment demands it.
- Negative: `action` reads as weaker than `write` but can cause writes. This
  is a documentation liability forever; every rendering surface carries the
  warning.
- Negative: the ADR-0007 byte-parity story gains an asterisk (13 + 1). The
  relaxation is contained to exactly one sanctioned tool and pinned by the
  amended fidelity test.
- Negative: wide churn: manifest engine, enforcement, advertisement, audit
  field, simulate, explain goldens, examples, templates, shared-format doc,
  and every test that spelled `access`/`observe`/`mutate`. Staged as the
  stage-3 task batch (`docs/tasks/stage-3/`).

## Future work (explicitly not in stage 3)

- Path-level deny rules at the navigate/landing chokepoints as defense in
  depth for sensitive areas inside a granted host. Explicitly leaky (SPA
  routing, encoding, fragments); documented as such if ever built.
- Network-layer enforcement (CDP `Fetch.requestPaused`: true method + full
  post-redirect URL) as the ground-truth intent boundary. The north star if
  intent fidelity ever becomes the product point; architecturally clean
  (extension relays, binary decides) but heavy; not built now.
- Capability qualifiers (fine-grained carve-outs inside a capability) only if
  a real deployment hits the coarseness limit.
- A grant-authoring ladder built on the vocabulary (e.g. observer = read;
  assistant = read+action; operator = read+action+write; developer =
  +execute) as documentation/template sugar, not new mechanism.

## Provenance

Designed in live review 2026-07-02/03 (user + Claude) after stage-2 live
verification. User-decided: the epistemic taxonomy and "action is not intent"
principle; javascript_tool = execute; per-action bound requirement sets with
empty-requires for browser-state tools; the directory as data in Rust code;
publishing it via a new `explain` entry in tools.json; capabilities REPLACING
tools/exclude_tools; host polarity with allow/deny lists; acceptance of
pinned-DENY default, specificity precedence, and the hosts (not urls) naming;
no warnings on explicit `allow: ["*"]`. Recommended-and-accepted: absent vs
empty directory invariant; per-grant deny scoping with sacred as the global
tier; restricted wildcard grammar; schema bump to 3; audit `rw` ->
`capability`; the `explain` tool's self-disqualifying description.
