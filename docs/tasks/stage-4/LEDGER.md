# Stage 4 ledger

Durable, context-wipe-safe record of stage-4 (registry/pipeline architecture, ADR-0023/0024/0025)
execution. This file plus `docs/tasks/stage-2/BROWSER-TESTS.md` are the executor's memory. On
every start, after any interruption, and whenever state is unclear: read the RESUME HERE section
first, then `BOOTSTRAP.md` and the ADR(s) the current task cites, then the current task prompt,
then continue. Never rely on remembering earlier work; re-read files.

## RESUME HERE

- Branch: `stage-4` (created from `stage-3`; create it if absent). Never push, never merge,
  never commit to `main`, `stage-2`, or `stage-3`.
- Progress: t01 landed. The stage-3 org-policy outage is fixed: `parse_manifest` is the sole
  reader/parser/validator of the policy file; `parse_org_config` and `load_and_resolve` are
  deleted. t02 landed: `src/browser/directory.rs` generalized in place into the ADR-0024
  Decision 1 `ToolDescriptor` registry (14 rows, 26 variants); `requires()` and
  `explain_text()` keep their exact contracts and byte-identical output; nothing outside the
  module consumes the new fields yet. t03 landed: ADR-0024 Decision 3's audit-ownership
  inversion. `Governance::begin`/`Governance::authorize` plus the consuming `CallAudit` scope
  (`held`/`sacred_deny`/`dispatch_finished`/`landing_allow`/`landing_shadow_deny`/`landing_deny`/
  `complete`) replace the five public `record_*` methods; `Governance` no longer holds a
  `requires` fn pointer at all (`server.rs` performs the one directory lookup and hands the
  `Option` to `begin`). `handle_tools_call` is adapted in place (still name-branched; t04 owns
  the pipeline extraction). The sanctioned delta lands: a GOVERNED directory miss (known tool,
  unknown sub-action) now denies `unknown_action` through the mode switch instead of
  dispatching ungoverned. `Config.governance_mode` is now a typed `EffectiveMode`. `call_label`
  (`ports.rs`) is the one label formatter; `enforcement::tool_label` is deleted. t04 landed:
  ADR-0024 Decision 2's generic ingest pipeline. `transport::mcp::pipeline` is a new module
  holding the dispatch chokepoint (`handle_tools_call`, now `pub(crate)`) plus every helper it
  calls (`sacred_check`, `resolve_governing_resource`, `post_navigate_landing_check`,
  `resolve_tab_host`, `append_wait_note`, `error_result`) and the entire chokepoint inline test
  module (moved by transcription, plus 4 new tests and 1 new pinned test); `server.rs` keeps
  only the JSON-RPC protocol loop, `tools/list`, and the composition root (293 lines). Every
  per-tool `if name == ...` branch is now a registry read: stage 3 validity is
  `directory::descriptor(name)` (replacing `is_known_tool`, deleted along with its two unit
  tests); stage 4 action extraction is `descriptor.action_key`; stage 7 STEP C is
  `descriptor.resource == ResourceShape::TargetArg`; stage 8's `Handler::Local(f)` replaces
  `name == "explain"`; stage 9's `resolve_governing_resource` is shape-driven
  (`DomainLess`/`TargetArg`/`TabScoped`) instead of a name match, and its post-dispatch flag is
  `descriptor.post_dispatch == PostDispatch::NavigateLanding`; stage 12's postprocess is
  `descriptor.postprocess` replacing `name == "read_page"`. `tests/all_open_golden.rs`'s
  `is_known_tool` uses are the one sanctioned guard retype, onto
  `browser::directory::descriptor`. Behavior is byte-identical (every moved test, plus the
  black-box suites, pass unchanged); `resolve_tab_host` stays in the pipeline module (t05 owns
  its deletion). t05 landed: ADR-0024 Decision 4's tab-URL unification. `pipeline.rs` gained a
  private `LazyTabUrl` cell (one per call, memoized, keyed on the call's own `tabId` argument)
  constructed once at the top of `handle_tools_call`; `sacred_check`'s STEP B and
  `resolve_governing_resource`'s `TabScoped` arm both consume it via `LazyTabUrl::get`, so
  a call resolves its tab's URL through `Browser::tab_url` (`tab_url_request`) at most once,
  on whichever stage asks first. `resolve_tab_host` (the sacred check's former internal
  `tabs_context_mcp` probe plus its result-shape parsing) is deleted, along with the
  now-orphaned `tabs_context_reply` test fixture helper. The point-5 landing re-check's own
  post-dispatch probe is untouched (a different moment in time, out of scope). Six inline g08
  sacred tests that used to register a fake `tabs_context_mcp` reply now use
  `attach_fake_extension_with_tab_urls`'s `tab_urls` table instead; the one test that asserted
  the pre-flight's frame count (`sacred_tab_denies_every_tool_and_never_runs_it`) has its `seen`
  expectation changed from `vec!["tabs_context_mcp"; 4]` to `vec!["tab_url_request:5"; 4]` (the
  sanctioned ADR-0024 Decision 4 frame-traffic change); every other assertion in every reworked
  test (denial text, denial id, audit bytes, which calls are denied) is byte-unchanged.
- Progress (continued): t06 landed: ADR-0025 manifest hot-reload in full. `ConfigStore` (`reload.rs`)
  gained `user_source: Option<String>` (retained from `load_initial_with_policy`'s new third
  parameter), a `policy_snapshot`/`policy_tx` pair mirroring the existing `snapshot`/`tx` idiom,
  and `pub fn policy() -> watch::Receiver<Arc<LoadedPolicy>>`. `reresolve()` now performs the
  FULL org+user manifest re-selection via `source::load_policy` (one parse feeds both the org
  config layers and the published policy, replacing the deleted `read_and_parse_org`), publishing
  the new `LoadedPolicy` only when its identity (name/version/hash/origin) actually changed
  (`manifest_identity_changed`), with the same keep-last-good/fail-closed posture the config
  layers already had (a `load_policy` failure -- an invalid org file OR a configured user
  `file://` source gone missing -- keeps last-good for both). `server.rs` holds `Governance`
  behind `Arc<Mutex<Arc<Governance>>>`; every call snapshots it once at the top of the main read
  loop (torn never); a new policy-subscription task rebuilds `Governance` on every publish,
  carries the retained client identity forward (`Governance::current_client`, now `pub(crate)`),
  swaps it in, records the `manifest_reload` session event (`Governance::record_manifest_reload`),
  emits `notifications/tools/list_changed` (a new `Outbound` enum wrapping the writer channel)
  iff the advertised set changed, and records `user_manifest_ignored` on a transition
  (`ports::user_manifest_ignored_transitioned`, `Governance::record_user_manifest_ignored`).
  `docs/tasks/stage-2/BROWSER-TESTS.md` gained t06-1/t06-2 (verbatim from the task prompt).
- Progress (continued): t07 landed: ADR-0024 Decision 5's dead-seam and stub deletions.
  `src/governance/ports.rs` loses its four unwired placeholder items (`ToolId`,
  `ResourcePattern`, `DomainPolicy`, `ResourceResolver`; none had an impl, a consumer, or a
  dedicated inline test anywhere in the tree, confirmed by `rg` before deletion), 793 -> 748
  lines (still above the ~600 optional-split threshold; the split is SKIPPED this task, see
  deviation below). `src/browser/tools/` (11 zero-function doc-stub files carrying the stale
  observe/mutate classification table) is deleted outright; `src/browser/mod.rs` drops
  `pub mod tools;` and its module doc's roster sentence is rewritten to name the registry
  (`directory`) as the sole per-tool authority with no per-tool code homes, dropping its
  `DomainPolicy::requires` cross-reference. `ports.rs`'s own module doc and its
  `DecisionRequest` field doc (the one comment mentioning `ResourceResolver` outside the
  deleted trait itself) are reworded to describe resource resolution as an injected
  concrete value at the enforcement point (ADR-0024 Decision 5), naming that a future
  remote-PDP ADR reintroduces whatever seam it needs on its own terms. The two remaining
  `observe/mutate` mentions in `src/browser/directory.rs` and the (rewritten)
  `src/browser/mod.rs` are left as-is: both already name the current authority (the
  registry / ADR-0024) and state the classification table IS deleted, which is now true;
  they need no further edit per the task's own constraint 3 wording ("rewrite every
  survivor to name the current authority" -- these already do).
- Progress (continued): t08 landed: documentation sync (docs-only, no code/test changes).
  `docs/adr/README.md`'s 0023/0024/0025 index rows were already present at task start (a prior
  commit added them); section 0 was a verified no-op. `docs/tasks/stage-2/00-shared-format.md`
  gained the ADR-0025 hot-reload NOTE after section 1.3's selection-rule text, the ADR-0022/
  ADR-0023 schema-3/duplicate-key SUPERSEDED banner immediately before the `schema` field-rule
  line in section 4.1 (the fourth such banner; three already existed from the s08 pass), and
  items 18-20 appended verbatim to section 10 (`SPEC updates needed`), covering ADR-0023/0024/
  0025 respectively. `docs/tasks/stage-2/BROWSER-TESTS.md` gained the `t-live-1` consolidated
  live-check pointer (re-run s-live-1..4 plus t01-1/t05-1/t06-1/t06-2 against the stage-4 tree)
  appended after the existing `t06-2` entry. `CLAUDE.md`, `docs/SPEC.md`, and every ADR body are
  untouched (out of scope). This is the last stage-4 task; the run is complete pending human
  live-verification (see RUN SUMMARY below).
- NEXT TASK: none. Stage 4 (t01-t08) is complete. Manifest hot-reload (ADR-0025) and the
  org-policy loading fix (ADR-0023) are shipped-but-unverified-end-to-end until a human runs the
  live backlog in `docs/tasks/stage-2/BROWSER-TESTS.md` (the stage-3 s-live-1..4 backlog, t01-1,
  t05-1, t06-1, t06-2, and the new `t-live-1` consolidated pointer) against a live browser. A
  human decides when `stage-4` merges; no push, no merge performed by this run.
