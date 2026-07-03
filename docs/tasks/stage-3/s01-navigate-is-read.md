# S01: navigate is read (reclassification on the current stage-2 model)

## Goal

Fix the stage-2 category error: `navigate` is classified `RwClass::Mutate`, so a grant with
`access: "read"` cannot navigate to the very domains it grants. Flip the one table row to
`RwClass::Observe` in the CURRENT (pre-capability) model and update every affected test
expectation. A standalone bugfix, correct under both the old model's own scroll rationale
(a read-only grant that cannot reach the page cannot read it) and ADR-0022 (navigate is
provably a GET). No new types, no schema change, no directory work: those are s02+.

## Authority

ADR-0022 (`docs/adr/0022-intent-calibrated-capabilities.md`) is normative; where this
prompt and the ADR disagree, THE ADR WINS (record the deviation in the ledger). Its Context
and Decision 2 (the `navigate` row: "Provably a GET (top-level document load)") justify
this reclassification; this task implements ONLY that row's consequence on the stage-2
`RwClass` model, nothing else from the ADR.

## Depends on

Nothing; first stage-3 task. Per BOOTSTRAP: work on branch `stage-3` (create from
`stage-2` if absent), confirm LEDGER.md RESUME HERE names s01 and the tree is clean.

## Current behavior (verify against the tree before editing)

- `src/browser/classify.rs`: `TOOL_CLASSES` has the row `("navigate", RwClass::Mutate)`;
  the inline test `classification_matches_the_shared_format_table` asserts
  `classify("navigate", None) == Some(RwClass::Mutate)`.
- `src/browser/advertise.rs`: `access_class_permits` consults `classify::classify`, so
  advertisement follows the table with no separate list to edit. Its inline test
  `read_only_manifest_yields_the_exact_eight_tool_set_in_fixture_order` pins an 8-tool set
  WITHOUT `navigate`; `tests/tool_advertisement.rs`
  (`read_only_manifest_restricts_tools_list_to_the_observe_set`) pins the same 8 names end
  to end. In the `src/transport/mcp/schemas/tools.json` fixture, `navigate` sits between
  `tabs_create_mcp` and `computer`.
- `tests/tool_enforcement.rs`: `denied_access_names_the_grant_and_read_only_wording` uses
  `navigate` on the read grant as the mutate-on-read example. In this no-extension
  subprocess harness, `navigate` is the ONLY tool that resolves a host (every other
  tab-scoped tool resolves `Indeterminate` and denies `unmatched_domain`, not `access`);
  the union rule (`GoverningResource::None`, e.g. `tabs_create_mcp`) is the only other
  path to an `access` denial. Every other test in the file is class-independent for
  `navigate`.
- `tests/shadow_mode.rs`: `denied_call_requests()` uses `navigate` on the read-only
  grant's own domain as the would-deny call; both audit records pin `grant_id: "read-only"`.
- `src/governance/enforcement.rs` inline tests: `check_call` takes `rw` as an injected
  parameter and never consults the table, so no expectation there DEPENDS on the flip; but
  many tests pair the literal `"navigate"` with `RwClass::Mutate`, which becomes untruthful.
- `src/transport/mcp/server.rs` inline tests:
  `tools_call_produces_one_audit_record_with_client_identity` asserts
  `rec["rw"] == "mutate"` for a `navigate` call under the REAL `classify::classify` (breaks
  on the flip); `grant_shadow_deny_runs_the_tool_and_matches_the_enforce_denial_id` uses
  `navigate` on a read-only grant as the would-deny call (breaks: it becomes an allow).
  The point-5 landing tests (`Access::All`, domain coverage not class) and the g08 sacred
  tests (run before classification) stay green -- verified.
- `tests/policy_simulate.rs`: `restrictive_manifest_golden` replays
  `tests/fixtures/simulate/audit.jsonl` through the REAL binary (real classify; the
  recorded `rw` is ignored by design). Fixture line 2 is `navigate` on `docs.example.com`
  under the read-only `docs-read` grant: today a would-deny (rule `access`); after the
  flip a would-allow.
