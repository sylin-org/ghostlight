# T04: the generic ingest pipeline

## Goal

Implement ADR-0024 Decision 2: the dispatch chokepoint moves to its own module
(`src/transport/mcp/pipeline.rs`) and becomes generic -- every per-tool `if name == ...`
branch is replaced by a registry read (t02's descriptors), and every audit/decision touch
goes through t03's `begin`/`authorize`/`CallAudit` API. `server.rs` keeps the JSON-RPC
protocol loop, `tools/list`, and the composition root, nothing else. Behavior is
byte-identical: same stage order, same texts, same audit bytes, same frames.

## Authority

ADR-0024 Decisions 1-3 (registry as data, pipeline stage order, audit ownership) are
normative. ADR-0022 remains normative for enforcement semantics. Where this prompt and an
ADR disagree, THE ADR WINS.

## Depends on

t02 (registry) and t03 (authorize/CallAudit) landed. STOP if `descriptor()` is missing
from `src/browser/directory.rs` or `Governance::begin`/`authorize` are missing from
`src/governance/dispatch.rs`.

## Current behavior (verified 2026-07-03 against `b4b2faf`, as adapted by t03; re-read)

- `src/transport/mcp/server.rs` (~1800 lines with tests) holds: `run()` (composition
  root + line loop + single stdout writer task), `tools_list_result`,
  `handle_tools_call` (the 12-stage chokepoint, ADR-0024 Decision 2), `sacred_check` +
  `resolve_tab_host`, `resolve_governing_resource` + `post_navigate_landing_check`,
  `append_wait_note`, `error_result`, and the chokepoint's inline test module (g08
  sacred tests, hold tests, point-5 landing tests, free-action test, explain tests,
  shadow tests).
- The five inline tool-name branches: `name == "computer"` (action extraction, ~325);
  `name == "explain"` (server-side handler; position pinned by the b4b2faf free-actions
  gate: after hold AND after the sacred check, before grant enforcement -- re-read and
  preserve the CURRENT order); `name == "navigate"` (post-check flag ~416, sacred target
  step ~583, resource arm ~616); `name == "read_page"` (redaction ~521); the domain-less
  name list (~631). Additionally `post_navigate_landing_check` hardcodes
  `governance.decide("navigate", ...)` (~677).
- `is_known_tool` (transport/mcp/tools.rs ~19) re-parses TOOLS_JSON per call; consumers:
  server.rs stage 3, its two unit tests in tools.rs
  (`is_known_tool_recognizes_advertised_names`, `is_known_tool_rejects_unknown_names`),
  AND the guard file `tests/all_open_golden.rs` (import ~line 18; assertions ~lines
  60-62 inside `tools_list_is_byte_stable_through_the_move`).
- Stage order and its pinned tests: BOOTSTRAP-quality map in the ADR-0024 Decision 2
  list; the ordering oracles are the inline tests named in the survey (hold beats
  everything incl. explain and not-connected; sacred always-on and never shadowable;
  free actions probe no tab_url; explain never dispatches; landing sequence
  [navigate, tab_url_request:5, navigate]).

## Required behavior

### 1. The module move

Create `src/transport/mcp/pipeline.rs`; MOVE (not rewrite) `handle_tools_call`,
`sacred_check`, `resolve_tab_host`, `resolve_governing_resource`,
`post_navigate_landing_check`, `append_wait_note`, `error_result`, and the ENTIRE
chokepoint inline test module -- including `pinned_explain_text` and its drift-guard
test (t02's "untouched" constraint applied during t02 only; moving them with unchanged
bytes is transcription, not an edit) and ALL shared test helpers
(`attach_fake_extension`, `attach_fake_extension_with_tab_urls`, `temp_audit_path`,
`read_lines`, `config_with_sacred_domains`, `wait_connected`, `full_grant`,
`governed_with_grants*`). Visibility pins: `handle_tools_call` becomes `pub(crate)` in
pipeline.rs; `handle_line` in server.rs becomes `pub(super)` so the moved
client-identity test (`tools_call_produces_one_audit_record_with_client_identity`,
which drives both) still compiles -- a compile-necessary visibility widening, noted in
the ledger. `server.rs` retains `run`, the writer task, `tools_list_result`,
initialize/ping handling, and the composition root; it calls
`pipeline::handle_tools_call`. Register `pub mod pipeline;` in `transport/mcp/mod.rs`.
Module doc for pipeline.rs: the generic ingest pipeline (ADR-0024 Decision 2), the
12-stage list in the ADR's order, and the rule that per-tool variance lives in the
registry, not here.

### 2. Registry-driven stages (inside the moved function)

- Stage 3 validity: `directory::descriptor(name)` replaces `is_known_tool`; a miss
  produces the byte-identical "Unknown tool: {name}" invalid_request result. DELETE
  `is_known_tool` from `transport/mcp/tools.rs` AND its two unit tests
  (`is_known_tool_recognizes_advertised_names`, `is_known_tool_rejects_unknown_names`)
  WITH it -- their intent is covered by t02's `registry_covers_the_sacred_surface_exactly`
  plus this task's `unknown_tool_is_a_registry_miss`; add nothing to directory.rs (its
  diff stays empty this task); list both deleted test names in the ledger. `TOOLS_JSON`
  stays. SANCTIONED GUARD RETYPE (BOOTSTRAP rule 8 names this task): in
  `tests/all_open_golden.rs`, retarget the `is_known_tool` import (~line 18) and its two
  assertion sites (~lines 60-62) onto
  `browser_mcp::browser::directory::descriptor(name).is_some()` /
  `descriptor("bogus_tool").is_none()` -- the assertion MEANING and every other
  expectation (including `GOLDEN_TOOL_NAMES`) stay byte-identical; update the module-doc
  sentence naming is_known_tool; record in the ledger.