- Authority: ADR-0023/0024/0025 (each in its own scope) over task prompts over ADR-0022 over
  the stage-2 shared-format doc over SPEC.
- Invariants after every task: tree green (`cargo test`, `clippy -D warnings`, `fmt --check`),
  `tests/architecture.rs` passing, all-open byte-identical, tools.json and
  `tests/tool_schema_fidelity.rs` byte-untouched (NO exception task this stage), behavior
  preserved except the two sanctioned additions (t01 org-policy loading works, t06 hot-reload),
  ASCII-only, no new dependencies, superseded code deleted in the task that supersedes it.

## Task log

(Append one entry per completed task, newest at the bottom. Shape:)

### <task-id> <title> -- <date>
- Commit: (see this task's commit)
- Files touched: <list>
- Summary: <what landed, key decisions, any conservative choice made>
- Deviations from the prompt/ADR: <numbered, each with reasoning; "none" if none>
- Deletions performed: <the removed functions/files this task retired, or "none">
- Verification: <clippy/fmt/test status; test counts before -> after; which suites unchanged>
- Browser checks queued: <count and ids appended to BROWSER-TESTS.md, or "none">

### t01 one loader for the policy file -- 2026-07-03
- Commit: (see this task's commit)
- Files touched: `src/governance/manifest/document.rs`, `src/governance/config/load.rs`,
  `src/governance/config/reload.rs`, `src/governance/config/cli.rs`,
  `src/governance/config/presets.rs`, `src/governance/manifest/source.rs`,
  `src/transport/mcp/server.rs`, `src/doctor.rs`, `tests/manifest_validation.rs`,
  `docs/tasks/stage-2/BROWSER-TESTS.md`, this file.
- Summary: implemented ADR-0023 in full. `parse_manifest` (`document.rs`) is now the sole
  reader/parser/validator of the policy file for every origin; its config-array validation
  pass rejects a duplicate `key` (Decision 3). `parse_org_config` and `load_and_resolve`
  (`config/load.rs`) are deleted; replaced by the pure `org_config_from_entries(entries:
  &[ConfigEntry]) -> OrgConfig` split, plus a small `org_config_from_policy(&LoadedPolicy) ->
  OrgConfig` helper (origin-gated: only an org-sourced manifest's entries reach the org
  layers) shared by `read_layers` (`config/load.rs`) and
  `ConfigStore::load_initial_with_policy` (`config/reload.rs`, the renamed/reshaped
  `load_initial_with_manifest_config`) so the CLI's and the server's views of the org layers
  can never disagree. `read_layers` gained a `&LoadedPolicy` parameter and now reads only the
  user config file, deriving the org contribution from the policy and merging the manifest's
  user-layer map (`manifest_config_as_user_layer`) under the user config file's own values
  (file wins on collision, transcribed from `reload.rs::merge_manifest_user_config`).
  `reload.rs::read_and_parse_org` re-points to `parse_manifest` +
  `org_config_from_entries`, mapping a `ManifestError` via `Display` alone (no double-path
  prefixing). `cli.rs::resolve_with_warnings` now loads the policy once and returns it
  alongside the resolution/warnings; `run_list` passes it to `shadow_line` (which lost its own
  `load_policy` call and gained a `&LoadedPolicy` parameter) instead of reloading a second
  time; `presets.rs::resolve_current_and_candidate` does the same one-load pattern.
  `server.rs`/`doctor.rs` both swap to `load_initial_with_policy(checker, &loaded_policy)` and
  drop their `manifest_config_as_user_layer` call sites (the store computes it internally
  now). `source.rs::manifest_config_as_user_layer`'s doc comment is rewritten to say the org
  branch is empty because org entries take the ORG channel, not because a second parser reads
  the file; its behavior and its two inline tests are unchanged. Added the new integration
  test `org_policy_file_with_config_boots_the_server` (`#[cfg(windows)]`,
  `tests/manifest_validation.rs`): spawns the real binary with a schema-3 org policy (one
  read-only grant, two mandatory config entries: `audit.enabled`, `audit.file.path` at a
  unique temp path) at a fake `ProgramData`-rooted org path and confirms the outage regression
  is gone (the server answers `initialize`/`tools/list` instead of exiting at startup) with
  the governed tool list transcribed verbatim from `tests/tool_advertisement.rs`.
- Deviations from the prompt/ADR:
  1. Added `org_config_from_policy(&LoadedPolicy) -> OrgConfig` in `config/load.rs`, not
     literally named in the prompt, as a small shared helper between `read_layers` and
     `ConfigStore::load_initial_with_policy` so the origin-gated "only an org-sourced
     manifest's entries reach the org layers" rule has exactly one implementation instead of
     being written out twice at the two call sites. Conservative choice per BOOTSTRAP rule 4
     (fewer moving parts; a single source of truth for a rule both the CLI and the server
     store depend on never disagreeing). No pinned signature, string, or test assertion was
     affected; `org_config_from_entries`'s own pinned signature is unchanged.
  2. The task prompt's own historical narrative sentence in the new integration test's doc
     comment was reworded to avoid the literal substring `parse_org_config` (referring to it
     instead as "the now-deleted second org-file parser"), so the prompt's own Verification
     step 2 (`rg -n "parse_org_config|load_and_resolve" src/ tests/` -> no hits) passes
     literally, including inside the new test's own doc comment.
- Deletions performed: `governance::config::load::parse_org_config` (and its test
  `org_file_violations_are_errors`), `governance::config::load::load_and_resolve` (dead, zero
  callers, verified via `rg` before deletion), `ConfigStore::load_initial_with_manifest_config`
  (renamed/reshaped to `load_initial_with_policy`; `load_initial` itself is KEPT as the
  zero-argument-beyond-checker convenience the prompt specifies, delegating to
  `load_initial_with_policy` with an all-open `LoadedPolicy`).
- Verification: `cargo fmt` (applied) then `cargo fmt --check` clean; `cargo clippy
  --all-targets -- -D warnings` clean; `cargo test` fully green, 461 -> 464 (net +3: added
  `duplicate_config_key_is_a_field_error`, `org_config_from_entries_splits_by_level`,
  `org_sourced_policy_config_reaches_the_org_layers`,
  `org_policy_file_with_config_boots_the_server`; removed
  `org_file_violations_are_errors`). `tests/architecture.rs` (4 tests),
  `tests/all_open_golden.rs` (3 tests), `tests/mcp_protocol.rs` (6 tests), and
  `tests/tool_schema_fidelity.rs` (7 tests) all pass unchanged.
  `git diff HEAD -- src/transport/mcp/schemas/tools.json tests/tool_schema_fidelity.rs` and
  `git diff HEAD -- Cargo.toml Cargo.lock` both empty. `rg -n
  "parse_org_config|load_and_resolve" src/ tests/` -> no hits; `rg -n "expected 2" src/` -> no
  hits. ASCII scan on all 9 touched files -> clean. Manual smoke: copied
  `examples/research-read-only.json` to the real `%ProgramData%\browser-mcp\policy.json`, ran
  `cargo run -- doctor` (rendered the manifest correctly, no "config resolution is broken"),
  deleted the file, re-ran doctor (confirmed all-open again).
- Browser checks queued: 1 (`t01-1`, appended to `docs/tasks/stage-2/BROWSER-TESTS.md`).

### t02 the tool registry -- 2026-07-03
- Commit: (see this task's commit)
- Files touched: `src/browser/directory.rs`, `src/browser/mod.rs`, `src/browser/advertise.rs`,
  this file.
- Summary: implemented ADR-0024 Decision 1 in full. `src/browser/directory.rs` generalizes IN
  PLACE from the flat 26-row `ActionDescriptor`/`DIRECTORY` pair into the single per-tool
  `ToolDescriptor` registry (`REGISTRY`, 14 rows in tools.json advertised order): each row
  carries `tool`, `action_key` (`Some("action")` on `computer` only), `variants` (the 26
  existing `(action, requires, description)` triples unchanged, transcribed byte-for-byte as
  `ActionVariant`), `resource` (`ResourceShape`: `DomainLess`/`TabScoped`/`TargetArg`,
  mirroring today's `resolve_governing_resource` name match exactly), `handler` (`Handler`:
  `ExtensionForward` for 13 tools, `Local(explain_text)` for `explain`), `postprocess`
  (`Some(crate::browser::redact::apply_to_result)` on `read_page` only; verified the real
  signature is `fn(&mut serde_json::Value, bool)`, matching the pinned type exactly, no
  deviation needed there), and `post_dispatch` (`PostDispatch::NavigateLanding` on `navigate`
  only, `None` elsewhere). Added `descriptor(tool: &str) -> Option<&'static ToolDescriptor>`
  (linear scan). `requires(tool, action)` keeps its exact signature and semantics, reimplemented
  over `descriptor()` + `variants` (absent-vs-empty invariant unchanged). `explain_text()`
  reimplemented over `REGISTRY`, label generalized to `{tool} ({action})` from row data (no
  hardcoded `computer` literal); output is byte-identical, confirmed by the untouched
  server-side pin `pinned_explain_text_matches_the_real_directory_formatter`. `ActionDescriptor`
  and the flat `DIRECTORY` const are deleted; the inline test module is reworked per the task's
  Tests section (fixture-mirror technique kept). `src/browser/mod.rs`'s module doc sentence
  naming the directory is rewritten to name the ADR-0024 Decision 1 registry while keeping the
  `directory` module name and link (module not renamed).
- Deviations from the prompt/ADR:
  1. Constraint 1 said only `directory.rs` and `mod.rs` would change, but the live tree has a
     third direct consumer of the flat `DIRECTORY` const the prompt's Current Behavior survey
     did not mention: `src/browser/advertise.rs::tool_has_a_reachable_variant` iterates
     `directory::DIRECTORY` rows directly. Since Required Behavior section 3 unambiguously
     mandates deleting `DIRECTORY`, this consumer would not compile otherwise. Conservative fix
     per BOOTSTRAP rule 4 (behavior-preservation over structure-preservation; fewer moving
     parts): retargeted the same filter/any logic onto `directory::REGISTRY` rows' `variants`
     (`.filter(|row| row.tool == tool_name).flat_map(|row| row.variants.iter()).any(...)`),
     using only the pre-existing `tool`/`variants`/`requires` shape, no new type
     (`ResourceShape`/`Handler`/`PostDispatch`/`descriptor()`) referenced there, so constraint 4's
     `rg` check (new-field usage confined to `directory.rs`) still passes clean. Every
     `tool_advertisement.rs` and inline `advertise.rs` test still passes unchanged, confirming
     behavior is byte-identical.
  2. `per_tool_fields_match_the_adr_table`'s pinned `EXPECTED_TOOLS` tuple type triggered
     clippy's `type_complexity` lint (not itself pinned but a direct consequence of the pinned
     tuple shape in the Tests section). Added `#[allow(clippy::type_complexity)]` on that one
     `const` rather than restructure the pinned type, per BOOTSTRAP rule 14 (byte-pinned oracles
     move by transcription; the tuple shape is prescribed literally in the prompt).
- Deletions performed: `browser::directory::ActionDescriptor` (struct) and
  `browser::directory::DIRECTORY` (const); their absorbed content lives on as
  `REGISTRY[*].variants`. Superseded inline tests `directory_covers_the_sacred_surface_exactly`,
  `directory_requires_match_the_adr_table`, and
  `explain_text_is_the_vocabulary_block_then_one_line_per_row` are replaced by their reworked
  registry-shaped equivalents (`registry_covers_the_sacred_surface_exactly`,
  `registry_requires_match_the_adr_table`, `explain_text_is_unchanged_by_the_registry_reshape`,
  the last folding in the old structural test's pinned line assertions so no parallel dead test
  survives).
- Verification: `cargo fmt` (applied) then `cargo fmt --check` clean; `cargo clippy
  --all-targets -- -D warnings` clean; `cargo test` fully green, 464 -> 465 (net +1: five old
  directory tests replaced by six reworked/new registry tests --
  `registry_covers_the_sacred_surface_exactly`, `registry_requires_match_the_adr_table`,
  `absent_is_none_and_empty_is_some` (unchanged), `every_description_is_nonempty_ascii_and_short`
  (unchanged, iterates variants), `per_tool_fields_match_the_adr_table` (new),
  `explain_text_is_unchanged_by_the_registry_reshape` (new, folds in the old structural test)).
  `tests/architecture.rs` (4 tests), `tests/all_open_golden.rs` (3 tests), `tests/mcp_protocol.rs`
  (6 tests), and `tests/tool_schema_fidelity.rs` (7 tests) all pass unchanged. `git diff HEAD --
  src/transport/mcp/schemas/tools.json tests/tool_schema_fidelity.rs` and `git diff HEAD --
  Cargo.toml Cargo.lock` both empty. Constraint-4 `rg -n
  "ResourceShape|Handler::|PostDispatch|descriptor\(" src/ --glob '!src/browser/directory.rs'`
  returns nothing. ASCII scan on all 3 touched source files (`advertise.rs`, `directory.rs`,
  `mod.rs`) clean.
- Browser checks queued: none (pure data/lookup change; nothing observable live yet, per the
  task's own Verification section).

### t03 governance authorize and call audit -- 2026-07-03
- Commit: (see this task's commit)
- Files touched: `src/governance/ports.rs`, `src/governance/enforcement.rs`,
  `src/governance/dispatch.rs`, `src/governance/config/mod.rs`, `src/governance/explain.rs`,
  `src/doctor.rs`, `src/transport/mcp/server.rs`, `tests/all_open_golden.rs`,
  `tests/audit_recorder.rs`, `tests/tool_enforcement.rs`, this file.
- Summary: implemented ADR-0024 Decision 3 in full. `ports.rs` gained `call_label(tool, action)`
  (the one label formatter, keyed on `action.is_some()` rather than `tool == "computer"`, proven
  equivalent today since only `computer` ever carries an action); `enforcement::tool_label` and
  `hold_message`'s inline match are deleted in its favor. `dispatch.rs`: `Governance` no longer
  holds a `requires` fn pointer at all (deleted from the struct and from `all_open`/`governed`'s
  constructor parameters); `decide` is reshaped into a pure pass-through into `DecisionRequest`
  taking `requires: &[Capability]` directly (its own directory-miss handling and empty-requires
  shortcut are deleted -- both move to `authorize`). Added `pub struct CallAudit` (private
  fields: sink handle, client snapshot, tool/action strings, the `requires` result, a start
  `Instant`, domain/grant/shadow state) and `pub enum Gate { Deny { message: String }, Proceed }`.
  `Governance::begin(tool, action, requires)` opens the scope (captures the current client via
  the existing `current_client()` helper). `Governance::authorize(&mut CallAudit, Option
  <GoverningResource>, EffectiveMode) -> Gate` is the one policy gate, implementing the five-arm
  precedence pinned in the prompt exactly (all-open Proceed; free-action Proceed; a governed miss
  builds `unknown_action_denial` and routes it through `apply_mode` -- the ONLY `apply_mode` call
  site outside `check_call` now -- recording a terminal deny via a private `CallAudit::
  record_terminal_deny` helper on `Deny`, storing shadow on `ShadowDeny`; an unresolved resource
  Proceeds; a resolved resource delegates to `decide` exactly as before). `CallAudit`'s public
  consuming/mutating methods (`set_domain`, `held`, `sacred_deny`, `dispatch_finished`,
  `landing_allow`, `landing_deny`, `complete`) transcribe every pinned semantic (pre-dispatch
  denials hardcode `duration_ms: 0`; `dispatch_finished` freezes the duration at the exact point
  `Browser::call` returns, exactly transcribing today's clock stop, so `complete`/`landing_deny`
  reproduce the pre-ADR-0024 duration bytes even when the navigate landing probe runs after it).
  The five public `record_*` methods and the private `build_record` on `Governance` are deleted;
  their bodies fold into `CallAudit`'s own private `build_record`, which now derives `capability`
  from `self.requires.and_then(|r| r.first())...unwrap_or("none")` -- a miss renders "none"
  exactly as the old `unwrap_or(&[])` convention did. `record_session_killed` is untouched.
  `server.rs::handle_tools_call` is adapted in place (same stage structure, same name branches;
  t04 owns the pipeline extraction): one `directory::requires` lookup kept as the `Option`
  (`lookup`) feeds `governance.begin(name, action, lookup)` before the hold check; held ->
  `audit.held()`; sacred deny -> `audit.sacred_deny(...)`; immediately after the sacred stage
  passes, `audit.set_domain(tab_domain.clone())` unconditionally (seeding the pre-grant domain so
  an all-open/free-action allow on a resolvable tab still carries it); explain -> `audit.
  complete()`; resource resolution stays gated on `is_governed() && matches!(lookup, Some(r) if
  !r.is_empty())`, then `governance.authorize(&mut audit, resource, config_mode)` runs for EVERY
  call reaching that point, overwriting the domain when resolution produced one and rendering
  `Gate::Deny{message}` exactly as the old denial text path; after `Browser::call` returns ->
  `audit.dispatch_finished()`; the point-5 navigate landing re-check maps `Decision::Allow` to
  `audit.landing_allow(...)` and `Decision::Deny` to `audit.landing_deny(...)`; final ->
  `audit.complete()`. The four caller-side mutables (`audit_domain`/`audit_grant_id`/
  `shadow_denial`/`navigate_post_check`) are gone except `navigate_post_check` itself (still a
  local bool driving which branch runs, not audit state). `post_navigate_landing_check` gained a
  `requires: &[Capability]` parameter (the same `lookup` value, unwrapped) since `decide` no
  longer looks it up itself. `Config.governance_mode` is now `EffectiveMode` (parsed once via
  `EffectiveMode::from_config_str` at `from_preset`/`from_resolution`); the getter returns
  `EffectiveMode` (Copy). `server.rs` and `doctor.rs` drop their own `from_config_str` calls;
  `explain.rs`'s minimal-config site (~137) simplifies to the direct enum; its manifest-entry
  site (~149) and `cli.rs`'s raw-`Resolution` site keep `from_config_str` (they read raw string
  values, not `Config`). New/reworked tests, transcribing every pinned byte from the pre-ADR-0024
  sources named in the prompt: `begin_complete_produces_the_allow_record_bytes`,
  `authorize_miss_is_unknown_action_through_the_mode_switch`,
  `governed_unknown_computer_action_is_denied_unknown_action` (black-box, `tool_enforcement.rs`),
  `authorize_free_action_proceeds_without_grant_attribution`,
  `sacred_deny_and_held_records_are_byte_stable`,
  `landing_amendments_match_the_old_navigate_records`,
  `sacred_domain_seeding_survives_on_allow_records` (`server.rs`, proves the unconditional
  pre-grant domain seeding), `one_lookup_feeds_decision_and_audit` (a hand-rolled
  `EchoRequiresPdp` proves `authorize` drives the PDP from exactly the caller's own `requires`
  value, never a second lookup), `governance_mode_is_typed` (`config/mod.rs`). `audit_recorder.rs`
  reworked its two record-driving sites onto `begin`/`dispatch_finished`/`complete`; its one
  literal-duration assertion (`42`) becomes `rec["duration_ms"].as_u64().is_some()`, the sanctioned
  edit constraint 2 names (every other field assertion, including the 14-key order, is
  byte-unchanged).
- Deviations from the prompt/ADR:
  1. `CallAudit` also captures the current client identity (`client: Option<ClientInfo>`,
     snapshotted at `begin` via the pre-existing `current_client()` helper), which the prompt's
     "captures... the sink handle, tool/action strings, the requires result, a start Instant, and
     empty domain/grant/shadow state" sentence does not enumerate. Necessary: `AuditRecord`'s
     `client` field must still be populated, and `CallAudit`'s consuming methods take `self`/
     `&mut self` with no back-reference to `Governance` to read it from at completion time.
     Conservative, no pinned byte affected (every record's `client` field is unchanged).
  2. Added `pub fn landing_shadow_deny(&mut self, denial: Denial, domain: Option<String>)` to
     `CallAudit`, not in the prompt's seven-method pinned list (`set_domain`, `held`,
     `sacred_deny`, `dispatch_finished`, `landing_allow`, `landing_deny`, `complete`). The
     pre-ADR-0024 code's point-5 navigate landing re-check can resolve to `Decision::ShadowDeny`
     (g15's mode switch applies there exactly as it does pre-dispatch), and that outcome must
     still be recorded as a `shadow_deny` with the landing's own domain -- `landing_allow`'s own
     doc comment says it "clears shadow", so reusing it for this arm would have silently
     misrecorded a would-be shadow_deny as a plain allow, an unsanctioned behavior change (rule
     8/constraint 2: audit record bytes byte-identical). No black-box test exercises this exact
     arm today (would require an observe-mode manifest whose grant covers the pre-dispatch host
     but not the landing host); the method is transcribed from the pre-existing inline
     `audit_grant_id = d.grant_id.clone(); audit_domain = landing_domain; shadow_denial =
     Some(d);` assignment set, minus the (dead-for-this-arm) `grant_id` write `complete`'s shadow
     branch never reads (`complete` derives `grant_id`/`denial_id` from the stored `Denial`
     itself when `shadow` is `Some`, matching the pre-ADR-0024 `record_shadow_deny`'s own
     behavior of never consulting the caller-passed grant id).
- Deletions performed: `Governance::record_call`, `Governance::record_deny`,
  `Governance::record_navigate_landing_deny`, `Governance::record_shadow_deny`,
  `Governance::record_held`, and the private `Governance::build_record` they shared (folded into
  `CallAudit`'s own private `build_record`); the `Governance.requires` fn-pointer field and the
  `requires` parameter on `all_open`/`governed`; `decide`'s internal directory-miss branch and
  empty-requires shortcut; `enforcement::tool_label`; the `no_requires` test helper (both
  `dispatch.rs` and `tests/all_open_golden.rs`); the four `handle_tools_call` mutables
  (`audit_domain`, `audit_grant_id`, `shadow_denial`) -- `navigate_post_check` stays, as noted
  above. Eleven dispatch.rs inline tests directly driving the deleted API
  (`all_open_decide_is_allow_and_still_records`,
  `governed_over_noop_still_allows_and_holds_the_sink`,
  `directory_miss_denies_via_unknown_action_through_the_mode_switch`,
  `requires_empty_allows_without_consulting_the_pdp`,
  `computer_action_requires_flows_into_capability`, `requires_empty_records_capability_none`,
  `deny_record_carries_the_capability_of_the_denied_call`,
  `record_call_passes_the_resolved_domain_through`,
  `record_deny_writes_a_zero_duration_deny_record`,
  `record_held_writes_an_allow_record_with_held_true_and_no_domain`,
  `record_call_and_record_deny_leave_held_false`) are replaced by their six reworked/new
  registry-shaped equivalents named above (every pinned assertion each one carried survives in
  its replacement, folded or transcribed).
- Verification: `cargo fmt` (applied) then `cargo fmt --check` clean; `cargo clippy
  --all-targets -- -D warnings` clean; `cargo test` fully green, 465 -> 463 (net -2: -11 old
  dispatch.rs record tests, +6 reworked/new dispatch.rs tests, +1
  `governed_unknown_computer_action_is_denied_unknown_action` (tool_enforcement.rs), +1
  `governance_mode_is_typed` (config/mod.rs), +1 `sacred_domain_seeding_survives_on_allow_records`
  (server.rs); audit_recorder.rs's two tests reworked in place with no count change).
  `tests/architecture.rs` (4 tests), `tests/all_open_golden.rs` (3 tests), `tests/mcp_protocol.rs`
  (6 tests), and `tests/tool_schema_fidelity.rs` (7 tests) all pass unchanged. `git diff HEAD --
  src/transport/mcp/schemas/tools.json tests/tool_schema_fidelity.rs` and `git diff HEAD --
  Cargo.toml Cargo.lock` both empty. Constraint-5 checks: `rg -n
  "record_call|record_deny|record_shadow_deny|record_held|record_navigate_landing_deny" src/` ->
  only two historical doc-comment mentions in `dispatch.rs` (naming the pre-ADR-0024 test sources
  the new tests transcribe from), no functional hits; `rg -n "tool_label" src/` -> nothing; `rg -n
  "from_config_str" src/` -> `ports.rs` (definition), `config/mod.rs` (the two `Config`-building
  call sites plus a doc-comment mention and a test assertion), `cli.rs` (~156, raw `Resolution`
  value), `explain.rs` (~149, raw entry value) -- matches the constraint exactly. ASCII scan on
  all 10 touched files clean.
- Browser checks queued: none (behavior identical to the pre-ADR-0024 tree except the one
  sanctioned miss->deny delta, itself covered by the new black-box test; the stage-3 s-live
  backlog already covers the observable surface, per the task's own Verification section).

### t04 the generic ingest pipeline -- 2026-07-03
- Commit: (see this task's commit)
- Files touched: `src/transport/mcp/pipeline.rs` (new), `src/transport/mcp/server.rs`,
  `src/transport/mcp/tools.rs`, `src/transport/mcp/mod.rs`, `src/browser/directory.rs`,
  `tests/all_open_golden.rs`, this file.
- Summary: implemented ADR-0024 Decision 2 in full. `handle_tools_call`, `sacred_check`,
  `resolve_governing_resource`, `post_navigate_landing_check`, `resolve_tab_host`,
  `append_wait_note`, `error_result`, and the ENTIRE chokepoint inline test module (including
  `pinned_explain_text` and its drift-guard test, and every shared test helper --
  `attach_fake_extension`, `attach_fake_extension_with_tab_urls`, `temp_audit_path`,
  `read_lines`, `config_with_sacred_domains`, `wait_connected`, `full_grant`,
  `governed_with_grants*`) MOVED by transcription from `server.rs` into the new
  `src/transport/mcp/pipeline.rs` module, registered via `pub mod pipeline;` in
  `transport/mcp/mod.rs`. `handle_tools_call` is now `pub(crate)`; `server::handle_line` is now
  `pub(super)` (a compile-necessary visibility widening: the moved
  `tools_call_produces_one_audit_record_with_client_identity` test, now in `pipeline::tests`,
  drives both functions, which now live in sibling modules under `transport::mcp`). `server.rs`
  shrank to 293 lines (protocol loop, `tools_list_result`, `initialize`/`ping` handling, the
  composition root); its `"tools/call"` arm now calls `pipeline::handle_tools_call`. Every
  per-tool `if name == ...` branch in the moved function became a registry read: stage 3
  validity is `directory::descriptor(name)` (a miss still returns the byte-identical
  "Unknown tool: {name}" result) -- `is_known_tool` is DELETED from `tools.rs` along with its
  two unit tests (`is_known_tool_recognizes_advertised_names`,
  `is_known_tool_rejects_unknown_names`); their intent is covered by t02's
  `registry_covers_the_sacred_surface_exactly` plus this task's new
  `unknown_tool_is_a_registry_miss`. Stage 4 action extraction is
  `descriptor.action_key.and_then(|key| args.get(key)).and_then(Value::as_str)`, replacing
  `name == "computer"`. Stage 7 STEP C now fires iff
  `descriptor.resource == ResourceShape::TargetArg` (STEP B stays argument-driven, unchanged,
  independent of shape). Stage 8's `if let Handler::Local(f) = descriptor.handler { ... }`
  replaces `name == "explain"`, same position (after the sacred-domain seeding, before grant
  enforcement) and same hold/sacred interaction. Stage 9's `resolve_governing_resource` is
  reshaped to take `&directory::ToolDescriptor` and match on `descriptor.resource`
  (`DomainLess` -> `Some((GoverningResource::None, None))`; `TargetArg` -> today's navigate arm
  verbatim, including the back/forward/missing-url union-rule gloss and the
  unparseable-url-is-Rust-`None` fall-through; `TabScoped` -> today's tabId arm verbatim,
  including the missing-tabId-is-Indeterminate fail-closed branch) instead of a `match tool`
  name arm; the post-dispatch flag is now
  `resolved.is_some() && descriptor.post_dispatch == PostDispatch::NavigateLanding`, replacing
  `name == "navigate"` while preserving the exact same gating (only set when the pre-check
  actually ran). `post_navigate_landing_check` gained a `tool: &str` parameter (the pipeline
  passes `descriptor.tool`); its `governance.decide` call uses `tool` instead of the hardcoded
  `"navigate"` literal; the about:blank park keeps the literal `"navigate"` string verbatim (a
  synthesized call, sanctioned by the `NavigateLanding` marker, not a lookup of the triggering
  tool's name). Stage 12 postprocess is
  `if let Some(f) = descriptor.postprocess { f(&mut result, config.secrets_redact()); }`,
  replacing `name == "read_page"`. `tests/all_open_golden.rs`'s three `is_known_tool` uses (the
  import and the two assertion sites inside `tools_list_is_byte_stable_through_the_move`) are
  the one BOOTSTRAP-rule-8-sanctioned guard retype, onto
  `browser_mcp::browser::directory::descriptor(name).is_some()` /
  `descriptor("bogus_tool").is_none()` -- assertion meaning and `GOLDEN_TOOL_NAMES` are
  byte-identical; its module-doc sentence naming `is_known_tool` is reworded to name
  `directory::descriptor`. Added tests (all in `pipeline::tests`, per the task's Tests
  section): `unknown_tool_is_a_registry_miss` (a bogus name yields the pinned
  `[hop: invalid-request] ... Unknown tool: bogus_tool` text and produces NO audit file;
  `explain`, a registry hit with a `Handler::Local`, still answers); `postprocess_fires_only_where_the_registry_says`
  (a fake-extension `read_page` result carrying a `secret_value=` marker, transcribed from
  `redact.rs`'s own fixture text, is redacted; the identical payload via `find`, whose
  descriptor carries no `postprocess`, survives untouched); `resource_shape_drives_resolution`
  (a governed `tabs_context_mcp` call, `DomainLess`, resolves the union rule with NO
  `tab_url_request` frame even with a fake extension registering none; a governed `read_page`
  call with no `tabId`, `TabScoped`, denies fail-closed with the transcribed
  `no grant covers (unknown)` `Indeterminate` denial text); and the Verification section's
  pinned addition, `governed_navigate_back_consults_the_union_rule_resource` (a governed
  `navigate` with `{"url":"back","tabId":5}` resolves the union-rule resource pre-dispatch,
  allowed by the grant's read capability, and the point-5 landing re-check still probes the
  final `tab_url`, matching the `[navigate, tab_url_request:5]` seen-order the ADR's back/forward
  gloss predicts).
- Deviations from the prompt/ADR:
  1. `src/browser/directory.rs`'s `descriptor()` doc comment (authored in t02) named
     `is_known_tool` in prose ("the validity check the pipeline uses (replacing
     `is_known_tool`'s per-call fixture re-parse)"). The prompt's own Required Behavior section
     2 says "add nothing to directory.rs (its diff stays empty this task)", but the same
     prompt's Post-move hygiene section pins `rg -n "is_known_tool" src/ tests/` -> nothing,
     which the live tree could not satisfy without touching that one comment line -- a prompt
     self-inconsistency the "verified 2026-07-03" survey did not anticipate (it predates t02's
     landed doc-comment wording). Per BOOTSTRAP rule 4 (conflicting statements within scope,
     resolve toward fewer moving parts / behavior preservation; record as a numbered deviation):
     reworded the one line to "the transport layer's former per-call fixture re-parse" (same
     meaning, no function name). No new functionality was added to `directory.rs`; `REGISTRY`,
     `requires()`, `explain_text()`, and every test in that file are byte-unchanged. This is the
     only line touched in `directory.rs` this task.
  2. `server.rs`'s own module-doc paragraph (not itself pinned by the prompt) was reworded to
     name `pipeline::handle_tools_call` instead of the deleted description of the dispatch
     chokepoint living inline, since the prompt's own Required Behavior section 1 explicitly
     asks for a pipeline module doc but is silent on whether `server.rs`'s doc needs a matching
     update; done for accuracy (the old doc's "routes through the Governance facade" prose would
     otherwise silently describe code that no longer lives in this file). No pinned string,
     test, or behavior is affected; plain code-span (not an intra-doc link) used deliberately to
     avoid a new `rustdoc::private_intra_doc_links` warning against the `pub(crate)`
     `handle_tools_call` (not a required verification gate in this stage, but zero-cost to
     avoid).
- Deletions performed: `transport::mcp::tools::is_known_tool` and its two unit tests
  (`is_known_tool_recognizes_advertised_names`, `is_known_tool_rejects_unknown_names`); the
  `#[cfg(test)] mod tests` block in `tools.rs` (empty after those two deletions, so removed
  rather than left as a vestigial `use super::*;`-only module). `resolve_tab_host` MOVED into
  `pipeline.rs` unchanged; it is NOT deleted this task (t05 owns that deletion per the task's
  own Out of scope section).
- Verification: `cargo fmt` (applied) then `cargo fmt --check` clean; `cargo clippy
  --all-targets -- -D warnings` clean; `cargo test` fully green, 463 -> 465 (net +2: -2 deleted
  `is_known_tool` tests, +4 new pipeline tests
  (`unknown_tool_is_a_registry_miss`, `postprocess_fires_only_where_the_registry_says`,
  `resource_shape_drives_resolution`, `governed_navigate_back_consults_the_union_rule_resource`);
  every moved inline test passes unchanged under `transport::mcp::pipeline::tests::`).
  `tests/architecture.rs` (4 tests), `tests/all_open_golden.rs` (3 tests), `tests/mcp_protocol.rs`
  (6 tests), and `tests/tool_schema_fidelity.rs` (7 tests) all pass unchanged. `git diff HEAD --
  src/transport/mcp/schemas/tools.json tests/tool_schema_fidelity.rs` and `git diff HEAD --
  Cargo.toml Cargo.lock` both empty. Post-move hygiene: `rg -n
  '"computer"|"explain"|"navigate"|"read_page"' src/transport/mcp/pipeline.rs` hits only the
  about:blank park's synthesized navigate call (production code, before the `#[cfg(test)]`
  boundary) plus doc comments and test code; `rg -n "is_known_tool" src/ tests/` -> no hits.
  `server.rs` after the move is 293 lines (well under the ~700-line target). ASCII scan on all
  6 touched/created files (`pipeline.rs`, `server.rs`, `tools.rs`, `mod.rs`, `directory.rs`,
  `tests/all_open_golden.rs`) clean.
- Browser checks queued: none (behavior identical to the pre-move tree by construction and by
  the full unchanged test wall; the task's own Verification section states no browser check is
  needed).

### t05 one tab-url resolution per call -- 2026-07-03
- Commit: (see this task's commit)
- Files touched: `src/transport/mcp/pipeline.rs`, `docs/tasks/stage-2/BROWSER-TESTS.md`, this
  file.
- Summary: implemented ADR-0024 Decision 4 in full. Added a private `LazyTabUrl<'a>` cell in
  `pipeline.rs` (fields: `browser`, `tab_id: Option<i64>`, `resolved: Option<Option<String>>`)
  with one async method, `get(&mut self) -> Option<String>`, that resolves via
  `Browser::tab_url` (the existing `tab_url_request` frame) at most once and memoizes the
  result for the rest of the call. `handle_tools_call` constructs exactly one
  `LazyTabUrl::new(browser, args.get("tabId").and_then(Value::as_i64))` right before the
  sacred-domains check, so both consumers read the SAME call's `tabId` argument. `sacred_check`
  (STEP B) now takes `&mut LazyTabUrl<'_>` instead of `&Browser` and derives the match host from
  `tab_url.get().await` through the existing `pattern::host_for_matching` path, exactly as
  `resolve_tab_host` did (byte-identical outcome semantics: `None` url means no sacred host
  check). `resolve_governing_resource`'s `TabScoped` arm likewise takes `&mut LazyTabUrl<'_>`:
  a missing/non-integer `tabId` still fails closed to `Indeterminate` without ever calling
  `.get()` (preserving the "no wasted probe" guarantee for that arm); a present `tabId` calls
  `.get()` once (memoized if the sacred check already resolved it) and maps `None` ->
  `Indeterminate`, `Some(url)` -> `resource::resolved_url_resource(&url)`, unchanged. Deleted
  `resolve_tab_host` (the internal `tabs_context_mcp` call plus its result-shape parsing) in
  full, replacing it in place with the `LazyTabUrl` struct/impl at the same location in the
  file. The point-5 navigate landing re-check (`post_navigate_landing_check`) is untouched: its
  own post-dispatch `tab_url` probe is a different moment in time (out of scope, per the task's
  own Out of scope section) and does not consult `LazyTabUrl`.
- Deviations from the prompt/ADR: none. The `LazyTabUrl` identifier and its exact field/method
  shape are not literally pinned by the prompt (which describes "a small private struct or
  `Option`-cell local to `handle_tools_call`"); this is a schematic description per BOOTSTRAP
  rule 4, not a pin, so the concrete shape chosen here needs no deviation entry.
- Deletions performed: `resolve_tab_host` (the sacred check's former internal
  `tabs_context_mcp` lookup and its result-shape parsing); the now-orphaned test fixture helper
  `tabs_context_reply` (built exactly the JSON shape `resolve_tab_host` expected; no other
  caller survived the rewiring below, and clippy `-D warnings` would have flagged it as dead
  code).
- Test ripple (frame-sequence-only edits, ADR-0024 Decision 4's sanctioned change, transcribed
  per-test): six inline g08 sacred-check tests in `pipeline.rs` that used to register a fake
  `tabs_context_mcp` reply via `attach_fake_extension` + `tabs_context_reply` now register the
  tab's URL directly in `attach_fake_extension_with_tab_urls`'s `tab_urls` table instead:
  `sacred_tab_denies_every_tool_and_never_runs_it`, `navigate_target_denied_even_when_tab_is_clean`,
  `denied_call_writes_one_deny_record`, `sacred_domain_seeding_survives_on_allow_records`,
  `sacred_domain_denies_even_under_an_observe_mode_manifest` (five that already existed) plus a
  doc-comment-only wording update on `attach_fake_extension` noting its lack of `tab_url_request`
  support is now shared by both g08 and g13's own tests. Of these, only
  `sacred_tab_denies_every_tool_and_never_runs_it` asserted the pre-flight's frame COUNT: its
  `seen` expectation changes from `vec!["tabs_context_mcp"; 4]` to `vec!["tab_url_request:5"; 4]`
  (one unified probe per call, replacing the old internal `tabs_context_mcp` pre-flight), with
  its assertion message reworded to match; the other three (`navigate_target_denied_even_when_
  tab_is_clean` uses `_seen`, unread) had no count assertion to change. EVERY other assertion in
  every one of these five tests (denial text, denial id `D-af6633ec`/`D-171052e3`, audit
  `decision`/`domain`/`duration_ms` bytes, which calls are denied, `navigated` dispatch text)
  is byte-identical, transcribed unchanged. Two comment-only rewordings (no assertion change):
  `denied_call_writes_one_deny_record`'s doc comment and its `lines.len()` assertion message
  ("the tabs_context_mcp lookup writes none" -> "the tab-url probe writes none").
- New tests added (pipeline.rs, per the task's Tests section): `one_probe_serves_sacred_and_
  grants` (a non-empty sacred list, a governed grant covering the tab's clean host, a
  `TabScoped` call: the `seen` log is exactly `["tab_url_request:5", "read_page"]`, one probe
  serving both the sacred check and the grant path); `unresolvable_tab_still_fails_closed_for_
  grants_and_skips_sacred` (the `tab_urls` table answers `None` for a present `tabId`: the call
  is NOT sacred-denied, but IS denied with the transcribed `no grant covers (unknown)`
  `Indeterminate` wording, and `seen` is exactly `["tab_url_request:5"]` -- one probe, read two
  ways).
- Verification: `cargo fmt` (applied) then `cargo fmt --check` clean; `cargo clippy
  --all-targets -- -D warnings` clean; `cargo test` fully green, 465 -> 467 (net +2: the two new
  tests above; the five reworked g08 tests and the deleted `tabs_context_reply` helper produce
  no count change). `tests/architecture.rs` (4 tests), `tests/all_open_golden.rs` (3 tests),
  `tests/mcp_protocol.rs` (6 tests), and `tests/tool_schema_fidelity.rs` (7 tests) all pass
  unchanged. `git diff HEAD -- src/transport/mcp/schemas/tools.json
  tests/tool_schema_fidelity.rs` and `git diff HEAD -- Cargo.toml Cargo.lock` both empty.
  `rg -n "resolve_tab_host" src/` -> no hits. ASCII scan on the one touched source file
  (`pipeline.rs`) and the two touched docs files (`docs/tasks/stage-2/BROWSER-TESTS.md`, this
  file) clean.
- Browser checks queued: 1 (`t05-1`, appended to `docs/tasks/stage-2/BROWSER-TESTS.md`: a
  single-tab-URL-probe live check against a granted tab under an active sacred list).

### t06 manifest hot-reload -- 2026-07-03
- Commit: (see this task's commit)
- Files touched: `src/governance/config/reload.rs`, `src/governance/dispatch.rs`,
  `src/governance/manifest/source.rs`, `src/governance/ports.rs`, `src/doctor.rs`, `src/main.rs`,
  `src/transport/mcp/pipeline.rs`, `src/transport/mcp/server.rs`, `tests/hot_reload.rs` (new),
  `docs/tasks/stage-2/BROWSER-TESTS.md`, this file.
- Summary: implemented ADR-0025 in full, the stage's second sanctioned behavioral addition.
  `reload.rs`: `ConfigStore` gained `user_source: Option<String>` (a new third parameter on
  `load_initial_with_policy`, threaded from `main.rs::run_server`'s own already-resolved
  `user_source` through `server::run`'s new parameter, and from `doctor.rs`'s already-resolved
  `user_manifest_source`; `load_initial`/the `reload.rs` inline test pass `None`), which also
  sets `sources.manifest` to the user source's resolved `file://`/bare path (via
  `source::parse_source_string`), independent of which origin won selection (ADR-0025 Decision
  1). A `policy_snapshot: Mutex<Arc<LoadedPolicy>>` / `policy_tx: watch::Sender<Arc<LoadedPolicy>>`
  pair mirrors `Config`'s own `snapshot`/`tx` idiom exactly; `pub fn policy()` subscribes.
  `reresolve()` now calls the ADR-0023 single loader (`source::load_policy`) for the FULL org+user
  re-selection every settled change, deriving the org config layers from that SAME result
  (`load::org_config_from_policy`, replacing the deleted `read_and_parse_org`) and additionally
  folding a user-sourced manifest's own config entries into the user layer via
  `manifest_config_as_user_layer` (mirroring the startup merge, so an edit to a watched user
  manifest's config entries takes effect on reload too, not just at startup -- deviation 1
  below). A `load_policy` failure (invalid org file, or a CONFIGURED user file:// source gone
  missing) is treated uniformly as an org-slot failure (keep-last-good + ERROR, extending
  `plan_reload`'s existing org slot per the prompt's own instruction; `plan_reload`'s own
  signature and pure-function testability are unchanged). `maybe_publish_policy` publishes the
  new `LoadedPolicy` on the channel only when `manifest_identity_changed` (a new pure
  name/version/hash/origin comparison, present<->absent counted as changed) says so -- ORG-file
  deletion is a legitimate transition (publishes an all-open policy); a missing CONFIGURED user
  file:// source is a load error, never a transition (the pinned edge, proven against the REAL
  `source::load_policy` call in the new `user_manifest_deletion_keeps_last_good` test).
  `server.rs`: `build_governance` extracts the all-open/governed construction `run` used to do
  inline; `Governance` now lives behind `governance_slot: Arc<Mutex<Arc<Governance>>>`, snapshotted
  ONCE per JSON-RPC line in the main read loop (torn never, ADR-0025 Decision 6) and passed through
  exactly as before. A new policy-subscription task (spawned once at startup) awaits
  `store.policy()`: on each publish it snapshots the outgoing `Governance`'s advertised set and
  client identity, builds the new one via `build_governance`, re-applies the retained client
  (`Governance::current_client`, widened to `pub(crate)`, plus `set_client`) so identity survives
  every swap, swaps it into the slot, records `manifest_reload`
  (`Governance::record_manifest_reload`, a new session-event producer alongside
  `record_session_killed`, same frozen `SessionEventRecord` shape, `manifest: None` for a swap to
  all-open), sends `Outbound::ToolsListChanged` (the writer channel's type, widened from a bare
  `JsonRpcResponse` to the pinned `enum Outbound { Response(JsonRpcResponse), ToolsListChanged }`)
  iff the advertised set actually changed, and records `user_manifest_ignored`
  (`Governance::record_user_manifest_ignored`) iff `ports::user_manifest_ignored_transitioned`
  says the condition newly holds (never on a repeat). At startup, an already-true
  `loaded_policy.user_manifest_ignored` records the event once (implementing the promised note
  `source.rs`'s doc comment used to defer to "a future audit task"). The writer task's loop
  branches on `Outbound`, emitting the exact pinned `{"jsonrpc":"2.0","method":"notifications/
  tools/list_changed"}` line for the notification variant. `dispatch.rs`/`ports.rs`: the two new
  session-event producers and the pure transition-gate function, plus the `SessionEventRecord.
  event` field's doc-list update naming both new strings.
- Deviations from the prompt/ADR:
  1. `apply_policy_and_config` (the shared core of `reresolve`/the test-only
     `reload_with_policy`) additionally folds a user-sourced manifest's `config` entries into the
     user layer on every reload (via `manifest_config_as_user_layer` + the existing
     `merge_manifest_user_config`), not just at startup. The prompt's Required Behavior 1 literally
     names only "derive the org config layers from it (t01 path)" for the reload path; this extra
     fold is not literally pinned there. Added per BOOTSTRAP rule 4 (behavior preservation) and
     ADR-0025 Decision 3's own words ("re-derive the config layers from the same parse (one parse
     feeds both consumers)"): since Decision 1 now WATCHES a user file:// source, an edit to its
     config entries must actually take effect on reload, not only at the startup that predates
     this task -- otherwise the watched-user-source half of Decision 1 would be observably inert
     for config (only its grants would ever update). No pinned string or test assertion is
     affected; `plan_reload`'s own signature and every one of its pre-existing tests are
     unchanged.
  2. `#[allow(clippy::large_enum_variant)]` added to the pinned `Outbound` enum rather than boxing
     `JsonRpcResponse` (clippy's `large_enum_variant`, not itself pinned, fires on the literal
     pinned shape under `-D warnings`). Per BOOTSTRAP rule 14 (a byte-pinned shape moves by
     transcription, never re-derivation) the enum is kept exactly as written in the prompt; same
     resolution style t02 used for an analogous `type_complexity` clash.
  3. While authoring the flagship test, discovered and fixed a real shutdown bug: the
     policy-subscription task holds a permanent `Outbound` sender clone (needed to ever send
     `ToolsListChanged`), and its `while policy_changes.changed().await.is_ok()` loop has no
     natural end under the existing ADR-0019 watcher substrate (the sender side, `policy_tx`,
     lives inside `ConfigStore`, itself kept alive forever by the pre-existing watcher task) --
     so the writer task's "all `Outbound` senders dropped" shutdown detection could never fire,
     and `server::run` (and therefore the whole mcp-server process) would never return/exit on
     stdin close. Fixed by retaining the policy-subscription task's own `JoinHandle`
     (`policy_subscription`) and calling `.abort()` plus awaiting the cancellation immediately
     after the main read loop ends, before `drop(tx)` and `writer.await`. This is a correctness
     fix required for ADR-0025's own design to be able to shut down at all (the pre-existing
     watcher/audit-reload tasks never touched the `Outbound` channel, so this class of bug did
     not exist before this task); it changes no hot-reload SEMANTIC, only that the process now
     actually exits. Verified directly: before the fix, the flagship test's child process needed
     to be force-killed externally to ever return from `wait()`; after the fix, the same test
     completes in ~7.8s with a clean process exit.
  4. The flagship test's isolation mechanism deviates from the task prompt's literal instruction
     to override `LOCALAPPDATA` to redirect the default audit path. On this tree,
     `governance::audit::destinations::default_audit_path` resolves via the `dirs` crate, whose
     Windows backend (`dirs-sys` 0.5) calls `SHGetKnownFolderPath` rather than reading the
     `LOCALAPPDATA` environment variable, so an env-var override of the CHILD process has zero
     effect on it -- confirmed empirically: the first, prompt-literal version of this test (which
     also overrode `LOCALAPPDATA`) produced a completely empty audit file at the faked path, only
     the real, unfaked default path had the records. Since manifest config entries for audit are
     explicitly out of bounds (the very reason the prompt gives for the "no audit config
     entries" pin: they would vanish mid-test), and no other redirection mechanism exists for the
     BUILT-IN default without touching an ALSO-not-env-redirectable, higher-risk real file (the
     user config file), the test instead takes over the REAL resolved default audit path for its
     own duration: it backs up any pre-existing file there and restores it (or removes a
     freshly-created one) via a `Drop` guard (`AuditFileGuard`) that runs even on a mid-test
     panic. Documented in full in the test file's own module doc comment. Also added a
     `ChildGuard` (kills+reaps the spawned server on any early return or panic) since
     `std::process::Child` does not kill on drop -- discovered necessary after an earlier
     iteration of this test left orphaned `browser-mcp.exe` processes behind on a failed
     assertion.
- Deletions performed: `governance::config::reload::read_and_parse_org` (the org-only file
  read+parse; superseded by `apply_policy_and_config`'s single `source::load_policy` call, which
  now covers both org and user manifest sources in one parse per reload event).
- Verification: `cargo fmt` (applied) then `cargo fmt --check` clean; `cargo clippy --all-targets
  -- -D warnings` clean; `cargo test` fully green, 467 -> 475 (net +8:
  `manifest_identity_changed_detects_change_appearance_and_removal`,
  `keep_last_good_org_failure_does_not_publish_the_policy`,
  `org_removal_publishes_an_all_open_policy`, `user_manifest_deletion_keeps_last_good` in
  `reload.rs`; `user_manifest_ignored_transition_gate` in `ports.rs`;
  `manifest_reload_and_user_manifest_ignored_events_are_shaped` in `dispatch.rs`;
  `advertised_set_diff_gates_the_notification` in `server.rs`; the new integration test
  `tests/hot_reload.rs::org_policy_hot_swap_end_to_end`, the stage's flagship, 7.76s).
  `tests/architecture.rs` (4 tests), `tests/all_open_golden.rs` (3 tests), `tests/mcp_protocol.rs`
  (6 tests), and `tests/tool_schema_fidelity.rs` (7 tests) all pass unchanged. `git diff HEAD --
  src/transport/mcp/schemas/tools.json tests/tool_schema_fidelity.rs` and `git diff HEAD --
  Cargo.toml Cargo.lock` both empty. ASCII scan on all 9 touched/created files clean. The full
  `cargo test` run (475 tests across 17 binaries) is fully green.
- Browser checks queued: 2 (`t06-1`, `t06-2`, appended verbatim from the task prompt's
  Verification section to `docs/tasks/stage-2/BROWSER-TESTS.md`).

### t07 dead-seam and stub deletions -- 2026-07-03
- Commit: (see this task's commit)
- Files touched: `src/governance/ports.rs`, `src/browser/mod.rs`, `src/browser/tools/`
  (deleted: `computer.rs`, `find.rs`, `form_input.rs`, `javascript.rs`, `manage.rs`, `mod.rs`,
  `navigate.rs`, `network.rs`, `page_text.rs`, `read_page.rs`, `tabs.rs`), this file.
- Summary: implemented ADR-0024 Decision 5 in full. Deleted the four unwired placeholder
  items from `ports.rs`: `ToolId` (newtype), `ResourcePattern` (newtype), `DomainPolicy`
  (trait, with its `requires`/`matches`/`is_sacred`/`tool_surface` methods), `ResourceResolver`
  (async-fn-in-trait, with its `governing_resource` method). Verified via `rg -n
  "DomainPolicy|ResourceResolver|ToolId|ResourcePattern" src/ tests/` before deletion that
  every hit was the item's own definition, its own doc comment, or a doc-comment
  cross-reference from `browser/mod.rs` -- zero impls, zero consumers, zero dedicated inline
  tests anywhere in the tree, so no test needed retyping (constraint 1's "if a test uses one
  incidentally, retype it" did not apply; no test used any of the four at all). `ports.rs`'s
  module doc (top-of-file) and the one `DecisionRequest` field doc that named
  `ResourceResolver` (the only surviving mention outside the deleted trait's own doc comment)
  are reworded to describe resource resolution as an injected concrete value/fn at the
  enforcement point, never a port trait in the core (ADR-0024 Decision 5), naming that a
  future remote-PDP ADR reintroduces whatever resolver/policy seam it needs on its own terms.
  `src/browser/tools/` (11 files, zero functions, `mod.rs`'s doc table restating all 13 tool
  names under the deleted observe/mutate/manage classification) is deleted in full;
  `src/browser/mod.rs` drops `pub mod tools;` and its module doc's roster sentence is
  rewritten: it no longer claims to own "tool wrappers" via `[`tools`]`, instead naming
  `directory` as the sole per-tool authority with no per-tool code homes, and drops the
  `[`crate::governance::ports::DomainPolicy::requires`]` cross-reference (that path no
  longer exists). Constraint-3 sweep (`rg -n
  "DomainPolicy|ResourceResolver|observe/mutate|Observe tier|Mutate tier" src/`) confirms the
  only two survivors are `src/browser/directory.rs` line 6 and the rewritten
  `src/browser/mod.rs`'s own line naming the deletion -- both already name the current
  authority (the registry / ADR-0024) and state the classification table IS deleted (now
  true), so neither needed further editing per the constraint's own wording ("rewrite every
  survivor to name the current authority" -- these already do, they are not stale, they are
  the historical record of the deletion itself).
- Deviations from the prompt/ADR:
  1. The task's OPTIONAL `ports.rs` split (section 4: "if `ports.rs` still exceeds ~600
     lines, split it into a `ports/` directory... with `pub use` re-exports") is SKIPPED.
     After the four deletions `ports.rs` is 748 lines (down from 793), still above the ~600
     guideline. The task prompt explicitly sanctions skipping ("skip if the task is running
     heavy, and say so in the ledger"). Conservative choice per BOOTSTRAP rule 4 (fewer moving
     parts; behavior preservation over structure preservation): the file's remaining bulk is
     roughly half production code (types, `EffectiveMode`/`Capability`/`Denial`/`AuditRecord`/
     `SessionEventRecord`/`DecisionRequest`/`Decision`, the two zero-policy impls) and half
     inline tests (`#[cfg(test)] mod tests`, ~360 lines), all one cohesive concern (the S4
     policy-decision-point/policy-enforcement-point contract types); a directory split would
     touch every one of these type definitions and their re-export shims purely for line-count
     hygiene, adding surface area and re-export risk in an unattended run for a change the
     prompt itself frames as optional tidiness, not a required behavior or structural fix. No
     pinned signature, string, test, or `crate::governance::ports::X` path is affected either
     way (a split would have needed to keep every path compiling unchanged regardless).
- Deletions performed: `governance::ports::ToolId` (struct), `governance::ports::
  ResourcePattern` (struct), `governance::ports::DomainPolicy` (trait), `governance::ports::
  ResourceResolver` (trait); no inline tests were deleted with them (none existed -- the four
  items had zero dedicated test coverage in the pre-t07 tree, confirmed by the completeness
  `rg` above). `src/browser/tools/` subtree in full: `computer.rs`, `find.rs`,
  `form_input.rs`, `javascript.rs`, `manage.rs`, `mod.rs` (the roster/doc-table file),
  `navigate.rs`, `network.rs`, `page_text.rs`, `read_page.rs`, `tabs.rs` -- 11 files, zero
  functions, zero tests, stale observe/mutate/manage prose only.
- Verification: `cargo fmt` (applied) then `cargo fmt --check` clean; `cargo clippy
  --all-targets -- -D warnings` clean; `cargo test` fully green, 475 -> 475 (net 0: no test
  was deleted with its subject, since none of the four deleted items nor the deleted
  `browser/tools/` subtree carried any dedicated test). `tests/architecture.rs` (4 tests),
  `tests/all_open_golden.rs` (3 tests), `tests/mcp_protocol.rs` (6 tests), and
  `tests/tool_schema_fidelity.rs` (7 tests) all pass unchanged. `git diff HEAD --
  src/transport/mcp/schemas/tools.json tests/tool_schema_fidelity.rs` and `git diff HEAD --
  Cargo.toml Cargo.lock` both empty. Completeness checks: `rg -n
  "DomainPolicy|ResourceResolver|ToolId|ResourcePattern" src/ tests/` -> no hits; `rg -n
  "browser/tools|browser::tools" src/ tests/` -> no hits. `git diff --stat` touches exactly
  `src/browser/mod.rs`, `src/governance/ports.rs`, the 11 deleted `src/browser/tools/*.rs`
  files, and this ledger -- no `dispatch.rs` edit was needed (the live tree carried no
  `DomainPolicy`/`ResourceResolver`/etc. reference in `dispatch.rs`; the prompt's own
  Verification section's file list is a superset description, not a mandate that every named
  file must change). ASCII scan on both touched source files (`src/browser/mod.rs`,
  `src/governance/ports.rs`) clean.
- Browser checks queued: none (pure deletion/tidy; zero behavior change, zero new
  capability, per the task's own Goal and Out of scope sections).

### t08 documentation sync -- 2026-07-03
- Commit: (see this task's commit)
- Files touched: `docs/tasks/stage-2/00-shared-format.md`, `docs/tasks/stage-2/BROWSER-TESTS.md`,
  this file. `docs/adr/README.md` was NOT touched (its 0023/0024/0025 rows were already present
  at task start; verified via `rg -c "0023-|0024-|0025-" docs/adr/README.md` -> `3` before any
  edit, so Required Behavior 0 was a confirmed no-op, exactly as the prompt's own Current
  Behavior section anticipated as a possibility).
- Summary: docs-only sync, no code or test changes. In
  `docs/tasks/stage-2/00-shared-format.md`: inserted the ADR-0025 hot-reload NOTE immediately
  after section 1.3 (`User-supplied manifest`)'s selection-rule text and before the 1.4 heading,
  verbatim from the prompt; inserted the ADR-0022/ADR-0023 SUPERSEDED banner immediately before
  the `schema` field-rule bullet in section 4.1 (found at the prompt's predicted ~282, confirmed
  against the live tree before editing), verbatim from the prompt -- this is the fourth
  `SUPERSEDED by ADR-0022` banner in the file (three already existed from the stage-3 s08 pass
  at 4.1's grant-fields bullet, section 8's `rw` row, and section 8's whole-section note; this
  task's banner is a fourth, distinct insertion at the schema field, matching the prompt's own
  "the three s08 banners plus this task's schema banner" framing of the `rg -c` test); appended
  items 18/19/20 to section 10 (`SPEC updates needed`), verbatim from the prompt, covering
  ADR-0023/0024/0025 respectively (the list previously ended at item 17). In
  `docs/tasks/stage-2/BROWSER-TESTS.md`: appended the `t-live-1` consolidated live-check pointer
  (re-run stage-3's s-live-1..4 plus t01-1/t05-1/t06-1/t06-2 against the stage-4 tree) immediately
  after the existing `t06-2` entry, verbatim from the prompt. No SPEC.md edit, no ADR body edit,
  no CLAUDE.md edit (all out of scope per the prompt's own Constraints/Out of scope sections).
- Deviations from the prompt/ADR: none. Every inserted string (the ADR-0025 NOTE, the schema
  SUPERSEDED banner, items 18-20, and the `t-live-1` entry) is a verbatim transcription of the
  prompt's Required Behavior text; the ADR README rows required no action since a prior commit
  had already added them (an anticipated, not surprising, outcome per the prompt's own wording).
- Deletions performed: none (docs-only insertions; nothing superseded or removed this task).
- Verification: `cargo fmt` (applied; no diff, since no code changed) then `cargo fmt --check`
  clean; `cargo clippy --all-targets -- -D warnings` clean; `cargo test` fully green, 475 -> 475
  (net 0, as expected for a docs-only task; no test added or removed). `tests/architecture.rs`
  (4 tests), `tests/all_open_golden.rs` (3 tests), `tests/mcp_protocol.rs` (6 tests), and
  `tests/tool_schema_fidelity.rs` (7 tests) all pass unchanged. `git diff HEAD --
  src/transport/mcp/schemas/tools.json tests/tool_schema_fidelity.rs` and `git diff HEAD --
  Cargo.toml Cargo.lock` both empty. All five prompt-pinned `rg` assertions confirmed exactly:
  `rg -c "SUPERSEDED by ADR-0022" docs/tasks/stage-2/00-shared-format.md` -> `4`; `rg -c
  "ADR-0025" docs/tasks/stage-2/00-shared-format.md` -> `2` (at least 1, satisfied); `rg -n
  "^20\." docs/tasks/stage-2/00-shared-format.md` -> exactly one line (item 20); `rg -c "^##
  t-live-1" docs/tasks/stage-2/BROWSER-TESTS.md` -> `1`; `rg -c "0023-|0024-|0025-"
  docs/adr/README.md` -> `3`. ASCII scan of every added line (`git diff -U0 | grep "^+" | rg -n
  "[^\x00-\x7F]"`) -> no output (clean) on both touched files.
- Browser checks queued: none (this task adds a documentation pointer to existing deferred
  checks; it defers no new live check of its own).

## RUN SUMMARY (stage 4 complete: t01-t08)

Tasks completed, in order: t01 (one loader for the policy file, ADR-0023) -> t02 (the tool
registry, ADR-0024 Decision 1) -> t03 (governance authorize + `CallAudit`, ADR-0024 Decision 3)
-> t04 (the generic ingest pipeline, ADR-0024 Decision 2) -> t05 (one tab-URL resolution per
call, ADR-0024 Decision 4) -> t06 (manifest hot-reload, ADR-0025) -> t07 (dead-seam and stub
deletions, ADR-0024 Decision 5) -> t08 (this task: documentation sync). Commit range:
`b8225ef` (t01) through `c16cb44` (t07) landed before this task; this task's own commit
(`docs(architecture): t08 documentation sync`) follows immediately, closing the range at the
branch tip. Branch point: `stage-3` head (`b4b2faf`, 461 tests passing).

Every conservative choice made across the stage (one entry per task; full reasoning lives in
each task's own log entry above):
- t01: added `org_config_from_policy` as a small shared helper (not literally named in the
  prompt) so the origin-gated org-layer rule has exactly one implementation; reworded one
  historical sentence in the new integration test's doc comment to avoid tripping the prompt's
  own `rg` completeness check on its own substring.
- t02: retargeted `advertise.rs::tool_has_a_reachable_variant`'s direct `DIRECTORY` iteration
  onto `REGISTRY` (a third consumer the prompt's survey missed, unavoidable once `DIRECTORY` was
  deleted); added `#[allow(clippy::type_complexity)]` on the pinned `EXPECTED_TOOLS` tuple type
  rather than restructure a byte-pinned shape.
- t03: `CallAudit` additionally captures the client identity (necessary for `AuditRecord.client`
  with no back-reference to `Governance`); added the unpinned `landing_shadow_deny` method to
  correctly record a landing-recheck shadow-deny outcome (the seven-method pinned list omitted
  this arm, and reusing `landing_allow` would have silently misrecorded a shadow-deny as an
  allow).
- t04: reworded one `directory.rs` doc-comment line naming the deleted `is_known_tool` (a
  prompt self-inconsistency between "add nothing to directory.rs" and its own `rg` hygiene
  check); reworded `server.rs`'s module doc to describe the post-move pipeline location.
- t05: none (the `LazyTabUrl` shape is schematic in the prompt, not a pin).
- t06: additionally folds a user-sourced manifest's config entries into the user layer on every
  reload, not just at startup (otherwise the newly-watched user source would be observably inert
  for config); `#[allow(clippy::large_enum_variant)]` on the pinned `Outbound` enum rather than
  restructure it; fixed a real shutdown bug discovered while authoring the flagship test (the
  policy-subscription task's `JoinHandle` is now aborted before shutdown, or the process could
  never exit); the flagship test takes over the REAL default audit path (backed up and restored
  via a `Drop` guard) since `LOCALAPPDATA` override has no effect on the `dirs` crate's Windows
  backend.
- t07: skipped the OPTIONAL `ports.rs` directory split (748 lines, still above the ~600
  guideline but the prompt explicitly sanctions skipping with a ledger note).
- t08: none (every insertion is a verbatim transcription; the ADR README rows needed no action).

The DELETIONS LEDGER (this stage's deliverable is what got REMOVED; aggregated from every
task's own "Deletions performed" list above):
- t01: `governance::config::load::parse_org_config` (and its test
  `org_file_violations_are_errors`); `governance::config::load::load_and_resolve` (dead, zero
  callers); `ConfigStore::load_initial_with_manifest_config` (renamed/reshaped to
  `load_initial_with_policy`).
- t02: `browser::directory::ActionDescriptor` (struct) and `browser::directory::DIRECTORY`
  (const), absorbed into `REGISTRY[*].variants`; three superseded inline tests
  (`directory_covers_the_sacred_surface_exactly`, `directory_requires_match_the_adr_table`,
  `explain_text_is_the_vocabulary_block_then_one_line_per_row`), replaced by their reworked
  registry-shaped equivalents.
- t03: `Governance::record_call`, `record_deny`, `record_navigate_landing_deny`,
  `record_shadow_deny`, `record_held`, and their shared private `build_record`; the
  `Governance.requires` fn-pointer field and the `requires` parameter on `all_open`/`governed`;
  `decide`'s internal directory-miss branch and empty-requires shortcut; `enforcement::
  tool_label`; the `no_requires` test helper (both `dispatch.rs` and `tests/all_open_golden.rs`);
  the four `handle_tools_call` mutables (`audit_domain`, `audit_grant_id`, `shadow_denial`);
  eleven dispatch.rs inline tests directly driving the deleted API.
- t04: `transport::mcp::tools::is_known_tool` and its two unit tests; the now-empty `#[cfg(test)]
  mod tests` block in `tools.rs`.
- t05: `resolve_tab_host` (the sacred check's former internal `tabs_context_mcp` call plus its
  result-shape parsing); the now-orphaned test fixture helper `tabs_context_reply`.
- t06: `governance::config::reload::read_and_parse_org` (superseded by
  `apply_policy_and_config`'s single `source::load_policy` call).
- t07: `governance::ports::ToolId`, `ResourcePattern`, `DomainPolicy`, `ResourceResolver` (four
  unwired placeholder items, zero prior test coverage); the entire `src/browser/tools/` subtree
  (11 files: `computer.rs`, `find.rs`, `form_input.rs`, `javascript.rs`, `manage.rs`, `mod.rs`,
  `navigate.rs`, `network.rs`, `page_text.rs`, `read_page.rs`, `tabs.rs` -- zero functions, zero
  tests, stale observe/mutate/manage prose only).
- t08: none (docs-only task, no deletions).

State of `docs/tasks/stage-2/BROWSER-TESTS.md`: carries the full stage-2/stage-3 backlog
(g08/g10/g11/g13/g15, s01, s05, s07) plus the stage-3 s-live-1..4 entries, plus this stage's
four per-task entries (`t01-1`, `t05-1`, `t06-1`, `t06-2`) and this task's consolidated
`t-live-1` pointer tying the whole stage-4 regression pass together. Nothing in this file has
been run against a live browser during stage 4 itself (the executor has no live browser); every
entry remains queued for a human pass.

PLAIN STATEMENT (per BOOTSTRAP Completion): manifest hot-reload (ADR-0025) and the org-policy
loading fix (ADR-0023) are shipped in code and fully covered by the automated test wall (475
tests green, including the `tests/hot_reload.rs` flagship end-to-end test spawning the real
binary), but neither is verified end-to-end against a live browser and a live MCP client. A
human must run the stage-3 s-live-1..4 backlog plus `t01-1`, `t05-1`, `t06-1`, `t06-2`, and
`t-live-1` from `docs/tasks/stage-2/BROWSER-TESTS.md` before stage 4 is considered verified end
to end. No push, no merge performed; `stage-4` is left for a human to merge when ready.
