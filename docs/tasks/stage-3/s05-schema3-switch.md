# S05: the schema-3 switch (manifest, enforcement, dispatch, advertisement, explain, simulate, examples)

## Goal

Replace the schema-2 grant model (`domains` + `access` + `tools`/`exclude_tools`, evaluated
over `RwClass`) with the ADR-0022 model (`hosts` polarity + `allowed` capability sets,
evaluated by requirement-subset containment) in ONE atomic task. The `Grant` type is
compile-coupled to enforcement, dispatch, advertisement, explain, simulate, and templates;
this task changes all of them together and leaves the tree green. After this task, no code
path reads `access`, `tools`, or `exclude_tools`, and no schema-2 manifest parses.

## Authority

ADR-0022 (`docs/adr/0022-intent-calibrated-capabilities.md`) Decisions 3, 4, 5, 6, and 8 are
normative for every semantic here. Where this prompt pins a string or algorithm, it is
transcribing the ADR; if you find a conflict, THE ADR WINS and you record the deviation.

## Depends on

- s02 (`Capability` in `src/governance/ports.rs`, with serde names and the subset helper).
- s03 (`src/browser/directory.rs`: `requires(tool, action) -> Option<&'static [Capability]>`,
  absent = None = deny, `Some(&[])` = unconditionally allowed).
- s04 (`src/governance/ports.rs::HostRuleOutcome { Allowed, Denied, Unmatched }`;
  `src/browser/polarity.rs::evaluate_host(host, allow, deny) -> HostRuleOutcome` implementing
  ADR Decision 4 including the bare `*` token; `is_valid_host_rule(pattern)` accepting `*` or
  any pattern `browser::pattern::is_valid_pattern` accepts).

If any of these is missing, STOP: earlier tasks did not land.

## Current behavior (verify against the tree before editing)

- `src/governance/manifest/document.rs`: `Grant { id, domains: Vec<String>, access: Access,
  tools: Option<Vec<String>>, exclude_tools: Option<Vec<String>>, description, mode }`;
  `Access { Read, Write, All }`; `parse_manifest(text, source_label, domain_pattern_valid,
  is_known_tool)` gates `schema == 2` and validates `tools`/`exclude_tools` names via
  `is_known_tool`.
- `src/governance/enforcement.rs`: `check_call(grants, tool, action, rw: RwClass, resource,
  manifest_hash, domain_matches: fn(&str,&str)->bool, manifest_mode, config_mode)`;
  `decide_for_host` does first-matching-grant (any domain pattern matches) -> tool-list check
  (rule `tool/<name>`) -> access check (rule `access`); `decide_no_page` is the union rule.
- `src/governance/dispatch.rs`: `Governance` holds `classify: fn(&str, Option<&str>) ->
  Option<RwClass>`; a classification miss becomes an ordinary `tool/<name>`-rule deny routed
  through the mode switch; `decide` builds `DecisionRequest` (which carries `rw: RwClass`).
- `src/browser/advertise.rs`: `advertised_tools(fixture, grants)` keeps a tool when any
  grant's tool-list AND access-class permit it; `computer` special-cased before `classify`.
- `src/governance/explain.rs`: `grant_line` renders `access` sentences (e.g. Access::All ->
  "Full access on {domains}: agents may read and act."), `tools`/`exclude_tools` sentences,
  and a write-without-read warning; goldens in `tests/fixtures/explain/*.txt`.
- `src/governance/simulate.rs`: `run_simulate(manifest_path, replay_path,
  domain_pattern_valid, is_known_tool, classify, domain_matches)`; the bucket table consults
  `classify`; fixtures in `tests/fixtures/simulate/`.
- `examples/`: five schema-2 manifests. `src/governance/templates.rs` embeds three of them
  via `include_str!` and validates them in tests with local stubs including a
  `test_is_known_tool`.
- `src/transport/mcp/server.rs`: resolves the call's `GoverningResource` (tab-URL query when
  governed), calls `governance.decide`, computes `rw` via `browser::classify::classify` for
  the audit record.

## Required behavior

### 1. Manifest schema 3 (`src/governance/manifest/document.rs`)

- New types (serde, `deny_unknown_fields` on both):

      pub struct HostRules {
          #[serde(default)] pub allow: Vec<String>,
          #[serde(default)] pub deny: Vec<String>,
      }
      pub struct Grant {
          pub id: String,
          pub hosts: HostRules,
          pub allowed: Vec<Capability>,
          pub description: Option<String>,
          pub mode: Option<EffectiveMode>,
      }

  `Access` is DELETED. `domains`, `tools`, `exclude_tools` are gone (unknown fields, rejected
  by `deny_unknown_fields`). `hosts` itself is required on every grant (a grant without a
  `hosts` member is a shape error); its two members default to empty.
