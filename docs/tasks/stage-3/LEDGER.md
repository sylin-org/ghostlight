# Stage 3 ledger

Durable, context-wipe-safe record of stage-3 (capability model, ADR-0022) execution. This file
plus `docs/tasks/stage-2/BROWSER-TESTS.md` are the executor's memory. On every start, after any
interruption, and whenever state is unclear: read the RESUME HERE section first, then
`BOOTSTRAP.md` and ADR-0022, then the current task prompt, then continue. Never rely on
remembering earlier work; re-read files.

## RESUME HERE

- Branch: `stage-3` (created from `stage-2`; create it if absent). Never push, never merge,
  never commit to `main` or `stage-2`.
- Progress: `s01`, `s02`, `s03`, `s04`, `s05`, `s06`, `s07`, `s08` landed. Stage 3's task
  sequence is COMPLETE. See the RUN SUMMARY at the bottom of this file.
- NEXT TASK: none. Stage 3 is code-complete and docs-synced. Remaining work is human-driven:
  live-browser verification of the `docs/tasks/stage-2/BROWSER-TESTS.md` `s-live-1` through
  `s-live-4` entries (and the earlier `s01-1`/`s05-1`/`s07-1` entries), then a human decision
  on merging `stage-3`.
- Authority: ADR-0022 (`docs/adr/0022-intent-calibrated-capabilities.md`) over task prompts over
  the stage-2 shared-format doc (superseded in sections 4.3 / 6.1-rw / 8, now marked with
  `SUPERSEDED by ADR-0022` banners) over SPEC.
- Invariants after every task: tree green (`cargo test`, `clippy -D warnings`, `fmt --check`),
  `tests/architecture.rs` passing, all-open byte-identical (now 14 tools:
  `tests/all_open_golden.rs`/`tests/mcp_protocol.rs` updated in s07 to the sanctioned new
  count), the 13 trained tool schemas byte-identical plus the one sanctioned `explain` 14th
  (landed in s07; no other tools.json change ever), ASCII-only, no new dependencies,
  superseded code deleted in the task that supersedes it.

## Task log

(Append one entry per completed task, newest at the bottom. Shape:)