- Stage 4 action extraction: `descriptor.action_key` drives
  `args.get(key).and_then(Value::as_str)`; no name check.
- Stage 5: `descriptor.requires(action)`-equivalent via the existing `requires()`
  contract feeding `governance.begin` (t03).
- Stage 7 sacred: STEP B stays ARGUMENT-driven (any call carrying a numeric `tabId`),
  exactly as today. STEP C (target host) fires iff
  `descriptor.resource == ResourceShape::TargetArg` (replacing `tool == "navigate"`).
- Stage 8 free actions + Local handler: the short-circuit keys on the looked-up requires
  (unchanged); `Handler::Local(f)` replaces `name == "explain"` -- call `f()`, wrap in
  `text_content`, record via `audit.complete()`, return. Position and hold/sacred
  interaction byte-identical to the current tree.
- Stage 9 resource resolution, shape-driven (replacing `resolve_governing_resource`'s
  name match; keep a function of the same name taking the descriptor):
  `DomainLess` -> `Some((GoverningResource::None, None))` (the union-rule resource);
  `TargetArg` -> today's navigate arm VERBATIM, whose actual semantics are:
  back/forward/missing url -> `Some((GoverningResource::None, None))` (the union-rule
  pre-check runs AND the landing post-check arms); unparseable url -> Rust `None` (the
  true no-pre-check fall-through: dispatch ungoverned, no post-check); else the parsed
  resource + domain;
  `TabScoped` -> today's tabId arm verbatim (missing tabId -> Indeterminate fail-closed;
  else `browser.tab_url` probe + `resolved_url_resource`).
  The post-dispatch flag becomes `descriptor.post_dispatch == PostDispatch::NavigateLanding
  && <the pre-check actually ran>`, preserving today's exact gating.
- Stage 12 postprocess (after audit completion, per the ADR stage list):
  `if let Some(f) = descriptor.postprocess { f(&mut result, config.secrets_redact()) }`
  replaces `name == "read_page"`.
- The landing re-check keeps using the scope (`audit.landing_allow`/`landing_deny`);
  `post_navigate_landing_check` gains a `tool: &str` parameter (the pipeline passes
  `descriptor.tool`) and its `governance.decide` call uses it, removing the hardcoded
  `"navigate"` literal. The about:blank park stays verbatim (a synthesized
  `browser.call("navigate", ...)` -- navigate-specific behavior sanctioned by the
  `NavigateLanding` marker).

### 3. Post-move hygiene

`rg -n '"computer"|"explain"|"navigate"|"read_page"' src/transport/mcp/pipeline.rs`
must hit ONLY: the about:blank park's synthesized navigate call, test code, and
comments. `rg -n "is_known_tool" src/ tests/` -> nothing.

## Constraints

1. One commit: `refactor(architecture): t04 generic ingest pipeline`.
2. Byte-identical behavior: every existing test expectation across the whole suite is
   unchanged (the moved inline tests keep their names and assertions; only `use` paths
   and the module home change). tools.json/fidelity untouched; all-open goldens and
   mcp_protocol expectations untouched; `tests/architecture.rs` green.
3. The zero-cost paths survive: no fixture parse per call, no resource work under
   all-open, no frames for free actions, empty-sacred fast path intact (the existing
   pinned tests prove each; run them).
4. ASCII; no new deps; delete what you replace (BOOTSTRAP rule 13).

## Tests (minimum)

1. Every moved inline test green under `pipeline::` with unchanged names/assertions.
2. `unknown_tool_is_a_registry_miss` (NEW, pipeline.rs): a bogus name yields the exact
   current message and produces NO audit record; `explain` (registry hit with Local
   handler) still answers -- pinning that validity now comes from the registry.
3. `postprocess_fires_only_where_the_registry_says` (NEW): a fake-extension `read_page`
   result containing a `secret_value=` marker is redacted; the same payload via `find`
   is untouched (transcribe marker/expected strings from redact.rs's existing tests).
4. `resource_shape_drives_resolution` (NEW): with a governed store and fake extension,
   `update_plan` (DomainLess, requires non-empty? it is `[]` -- use `tabs_context_mcp`)
   resolves the union-rule path with NO tab_url probe; a TabScoped call without `tabId`
   denies fail-closed exactly as today (transcribe the current Indeterminate denial
   text).
5. The full suite: `cargo test` green with zero expectation edits outside moved paths.

## Verification

fmt/clippy/test green; the two rg checks of section 3; ASCII scan; `git diff --stat`
shows pipeline.rs (new), server.rs (shrunk), tools.rs, tests/all_open_golden.rs (the
sanctioned is_known_tool retype only), mod.rs, LEDGER. server.rs after the move must be
under ~700 lines (protocol + composition + its own remaining tests); note the achieved
count in the ledger. Add one small pinned test if practical within the moved suite:
governed `navigate` with `{"url":"back","tabId":5}` consults the decision path with the
union-rule resource (pinning the back/forward gloss above). No browser checks queued
(behavior identical). LEDGER + RESUME HERE -> t05; commit.

## Out of scope

- Tab-URL unification (`resolve_tab_host` MOVES here but is deleted in t05, not now).
- Hot-reload (t06); ports deletions (t07); docs (t08).
- Any ordering, text, or audit change; any registry data change (t02 owns the table).