- Schema gate: `schema == 3`. The unsupported-schema error keeps its current shape but says
  `(expected 3)`; when the found value is exactly `2`, append this sentence to the message:
  `schema 2 is superseded by schema 3 (ADR-0022); update the manifest's grants to hosts/allowed form.`
- Validation: every pattern in `allow` and `deny` must pass `is_valid_host_rule` (s04; bare
  `*` is legal ONLY here, never in `content.security.sacred_domains`); `allowed` entries must
  be valid capability names with no duplicates; grant ids unique and non-empty as today.
  `allowed: []` and empty `hosts` are VALID (they express "nothing"); no parse-time warning
  exists for capability sets (the acting-without-read lint is explain's job, section 6).
- `parse_manifest` signature loses `is_known_tool` entirely:
  `parse_manifest(text, source_label, domain_pattern_valid)`. Remove the parameter through
  the whole chain: `manifest/source.rs` (`load_policy`, `load_org_manifest_at`,
  `load_user_manifest`), `explain.rs` (`explain_file` and friends), `simulate.rs`
  (`run_simulate`), `config/cli.rs` (`shadow_line`, `run`, `run_list`), `doctor.rs`,
  `src/main.rs` (every call site), and every test. Run `rg -n "is_known_tool"` and remove
  every threading EXCEPT `transport::mcp::tools::is_known_tool` itself and its use in
  `transport/mcp/server.rs` (the server still rejects unknown tool names before dispatch;
  `tests/mcp_protocol.rs::unknown_tool_name_is_rejected_before_dispatch` must keep passing).

### 2. Ports (`src/governance/ports.rs`)

- `DecisionRequest`: replace `rw: RwClass` with `requires: Vec<Capability>`. `RwClass` itself
  is NOT deleted in this task (the audit `rw` field still uses it until s06); only the
  decision path stops consuming it.

### 3. Enforcement (`src/governance/enforcement.rs`)

Rewrite `check_call` to the ADR Decision 5 algorithm:

    pub fn check_call(
        grants: &[Grant],
        tool: &str,
        action: Option<&str>,
        requires: &[Capability],
        resource: &GoverningResource,
        manifest_hash: &str,
        evaluate_host: fn(&str, &[String], &[String]) -> HostRuleOutcome,
        manifest_mode: Option<EffectiveMode>,
        config_mode: EffectiveMode,
    ) -> Decision

- FIRST: `requires.is_empty()` -> `Allow { grant_id: None }` (before any resource matching).
- Then match `resource` exactly as today for `AlwaysAllow` / `OutOfScope` / `Indeterminate`.
- `Resource(host)`: walk grants in manifest order calling
  `evaluate_host(host, &g.hosts.allow, &g.hosts.deny)`. First grant returning `Allowed` is
  the resolving grant (stop walking). A grant returning `Denied` does not cover the host;
  remember the FIRST such grant and keep walking. If a resolving grant exists: allow iff
  `requires` is a subset of its `allowed` (rule `capability` otherwise, attributed to it,
  naming the first missing capability). If none resolves: rule `denied_domain` attributed to
  the first denying grant if any, else rule `unmatched_domain` with no grant.
- `None` (domain-less with non-empty requires): allow iff ANY grant's `allowed` covers
  `requires`, attributed to the first such grant; else rule `capability` attributed to the
  first grant; with zero grants, rule `unmatched_domain` over `"(unknown)"` (mirrors today's
  union-rule shape).
- `apply_mode`/`effective_mode` (g15) unchanged. `tool_list_denial` and `access_covers` are
  DELETED. `LocalPdp` passes the new request fields through; it now takes `evaluate_host`
  instead of `domain_matches`.

Denial rules and message templates (7.2 voice; `{label}` is `tool_label`; denial-id mechanics
unchanged):

- rule `capability`: message
  `'{label}' needs the '{missing}' capability on {host}, and grant '{grant_id}' allows {allowed}. Give this denial id to your administrator to request '{missing}' access.`
  where `{allowed}` is the comma-joined capability names or `no capabilities`, and `{host}`
  is `(unknown)` on the no-page path.
- rule `denied_domain`: message
  `{host} is excluded by grant '{grant_id}': your policy denies this site explicitly. Do not retry or work around this; ask the user or an administrator if access is needed.`
- rules `unmatched_domain`, `scheme/<scheme>`, `sacred`: keep existing texts verbatim.
- rule `access` and rule `tool/<name>` no longer exist anywhere.

### 4. Dispatch (`src/governance/dispatch.rs`) and server wiring

- `Governance` replaces `classify` with
  `requires: fn(&str, Option<&str>) -> Option<&'static [Capability]>` (constructor params on
  `all_open` and `governed` renamed accordingly; `src/main.rs` and
  `src/transport/mcp/server.rs` pass `browser::directory::requires`).
- `decide`: directory miss (`None`) -> deny with rule `unknown_action` (rename the existing
  classification-miss rule; keep its message text unless it names the old rule, adjusting
  minimally), still routed through the mode switch as today. `Some(&[])` -> immediate
  `Allow { grant_id: None }` without building a `DecisionRequest`. `Some(reqs)` -> build the
  request with `requires: reqs.to_vec()` and delegate.
- `LocalPdp::new` takes `browser::polarity::evaluate_host` at the composition root.
- Server (`handle_tools_call`): query the directory ONCE per call; when it returns
  `Some(&[])`, skip the governed tab-URL/resource resolution entirely (ADR Decision 5 step
  2) and let `decide` short-circuit. The audit `rw` value continues to come from
  `browser::classify::classify` in this task (s06 replaces it); `classify.rs` stays alive
  for that single purpose and its module docs gain one line saying so.

### 5. Advertisement (`src/browser/advertise.rs`)

Rewrite the filter per ADR Decision 8: with a manifest, a fixture tool is kept iff it has at
least one directory variant (for `computer`, any of its 13 action rows; for every other tool
its single row) whose `requires` is empty OR is a subset of ANY single grant's `allowed`.
No-manifest behavior (fixture verbatim, byte-identical) is unchanged. `grant_permits`,
`tool_list_permits`, and `access_class_permits` are deleted. Consequences to pin in tests:
a read-only grant now advertises everything except `form_input` and `javascript_tool`
(navigate and the requires-empty tools join the set); an empty-grants manifest advertises
exactly the tools with a requires-empty variant (`tabs_create_mcp`, `resize_window`,
`update_plan`, and `computer` via its `wait` row), in fixture order, not an empty list.

### 6. Explain (`src/governance/explain.rs`)

Grant rendering replaces the access/tools sentences with, in order:

1. `Allowed on {hosts}: {phrases}.` where `{phrases}` joins, comma-separated in this fixed
   order, one phrase per granted capability: read -> `read pages`; action ->
   `operate page controls (clicks and typing; this can trigger writes)`; write ->
   `submit forms and structured writes`; execute -> `run arbitrary JavaScript`. An empty
   `allowed` renders `Allowed on {hosts}: nothing (no capabilities granted).`
2. `{hosts}` renders from `hosts.allow`: comma-joined patterns; the single pattern `*`
   renders as `every site`; an empty allow list renders as `no sites`.
3. If `hosts.deny` is non-empty, append the sentence `Excluded: {deny list, comma-joined}.`
4. The per-grant mode sentences (g15/g16) are unchanged.

Warning lints: replace the write-without-read lint with: any of action/write/execute present
while read is absent ->
`grant '{id}': allowed includes acting capabilities without 'read'; agents can act on pages they cannot see.`
Keep every other template (header, identity, mode, settings, denial, user-config) verbatim.
Regenerate the three goldens by the g16 procedure (run the binary on each example, review
every line against the templates and the source manifest, then pin byte-for-byte) and update
`tests/fixtures/explain/*.txt` plus the inline sentence tests.

### 7. Simulate (`src/governance/simulate.rs`)

- `run_simulate(manifest_path, replay_path, domain_pattern_valid, requires_fn:
  fn(&str, Option<&str>) -> Option<&'static [Capability]>, evaluate_host: fn(&str, &[String],
  &[String]) -> HostRuleOutcome)`. The bucket table keeps its exact reasons; the
  classification step consults `requires_fn` (None cases map to the same three reasons:
  unknown tool / computer action missing / unknown action). Evaluable records call the new
  `check_call`. `main.rs` passes `browser::directory::requires` and
  `browser::polarity::evaluate_host`.
- Fixture manifests become schema 3:
  `manifest-permissive.json`: one grant `all-access`, hosts.allow the same four patterns,
  `allowed: ["read", "action", "write", "execute"]` (execute added deliberately: this
  fixture's purpose is the zero-denial path; note it in a JSON-adjacent comment is
  impossible, so note it in the ledger).
  `manifest-restrictive.json`: `docs-read` allow `["docs.example.com"]` allowed `["read"]`;
  `wiki-wildcard` allow `["*.wiki.example.org"]` allowed `["read","action","write"]`;
  `forms-noscript` allow `["forms.example.net"]` allowed `["read","action","write"]`.
- `audit.jsonl` is unchanged. New pinned expectations for `tests/policy_simulate.rs`:
  permissive -> `would allow: 9`, `would deny: 0`, `not evaluable: 4`, exit 0.
  restrictive -> `total actions: 13`, `would allow: 4` (read_page and navigate on
  docs.example.com now both allow: navigate requires read; wiki read_page; update_plan via
  requires-empty), `would deny: 5`, `not evaluable: 4`, exit 2, result line
  `result: 5 would-denies (exit 2)`, and exactly these group lines in this order:

      count=1 grant=- domain=unknown.example tool=read_page rule=unmatched_domain
      count=3 grant=docs-read domain=docs.example.com tool=computer rule=capability
      count=1 grant=forms-noscript domain=forms.example.net tool=javascript_tool rule=capability

  The four not-evaluable lines are unchanged.

### 8. Examples and templates

Rewrite all five `examples/*.json` to schema 3, preserving every id, description, mode,
identity, version, and config entry, translating grants per ADR Decision 6 (`read` ->
`["read"]`; `all` -> `["read","action","write"]`; `all` + `exclude_tools:
["javascript_tool"]` -> `["read","action","write"]` (the exclusion IS the missing execute);
`write` + `tools: ["form_input"]` -> `["write"]`; `domains` -> `hosts.allow` verbatim).
`execute` appears in NO example. Update `tests/manifest_validation.rs` assertions and
`src/governance/templates.rs` (drop its `test_is_known_tool` stub; keep the qa-staging
grant-order pin `["staging", "production-readonly"]`). `tests/policy_init.rs` needs no
logic change (byte-equality against the rewritten examples holds by construction).

### 9. Remaining test ripple

`tests/tool_enforcement.rs`, `tests/shadow_mode.rs`, `tests/tool_advertisement.rs`, and
every inline test constructing a `Grant` or embedded manifest: translate to schema 3 and the
new rules/messages. The denial-id determinism test keeps its shape (ids remain functions of
manifest hash + grant id + rule; the rule strings changed, so pinned literal ids in tests
are recomputed, never hand-edited to pass).

## Constraints

1. One atomic task, one commit; the tree is green at the end and at no intermediate commit.
2. `tests/architecture.rs` must pass: `Capability`, `HostRuleOutcome`, and `check_call` stay
   in `governance/`; the directory and polarity stay in `browser/`; only function pointers
   cross the boundary.
3. Do not touch `src/transport/mcp/schemas/tools.json` or `tests/tool_schema_fidelity.rs`.
4. All-open stays byte-identical: `tests/all_open_golden.rs` and `tests/mcp_protocol.rs`
   pass without modification.
5. Delete what you replace: `Access`, `tool_list_denial`, `access_covers`,
   `grant_permits`/`tool_list_permits`/`access_class_permits`, the `access`/`tool/<name>`
   rules, and every schema-2 fixture. `classify.rs` and `RwClass` survive ONLY for the audit
   `rw` field (s06 removes them).
6. ASCII only; no new dependencies.

## Tests (minimum)

- document.rs: schema-3 shape acceptance; schema-2 precise error including the ADR pointer;
  unknown fields rejected; invalid capability name rejected; duplicate capability rejected;
  bare `*` accepted in hosts and still rejected by config sacred-domain validation.
- enforcement.rs inline: requires-empty short-circuits before any grant walk (use a grants
  slice whose evaluation would deny); subset containment allow/deny per capability; the
  denied_domain attribution (first denying grant); unmatched vs denied precedence; the
  no-page union rule incl. zero grants; mode switch still shadows a capability deny;
  the exact pinned messages for `capability` and `denied_domain`.
- dispatch inline: directory miss -> `unknown_action` through the mode switch; requires-empty
  -> allow with no grant id.
- advertise inline: the two pinned consequence sets above, plus no-manifest verbatim.
- explain inline: the new sentence templates exact (each capability phrase, empty allowed,
  `*` -> `every site`, deny -> `Excluded:`), the acting-without-read lint exact; goldens.
- simulate: updated inline unit tests + the pinned integration expectations above.

## Verification

`cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test` all green;
`rg -n "Access::|exclude_tools|\"access\"|tool/" src/ tests/ examples/` returns only
historical docs/ledger references, no live code; ASCII scan on every touched file;
`git diff --stat` shows no change to tools.json or the fidelity test. Manual: `cargo run --
policy explain examples/enterprise-healthcare.json` renders the new sentences; `cargo run --
policy simulate tests/fixtures/simulate/manifest-restrictive.json --replay
tests/fixtures/simulate/audit.jsonl` prints the pinned report and exits 2.

## Out of scope

- The audit `rw` -> `capability` field change and deleting `classify.rs`/`RwClass` (s06).
- Any tools.json or fidelity-test change (s07).
- Documentation outside code comments and goldens (s08).
- Path rules, network-layer enforcement, capability qualifiers (ADR Future work).
- Manifest hot-reload (unchanged; still fixed at startup).