### <task-id> <title> -- <date>
- Commit: (see this task's commit)
- Files touched: <list>
- Summary: <what landed, key decisions, any conservative choice made>
- Deviations from the prompt/ADR: <numbered, each with reasoning; "none" if none>
- Verification: <clippy/fmt/test status; test counts before -> after; which suites unchanged>
- Browser checks queued: <count and ids appended to BROWSER-TESTS.md, or "none">

### s01 navigate is read -- 2026-07-03
- Commit: (see this task's commit, `fix(governance): s01 navigate is read`)
- Files touched: `src/browser/classify.rs`, `src/browser/advertise.rs`,
  `src/governance/enforcement.rs`, `src/governance/simulate.rs`,
  `src/transport/mcp/server.rs`, `tests/tool_advertisement.rs`, `tests/tool_enforcement.rs`,
  `tests/shadow_mode.rs`, `tests/policy_simulate.rs`, `docs/tasks/stage-2/BROWSER-TESTS.md`,
  `docs/tasks/stage-3/LEDGER.md`.
- Summary: flipped the single `("navigate", RwClass::Mutate)` row in
  `src/browser/classify.rs` to `RwClass::Observe` (navigate is provably a GET, per
  ADR-0022 Context/Decision 2), with the exact banner comment the prompt pinned. Updated
  every dependent expectation per the prompt's per-file instructions: the read-only
  advertisement fixture grows from 8 to 9 tools (navigate now included, in fixture order,
  in both `advertise.rs` and `tests/tool_advertisement.rs`); `tests/tool_enforcement.rs`'s
  mutate-on-read-grant example moved from `navigate` to the domain-less `tabs_create_mcp`
  union-rule path (a local `research-read` grant, not the shared
  `EXAMPLE_FULL_AND_RESEARCH_READ` constant, so the all-access `example-full` grant cannot
  mask the denial), and a new test `navigate_is_permitted_on_a_read_only_grant` pins the
  bugfix end to end (allow, `grant_id: research-read`, `rw: observe`, correct audit domain);
  `tests/shadow_mode.rs`'s would-deny call moved to `tabs_create_mcp` for the same reason;
  `src/transport/mcp/server.rs`'s two inline tests updated (`rw` expectation to
  `"observe"`; the shadow-deny pair's shared call switched to `tabs_create_mcp` with a
  matching fake-extension response of `"created"`); `tests/policy_simulate.rs`'s golden
  totals moved from 3/6/4 to 4/5/4 (13 total unchanged) and the now-stale
  `docs-read`/`navigate`/`access` group line was deleted, leaving three groups in the
  pinned order. Renamed stub-driven `"navigate"`+`RwClass::Mutate` pairings in
  `enforcement.rs` (11 call sites across 8 tests) to `"form_input"` (with the derived
  `tool/navigate` -> `tool/form_input` rule strings and the two
  `exclude_tools: ["navigate"]` -> `["form_input"]`), except the deliberate exception in
  `scheme_and_about_blank`, which keeps the `"navigate"` literal and flips its two
  `RwClass::Mutate` arguments to `RwClass::Observe` per the prompt. Did the equivalent
  truthfulness rename in `simulate.rs`'s `stub_classify` (navigate entry to `Observe`) and
  its three consuming tests (two `navigate` replay lines switched to `javascript_tool`,
  which was already `Mutate` in the stub, per the prompt's exact instruction);
  `totals_arithmetic_holds` left untouched as instructed (its navigate line now flips to
  allow, but the sum invariant it checks still holds).
- Deviations from the prompt/ADR: none. Every literal, table, rename, and test name
  transcribed as pinned by the prompt; no ADR/prompt conflict encountered.
- Verification: `cargo fmt` (reformatted 2 files, whitespace/wrapping only, re-verified
  with a full re-run of `cargo test` afterward -- unchanged pass count) then
  `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean; `cargo
  test` 430 -> 431 (one net new test, `navigate_is_permitted_on_a_read_only_grant`;
  `tests/tool_enforcement.rs` 7 -> 8), all passing, 0 failed. Confirmed byte-unchanged and
  green: `tests/architecture.rs` (4 tests), `tests/all_open_golden.rs` (3 tests),
  `tests/mcp_protocol.rs` (4 tests), `tests/tool_schema_fidelity.rs` (6 tests),
  `tests/audit_recorder.rs` (2 tests) -- none of these five files were touched by this
  task's diff (`git diff --stat` confirms). ASCII scan on every touched file (`rg -n
  "[^\x00-\x7F]" <files>`) printed nothing.
- Browser checks queued: 1 (`s01-1` appended to `docs/tasks/stage-2/BROWSER-TESTS.md`, the
  exact text pinned by the task prompt).

### s02 capability vocabulary in the governance core -- 2026-07-03
- Commit: (see this task's commit, `feat(governance): s02 capability vocabulary in the
  governance core`)
- Files touched: `src/governance/ports.rs`, `docs/tasks/stage-3/LEDGER.md`.
- Summary: added the ADR-0022 Decision 1 capability taxonomy as a pure, additive type in
  the governance core: the `Capability` enum (`Read`, `Action`, `Write`, `Execute`,
  `#[serde(rename_all = "lowercase")]`), its `as_str`/`from_name` helpers, and the
  free-standing `capability_subset(requires, allowed)` containment helper, inserted
  verbatim from the task prompt immediately after the `impl EffectiveMode` block and
  before `ToolId`, doc comments included. Nothing consumes the new type in this task
  (s05 wires it in); `RwClass` is untouched and stays the classification in force until
  s06. The diff is additive-only: every pre-existing line in `ports.rs` is byte-unchanged
  (`git diff --stat` and manual read confirm only inserted lines).
- Deviations from the prompt/ADR: none. The enum, helpers, and all three named tests were
  transcribed verbatim from the prompt; no ADR/prompt conflict encountered.
- Verification: `cargo fmt` (no changes beyond what was written) then `cargo fmt --check`
  clean; `cargo clippy --all-targets -- -D warnings` clean; `cargo test` 431 -> 434 (three
  net new tests: `capability_wire_names_round_trip`,
  `capability_from_name_rejects_unknown_and_case_variants`,
  `capability_subset_truth_table`, all in `src/governance/ports.rs`'s `mod tests`, which is
  part of the lib unit-test binary, 370 -> 373), all passing, 0 failed. Baseline of 431 was
  independently reconfirmed by stashing this task's diff and re-running the full suite
  before restoring it. Confirmed unchanged and green: `tests/architecture.rs` (4 tests),
  `tests/all_open_golden.rs` (3 tests), `tests/mcp_protocol.rs` (4 tests),
  `tests/tool_schema_fidelity.rs` (6 tests) -- none of these four files appear in this
  task's `git diff --stat` (only `src/governance/ports.rs` and this ledger changed). ASCII
  scan (`rg -n "[^\x00-\x7F]" src/governance/ports.rs`) printed nothing.
- Browser checks queued: none (a pure type addition; no BROWSER-TESTS.md entry, per the
  task prompt's Verification section).

### s03 the action directory in the browser plugin -- 2026-07-03
- Commit: (see this task's commit, `feat(governance): s03 action directory in the browser
  plugin`)
- Files touched: `src/browser/directory.rs` (new), `src/browser/mod.rs`,
  `docs/tasks/stage-3/LEDGER.md`.
- Summary: added `src/browser/directory.rs`, a new pure module holding the ADR-0022
  Decision 2 action directory as static data: the `ActionDescriptor` struct (`tool`,
  `action`, `requires`, `description`), the `DIRECTORY` const of 25 `ActionDescriptor`
  rows (12 tools + 13 `computer` actions, tools.json advertised order with `computer`
  expanded in place in tools.json `action` enum order), and the `requires(tool, action) ->
  Option<&'static [Capability]>` lookup function with the same lookup shape as
  `classify::classify` (action consulted only for `"computer"`; ignored otherwise). Every
  `requires` value and description string transcribed verbatim from the task prompt's
  table (itself a verbatim transcription of ADR-0022 Decision 2). Registered
  `pub mod directory;` in `src/browser/mod.rs` between `classify` and `pattern`
  (alphabetical) and inserted the exact pinned doc sentence about the s05/s06 switch
  immediately before the "It may depend on" sentence. Purely additive: `classify.rs` was
  not opened for editing and its byte content is unchanged (confirmed by `git status
  --short` showing no modification to it). Nothing in the tree consumes `directory` yet;
  enforcement, dispatch, advertisement, audit, simulate, and explain all still run on
  `classify.rs` per the task's Out-of-scope section.
- Deviations from the prompt/ADR: none. The struct shape, the 25 rows (tool, action,
  requires, description), the lookup function's absent-vs-empty semantics, the module
  registration point, and all four named tests were transcribed verbatim from the prompt;
  no ADR/prompt conflict encountered.
- Verification: `cargo fmt` (reformatted the new file's multi-line string literals only;
  re-ran `cargo test` afterward, unchanged pass count) then `cargo fmt --check` clean;
  `cargo clippy --all-targets -- -D warnings` clean; `cargo test` 434 -> 438 (four net new
  tests, all in `src/browser/directory.rs`'s `mod tests`: `directory_covers_the_sacred_
  surface_exactly`, `directory_requires_match_the_adr_table`,
  `absent_is_none_and_empty_is_some`, `every_description_is_nonempty_ascii_and_short`; lib
  unit-test binary 373 -> 377), all passing, 0 failed. Confirmed unchanged and green:
  `tests/architecture.rs` (4 tests), `tests/all_open_golden.rs` (3 tests),
  `tests/mcp_protocol.rs` (4 tests), `tests/tool_schema_fidelity.rs` (6 tests) -- `git diff
  --stat` shows only `src/browser/mod.rs` modified (7 insertions/2 deletions) plus the new
  untracked `src/browser/directory.rs`; none of the four guard files appear in the diff.
  ASCII scan (`rg -n "[^\x00-\x7F]" src/browser/directory.rs src/browser/mod.rs`) printed
  nothing.
- Browser checks queued: none (pure static data; nothing observable in a live browser
  yet, per the task prompt's Verification section).

### s04 host polarity evaluation in the browser plugin -- 2026-07-03
- Commit: (see this task's commit, `feat(governance): s04 host polarity evaluation in the
  browser plugin`)
- Files touched: `src/governance/ports.rs`, `src/browser/polarity.rs` (new),
  `src/browser/mod.rs`, `docs/tasks/stage-3/LEDGER.md`.
- Summary: added the ADR-0022 Decision 4 host-polarity evaluator as a pure, additive
  addition, split across the core and the browser plugin exactly as the prompt specified.
  `HostRuleOutcome` (`Allowed`/`Denied`/`Unmatched`, no serde derives, doc comment naming
  no `crate::browser` path) was inserted verbatim into `src/governance/ports.rs`
  immediately after `capability_subset` and before `ToolId` -- the prompt said "immediately
  after the `EffectiveMode` impl block and before `ToolId`", but s02 already occupies that
  exact span with the `Capability` enum/impl/`capability_subset`, so "before `ToolId`" (the
  literal, still-true half of the placement instruction) governed; see deviation 1. The new
  module `src/browser/polarity.rs` holds `is_valid_host_rule` (`pattern == "*" ||
  is_valid_pattern(pattern)`), `evaluate_host` (empty-allow short-circuits to `Unmatched`;
  otherwise computes best specificity per list via the pinned `specificity`/`rule_matches`/
  `best_specificity` helpers and applies the `(None,None)`/`(Some,None)`/`(None,Some)`/
  `(Some(a),Some(d)) if d>=a` decision table, tie-to-deny via `>=`), transcribed verbatim
  from the prompt's reference implementation. Registered `pub mod polarity;` in
  `src/browser/mod.rs` between `pattern` and `redact` and wove the pinned polarity clause
  into the module doc comment immediately after the `pattern` clause and before the
  `sacred` clause, rewrapping the whole paragraph to the file's existing line width by hand
  (rustfmt does not reflow doc comments by default). Nothing in the tree consumes
  `HostRuleOutcome`/`evaluate_host`/`is_valid_host_rule` yet (s05 wires them into manifest
  parsing and enforcement); `classify.rs`, `resource.rs`, `sacred.rs`, enforcement, and
  dispatch were not opened for editing.
- Deviations from the prompt/ADR: 1. The prompt's insertion point ("immediately after the
  `EffectiveMode` impl block and before `ToolId`") describes the pre-s02 tree; s02 already
  inserted `Capability`/`capability_subset` in that exact span. Per BOOTSTRAP.md's
  standing instruction to trust names/prose over stale line references, and since the ADR
  states no ordering requirement among ports.rs types, `HostRuleOutcome` was placed
  immediately after `capability_subset` and before `ToolId`, satisfying the literal,
  still-current half of the instruction ("before `ToolId`") without disturbing s02's
  already-landed insertion. No semantic effect; purely a documentation-order
  reconciliation, recorded here per BOOTSTRAP.md rule 4.
- Verification: `cargo fmt` (no changes beyond the hand-wrapped doc-comment paragraph,
  which rustfmt leaves alone since it does not reflow comments) then `cargo fmt --check`
  clean; `cargo clippy --all-targets -- -D warnings` clean; `cargo test` 438 -> 449 (eleven
  net new tests, all pinned by name in `src/browser/polarity.rs`'s `mod tests`: lib unit
  test binary 377 -> 388), all passing, 0 failed. Confirmed unchanged and green:
  `tests/architecture.rs` (4 tests), `tests/all_open_golden.rs` (3 tests),
  `tests/mcp_protocol.rs` (4 tests), `tests/tool_schema_fidelity.rs` (6 tests) -- `git
  status --short` shows only `src/governance/ports.rs`, `src/browser/mod.rs` modified plus
  the new untracked `src/browser/polarity.rs` (and this ledger); none of the four guard
  files appear in the diff. `rg -n "HostRuleOutcome|evaluate_host|is_valid_host_rule"
  src/` hits only `src/governance/ports.rs` and `src/browser/polarity.rs`, confirming the
  task is purely additive. ASCII scan (`rg -n "[^\x00-\x7F]" src/governance/ports.rs
  src/browser/polarity.rs src/browser/mod.rs`) printed nothing.
- Browser checks queued: none (a pure function; no BROWSER-TESTS.md entry, per the task
  prompt's Verification section).

### s05 the schema-3 switch -- 2026-07-03
- Commit: (see this task's commit, `feat(governance): s05 schema-3 switch (grants,
  enforcement, dispatch, advertisement, explain, simulate, examples)`)
- Files touched: `src/governance/manifest/document.rs`, `src/governance/ports.rs`,
  `src/governance/enforcement.rs`, `src/governance/dispatch.rs`,
  `src/browser/advertise.rs`, `src/browser/classify.rs`, `src/governance/explain.rs`,
  `src/governance/simulate.rs`, `src/governance/manifest/source.rs`,
  `src/governance/config/cli.rs`, `src/doctor.rs`, `src/main.rs`,
  `src/transport/mcp/server.rs`, `src/governance/templates.rs`,
  `examples/enterprise-healthcare.json`, `examples/developer-observe.json`,
  `examples/developer-unrestricted.json`, `examples/qa-staging.json`,
  `examples/research-read-only.json`, `tests/fixtures/simulate/manifest-permissive.json`,
  `tests/fixtures/simulate/manifest-restrictive.json`,
  `tests/fixtures/explain/enterprise-healthcare.txt`,
  `tests/fixtures/explain/qa-staging.txt`, `tests/fixtures/explain/research-read-only.txt`,
  `tests/manifest_validation.rs`, `tests/policy_simulate.rs`, `tests/tool_enforcement.rs`,
  `tests/shadow_mode.rs`, `tests/tool_advertisement.rs`, `tests/all_open_golden.rs`,
  `tests/audit_recorder.rs`, `docs/tasks/stage-2/BROWSER-TESTS.md`,
  `docs/tasks/stage-3/LEDGER.md`.
- Summary: replaced the whole schema-2 grant model (`domains`/`access`/`tools`/
  `exclude_tools`, evaluated over `RwClass`) with the ADR-0022 schema-3 model (`hosts`
  allow/deny polarity + `allowed` capability sets, evaluated by requirement-subset
  containment) in one atomic task, per ADR-0022 Decisions 3-6 and 8.
  `manifest/document.rs`: new `HostRules`/`Grant` shape, `Access` deleted, schema gate
  changed to `== 3` with the ADR-0022 migration sentence appended when the found value is
  exactly `2`; `parse_manifest` lost the `is_known_tool` parameter entirely (removed
  through the whole call chain: `manifest/source.rs`, `explain.rs`, `simulate.rs`,
  `config/cli.rs`, `doctor.rs`, `main.rs`, and every test). `governance/ports.rs`:
  `DecisionRequest.rw: RwClass` replaced with `requires: Vec<Capability>`; `Capability`
  gained `#[derive(Hash)]` (needed for the manifest's duplicate-capability check).
  `enforcement.rs::check_call` rewritten to the ADR Decision 5 algorithm: an empty
  `requires` short-circuits to `Allow` before any resource matching; grant resolution
  walks host polarity via the injected `evaluate_host` fn (first `Allowed` wins, first
  `Denied` remembered for `denied_domain` attribution); capability check is subset
  containment. New rules `capability` and `denied_domain` (with the pinned message
  templates, `Denied ({id}): ...` prefix kept for voice consistency with every other
  rule); `unknown_action` renames the old classification-miss rule (was `tool/<name>`);
  `access`/`tool/<name>` deleted along with `tool_list_denial`/`access_covers`.
  `dispatch.rs::Governance` gained a `requires: fn(&str, Option<&str>) ->
  Option<&'static [Capability]>` field ALONGSIDE the pre-existing `classify` field
  (kept solely for the audit `rw` value per the task's own instruction that `classify.rs`
  survives for that single purpose until s06) -- see deviation 1. `decide()`: a directory
  miss denies via `unknown_action` through the same mode switch; `Some(&[])` allows
  immediately with no grant id and no `DecisionRequest` built; `Some(reqs)` builds the
  request and delegates. `browser/advertise.rs` rewritten to the ADR Decision 8 rule (a
  tool is kept iff it has a directory variant that is `requires: []` or a subset of any
  single grant's `allowed`); `grant_permits`/`tool_list_permits`/`access_class_permits`
  deleted. `explain.rs`: grant rendering replaced with the `Allowed on {hosts}: {phrases}.`
  sentence (fixed read/action/write/execute phrase order, the mandated action-can-cause-
  writes wording), the `Excluded: ...` sentence for non-empty `hosts.deny`, and the
  acting-without-read warning lint; three goldens regenerated by running the real binary
  on each example and reviewing every line against the templates before pinning.
  `simulate.rs` consults `requires_fn`/`evaluate_host` instead of `classify`/
  `domain_matches`; its two fixture manifests rewritten to schema 3 exactly as pinned
  (permissive's `all-access` grant deliberately includes `execute`, per the prompt, to
  keep the zero-denial-path purpose of that fixture). All five `examples/*.json`
  rewritten to schema 3 per the ADR Decision 6 translation table (`all` ->
  `["read","action","write"]`; `all`+`exclude_tools:[javascript_tool]` -> the same, since
  the exclusion IS the missing execute; `write`+`tools:[form_input]` -> `["write"]`;
  `read` -> `["read"]`); `execute` appears in no example. `templates.rs` dropped its
  `test_is_known_tool` stub; the qa-staging grant-order pin is untouched.
  `transport/mcp/server.rs` wires `browser::polarity::evaluate_host` into `LocalPdp::new`
  and `browser::directory::requires` into both `Governance` constructors; its own inline
  tests' `Grant` literals and the mutate-on-read-only-grant scenario were translated to
  schema 3 (see deviation 4). `classify.rs` gained the one required doc line (and a
  refreshed module doc / doc-comment) stating it now survives ONLY for the audit `rw`
  field until s06 deletes it.
- Deviations from the prompt/ADR: 1. `Governance`'s constructor signature adds a new
  `requires` parameter ALONGSIDE the existing `classify` parameter, rather than literally
  replacing it: the prompt says "`Governance` replaces `classify` with `requires`" but
  also says, in the very same section, that "the audit `rw` value continues to come from
  `browser::classify::classify` in this task" -- and `build_record` (which computes that
  `rw` value) lives inside `Governance` in `governance/dispatch.rs`, which cannot call
  `crate::browser::classify::classify` directly (the a7 arch-test forbids a `governance ->
  browser` edge). The only way to honor both sentences is for `Governance` to hold both
  function pointers: `requires` feeds `decide()` (the literal "replaces" for the DECISION
  path), `classify` still feeds `build_record` unchanged. No behavior change; recorded per
  BOOTSTRAP.md rule 4. 2. `parse_manifest`'s single `domain_pattern_valid` parameter
  stays the STRICT checker (`browser::pattern::is_valid_pattern`, unchanged from
  schema-2's wiring at every call site) rather than becoming `browser::polarity::
  is_valid_host_rule`: since the identical parameter is also used for
  `content.security.sacred_domains` config validation, and the ADR requires bare `*` to
  be legal in grant `hosts` but NEVER in sacred domains, one shared injected checker
  cannot itself decide the carve-out. `document.rs` now applies the `pattern == "*" ||
  domain_pattern_valid(pattern)` carve-out INLINE, only at the two `hosts.allow`/
  `hosts.deny` validation call sites (mirroring `is_valid_host_rule`'s own shape without
  crossing the arch boundary to call it); `validate_config_entry` calls
  `domain_pattern_valid` directly, with no carve-out, so `*` is still rejected there. A
  new test (`config_entry_star_pattern_is_rejected_even_though_hosts_accept_it`) pins
  this. No caller needed to change which function it passes for manifest loading (only
  `is_known_tool` was removed from every call site) -- a smaller ripple than injecting a
  second checker parameter would have required, and the pinned three-argument
  `parse_manifest(text, source_label, domain_pattern_valid)` signature is honored
  literally. 3. The `capability` and `denied_domain` denial message templates, as quoted
  literally in the prompt, do not show a leading `Denied ({denial_id}): ` prefix; every
  other denial rule (old and new) in this codebase, and shared-format section 7.2's own
  voice, always leads with that prefix. Implemented WITH the prefix for consistency (the
  prompt's quoted text is read as omitting shared boilerplate, not specifying a divergent
  voice for exactly these two rules); pinned by
  `capability_denial_message_is_exact`/`denied_domain_message_is_exact`. 4. Two
  integration-test scenarios (`tests/tool_enforcement.rs` test 3 and 6,
  `tests/shadow_mode.rs`'s would-deny call) previously used `tabs_create_mcp` as a
  "domain-less, would-deny" example (s01's own choice, made under the pre-ADR-0022 rw
  model). Under ADR-0022 `tabs_create_mcp` requires `[]` and short-circuits to Allow
  unconditionally -- it, `update_plan`, and `resize_window` are now the ONLY tools that
  can never demonstrate a would-deny at all. Substituted `tabs_context_mcp` (the sole
  domain-less tool with a non-empty capability requirement, `read`) under a grant that
  permits `action`/`write` but not `read`, preserving each test's original intent (a
  domain-less capability denial via the union rule); the prompt's own Required Behavior
  section 9 left this translation to judgment ("translate to schema 3 and the new
  rules/messages") without pinning specific tool choices. 5. `tests/all_open_golden.rs`
  and `tests/audit_recorder.rs` needed a compile-only change (threading the new
  `requires` parameter through `Governance::all_open` call sites) to build against the
  changed function signature; no assertion, golden text, or observable behavior changed.
  Read per BOOTSTRAP.md rule 4: the constraint that these files "pass without
  modification" describes the all-open BEHAVIORAL invariant (nothing observable changes),
  not a literal zero-byte-diff rule that would leave the tree uncompilable -- the
  alternative reading is incoherent given the function signature genuinely changed. 6.
  `tests/policy_simulate.rs::structurally_invalid_manifest_exits_one` used a schema-2
  `exclude_tools` field to construct a structurally-invalid manifest; rewritten to a
  schema-3 manifest with an invalid capability name (`"mutate"`) so the test keeps
  proving the same invariant (a manifest failing validation always exits 1) with a
  schema-3-relevant defect rather than a now-nonexistent field.
- Verification: `cargo fmt` (reformatted after each of several passes; final pass clean)
  then `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean;
  `cargo test` 449 -> 457 (net 8 new tests: 2 in `document.rs`
  [`bare_star_is_accepted_in_hosts_allow_and_deny`,
  `config_entry_star_pattern_is_rejected_even_though_hosts_accept_it`, plus the schema-3
  rewrite of every existing case], several in `enforcement.rs`'s full rewrite pinning
  every named required test, 2 new in `dispatch.rs`
  [`directory_miss_denies_via_unknown_action_through_the_mode_switch`,
  `requires_empty_allows_without_consulting_the_pdp`], 2 new in `advertise.rs`'s rewrite,
  several new in `explain.rs`'s rewrite; exact per-file deltas: `governance::ports`
  unchanged test count with updated bodies, `governance::manifest::document` unit tests
  grew by 2 net after replacing every schema-2 case 1:1, `governance::enforcement` grew
  from 16 to 20 named tests, `governance::dispatch` grew by 2, `browser::advertise` net
  count unchanged (2 replaced by 2 equivalent ADR-0022 cases plus 2 new), `governance::
  explain` net count unchanged (like-for-like translation) plus 1 new acting-without-read
  test replacing the old bare-write test 1:1), all passing, 0 failed. Confirmed unchanged
  and still green: `tests/architecture.rs` (4 tests, byte-identical file, confirmed via
  `git diff --stat`), `tests/mcp_protocol.rs` (4 tests, byte-identical file),
  `tests/tool_schema_fidelity.rs` (6 tests, byte-identical file); `src/transport/mcp/
  schemas/tools.json` byte-identical (`git diff --stat` shows no change to either sacred
  file). `tests/all_open_golden.rs` (3 tests) still green with the one unavoidable
  signature-only edit (deviation 5). Manual verification: `cargo run -- policy explain
  examples/enterprise-healthcare.json` renders the new capability sentences (reviewed by
  eye before pinning the golden); `cargo run -- policy simulate tests/fixtures/simulate/
  manifest-restrictive.json --replay tests/fixtures/simulate/audit.jsonl` prints exactly
  the pinned report (would allow: 4, would deny: 5, not evaluable: 4, the three pinned
  group lines in the pinned order, `result: 5 would-denies (exit 2)`) and exits 2.
  `rg -n "Access::|exclude_tools|\"access\"|tool/" src/ tests/ examples/` returns only
  historical doc-comment/string-literal references (module docs describing what was
  removed, and test literals proving the old fields are now rejected as unknown), no live
  code path. ASCII scan (`rg -n "[^\x00-\x7F]" <every touched file>`) printed nothing on
  every file this task touched.
- Browser checks queued: 1 (`s05-1` appended to `docs/tasks/stage-2/BROWSER-TESTS.md`:
  schema-3 capability grants end to end -- `policy explain` wording, the read-only
  advertised-tool-list consequence, a permitted read-class sequence, and the exact
  `capability` denial wording for both a `computer left_click` and a direct `form_input`
  call).

### s06 audit capability field; delete classify.rs and RwClass -- 2026-07-03
- Commit: (see this task's commit, `refactor(governance): s06 audit capability field;
  delete classify.rs and RwClass`)
- Files touched: `src/browser/classify.rs` (deleted), `src/browser/directory.rs`,
  `src/browser/mod.rs`, `src/governance/audit/mod.rs`, `src/governance/dispatch.rs`,
  `src/governance/ports.rs`, `src/governance/simulate.rs`, `src/transport/mcp/server.rs`,
  `tests/all_open_golden.rs`, `tests/audit_recorder.rs`, `tests/shadow_mode.rs`,
  `tests/tool_enforcement.rs`, `docs/tasks/stage-3/LEDGER.md`.
- Summary: finished the ADR-0022 Decision 8 audit switch and deleted the last remnant of
  the observe/mutate model. `ports.rs`: deleted the `RwClass` enum and impl entirely;
  replaced `AuditRecord.rw: RwClass` with `pub capability: &'static str` in the same
  position (between `action` and `domain`), doc comment transcribed verbatim from the
  prompt; replaced `DomainPolicy::classify` with
  `fn requires(&self, tool: &str, action: Option<&str>) -> Option<&'static [Capability]>`
  (the trait has no impl anywhere in the tree, a shape-only change); `Capability::as_str`
  already existed from s02, so no new method was needed. `dispatch.rs`: removed the
  `classify: fn(...) -> Option<RwClass>` field s05 had retained on `Governance` (the
  deviation-1 second fn pointer the ledger flagged at the time) -- `Governance` now holds
  exactly one browser-supplied fn pointer, `requires`, used only by `decide`; every public
  record function (`record_call`, `record_deny`, `record_navigate_landing_deny`,
  `record_shadow_deny`, `record_held`) gained a `requires: &[Capability]` parameter
  immediately after `action`, threaded into the now-private `build_record`, which derives
  `capability = requires.first().map(Capability::as_str).unwrap_or("none")` -- the single
  derivation point the prompt required. `server.rs`: added the one per-call action-directory
  lookup (`directory::requires(name, action).unwrap_or(&[])`) immediately after `action` is
  computed and before the held-call early return, so the server performs exactly one lookup
  per call and passes it (or `&[]` on a directory miss) to every `record_*` call site;
  removed `classify` from the `use crate::browser::{...}` import and both `Governance`
  constructor call sites (production and every inline test). Deleted `src/browser/
  classify.rs` in full (its own 5-test module went with it: `tool_table_matches_the_
  sacred_surface`, `computer_action_table_matches_the_sacred_enum`, `classification_
  matches_the_shared_format_table`, `unclassified_inputs_return_none`, `rw_class_
  strings_match_the_audit_vocabulary`); removed `pub mod classify;` from `browser/mod.rs`
  and rewrote its module doc to name `directory` as the sole enforcement/advertisement/audit
  authority; `directory.rs`'s own header dropped its "additive alongside classify" framing.
  `audit/mod.rs`'s `sample_record` test helper's third parameter became
  `capability: &'static str`, `"read"` passed at every existing call site.
  `simulate.rs::evaluate_line`'s doc comment reworded to name `capability` (and note that
  old rw-era lines replay identically) instead of the recorded `rw` value nobody reads.
- Deviations from the prompt/ADR: 1. Two pre-existing doc-comment lines outside this task's
  named files used the bare word "classify" as a verb (`ports.rs`'s `Capability` doc: "
  Capabilities classify an operation by..."; landed in s02) or would have if worded
  naively (`directory.rs`/`mod.rs`'s new prose about the deleted module). The task's own
  Verification section pins `rg -n "classify" src/ tests/` to return only the two
  `src/install/` hits; reworded all three to avoid the substring entirely ("categorize" in
  `ports.rs`, dropping the literal filename/module name in `directory.rs`/`mod.rs`) rather
  than leave the verification command failing on a pre-existing or newly-written prose hit.
  No semantic change; recorded per BOOTSTRAP.md rule 4. 2. Two doc-comment lines grew past
  100 columns when `rw` was replaced with the longer `capability` inside an inline
  backtick-field list (`ports.rs`'s `SessionEventRecord` doc, `dispatch.rs`'s
  `record_session_killed` doc); rewrapped by hand to match the file's existing line width,
  since `rustfmt` does not reflow doc comments. No other file in this task's diff needed
  the same treatment (confirmed by comparing per-file long-line counts against each file's
  pre-task baseline). 3. The prompt's Tests section for `tests/tool_enforcement.rs` says
  "if s05 changed the driven tool, use its ADR Decision 2 table value" for the
  allow/deny-line capability assertion; s05 did not change the driven tool (both lines are
  still `navigate`, per the pre-existing test body), so both assertions are pinned to
  `"read"` (navigate's Decision-2 value) with no further translation needed.
- Verification: `cargo fmt` (reformatted the two doc comments from deviation 2; re-ran
  `cargo test` afterward, unchanged pass count) then `cargo fmt --check` clean; `cargo
  clippy --all-targets -- -D warnings` clean; `cargo test` baseline independently
  reconfirmed at 457 by stashing this task's diff and re-running the full suite (matches
  the s05 ledger entry's own figure), 457 -> 454 after this task, all passing, 0 failed.
  The net -3 is fully accounted for: -5 from deleting `classify.rs`'s own test module, -1
  from deleting `classification_miss_records_mutate` (its premise, the internal `self.
  classify` derivation, no longer exists), +2 new named tests in `dispatch.rs`
  (`requires_empty_records_capability_none`, `deny_record_carries_the_capability_of_the_
  denied_call`), +1 new named test in `tests/tool_enforcement.rs`
  (`requires_empty_call_records_capability_none`); every rename (`computer_action_
  classification_flows_into_rw` -> `computer_action_requires_flows_into_capability`,
  `rw_and_mode_wire_names_are_lowercase` -> `mode_wire_names_are_lowercase`,
  `computer_call_records_action_and_observe_class` -> `computer_call_records_action_and_
  read_capability`) is a 1:1 delete+add, net zero. Confirmed unchanged and still green:
  `tests/architecture.rs` (4 tests, byte-identical file), `tests/all_open_golden.rs` (3
  tests, only its `no_classification`/`no_requires` test helpers and one constructor call
  site changed to compile against the new signature -- no assertion or golden text
  changed), `tests/mcp_protocol.rs` (4 tests, byte-identical file),
  `tests/tool_schema_fidelity.rs` (6 tests, byte-identical file); `git diff --stat` against
  `src/transport/mcp/schemas/tools.json`, `tests/tool_schema_fidelity.rs`, `tests/
  fixtures/simulate/audit.jsonl`, `examples/`, `src/governance/templates.rs`, and `tests/
  fixtures/explain/` shows no output (none touched). `rg -n "RwClass" src/ tests/` -> no
  output; `rg -n "classify" src/ tests/` -> only the two `src/install/` prose hits
  (unchanged from before this task); `rg -n "\"rw\"" src/ tests/ --glob '!tests/
  fixtures/**'` -> no output (the `tests/fixtures/simulate/audit.jsonl` rw-era fixture is
  deliberately untouched). ASCII scan (`rg -n "[^\x00-\x7F]" <every touched file>`) printed
  nothing on every file this task touched. Manual: `cargo run -- policy simulate tests/
  fixtures/simulate/manifest-restrictive.json --replay tests/fixtures/simulate/audit.jsonl`
  still prints the exact s05-pinned report (would allow: 4, would deny: 5, not evaluable:
  4, the three pinned group lines, `result: 5 would-denies (exit 2)`) and exits 2,
  confirming the old rw-era replay fixture remains replayable after the field rename.
- Browser checks queued: none (this task writes only code doc comments; BROWSER-TESTS.md
  entries are s08's job per the prompt's Out of scope section).

### s07 explain directory tool -- 2026-07-03
- Commit: (see this task's commit, `feat(governance): s07 explain directory tool`)
- Files touched: `src/transport/mcp/schemas/tools.json`, `src/browser/directory.rs`,
  `src/browser/advertise.rs`, `src/transport/mcp/server.rs`,
  `tests/tool_schema_fidelity.rs`, `tests/all_open_golden.rs`, `tests/mcp_protocol.rs`,
  `tests/tool_advertisement.rs`, `tests/tool_enforcement.rs`,
  `docs/tasks/stage-2/BROWSER-TESTS.md`, `docs/tasks/stage-3/LEDGER.md`.
- Summary: landed the ONE sanctioned addition to the sacred tool surface (ADR-0022
  Decision 7). `tools.json` gained one object at the tail (name `explain`, the pinned
  description string verbatim, `inputSchema` byte-identical to `tabs_create_mcp`'s
  no-argument shape); the first 13 entries are untouched (`git diff` shows only the tail
  addition, confirmed). `directory.rs` gained row 26 (`tool: "explain", action: None,
  requires: &[], description: "Show every action available here and the capability each
  one requires."`) at the end of `DIRECTORY`, plus a new pure formatter,
  `pub fn explain_text() -> String`, that renders the capability-vocabulary paragraph, a
  blank line, then one line per `DIRECTORY` row in fixture order (`{tool}: requires
  {cap|nothing}. {description}` / `computer ({action}): requires {cap|nothing}.
  {description}`) -- the single source `server.rs`'s handler and the pinned test both
  consume. `directory.rs`'s fixture-mirror tests were updated in place (25 -> 26 rows,
  `explain` added to the ADR-table test and the absent/empty test) plus one new
  structural test for `explain_text`'s shape. `server.rs::handle_tools_call` gained a
  dedicated `if name == "explain"` branch, positioned right after `dispatch_started` is
  taken and BEFORE the sacred-domains check and grant machinery (the hold check above it
  still applies to `explain` like any tool, per the prompt): it builds the text via
  `directory::explain_text()`, computes a real (not hardcoded) `duration_ms` from
  `dispatch_started.elapsed()`, calls `governance.record_call("explain", None, requires,
  duration_ms, None, None)` (an ordinary allow record, `capability` derives to `"none"`
  since `requires` is empty), and returns the text content directly -- the extension is
  never touched, so `explain` produces zero native-messaging frames. No change was
  needed to `resolve_governing_resource`, `governance::decide`, or the sacred check:
  `explain` never reaches any of them. Two tests pin the handler: a structural
  round-trip (`pinned_explain_text_matches_the_real_directory_formatter`) tying a
  hand-transcribed literal to the real formatter so they can never silently drift apart,
  and an end-to-end unit test
  (`explain_returns_the_pinned_text_and_is_audited_as_allow_none`) that calls
  `handle_tools_call` with NO extension attached at all and asserts the exact text plus
  the audit record's `capability: "none"`, `decision: "allow"`, `domain: null`, and a
  present `duration_ms`. The existing hold test
  (`held_call_returns_the_pause_text_before_the_not_connected_error`) was extended with
  an `explain`-while-held case proving the pause text still wins. `advertise.rs` needed
  no production-code change (the existing requires-empty rule already keeps any
  `requires: []` tool advertised under every posture); its own two exhaustive unit tests
  and their doc comments were extended to include `explain` in the expected lists.
  `tests/tool_schema_fidelity.rs` was amended ONCE, as sanctioned: `EXPECTED` (13) split
  into `EXPECTED_TRAINED` (the same 13, unchanged) plus a new
  `explain_tool_object_matches_the_pinned_adr_0022_decision_7_shape` test asserting
  `explain`'s exact description, its inputSchema's byte-identity with
  `tabs_create_mcp`'s, and that no other tool was added; the renamed
  `advertises_exactly_the_thirteen_trained_tools_plus_explain_positioned_last` test
  checks the 13-then-explain-last shape; a module-doc comment states the 13-plus-1
  invariant and marks the file/tools.json pairing as never touched again outside s07.
  `tests/all_open_golden.rs`, `tests/mcp_protocol.rs`, and
  `tests/tool_enforcement.rs::all_open_invariant_no_manifest_means_no_denials` had their
  literal "13 tools" counts and name arrays updated to 14 with `explain` appended
  (`all_open_golden.rs`'s byte-identity and `is_known_tool`/decide-loop assertions still
  pass unchanged in shape, just over the longer list); `tests/tool_advertisement.rs`'s
  two exact-name-list assertions (read-only and empty-grants manifests) gained `explain`
  at the tail, matching Required Behavior section 5. `tests/mcp_protocol.rs` also gained
  a new dedicated integration test,
  `explain_is_advertised_last_and_answers_with_no_extension_attached`, spawning the real
  binary with no manifest and no extension, proving `explain` is advertised last and
  answers instantly with the directory text.
- Deviations from the prompt/ADR: none. Every literal (the tools.json description
  string, the directory row, the response layout, the fidelity-test invariant) was
  transcribed from the prompt/ADR; the handler's placement (after the hold check,
  before the sacred check and grant machinery) matches the prompt's stated ordering
  exactly; no ADR/prompt conflict encountered.
- Verification: `cargo fmt` (reformatted a handful of assertions' line-wrapping across
  the touched files, whitespace only; re-ran `cargo test` afterward, unchanged pass
  count) then `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings`
  clean; `cargo test` 454 -> 459 (net 5 new tests: `directory.rs`'s
  `explain_text_is_the_vocabulary_block_then_one_line_per_row` [lib +1],
  `server.rs`'s `pinned_explain_text_matches_the_real_directory_formatter` and
  `explain_returns_the_pinned_text_and_is_audited_as_allow_none` [lib +2, so lib unit
  tests 392 -> 395], `tests/tool_schema_fidelity.rs` net +1 (one test renamed/reworded,
  one new: 6 -> 7), `tests/mcp_protocol.rs` +1 (4 -> 5)), all passing, 0 failed. Baseline
  of 454 independently reconfirmed before starting (summed every per-binary `test
  result:` line). Confirmed unchanged and green throughout:
  `tests/architecture.rs` (4 tests, byte-identical file; the handler lives in
  `transport`, the directory row in `browser`, nothing new in `governance` -- confirmed
  by `git status --short`, which shows no `src/governance/` file in this task's diff).
  `git diff src/transport/mcp/schemas/tools.json` shows ONLY the tail addition (the
  first 13 entries byte-identical, confirmed by inspection). ASCII scan (`rg -n
  "[^\x00-\x7F]" <every touched file>`) printed nothing on every file this task touched,
  including the new `docs/tasks/stage-2/BROWSER-TESTS.md` entry.
- Browser checks queued: 1 (`s07-1` appended to `docs/tasks/stage-2/BROWSER-TESTS.md`:
  `explain` appears in a live client's tool list and returns the directory text with
  zero native-messaging frames, plus a live-session watch for spurious `explain`
  invocation on ordinary "explain this page" style requests per ADR-0022 Decision 7's
  accepted risk).

### s08 documentation sync -- 2026-07-03
- Commit: (see this task's commit, `docs(governance): s08 documentation sync`)
- Files touched: `docs/tasks/stage-2/00-shared-format.md`, `CLAUDE.md`,
  `docs/tasks/stage-2/BROWSER-TESTS.md`, `docs/tasks/stage-3/LEDGER.md`.
  `docs/adr/README.md` was NOT touched: it already listed the ADR-0022 row (verified via
  `rg -c "0022-intent-calibrated-capabilities" docs/adr/README.md` -> `1`), so Required
  behavior 5 was the expected no-op.
- Summary: docs-only task, the last of the stage-3 sequence. Inserted the three pinned
  `SUPERSEDED by ADR-0022` banner paragraphs verbatim, immediately after the
  `### 4.3. Grants`, `### 6.1. Fields`, and `## 8. Read/write classification table`
  headings respectively, as pure insertions (no historical text rewritten or deleted).
  Appended the four pinned SPEC-updates-needed items (14-17) verbatim after the
  pre-existing item 13 in `## 10. SPEC updates needed`. Applied the four pinned surgical
  edits to repo-root `CLAUDE.md` (each fragment/paragraph/bullet/sentence appeared
  exactly once, confirmed before editing):
  Edit A (Project Identity) replaced `identity-bound access control, tool-level r/w
  classification, and structured audit logging` with `identity-bound access control,
  per-action capability classification (read, action, write, execute), and structured
  audit logging`. Edit B (Origin) replaced the whole `**Critical constraint:**`
  paragraph with the pinned 13-trained-plus-one-sanctioned-`explain` wording. Edit C
  (Phase 4) replaced the bullet `Implement computer sub-action classification (observe
  vs mutate).` with the per-action capability requirements wording. Edit D (Tool Schema
  Preservation) appended the pinned sentence about the one sanctioned `explain`
  exception to the paragraph ending "exact schema matching." CLAUDE.md's preexisting
  non-ASCII (section signs, box-drawing tree characters) was left untouched, per the
  prompt's explicit carve-out; only the four edited fragments/sentences were checked for
  ASCII-cleanliness. Appended the four pinned `s-live-1` through `s-live-4` entries to
  `docs/tasks/stage-2/BROWSER-TESTS.md` after the existing last entry (`s07-1`), verbatim
  from the prompt, in the file's own `Changed:`/`Steps:`/`Expect:` format, without
  touching or reordering any prior entry.
- Deviations from the prompt/ADR: none. Every banner, list item, CLAUDE.md edit, and
  BROWSER-TESTS.md entry was transcribed verbatim from the prompt; the ADR-0022-README
  check (Required behavior 5) was confirmed a no-op exactly as the prompt predicted; no
  ADR/prompt conflict encountered.
- Verification: `cargo fmt --check` clean (no Rust source touched); `cargo clippy
  --all-targets -- -D warnings` clean; `cargo test` 459 -> 459 (docs-only change,
  identical to the s07 run: summed every per-binary `test result:` line before and after
  this task's edits, both totals 459, 0 failed). All six pinned `rg` assertions from the
  prompt's Tests section passed: `rg -c "SUPERSEDED by ADR-0022"
  docs/tasks/stage-2/00-shared-format.md` -> `3`; `rg -n "observe vs mutate" CLAUDE.md`
  -> no output; `rg -n "tool-level r/w classification" CLAUDE.md` -> no output; `rg -c
  "^## s-live-" docs/tasks/stage-2/BROWSER-TESTS.md` -> `4`; `rg -c
  "0022-intent-calibrated-capabilities" docs/adr/README.md` -> `1`; `rg -n "^17\\."
  docs/tasks/stage-2/00-shared-format.md` -> exactly one line (item 17). `git status
  --short` before committing showed exactly the three edited files plus this ledger (no
  `docs/adr/README.md`, matching Constraint 2's "only if the row is missing" clause).
  ASCII scan of added lines only (`git diff -U0 -- docs/tasks/stage-2/00-shared-format.md
  CLAUDE.md docs/tasks/stage-2/BROWSER-TESTS.md docs/adr/README.md
  docs/tasks/stage-3/LEDGER.md | grep "^+" | rg -n "[^\x00-\x7F]"`) produced no output.
- Browser checks queued: 4 (`s-live-1` through `s-live-4` appended to
  `docs/tasks/stage-2/BROWSER-TESTS.md`, exact text pinned by the task prompt: read-grant
  enforcement end to end, `denied_domain` with an allow-`*`-plus-deny carve-out, the
  `explain` tool live including a spurious-invocation watch, and the audit `capability`
  field in a real JSONL file).

## RUN SUMMARY

Stage 3 (ADR-0022, intent-calibrated capabilities) is code-complete and documentation-synced
as of this commit. Tasks completed, in order: `s01` navigate is read; `s02` capability
vocabulary in the governance core; `s03` the action directory in the browser plugin; `s04`
host polarity evaluation in the browser plugin; `s05` the schema-3 switch (manifest grants,
enforcement, dispatch, advertisement, explain, simulate, examples, templates); `s06` audit
`capability` field, deletion of `classify.rs` and `RwClass`; `s07` the `explain` directory
tool (the one sanctioned tools.json addition); `s08` documentation sync. Commit range: 9
commits on branch `stage-3`, branched from `stage-2` at `1f22126` (`feat(governance): g18
presets and templates`): `b0a1164` (docs: ADR-0022 + stage-3 task batch setup), `2074786`
(s01), `15ef9fa` (s02), `7f9a54e` (s03), `bb5fdc2` (s04), `8ed5f82` (s05), `2215b02` (s06),
`f977c34` (s07), `0c829d5` (s08, docs sync -- the last commit before this RUN SUMMARY entry
was added). Final `cargo test` total: 459 passed, 0 failed (baseline before any stage-3
task: 430).

Every conservative choice made across the run is recorded as a numbered deviation in that
task's own log entry above; summarized together here by task: `s01`, `s02`, `s03`, `s07`,
`s08` -- none (every literal, table, rename, and test name transcribed verbatim from the
prompt/ADR with no conflict). `s04` -- one purely cosmetic reconciliation: the prompt's
insertion-point instruction ("immediately after the `EffectiveMode` impl block") described
the pre-s02 tree; s02 had already claimed that span, so `HostRuleOutcome` was placed after
`capability_subset` instead (still "before `ToolId`," the still-current half of the
instruction), with no semantic effect. `s05` -- six recorded choices, all read-preserving:
(1) `Governance` kept both `classify` and the new `requires` fn pointers instead of a
literal replacement, because `build_record`'s `rw` derivation cannot cross the
`governance -> browser` architecture boundary the arch test forbids; (2) the manifest's
domain-pattern validator stayed the strict checker with an inline `pattern == "*"` carve-out
at the two `hosts` call sites only, keeping sacred-domain config still rejecting bare `*`;
(3) the `capability`/`denied_domain` denial templates were implemented WITH the
`Denied ({id}): ` prefix for voice consistency with every other denial rule, reading the
prompt's quoted text as omitting shared boilerplate rather than specifying a divergent
voice; (4) two would-deny integration-test scenarios were moved from `tabs_create_mcp` (now
a `requires: []` tool that can never deny under ADR-0022) to `tabs_context_mcp`, preserving
each test's original intent; (5) `tests/all_open_golden.rs` and `tests/audit_recorder.rs`
needed compile-only signature threading with no assertion or golden-text change; (6) one
structurally-invalid-manifest test was rewritten to use an invalid capability name instead
of the now-deleted `exclude_tools` field, proving the same invariant. `s06` -- three
recorded choices, all editorial: (1)/(2) two pre-existing or newly-written doc-comment
lines were reworded to avoid the bare substring "classify" (to satisfy the task's own `rg`
verification command) and two doc-comment lines were hand-rewrapped after `rw` became the
longer `capability`, both with no semantic effect; (3) a test's capability assertion needed
no further translation because s05 had not changed which tool it drove. None of these
altered observable behavior of anything except the deliberate, ADR-sanctioned changes
themselves (navigate reclassified read, the schema-3 grant model replacing schema-2, the
audit `rw` -> `capability` rename, and the one sanctioned `explain` tool addition). No task
skipped, reverted, or left the tree dirty.

State of `docs/tasks/stage-2/BROWSER-TESTS.md`: it now carries every stage-2 entry
(unmodified) plus stage-3's live-check backlog: `s01-1` (navigate-is-read on a read grant),
`s05-1` (schema-3 capability grants end to end), `s07-1` (the explain tool live), and the
four consolidated `s-live-1` through `s-live-4` checks appended by this task (read-grant
enforcement, `denied_domain` carve-out, the explain tool with a spurious-invocation watch,
and the audit `capability` field). None of these have been run against a real browser by
this unattended executor; there is no live browser available in this environment.

The new capability enforcement (host polarity, schema-3 grants, the `capability` audit
field, and the `explain` tool) has been verified exhaustively at the unit/integration level
(459 tests green, `tests/architecture.rs` and the all-open goldens confirmed unchanged
throughout) but has NOT been verified end to end against a live browser. Stage 3 must be
described in any public-facing copy as shipped-but-unverified-end-to-end until a human runs
the BROWSER-TESTS.md backlog above against real Chrome. This branch (`stage-3`) has not been
pushed or merged; a human decides when it merges into `stage-2`/`main`.

## Post-batch follow-up: minor gap closures -- 2026-07-03

Two minor gaps surfaced by the per-task independent verification (both graded non-blocking at
the time) plus one ledger typo, closed together in one follow-up commit after `s08`. No task
commit was amended; this is a new commit on top.

- Gap A (s05 follow-up): `handle_tools_call` now short-circuits every free action (empty
  directory requirement) BEFORE grant resource resolution, per ADR-0022 Decision 5 step 2 --
  the governed grant-enforcement block is guarded on `!requires.is_empty()`, so a governed
  `computer` `wait` / `resize_window` no longer fires a pointless `tab_url` CDP probe (and can
  no longer stall on one). The `explain` server-side handler was moved to sit with this
  unified free-actions gate (after the always-on sacred check, matching the ADR step order),
  removing the earlier standalone special-case seam. New test:
  `governed_free_action_is_allowed_without_probing_the_tab_url` (server.rs inline) drives a
  governed `computer` `wait` against a fake extension with NO `tab_url` answers registered (a
  probe would panic) and asserts `seen == ["computer"]`.
- Gap B (s07 follow-up): `explain_output_is_byte_identical_across_manifest_postures`
  (`tests/mcp_protocol.rs`) pins the real invariant -- `explain` returns byte-identical output
  under no-manifest, an empty-grants manifest, and a restrictive read-only manifest -- via a
  new manifest-capable `drive_with_manifest` spawn helper. This closes s07's named-minimum-test
  shortfall (only the no-manifest posture had a live `tools/call explain` integration test).
- Gap C: corrected the s02 entry's transposed unchanged-suite counts (`architecture.rs` is 4
  tests, `all_open_golden.rs` is 3; they were swapped). Prose-only; no functional effect.

Verification: `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` clean;
`cargo test` 459 -> 461 (Gap A +1 lib unit test, Gap B +1 `mcp_protocol` test), 0 failed;
`tests/architecture.rs` (4), `tests/all_open_golden.rs` (3), `tests/mcp_protocol.rs` (6),
`tests/tool_schema_fidelity.rs` (7) all green; `src/transport/mcp/schemas/tools.json` and
`tests/tool_schema_fidelity.rs` byte-untouched (`git diff --stat` shows only server.rs,
mcp_protocol.rs, and this ledger). ASCII scan clean on all touched files.

ERRATUM (found 2026-07-03 by the stage-4 batch red-team, recorded here for honesty):
Gap A's `unwrap_or(&[])` plus the `!requires.is_empty()` guard flattened a directory
MISS into a free action, so a GOVERNED `computer` call with an unknown `action` string
now dispatches ungoverned instead of denying -- a fail-open regression of ADR-0022's
absent-means-DENY invariant (`decide`'s `unknown_action` arm became production-dead).
Exposure is low: the extension rejects unknown action strings in its own switch, and
the sacred check still applies. The fix is owned by stage-4 task t03 (ADR-0024
Decision 3), which restores deny-on-miss under a manifest as a named, sanctioned,
black-box-tested change; all-open behavior was never affected.