- Verified as needing NO change (class-independent, or stub/hand-built rw):
  `tests/audit_recorder.rs` (its `rw: "mutate"` assertion is for `computer left_click`),
  `src/governance/dispatch.rs` and `src/governance/audit/mod.rs` inline tests,
  `tests/mcp_protocol.rs`, `tests/all_open_golden.rs`, `src/governance/explain.rs`,
  `tests/policy_explain.rs`, `tests/golden/`, `src/governance/manifest/document.rs`,
  `src/governance/templates.rs`, `src/debug.rs`, `src/transport/mcp/tools.rs`, `examples/`.

## Required behavior

### 1. Flip the table row (`src/browser/classify.rs`)

Change `("navigate", RwClass::Mutate),` to exactly:

    // navigate is Observe: provably a GET (top-level document load), per ADR-0022
    // (Context + Decision 2). Reclassified by s01; supersedes the shared format doc
    // section 8 row (bannered in s08). Navigation remains the domain-enforcement point
    // (pre-dispatch target check + landing check); those are host checks, not class checks.
    ("navigate", RwClass::Observe),

In the inline test `classification_matches_the_shared_format_table`, change the `navigate`
assertion to `Some(RwClass::Observe)`. Change nothing else in the file.

### 2. Advertisement tests

In `src/browser/advertise.rs`, rename
`read_only_manifest_yields_the_exact_eight_tool_set_in_fixture_order` to
`read_only_manifest_yields_the_exact_nine_tool_set_in_fixture_order` and set its expected
vector to exactly this nine, in fixture order: `"tabs_context_mcp"`, `"navigate"`,
`"computer"`, `"find"`, `"get_page_text"`, `"read_console_messages"`,
`"read_network_requests"`, `"read_page"`, `"update_plan"`. Other tests unchanged. In
`tests/tool_advertisement.rs`, `read_only_manifest_restricts_tools_list_to_the_observe_set`
gets the same nine-name vector in the same order; note in its doc comment that the set is
the g14 doc's section-4 set PLUS `navigate`, reclassified observe by ADR-0022/s01.

### 3. Enforcement integration tests (`tests/tool_enforcement.rs`)

- Rework `denied_access_names_the_grant_and_read_only_wording` to use the union rule (no
  tab-scoped mutate tool resolves a host without an extension). Replace the grants with a
  local
  `json!([{ "id": "research-read", "domains": ["research.example.org"], "access": "read" }])`
  (do NOT reuse `EXAMPLE_FULL_AND_RESEARCH_READ`: its all-access `example-full` grant
  satisfies the union rule) and the call with `init_and_call("tabs_create_mcp", json!({}))`.
  Keep the three assertions byte-identical: starts with `Denied (D-`, contains
  `research-read`, contains `read only`. Update the doc comment (union rule since s01).
- Add a new test named `navigate_is_permitted_on_a_read_only_grant` (the bugfix pin),
  modeled on the file's test-1 pattern: manifest from `EXAMPLE_FULL_AND_RESEARCH_READ` via
  `manifest_value` with an audit path; drive
  `init_and_call("navigate", json!({"url": "https://research.example.org/", "tabId": 1}))`;
  assert on `by_id(&responses, 2)`: `result.isError == true`, text starts with
  `[hop: extension]` and contains `not connected`, and does NOT start with `Denied (`;
  the audit file has exactly 1 line with `decision == "allow"`,
  `grant_id == "research-read"`, `rw == "observe"`, `domain == "research.example.org"`.
  Clean up temp files like the sibling tests.

### 4. Shadow-mode integration test (`tests/shadow_mode.rs`)

In `denied_call_requests()`, replace the `navigate` request with
`json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"tabs_create_mcp","arguments":{}}})`
and update the helper's doc comment: the would-deny is now a mutate-class `tabs_create_mcp`
under the read-only grant via the union rule (access denial attributed to `read-only`).
Every assertion in `enforce_blocks_observe_dispatches_and_records_shadow_deny` stays
byte-identical (deny/shadow_deny, `grant_id: "read-only"`, `duration_ms` 0 vs nonzero).

### 5. Server inline tests (`src/transport/mcp/server.rs`)

- `tools_call_produces_one_audit_record_with_client_identity`: change
  `assert_eq!(rec["rw"], "mutate");` to `assert_eq!(rec["rw"], "observe");`.
- `grant_shadow_deny_runs_the_tool_and_matches_the_enforce_denial_id`: replace the shared
  `params` with `json!({ "name": "tabs_create_mcp", "arguments": {} })`; in the observe
  branch replace the `attach_fake_extension_with_tab_urls(...)` call with
  `attach_fake_extension(&observe_browser, vec![("tabs_create_mcp", json!({ "content": [{ "type": "text", "text": "created" }] }))])`;
  change the `observe_text` expectation from `"navigated"` to `"created"`. The enforce
  branch (no extension) is otherwise unchanged: the union-rule `access` denial is
  pre-dispatch, `duration_ms: 0`, and both runs derive the identical `grant_id`/`denial_id`
  (rule `access`, grant `r`, same `test-hash`). Update the test's doc comment accordingly.

### 6. Simulate golden (`tests/policy_simulate.rs`, `restrictive_manifest_golden`)

Fixture line 2 (`navigate` on `docs-read`) moves from would-deny to would-allow. Update:
`"would allow: 3"` to `"would allow: 4"`; `"would deny: 6"` to `"would deny: 5"`;
`assert_eq!(3 + 6 + 4, 13, ...)` to `assert_eq!(4 + 5 + 4, 13, ...)`; delete the
`"count=1 grant=docs-read domain=docs.example.com tool=navigate rule=access"` entry from
`expected_groups` (three groups remain, order preserved); `"result: 6 would-denies (exit 2)"`
to `"result: 5 would-denies (exit 2)"`. Exit code stays `Some(2)`. Do not touch the
fixture files: the recorded `rw:"mutate"` is read by nobody (simulate reclassifies), and
replaying old records is exactly what simulate is for.

### 7. Truthfulness renames in stub-driven inline tests (no expectation changes)

These tests inject `rw` explicitly and stay green either way; rename so no test pairs
`"navigate"` with a mutate class after the flip. Never weaken an assertion.

- `src/governance/enforcement.rs` inline tests: wherever the tool literal `"navigate"` is
  passed with `RwClass::Mutate`, replace it with `"form_input"` (genuinely mutate), and
  update the derived strings: the three `"tool/navigate"` rule expectations become
  `"tool/form_input"`, the two `exclude_tools: Some(vec!["navigate".to_string()])` become
  `"form_input"`. EXCEPTION: in `scheme_and_about_blank` keep `"navigate"` and change its
  two `RwClass::Mutate` arguments to `RwClass::Observe`.
- `src/governance/simulate.rs` inline tests: change the `stub_classify` entry
  `("navigate", None) => Some(RwClass::Mutate)` to `Some(RwClass::Observe)`. In
  `bucket_table_evaluable_allow_and_deny` and `group_sort_order_dash_entries_sort_first`,
  the `{"tool":"navigate","domain":"example.com"}` line becomes
  `{"tool":"javascript_tool","domain":"example.com"}` (already Mutate in the stub); in
  `simulate_and_the_decision_function_agree_on_the_same_call` the line becomes
  `{"tool":"javascript_tool","domain":"docs.example.com"}`, the group tool assertion and
  the direct `check_call` tool become `"javascript_tool"` (rw stays `RwClass::Mutate`).
  Leave `totals_arithmetic_holds` untouched (its navigate line flips to allow; the sum
  assertion still holds).

### 8. Stated consequence: a write-only grant now denies navigate

`access: "write"` DENIES `navigate` (rule `access`, "needs read access ... allows write
only"): write does not imply read, unchanged policy. Correct and intended; do not
compensate anywhere.

## Constraints

1. Do NOT touch anything under `docs/` except this stage's LEDGER.md and the
   BROWSER-TESTS.md append (the shared-format section 8 banner is s08's). Do NOT touch
   `docs/SPEC.md`, the stage-2 task docs, or `examples/`.
2. Do NOT touch `src/transport/mcp/schemas/tools.json` or `tests/tool_schema_fidelity.rs`
   (BOOTSTRAP rule 7), nor `tests/all_open_golden.rs` or `tests/mcp_protocol.rs`.
3. `src/browser/classify.rs` keeps its structure: no directory, no `requires` sets, no
   `Capability` type (s02/s03). The ONLY code change there is the one row plus its comment
   plus the one inline-test assertion.
4. Never weaken a test: substitute still-mutate tools exactly as specified; delete no
   scenario, relax no assertion.
5. ASCII only; no new dependencies; `tests/architecture.rs` stays green. One commit with
   code, tests, LEDGER.md entry, and the BROWSER-TESTS.md append. Message:
   `fix(governance): s01 navigate is read`.

## Tests (minimum)

1. `classification_matches_the_shared_format_table` (classify.rs, updated): navigate is
   `Some(RwClass::Observe)`; every other assertion unchanged.
2. `read_only_manifest_yields_the_exact_nine_tool_set_in_fixture_order` (advertise.rs,
   renamed) and `read_only_manifest_restricts_tools_list_to_the_observe_set`
   (tests/tool_advertisement.rs): the nine-name vector of Required behavior section 2.
3. `navigate_is_permitted_on_a_read_only_grant` (tests/tool_enforcement.rs, NEW) and
   `denied_access_names_the_grant_and_read_only_wording` (reworked), per section 3.
4. `enforce_blocks_observe_dispatches_and_records_shadow_deny` (tests/shadow_mode.rs):
   passes with the `tabs_create_mcp` would-deny call, assertions byte-identical.
5. `tools_call_produces_one_audit_record_with_client_identity` and
   `grant_shadow_deny_runs_the_tool_and_matches_the_enforce_denial_id` (server.rs inline),
   per section 5.
6. `restrictive_manifest_golden` and `restrictive_manifest_is_deterministic`
   (tests/policy_simulate.rs): green with the 4/5/4 totals.

## Verification

`cargo fmt` then `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings`
clean; `cargo test` fully green, including unmodified `tests/architecture.rs`,
`tests/all_open_golden.rs`, `tests/mcp_protocol.rs`, `tests/tool_schema_fidelity.rs`, and
`tests/audit_recorder.rs`. ASCII scan on every touched file
(`rg -n "[^\x00-\x7F]" <files>` prints nothing). Append exactly one deferred live check to
`docs/tasks/stage-2/BROWSER-TESTS.md` in its documented format:

    ## s01-1: read-only grant can navigate; acting on the page is still denied
    Changed: s01 reclassified navigate from mutate to observe (ADR-0022 Context/Decision 2)
    on the stage-2 schema-2 model; only a real browser proves the granted page loads for a
    read-only session.
    Steps: start the mcp-server with a schema-2 manifest whose only grant is
    {"id":"research-read","domains":["example.com","*.example.com"],"access":"read"} and
    audit enabled; then (1) navigate to https://example.com/ in an MCP tab, (2) computer
    screenshot, (3) computer left_click on the page.
    Expect: (1) and (2) succeed with no Denied text; the audit lines carry decision=allow,
    grant_id=research-read, and rw=observe for the navigate. (3) returns Denied (D-...)
    naming research-read and the read-only wording; its audit line is decision=deny.

Then update LEDGER.md (task-log entry + RESUME HERE pointing at s02) and commit.

## Out of scope

- Everything capability-related (s02+): the `Capability` type, the action directory and
  `requires` sets, host polarity, schema 3 / grant shape / enforcement rewrite, the audit
  `capability` field, deleting `RwClass`/`classify.rs`, the `explain` tool, any tools.json
  change.
- Documentation (s08): shared-format supersession banners, CLAUDE.md, SPEC notes, example
  manifests, templates. The stale `browser/tools/mod.rs` module-doc class table (predates
  classify.rs; already reads navigate as Observe) is also left alone.
- Any change to `tests/fixtures/simulate/*` or `tests/golden/*` files; hot-reload;
  re-advertisement; any behavior change beyond the one row flip and its test expectations.
