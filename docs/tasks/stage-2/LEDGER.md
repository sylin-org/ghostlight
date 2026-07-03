# Stage 2 ledger

Durable, context-wipe-safe record of stage-2 (governance) execution. This file plus
`BROWSER-TESTS.md` are the executor's memory. On every start, after any interruption, and whenever
state is unclear: read the RESUME HERE section first, then `PLAN.md` and `RECONCILIATION.md`, then the
current task prompt, then continue. Never rely on remembering earlier work; re-read files.

## RESUME HERE

- Branch: `stage-2` (off `main`, which has stage 1 merged). Never push, never merge, never commit to
  `main`.
- Progress: tasks `a1` (module reorg), `a2` (governance ports, + RwClass correction), `a3`
  (governance facade), `a7` (arch-test), `g01` (typed key registry), `g02` (layered
  resolution), `a5` (hot-reload substrate), `g03` (config CLI), `g04` (schema generation)
  landed. Phase A (foundations) is COMPLETE. `g05` (r/w classification), `g09` (manifest
  identity), `g06` (audit flight recorder), `g07` (domain matcher), `g08` (sacred domains,
  the FIRST real enforcement path), `g10` (take-the-wheel pause, the FIRST task touching
  `extension/`), `g11` (panic kill switch) landed. Phase C is COMPLETE. `g12` (manifest
  engine: parse/validate/load, no enforcement yet) landed. `g13` (grant enforcement at
  the five points -- the no-op policy seam is now real) landed. `g14` (tool advertisement
  filtering; dynamic re-advertisement deferred, see its ledger entry) landed. `g15`
  (shadow enforcement: the mode switch between real `deny` and observe-mode
  `shadow_deny`) landed. `g16` (`policy explain`: deterministic plain-language rendering,
  golden-tested) landed. `g17` (`policy simulate`: replays audit JSONL through the same
  `check_call` live enforcement uses, golden-tested) landed. `g18` (`config preset` +
  `policy init --template`, plus the preset-to-layer-4 wiring the whole feature depends
  on) landed. Phase D is COMPLETE. ALL 23 tasks in the BOOTSTRAP.md sequence are landed.
- NEXT TASK: none -- see the RUN SUMMARY at the end of this file. BOOTSTRAP.md's
  Completion section is done. Do not start a new task from this file; a human decides
  what happens next (BROWSER-TESTS.md, then a merge decision).
- Order authority: `PLAN.md` (Phase A -> B -> C -> D). Full linear sequence is in `BOOTSTRAP.md`.
- Reconciliation: `RECONCILIATION.md` is AUTHORITATIVE over any conflicting detail in a `g`-doc.
- Invariants that must hold after every task: all-open byte-identical (the all-open golden test +
  `tests/mcp_protocol.rs`), the sacred tool surface (`tests/tool_schema_fidelity.rs`), `cargo clippy
  --all-targets -- -D warnings` clean, `cargo fmt --check` clean, full `cargo test` green, ASCII-only.

## Task log

(Append one entry per completed task, newest at the bottom. Suggested shape:)

### <task-id> <title> -- <date>
- Commit: <hash>
- Files touched: <list>
- Summary: <what landed, key decisions, any conservative choice made>
- Deviations from the g-doc per RECONCILIATION.md: <placement / hot-reload / ports notes>
- Verification: clippy/fmt/test status; which tests were added
- Browser checks queued: <count> (appended to BROWSER-TESTS.md as <task-id>-<n>), or none

### a1 module reorg (governance/ browser/ transport/) -- 2026-07-02
- Commit: e66b02f
- Files touched: `git mv` of `src/dispatch.rs`, `src/policy/{mod.rs,redact.rs}`, `src/tools/**`,
  `src/native/**`, `src/mcp/**` (incl. `schemas/`), `src/browser.rs`; new
  `src/{governance,browser,transport}/mod.rs`; edited `src/lib.rs`, `src/main.rs`, `src/doctor.rs`,
  `src/install/native_host.rs`, `src/transport/executor.rs`, `src/transport/native/{ipc,messages}.rs`,
  `src/transport/mcp/server.rs`, `src/governance/policy/mod.rs`; new `tests/all_open_golden.rs`.
- Summary: pure move, zero behavior change. `governance/` got `dispatch.rs` + `policy/` (minus
  `redact.rs`); `browser/` got `tools/` + `redact.rs`; `transport/` got `native/`, `mcp/`, and
  `browser.rs` (renamed `executor.rs` to avoid colliding with the new `browser/` plugin module).
  Every `use crate::...` cross-reference rewritten to the new absolute path; the one cross-bucket
  call (`transport/mcp/server.rs` redacting `read_page` output) now calls
  `crate::browser::redact::apply_to_result` directly. `lib.rs` re-exports `pub use
  transport::{mcp, native};` so `tests/tool_schema_fidelity.rs` and `tests/mcp_protocol.rs` keep
  resolving `browser_mcp::mcp::...` / `browser_mcp::native::...` unchanged, per the task's compat-
  facade requirement.
- Deviations from the g-doc per RECONCILIATION.md: none (A1 is not a g-doc; it is one of the new
  a-prompts that already encodes the current vision). Followed a1-module-reorg.md as written.
- Verification: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings` clean, `cargo
  test` green (81 lib unit tests + 2 new `tests/all_open_golden.rs` + 4 `tests/mcp_protocol.rs`
  unchanged + 1 `tests/peer_death.rs` + 6 `tests/tool_schema_fidelity.rs` unchanged = 94 total).
  ASCII scan clean on every touched/moved file. Grep confirmed no stale `crate::browser::Browser`,
  `crate::dispatch`, `crate::policy`, `crate::mcp`, `crate::native`, `crate::tools` paths remain.
  `src/mcp/schemas/tools.json` -> `src/transport/mcp/schemas/tools.json` confirmed byte-identical
  (diff empty). One environment snag: `git mv src/mcp` (whole-directory rename) twice failed with
  Windows `Permission denied` (likely a transient AV/indexer lock); worked around by moving the 6
  files inside `src/mcp/` individually with `git mv`, then removing the resulting empty leftover
  `src/mcp/schemas/` and `src/mcp/` directories (untracked by git, harmless, but removed for
  tidiness) -- no conservative policy choice involved, purely a retry mechanic. No other locked-exe
  issue this task; `target/debug/browser-mcp.exe` needed the constraint-7 rename-aside once before
  the first build.
- Browser checks queued: none (binary-internal move; no user-visible behavior change per the task's
  own scope note).

### a2 governance ports (the seam contract) -- 2026-07-02
- Commit: 21994b6
- Files touched: new `src/governance/ports.rs`; one-line `pub mod ports;` edit to
  `src/governance/mod.rs`.
- Summary: purely additive seam contract. Added the axis/placeholder types (`RwClass`,
  `EffectiveMode`, `Grant`, `ToolId`, `ResourcePattern`, `Denial`, `AuditRecord`), the core
  decision types (`GoverningResource`, `DecisionRequest`, `Decision`), the traits
  (`PolicyDecisionPoint`, `DomainPolicy`, `ResourceResolver`, `AuditSink`), and the two
  zero-policy impls (`NoopPdp`, `NullSink`), exactly as specified in the task prompt. Nothing
  wired into `dispatch` yet (A3's job); no runtime behavior changed. `ResourceResolver` uses a
  native async fn in trait with `#[allow(async_fn_in_trait)]` (no `async-trait` dependency
  added), per constraint 9.
- Deviations from the g-doc per RECONCILIATION.md: the task prompt's literal example code used
  `RwClass::Read`/`RwClass::Write`; this was landed as-is and is WRONG per RECONCILIATION.md
  section 2, which is explicit that `RwClass` must be `Observe`/`Mutate` (distinct from a
  grant's `read`/`write`/`all` access field) and that a2/a3 prompt text using `Read`/`Write` is
  exactly the case to override. Caught before a3 consumed it; fixed in a follow-up correction
  commit (see the log entry below) rather than amending this commit, so the history stays
  linear per the one-task-one-commit rule.
- Verification: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings` clean
  (the single permitted `#[allow(async_fn_in_trait)]` suppression), `cargo test` green (88 lib
  unit tests, +7 new in `governance::ports::tests` covering noop-pdp-allows-all, null-sink-is-
  noop, both ports' dyn-object-safety, `DecisionRequest`/`Decision` serde round-trips, and the
  lowercase wire vocabulary for `RwClass`/`EffectiveMode`). `tests/tool_schema_fidelity.rs`,
  `tests/mcp_protocol.rs`, `tests/peer_death.rs`, and `tests/all_open_golden.rs` all unchanged
  and green. Arch-fence manual check: `ports.rs` has exactly one `use` statement (`use serde::
  {Deserialize, Serialize};`); `serde_json::Value` is referenced by full path inline. A grep
  for the bare word "browser" hits only doc-comment prose (e.g. "browser: a host such as
  github.com"), matching the task prompt's own example text verbatim -- no `use crate::browser`
  or similar import exists. ASCII scan clean.
- Browser checks queued: none (pure library addition; nothing runtime-observable changed).

### correction: RwClass Observe/Mutate rename -- 2026-07-02
- Commit: 8da1bee
- Files touched: `src/governance/ports.rs` only (variant names + doc comment + every test use).
- Summary: renamed `RwClass::{Read,Write}` to `RwClass::{Observe,Mutate}` per
  RECONCILIATION.md section 2, which is explicit-by-name that a2/a3 prompt text guessing
  `Read`/`Write` (or a bare `Observe` without a `Mutate` sibling) must be overridden to
  `Observe`/`Mutate`, kept distinct from a grant's `access: read|write|all` field. Wire form is
  now `"observe"`/`"mutate"` (was `"read"`/`"write"`). No other type or trait touched; caught
  during a3 prep, before any other file consumed the wrong names, so this is a single
  self-contained rename with zero blast radius beyond `ports.rs`.
- Verification: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings` clean,
  `cargo test` green (same 88 lib tests, all 7 `governance::ports::tests` still passing with the
  new variant names and wire strings).
- Browser checks queued: none.

### a3 governance facade (dispatch chokepoint) -- 2026-07-02
- Commit: (see this task's commit)
- Files touched: `src/governance/dispatch.rs` (rewritten: removed the no-op
  `PolicyDecision`/`policy_check`/`audit` seam, added the `Governance` facade); rewired
  `src/transport/mcp/server.rs` (threads `Arc<Governance>` through `run` -> `handle_line` ->
  `handle_tools_call`, replacing the two no-op seam calls with one `governance.decide(name)`);
  extended `tests/all_open_golden.rs` (added `facade_decide_is_all_open_after_the_move` and
  `read_page_redaction_is_still_wired_at_the_chokepoint`, renamed the old
  `dispatch_seam_is_all_open_after_the_move` since the free functions it tested no longer exist).
- Summary: `Governance` holds either `Mode::AllOpen` (zero-port, STEP-0 short-circuit to
  `Decision::Allow { grant_id: None }`) or `Mode::Governed(GovernedState)` (a boxed
  `PolicyDecisionPoint` + an `Arc<dyn AuditSink>`, exercised only by the new facade unit tests,
  not by any production path yet). `decide` stays sync; the `Governed` branch builds a
  placeholder `DecisionRequest` (empty grants, `RwClass::Observe`, `GoverningResource::None`,
  `EffectiveMode::Observe`) and asks the held PDP -- with `NoopPdp` the result is still `Allow`.
  The MCP server constructs `Governance::all_open()` once per session and calls `decide` at the
  same chokepoint position the old two-line seam occupied; the decision is still bound to
  `_decision` and ignored (no enforcement yet). `read_page` redaction is untouched in place.
- Deviations from the g-doc per RECONCILIATION.md: used A2's real port/type names throughout
  (`NoopPdp`, not the prompt's guessed `NoopPolicyDecisionPoint`; `RwClass::Observe`, already
  corrected in the prior ledger entry) per constraint 8 ("match A2's exact names").
- Verification: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings` clean (no
  `#[allow(dead_code)]` added; `pdp`/`audit` stay live via `decide`/`audit_sink`), `cargo test`
  green (90 lib unit tests incl. the 2 new `governance::dispatch::tests`; `tests/all_open_golden.rs`
  3 tests incl. the 2 new; `tests/mcp_protocol.rs` UNCHANGED and green -- exact byte-identical
  `tools/list` and the exact no-extension hop-attributed message; `tests/tool_schema_fidelity.rs`
  and `tests/peer_death.rs` unchanged). Grep confirmed `policy_check`/`PolicyDecision` no longer
  appear anywhere except one historical mention in `dispatch.rs`'s own module doc ("replaces the
  v1.0 no-op `policy_check` / `audit` seams"). ASCII scan clean.
- Browser checks queued: none (binary-only chokepoint change; manual verification note per the
  task's own Verification step 5 -- tools/list still shows 13 tools, a call with Chrome closed
  still times out at ~5s, read_page redaction still defaults on -- is covered by the automated
  `tests/all_open_golden.rs::read_page_redaction_is_still_wired_at_the_chokepoint` test added this
  task, so no live-browser check is queued).

### a7 arch-test (fail-closed governance/ boundary guard) -- 2026-07-02
- Commit: (see this task's commit)
- Files touched: new `tests/architecture.rs` only.
- Summary: a pure `std::fs` + text-scan integration test that recursively walks
  `src/governance/` and fails if any `.rs` file names `crate::browser`, `crate::transport`,
  `crate::mcp`, `crate::native`, or the `url` crate (path-token matched with identifier
  boundaries, scanning raw lines including comments/strings, not just compiled code). Both
  fail-closed properties are in place: a missing `src/governance/` fails loudly (does not
  skip), and an empty directory fails rather than passing vacuously. Landed exactly as the
  task's literal code specified, verbatim.
- Deviations from the g-doc per RECONCILIATION.md: none (A7 is an a-prompt, not a g-doc).
  Followed a7-arch-test.md as written, byte for byte.
- Verification: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings` clean,
  `cargo test` green (90 lib unit tests unchanged; new `tests/architecture.rs` 4 tests --
  `governance_core_has_no_forbidden_back_edges`, `scanner_detects_forbidden_crate_edges`,
  `scanner_detects_url_crate_reference`, `scanner_ignores_clean_lines`; `tests/all_open_golden.rs`
  3 unchanged; `tests/mcp_protocol.rs` 4 unchanged; `tests/peer_death.rs` 1 unchanged;
  `tests/tool_schema_fidelity.rs` 6 unchanged). Negative check per Verification step 4: added a
  temporary `use crate::browser::redact;` line to the end of `src/governance/dispatch.rs`, ran
  `cargo test --test architecture`, confirmed `governance_core_has_no_forbidden_back_edges`
  FAILED naming the exact file, line 138, and the edge `crate::browser`; reverted with
  `git checkout -- src/governance/dispatch.rs` and confirmed `git status` showed no diff before
  re-running green. Robustness check per step 5: ran `cargo test --test architecture` from `src/`
  (both with and without an explicit `--manifest-path`) and confirmed it still passes, since
  the scanner anchors on `CARGO_MANIFEST_DIR`, not the working directory. ASCII scan clean.
- Browser checks queued: none (pure build-time/test-time guard; no runtime or browser-facing
  behavior).

### g01 typed key registry (value types beyond bool) -- 2026-07-02
- Commit: (see this task's commit)
- Files touched: `src/governance/config/mod.rs` (renamed from `src/governance/policy/mod.rs`,
  rewritten: full typed registry replacing the bool-only prototype); `src/governance/mod.rs`
  (`pub mod policy;` -> `pub mod config;`); new `src/browser/pattern.rs`; `src/browser/mod.rs`
  (`pub mod pattern;`); `src/transport/mcp/server.rs` (Config import path, `&Config` threading,
  `FIRST_CALL_WAIT_MS` constant removed and replaced by `config.first_call_wait_ms()`).
- Summary: grew the registry to the full value model (`KeyValue`/`ConfigValue`/`KeyType`/
  `KeyConstraint`/`Preset`), registered the seven stage-2 keys exactly per shared-format-doc
  3.4 (`engine.connection.first_call_wait_ms`, `content.security.secrets.redact`,
  `content.security.sacred_domains`, `audit.enabled`, `audit.destination`, `audit.file.path`,
  `governance.mode`), added `KeyDef::parse_value` with the exact `ConfigValueError` display
  vocabulary, grew `Config` to seven owned fields (loses `Copy`, gains `Clone`), and wired
  `first_call_wait_ms` into the two `Duration::from_millis(FIRST_CALL_WAIT_MS)` call sites in
  the MCP server (the T04 timeout constant this task was scoped to retire). All defaults for
  `content.security.sacred_domains` are `StrList(&[])` for every preset, so `Config::from_preset`
  never needs the domain-pattern validator (it reads registry defaults directly, no JSON
  round-trip).
- Deviations from the g-doc per RECONCILIATION.md (both significant; g01's own doc predates
  A1 and assumes the flat `src/policy/mod.rs` layout):
  1. **Placement.** RECONCILIATION.md section 1 maps `src/policy/mod.rs` (registry, resolver,
     Config) to `governance/config/`, not the `governance/policy/` name A1 produced by a literal
     directory move. Renamed the directory as part of this task (`git mv
     src/governance/policy src/governance/config`), updated `governance/mod.rs`'s module
     declaration, and repointed the one external import site
     (`transport/mcp/server.rs`: `governance::policy::Config` -> `governance::config::Config`).
  2. **The domain-pattern validator (the RECONCILIATION section 2 "known integration point,
     resolve during g01/a1").** g01's own doc puts `pattern.rs` under `src/policy/pattern.rs`
     (i.e. inside governance) and has `parse_value` call
     `crate::policy::pattern::is_valid_pattern` directly. RECONCILIATION.md is explicit that the
     pattern grammar is browser-domain (`browser/pattern.rs`) and that `governance/config` must
     not name `browser::` (the a7 arch-test forbids it), offering two resolutions: inject a
     validator hook, or carry the domain-pattern key in a browser key catalog. Chose the
     injection hook (simpler than splitting `KEYS` into two composed catalogs, which would
     ripple into every later G02/G03/G04/G12 consumer of a single flat registry): `pattern.rs`
     landed in `src/browser/pattern.rs` (also the future home G07's matcher extends, per
     RECONCILIATION's own placement table), and `KeyDef::parse_value` gained a
     `domain_pattern_valid: fn(&str) -> bool` parameter, consulted only for the
     `DomainPatternList` constraint. `governance/config`'s own tests use a small test-local
     validator (duplicating the grammar) so they never depend on the browser plugin; the
     authoritative grammar and its exhaustive test list (part 5 of g01's doc) live in
     `browser/pattern.rs`'s own tests. Verified via `cargo test --test architecture`: zero
     forbidden edges.
  3. Minor: kept `audit.destination` / `audit.file.path` descriptions ending "Takes effect on
     restart" per g01's literal text -- RECONCILIATION.md section 3 says these should eventually
     drop that clause once hot-reload (A5) and the audit sink re-open (G06) exist, but neither
     has landed yet at this point in the task sequence (A5 is the very next task after G02), so
     the restart-only wording is still truthful today. Revisit when A5+G06 land.
- Verification: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings` clean,
  `cargo test` green (104 lib unit tests, up from 90: +13 new in `governance::config::tests`,
  +2 new in `browser::pattern::tests`; `tests/all_open_golden.rs` 3 unchanged;
  `tests/architecture.rs` 4 unchanged and still green after the `governance/config` rename
  -- confirms zero forbidden edges introduced; `tests/mcp_protocol.rs` 4 unchanged;
  `tests/peer_death.rs` 1 unchanged; `tests/tool_schema_fidelity.rs` 6 unchanged). Grep confirmed
  `FIRST_CALL_WAIT_MS` and `minimal_default` no longer appear anywhere in `src/`. ASCII scan
  clean on every touched/new file.
- Browser checks queued: none (binary-only config/registry growth; the wired
  `first_call_wait_ms` value is 5000 under the Safe/Minimal preset, byte-identical to the
  retired constant, so no behavior changed).

### g02 layered configuration resolution and file loading -- 2026-07-02
- Commit: (see this task's commit)
- Files touched: new `src/governance/config/layers.rs` (the ADR-0019 five-layer resolver) and
  `src/governance/config/load.rs` (paths, file parsing, orchestration); `src/governance/config/mod.rs`
  (`pub mod layers;`/`pub mod load;`, `Config::from_resolution` + four `resolved_*` helpers);
  `src/error.rs` (one new variant, `Error::Config(String)`); `src/transport/mcp/server.rs`
  (startup now calls `load::load_and_resolve` + `Config::from_resolution` instead of
  `Config::default()`).
- Summary: `layers::resolve` walks `KEYS` and picks, for each key, the first of
  org_mandatory/user/org_recommended/preset/builtin that defines it, returning the shared-format
  2.1 triple (value/source/locked); `layers::validate_value` delegates to G01's
  `KeyDef::parse_value`. `load::user_config_path`/`org_policy_path` implement the exact
  shared-format 1.1/1.2 per-platform paths (Windows/macOS/Linux `cfg` branches, `ProgramData`
  env fallback). `load::parse_user_config` is lenient per entry (warn + skip unknown keys,
  invalid values, unknown presets, unknown top-level members; hard error only on structurally
  broken JSON). `load::parse_org_config` is strict everywhere (every violation --
  bad/missing schema, non-array config, unknown key, invalid value, bad level, duplicate key,
  unexpected member -- is a hard `Error::Config`). `load_and_resolve` reads both files
  (`ErrorKind::NotFound` -> absent/empty layer; any other I/O error -> hard error), logs
  warnings via `tracing::warn!`, and resolves. `Config::from_resolution` builds the typed
  session `Config` from a `Resolution`, with a `debug_assert!`-guarded fallback to the Safe
  preset default on an unreachable-by-construction shape mismatch (mirroring the `preset_*`
  helpers' panic-is-unreachable reasoning from G01, but non-panicking since a resolution is
  runtime-influenced by file content rather than purely compile-time).
- Deviations from the g-doc per RECONCILIATION.md / carried forward from G01's precedent: g02's
  own doc (written pre-A1/G01) specifies `validate_value(def, value) -> Result<(), String>` and
  `load_and_resolve() -> Result<Resolution>` with NO domain-pattern-validator parameter, and has
  `parse_user_config`/`parse_org_config` likewise take no such parameter. Since G01 threaded a
  `domain_pattern_valid: fn(&str) -> bool` into `KeyDef::parse_value` (the RECONCILIATION
  section 2 "known integration point": the governance core cannot name the browser plugin's
  pattern grammar directly), every function in this task that ultimately validates a
  `content.security.sacred_domains` value inherits that same extra parameter:
  `validate_value`, `parse_user_config`, `parse_org_config`, and `load_and_resolve` all gained
  a `domain_pattern_valid: fn(&str) -> bool` parameter, threaded from `transport/mcp/server.rs`
  (which supplies `browser::pattern::is_valid_pattern`, the real grammar) down to
  `layers::validate_value`'s call into `KeyDef::parse_value`. This is the same shape of
  deviation G01 already made and is not a new architectural decision, just its continuation.
  Placement: `layers.rs` and `load.rs` land in `governance/config/` (not a flat
  `src/policy/{layers,load}.rs`), per RECONCILIATION section 1's mapping, consistent with G01.
- Verification: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings` clean
  (fixed two lints along the way: a doc-comment line break that clippy's `doc_lazy_continuation`
  read as an unclosed markdown blockquote due to a mid-sentence `>` at a line wrap -- reworded
  to avoid `>` entirely; and `needless_return` in `org_policy_path`'s per-platform `cfg` blocks,
  restructured to `#[cfg(..)] let path = ...;` per-platform bindings ending in a single tail
  `path` expression instead of early `return`s under an `#[allow(unreachable_code)]`). `cargo
  test` green (119 lib unit tests, up from 104: +6 new in `governance::config::layers::tests`,
  +8 new in `governance::config::load::tests`, including a windows-`cfg`-gated
  `paths_follow_the_shared_format_locations`; `tests/all_open_golden.rs` 3 unchanged;
  `tests/architecture.rs` 4 unchanged and still green -- confirms `governance/config/{layers,load}.rs`
  introduce zero forbidden edges despite doing real file I/O and platform-path logic;
  `tests/mcp_protocol.rs` 4 unchanged, including the byte-identical `tools/list` assertion --
  proves the layered resolver with both files absent is byte-identical to the old
  `Config::default()` path; `tests/peer_death.rs` 1 unchanged; `tests/tool_schema_fidelity.rs`
  6 unchanged). Confirmed no stray `%APPDATA%\browser-mcp\config.json` or
  `%ProgramData%\browser-mcp\policy.json` exists on the dev machine before running (both
  `Test-Path` false), so the live binary spawned by `tests/mcp_protocol.rs` resolves through
  the builtin layer only, exactly as required by the task's own verification note. ASCII scan
  clean on every touched/new file.
- Browser checks queued: none (binary-only startup wiring; no browser-facing behavior changed).

### a5 hot-reload substrate (atomic swap + debounced watch + validate-then-swap) -- 2026-07-02
- Commit: (see this task's commit)
- Files touched: new `src/governance/config/reload.rs`; `src/governance/config/mod.rs`
  (`pub mod reload;`); `src/governance/config/load.rs` (added `PartialEq` to `OrgConfig`, needed
  for the store's no-change swap check); `src/transport/mcp/server.rs` (startup now builds an
  `Arc<ConfigStore>` + spawns the watcher instead of a one-shot `load_and_resolve`; `handle_line`
  / `handle_tools_call` thread `&Arc<ConfigStore>` and take a fresh `store.current()` snapshot
  per call instead of a config value/reference).
- Summary: `ConfigStore` holds the in-force `Config` behind `Mutex<Arc<Config>>` (not `ArcSwap`;
  justified in the module doc -- the critical section is a single Arc clone, so a plain mutex
  costs nothing extra and adds zero dependencies), a monotonic generation counter, a
  `tokio::sync::watch` change signal for the future G14 `list_changed` emit, and the last-good
  layer inputs per source. `load_initial` is FAIL-LOUD (an invalid org file or broken user file
  refuses to start the server); `reresolve`/`notify_local_edit` are validate-then-swap and NEVER
  return an error -- `plan_reload` is the pure security-rule planner: an invalid user source
  keeps last-good and WARNs, an invalid org source keeps last-good and ERRORs (fail-closed,
  verified end-to-end through the resolver in `invalid_org_is_fail_closed`). The watcher is a
  zero-dependency debounced mtime poll (`fingerprint` + `settle`) over the three source paths
  (user config, org policy, and a `None` manifest slot marked as a G12 integration point),
  ticking every 750ms; `spawn_watcher` is called once at mcp-server startup only. The server's
  per-call config read (`store.current()`) is what makes a mid-session reload take effect on
  the very next call with no other plumbing.
- Deviations from the g-doc per RECONCILIATION.md / carried forward from G01/G02's precedent:
  the task's own literal signatures (`load_initial() -> Result<Arc<ConfigStore>>`, no parameter)
  predate G01/G02's domain-pattern-validator injection. Since `ConfigStore` calls the same G02
  parsers that need `domain_pattern_valid: fn(&str) -> bool`, and the watcher loop re-invokes
  `reresolve()` with no per-call way to supply one, the validator is stored as a `ConfigStore`
  field (set once via `load_initial(domain_pattern_valid)`) rather than threaded as a
  per-call parameter -- the same shape of deviation as G01/G02, just carried one level further
  since this is the first task where the same validator must be reused across many calls over
  the store's lifetime, not just one. `reload.rs` lands in `governance/config/` (not a flat
  `src/policy/reload.rs`), consistent with G01/G02's RECONCILIATION section 1 placement.
- Verification: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings` clean
  (one fix along the way: added `#[derive(PartialEq)]` to `OrgConfig`, needed for
  `plan.new_last_good.org == last_good.org`-style test assertions and, more importantly, for the
  `**slot == *candidate` no-change check inside `Config`'s derive chain -- `Config` itself
  already derived `PartialEq` from G01/G02, `OrgConfig` had not needed to until this task
  compared it directly in tests). `cargo test` green (129 lib unit tests, up from 119: +10 new
  in `governance::config::reload::tests` covering the pure planner's four security-rule cases,
  the store's swap/generation/signal behavior including a no-receivers case and a
  `#[tokio::test]` for the change-signal wake, the debounce settle function's create/delete/
  flicker cases, and the fail-loud-vs-keep-last-good boundary; `tests/all_open_golden.rs` 3
  unchanged; `tests/architecture.rs` 4 unchanged and still green -- confirms the new watcher/swap
  code introduces zero forbidden edges; `tests/mcp_protocol.rs` 4 unchanged, including the
  byte-identical `tools/list` assertion, now exercised against a binary that spawns a live
  background watcher task at startup -- proves the hot-reload substrate is behavior-preserving
  end to end; `tests/peer_death.rs` 1 unchanged; `tests/tool_schema_fidelity.rs` 6 unchanged).
  ASCII scan clean on every touched/new file.
- Browser checks queued: none (this task needs no browser at all). Not queued to
  BROWSER-TESTS.md, which is reserved for checks that need a live browser; noted here instead.
  The task's own Verification step 6 (manual hot-reload smoke: edit the real
  `%APPDATA%\browser-mcp\config.json` / `%ProgramData%\browser-mcp\policy.json` and watch
  stderr) was NOT run in this pass: it requires writing to fixed, non-bypassable system config
  paths shared with any other browser-mcp install/session on this machine (the org path
  requires admin rights on Windows), which is a live-system-state change outside this
  unattended run's scope. The automated suite exercises the same fail-closed/keep-last-good
  logic end to end through the resolver (see `invalid_org_is_fail_closed`), so the behavior is
  covered; only the literal file-watch-plus-stderr smoke is deferred, to a human, next to the
  standard BROWSER-TESTS.md pass.

### g03 config CLI (list / get / set with source and lock display) -- 2026-07-02
- Commit: (see this task's commit)
- Files touched: new `src/governance/config/cli.rs`; `src/governance/config/mod.rs`
  (`pub mod cli;`); `src/main.rs` (`ConfigArgs`/`ConfigAction` clap types, `Command::Config`
  variant, the `From` impl, and the match arm); `src/error.rs` (changed `Error::Config`'s
  Display from `"configuration error: {0}"` to `"{0}"`, see deviation below -- no new variant
  added, since G02 already introduced `Error::Config(String)`).
- Summary: `browser-mcp config list` prints the pinned ASCII table (key/value/source/locked/
  description, `{:<40}{:<24}{:<17}{:<8}{}` per row) in registry order; `config get <key>`
  prints the pinned five-line block; `config set <key> <value>` does lock-check (before any
  parsing or file access) -> parse-by-type -> registry validation -> value-preserving atomic
  write to the user config file (via the existing `install::native_host::write_file_atomic`)
  -> two-line success output. Unknown keys, invalid values, and locked keys all produce exact
  pinned `Error::Config` messages, surfaced by the top-level `anyhow` path as `Error: <message>`
  with exit code 1. Warnings from loading the user/org files print as `warning: <text>` on
  stderr via a dedicated `resolve_with_warnings` helper (not `tracing`, so the CLI's output
  stays exactly the pinned format rather than a logging format).
- Deviations from the g-doc per RECONCILIATION.md / carried forward from G01/G02/A5's
  precedent:
  1. **No new `Error::Config` variant.** The g03 doc (written before G02 landed) assumes it
     adds `Error::Config(String)` itself with Display `"{0}"` (no prefix). G02 already added
     `Error::Config(String)` with Display `"configuration error: {0}"` for file-load failures.
     Since a Rust enum cannot have two variants of the same name, and no existing test pins the
     "configuration error: " prefix text (grep-verified: only `.contains(...)` substring checks
     exist), changed the ONE existing variant's Display to `"{0}"` and updated its doc comment
     to cover both file-load and CLI-request failures. This reconciles the two tasks' needs
     with a single one-line change and produces exactly the pinned CLI messages (e.g.
     `Error: unknown config key '...'`, not `Error: configuration error: unknown config key ...`).
  2. **`domain_pattern_valid` threading, one level further than G01/G02/A5.** `cli.rs` lives in
     `governance/config/` (RECONCILIATION section 1 placement, consistent with G01/G02/A5) and
     therefore cannot name the browser plugin's real pattern-syntax checker directly (the a7
     arch-test). `ConfigCommand::run` and every function it calls take
     `domain_pattern_valid: fn(&str) -> bool` as an explicit parameter; `src/main.rs` (the
     composition root, free to depend on `browser::`) supplies
     `browser_mcp::browser::pattern::is_valid_pattern` at the one call site. This is the same
     deviation shape as G01/G02/A5, now propagated to the CLI's public entry point.
  3. **No shared loading function reused verbatim.** `resolve_with_warnings` re-implements the
     read+parse+resolve orchestration `load::load_and_resolve` already does, rather than calling
     it, because `load_and_resolve` (a) does not return warnings (it only logs them via
     `tracing`, wrong format for the CLI) and (b) its warning-emission is a side effect the CLI
     needs to suppress and re-render itself. Rather than widen `reload.rs`'s private
     `read_and_parse_org`/`read_and_parse_user` helpers to `pub(super)` for reuse (which would
     touch a file g03's own scope note does not list), `cli.rs` duplicates the ~25-line
     NotFound-tolerant read+parse orchestration locally. This keeps the touched-file list
     exactly what the task specifies (plus the one new file) at the cost of a third copy of
     that small orchestration (the other two are in `load.rs`'s own `load_and_resolve` and
     `reload.rs`'s `read_and_parse_*`); noted as a deliberate scope-discipline trade-off.
- Verification: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings` clean.
  `cargo test` green (143 lib unit tests, up from 129: +14 new in `governance::config::cli::tests`
  covering pinned list/get rendering across all five source values with a genuine locked key,
  every value-type parse path including all the specified accept/reject cases, the lock-refusal
  message text, and five user-file-write scenarios -- preserve siblings, replace in place,
  create missing file, refuse invalid JSON untouched, refuse non-object root untouched; all
  other suites unchanged: `tests/all_open_golden.rs` 3, `tests/architecture.rs` 4 (confirms
  `governance/config/cli.rs` introduces zero forbidden edges despite calling into file I/O and
  the write-atomic installer helper), `tests/mcp_protocol.rs` 4, `tests/peer_death.rs` 1,
  `tests/tool_schema_fidelity.rs` 6). Manual smoke per the task's own Verification step 4, run
  live since it needs no browser: `config list` (pinned table, all 7 keys, builtin source,
  exit 0), `config get audit.destination` (pinned 5 lines, exit 0), `config get`/`config set`
  on `no.such.key` (both exact unknown-key message, exit 1), `config set audit.destination
  bogus` (`Error: invalid value for audit.destination: expected one of: file, stderr`, exit 1),
  `config set audit.destination stderr` (two-line success, exit 0) followed by `config get`
  showing `source: user` and the real `%APPDATA%\browser-mcp\config.json` containing exactly
  `{"config":{"audit.destination":"stderr"}}` -- confirmed the file did not exist before this
  test and deleted it (and the now-empty parent dir) afterward, restoring the pre-test state;
  `cargo test` re-run clean after cleanup to confirm no stray state. ASCII scan clean on every
  touched/new file.
- Browser checks queued: none (this task needs no browser; the manual smoke above was run live
  in this pass since it required only the binary, not BROWSER-TESTS.md deferral).

### g04 generated JSON Schema and key reference docs -- 2026-07-02
- Commit: (see this task's commit)
- Files touched: new `src/governance/config/schema.rs`; `src/governance/config/mod.rs`
  (`pub mod schema;`); `src/governance/config/cli.rs` (added `Schema`/`Docs` variants to
  `ConfigCommand` and their `run()` arms, since g03 already landed a `ConfigCommand` enum this
  task's own doc only anticipated might exist); `src/main.rs` (`ConfigAction::{Schema,Docs}`,
  the `From` mapping); new `tests/config_schema_golden.rs`; new `tests/golden/config-schema.json`,
  `tests/golden/config-keys.md`, `tests/golden/.gitattributes` (`* text eol=lf`, the whole fix
  for CRLF-safe goldens on a Windows checkout, since the repo has no root `.gitattributes`).
- Summary: `key_value_schema` maps each registry key's type/constraint/description/Safe-preset
  default to a JSON Schema fragment (member order description/type/constraint-fields/default,
  exactly per the type table); `config_file_schema` assembles the full user-config-file
  document (the three pinned description/title strings verbatim, no `$id`, no `required`,
  `additionalProperties: false` at both the root and inside `properties.config`, one property
  per `KEYS` entry in registry order via `preserve_order`); `render_config_schema` is
  `to_string_pretty` plus one trailing LF. `render_key_reference` builds the markdown from a
  pinned header plus one section per key (type word, constraints phrase, three preset
  defaults as compact JSON), joined with exactly one blank line between sections. Wired as
  `browser-mcp config schema` / `config docs`, both synchronous, `print!` (not `println!`,
  since the rendered strings already end in one LF).
- Deviations from the g-doc per RECONCILIATION.md / carried forward from prior tasks: none of
  the "known integration point" domain-pattern-validator threading applies here (schema
  generation is pure registry introspection, no file loading, so no `fn(&str) -> bool` needed).
  One deliberate gap-fill beyond the task's own literal spec: the constraints-phrase table in
  the g04 doc enumerates bool/uint/enum/string(none)/string-list(with or without pattern), but
  does not address `KeyConstraint::EmptyOrAbsolutePath` (the constraint G01 gave
  `audit.file.path`, a Str-type key), since g04's doc predates that constraint variant.
  Rendering "none" for it would be untruthful documentation (constraint 5's "the engine is
  truthful" applies to generated docs too), so `constraints_phrase` gives it its own phrase,
  "empty string, or an absolute path" (mirroring `ConfigValueError::ExpectedAbsolutePath`'s
  wording), keyed on base type first so the fallback for any other unexpected
  type/constraint pairing is a safe "none" rather than a panic. Placement:
  `schema.rs` lands in `governance/config/` per the same RECONCILIATION section 1 mapping as
  `layers.rs`/`load.rs`/`reload.rs`/`cli.rs`.
- Verification: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings` clean.
  `cargo test` green (146 lib unit tests, up from 143: +3 new in `governance::config::schema::tests`;
  5 new in `tests/config_schema_golden.rs` -- byte-exact golden match for both outputs, full
  registry-coverage cross-check in both directions, section-count cross-check for the markdown,
  and an ASCII/no-CR check; all other suites unchanged: `tests/all_open_golden.rs` 3,
  `tests/architecture.rs` 4 -- confirms `governance/config/schema.rs` introduces zero forbidden
  edges, `tests/mcp_protocol.rs` 4, `tests/peer_death.rs` 1, `tests/tool_schema_fidelity.rs` 6).
  Golden bootstrap procedure followed exactly: implemented first, generated both goldens via
  `cargo run --quiet -- config schema`/`config docs`, reviewed both BY HAND against the task's
  sections 2-4 (member order, the three pinned description/title strings, all seven keys
  present in registry order, every constraint phrase, all three preset defaults per key) before
  writing the golden tests. Manual checks per the task's own Verification steps 4-6, run live
  (schema/docs generation needs no browser and touches no file outside `tests/golden/`):
  `config schema` output starts with `{` and contains the `$schema` URL; `config docs` output's
  first line is exactly `# Configuration key reference`; `cargo run -- config schema | diff -
  tests/golden/config-schema.json` and the `docs`/`config-keys.md` equivalent both report zero
  diff. Confirmed the staged golden blobs contain zero CR bytes (`git show :tests/golden/...`),
  proving the scoped `.gitattributes` actually takes effect on this Windows checkout. ASCII scan
  clean on every touched/new file including both golden files.
- Browser checks queued: none (pure registry introspection; needs no browser, no file I/O
  outside the two golden files this commit already carries).

### g05 read/write classification table -- 2026-07-02
- Commit: (see this task's commit)
- Files touched: `src/governance/ports.rs` (added `RwClass::as_str()`, no new type -- A2
  already created `RwClass` with `Observe`/`Mutate` variants); new `src/browser/classify.rs`
  (`TOOL_CLASSES`, `COMPUTER_ACTION_CLASSES`, `classify()`); `src/browser/mod.rs`
  (`pub mod classify;`).
- Summary: `classify(tool, action) -> Option<RwClass>` is the authoritative observe/mutate
  classification of the 13-tool sacred surface: 12 tools classified directly via
  `TOOL_CLASSES` (`computer` deliberately excluded), plus the 13 `computer` sub-actions via
  `COMPUTER_ACTION_CLASSES`, both authored in tools.json advertised/enum order and pinned
  call-by-call against the shared-format-doc section 8 table. Pure linear-scan lookup, no
  policy decision -- `None` is a classification miss, consumed by later tasks (G06 audit,
  grant enforcement), not acted on here.
- Deviations from the g-doc per RECONCILIATION.md (significant placement split, not carried
  from prior precedent): g05's own doc (written before A1/A2) puts BOTH the `RwClass` type
  AND the classification tables in one new file `src/policy/classify.rs`. RECONCILIATION.md
  section 1 explicitly splits this: "r/w classification TABLE (g05) -> browser/ (the 13-tool
  table) behind the Classifier port; the observe/mutate axis type in governance/ -- table is
  the plugin, axis is core." Since A2 already created `RwClass` (with the RECONCILIATION-
  corrected `Observe`/`Mutate` naming) inside `governance/ports.rs` as part of the
  `DomainPolicy::classify` port contract, this task did NOT create a new `RwClass` type (that
  would duplicate A2's); it added only the `as_str()` method g05 specifies (the audit-
  vocabulary accessor) to the EXISTING core type, then placed the tables and `classify()`
  function in `browser/classify.rs` -- the plugin implementation of `DomainPolicy::classify`
  (no concrete `impl DomainPolicy` exists yet; that composition lands with a later task, e.g.
  G07/G12/G13, which will wire this function in). This is the first task where a g-doc's own
  file-placement instruction is fully overridden by RECONCILIATION rather than merely
  adjusted for the post-A1 path (contrast G01-G04, A5, which all landed in `governance/config/`
  as g05's doc's own analogues would have predicted once translated to the new tree).
- Verification: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings` clean.
  `cargo test` green (151 lib unit tests, up from 146: +5 new in `browser::classify::tests`
  covering exhaustive cross-checks against the live sacred fixture in both directions
  (`tool_table_matches_the_sacred_surface`, `computer_action_table_matches_the_sacred_enum`),
  the full call-by-call pin against the shared-format table, the `None`-on-miss cases, and
  the `as_str()` vocabulary; all other suites unchanged and green: `tests/all_open_golden.rs`
  3, `tests/architecture.rs` 4 -- confirms `browser/classify.rs`'s import of
  `crate::governance::ports::RwClass` is a legal browser-depends-on-core edge and introduces
  no forbidden `governance -> browser` edge, `tests/mcp_protocol.rs` 4,
  `tests/peer_death.rs` 1, `tests/tool_schema_fidelity.rs` 6 -- the sacred surface this
  task's own tests parse against). ASCII scan clean on every touched/new file.
- Browser checks queued: none (pure classification lookup; nothing wired into dispatch or
  audit yet, no runtime-observable behavior change).

### g09 manifest identity (name, version, content hash) -- 2026-07-02
- Commit: (see this task's commit)
- Files touched: `Cargo.toml` (+`sha2 = "0.10"`, the one sanctioned new dependency) and its
  `Cargo.lock`; new `src/governance/manifest/mod.rs`, `src/governance/manifest/identity.rs`;
  `src/governance/mod.rs` (`pub mod manifest;`); `src/doctor.rs` (the "Policy manifest:"
  section, inserted after "MCP clients:" and before "IPC endpoint:").
- Summary: `ManifestIdentity { name, version, hash }` (declaration order load-bearing for the
  pinned `{"name":...,"version":...,"hash":...}` serialization) plus `canonical_hash` and
  `identity_from_source`, computing the shared-format-4.2 canonical bytes (BOM-strip, parse
  once, compact re-serialize via `preserve_order`, SHA-256, hand-rolled lowercase hex) exactly
  as specified -- verified independently in Python before writing any test (`sha256({"name":
  "a","version":"1","grants":[]})` and `sha256({})` both matched the task doc's pinned values
  bit for bit). Since neither prerequisite this task is conditional on has landed yet (G12
  manifest engine, G06 audit), followed Branch B for both source resolution (a standalone org-
  policy-file reader: `ManifestStatus`, `manifest_status`, `active_manifest_identity`, with the
  G12 integration-point doc comment verbatim) and record-stamping (no `src/audit/` created;
  the `ManifestIdentity` type plus its pinned-shape test and the audit integration-point doc
  comment are the whole deliverable there). Wired the MANDATORY doctor section 5a: a
  `manifest_section_lines(&ManifestStatus)` helper renders the three cases
  (`none (all-open)` / three `name`/`version`/`hash` lines / one `invalid (...)` line), called
  from `doctor::run`.
- Deviations from the g-doc per RECONCILIATION.md / a documented reuse decision (not a
  RECONCILIATION-driven placement split like g05, but the same "don't duplicate an existing
  primitive" spirit): g09's own Branch B spec asks for a NEW `org_policy_path() -> Option<PathBuf>`
  function inside `identity.rs`, re-deriving the shared-format-1.2 per-platform path a second
  time. Since `governance::config::load::org_policy_path()` (G02) ALREADY implements that exact
  path rule (and is the one actually used by the real config-loading path), adding a second,
  slightly different implementation (G02's returns a bare `PathBuf` with a `C:\ProgramData`
  fallback when the env var is absent; g09's own signature returns `Option<PathBuf>`, `None` on
  absence) would create two divergent sources of truth for the same path. `manifest_status()`
  calls `governance::config::load::org_policy_path()` directly instead; no second
  `org_policy_path` function was written. Module placement: `governance/manifest/` (a new
  sibling to `governance/config/`) per RECONCILIATION section 1's explicit row "manifest parse/
  identity (g09, g12) -> governance/ (core) -- generic over any policy doc"; structured as a
  `mod.rs` + `identity.rs` pair so the manifest engine (G12: parsing, grants, source selection)
  has a natural home to extend into, mirroring how `governance/config/` grew task by task.
- Verification: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings` clean.
  `cargo test` green (159 lib unit tests, up from 151: +8 new in
  `governance::manifest::identity::tests` covering the BOM/whitespace-insensitivity and key-
  order/content-sensitivity of the hash, the empty-object hash vector, hex-format shape, the
  name/version extraction error cases, the pinned serialization shape, the three doctor-line
  renderings, and a temp-directory-backed Absent/Active/Invalid read-the-file test via the
  `status_at` seam; all other suites unchanged: `tests/all_open_golden.rs` 3,
  `tests/architecture.rs` 4 -- confirms `governance/manifest/identity.rs` (despite doing real
  file I/O and pulling in the new `sha2` crate) introduces zero forbidden edges,
  `tests/mcp_protocol.rs` 4, `tests/peer_death.rs` 1, `tests/tool_schema_fidelity.rs` 6).
  Manual doctor check per the task's own Verification step 4, run live (no browser needed):
  confirmed no `%ProgramData%\browser-mcp\policy.json` exists on this dev machine, ran
  `browser-mcp doctor`, confirmed the `Policy manifest:` section renders exactly
  `  none (all-open)`. Did NOT attempt the Active/Invalid manual doctor checks against the real
  `%ProgramData%` path: writing a machine-wide, admin-scoped file as a side effect of an
  unattended pass is out of scope (and the harness's own auto-mode classifier declined the
  attempt for exactly that reason when tried). The Active/Invalid cases are covered by the
  automated `manifest_status_reads_the_org_policy_file` test instead, which exercises the
  identical `status_at` code path against a disposable temp directory. ASCII scan clean on
  every touched/new file.
- Browser checks queued: none (pure identity computation + doctor text; no browser-facing
  behavior).

### g06 audit flight recorder (JSONL records at the dispatch chokepoint) -- 2026-07-02
- Commit: (see this task's commit)
- Files touched: `Cargo.toml` (+`uuid = { version = "1", features = ["v4"] }`, +`chrono =
  { version = "0.4", default-features = false, features = ["clock", "std"] }`, the two
  sanctioned dependencies) and its `Cargo.lock`; `src/governance/ports.rs` (`AuditRecord`
  grown from A2's single-field placeholder to the full 13-field shape, new `Identity`/
  `ClientInfo` structs, a `sample_audit_record` test helper, 3 new tests); new
  `src/governance/audit/mod.rs` + `src/governance/audit/destinations.rs` (the `Recorder`);
  `src/governance/mod.rs` (`pub mod audit;`); `src/governance/config/mod.rs` (dropped
  "Takes effect on restart" from the `AUDIT_DESTINATION`/`AUDIT_FILE_PATH` descriptions);
  `src/governance/config/reload.rs` (new crate-visible test constructor
  `ConfigStore::for_test_with_config`); `src/governance/dispatch.rs` (`Governance` grown:
  `audit`/`classify`/`client` fields, `set_client`/`record_call` methods, a
  `CapturingAuditSink` test double, 3 new/strengthened tests); `src/transport/mcp/server.rs`
  (recorder construction + config-change watcher in `run`, `capture_client_info` helper
  wired into the `initialize` arm, timing + `record_call` at the dispatch chokepoint in
  `handle_tools_call`, 3 new server-wiring tests); `tests/all_open_golden.rs` (updated for
  the new `Governance::all_open` signature); `tests/golden/config-schema.json` +
  `tests/golden/config-keys.md` (regenerated: only the two description-string changes);
  new `tests/audit_recorder.rs`.
- Summary: every `tools/call` now produces exactly one audit JSON-Lines record (shared
  format doc section 6), written by `Recorder` to `file` (default
  `%LOCALAPPDATA%\browser-mcp\audit.jsonl` via `dirs::data_local_dir()`, or
  `audit.file.path` if set) or `stderr`, gated by `audit.enabled` (Minimal default: true).
  `Governance::record_call` builds the record: `event_id` (uuid v4), `ts` (RFC 3339 UTC,
  millisecond precision), `client` (captured once from the MCP `initialize` request's
  `clientInfo`, first-wins), `tool`/`action` (the `computer` sub-action only, no other
  argument ever read), `rw` (via the injected `classify` fn, a classification miss falling
  to `Mutate` -- never presented as harmless observation), `decision: "allow"` (no
  enforcement yet), and `identity`/`domain`/`grant_id`/`denial_id`/`manifest` all `None`
  until later tasks. `handle_tools_call` times the call (`dispatch_started` to completion)
  and records after the outcome resolves, so the record carries the real duration and
  covers both success and tool-execution-failure paths alike (an execution failure is
  still `decision: "allow"`; the field is about policy, not outcome). The early `-32602`
  return for a missing tool name records nothing.
- Deviations from the g-doc per RECONCILIATION.md (the largest deviation set so far; g06's
  own doc predates A1/A3/A5 and assumes a flat `src/audit/` + `src/dispatch.rs` +
  `src/mcp/server.rs` + `src/policy/mod.rs` tree):
  1. **Placement.** RECONCILIATION.md section 1 maps the whole audit subsystem to
     `governance/` ("audit record + recorder + sinks (g06) -> governance/"), not a
     sibling top-level `src/audit/`. Landed as `governance/audit/{mod.rs,destinations.rs}`,
     with the dispatch wiring in the already-existing `governance/dispatch.rs` (A3) and
     the server wiring in `transport/mcp/server.rs` (post-A1 path), not the doc's stale
     `src/dispatch.rs`/`src/mcp/server.rs`.
  2. **`AuditRecord`/`Identity`/`ClientInfo` land in `governance/ports.rs`, not a new
     `governance/audit/record.rs`.** A2 already owns the shared seam-contract types file;
     rather than split the record type across two files (ports.rs holding the placeholder,
     audit/record.rs holding the grown version), grew it in place in `ports.rs` and had
     `governance/audit/mod.rs` import it directly. Keeps one definition site for every
     governance-core wire type.
  3. **`rw: RwClass`, not `rw: &'static str`.** The doc's literal `AuditRecord` spec types
     `rw` as a bare `&'static str` (`"observe"`/`"mutate"`, hand-set by the recorder).
     Reusing A2's `RwClass` (already `snake_case`-renamed to serialize as exactly those two
     strings, per g05's `as_str()` addition) avoids a second, unsynchronized copy of the
     observe/mutate vocabulary; `record_call` builds `RwClass` directly from `classify`'s
     `Option<RwClass>` result with `.unwrap_or(RwClass::Mutate)` and lets serde render it.
  4. **`manifest: Option<ManifestIdentity>` reuses G09's type**, not a second
     `{name, version, hash}` struct as the doc's literal spec defines locally. One shape,
     one type, per the same "don't duplicate an existing primitive" principle G09 itself
     used for the org-policy path. Consequence: `AuditRecord` dropped `Deserialize` (kept
     only `Serialize`) since G09's `ManifestIdentity` does not derive `Deserialize` and
     retrofitting it was out of this task's scope (no test here needs record
     deserialization; a later task can add it if it turns out to).
  5. **Architecture gap found and fixed in A3's `Governance` facade.** A3's original
     design nested the audit sink only inside `Mode::Governed` (`GovernedState.audit`), so
     an all-open session (no manifest) had nowhere to record to. Shared format doc section
     4.5 is explicit that the flight recorder must record even under all-open, gated only
     by `audit.enabled`. Fixed by moving `audit: Arc<dyn AuditSink>` to be a direct field
     of `Governance` itself (both `all_open()` and `governed()` now take it as a
     constructor parameter), which is why `tests/all_open_golden.rs` and every existing
     `Governance` construction site needed updating alongside this task's own new code.
  6. **`classify` injected as a function pointer**, the same "known integration point"
     shape as `domain_pattern_valid` (G01/G02/A5/G03): `governance/dispatch.rs` cannot
     name `browser::classify::classify` directly (the a7 arch-test forbids a
     `governance -> browser` edge), so `Governance::all_open`/`governed` take
     `classify: fn(&str, Option<&str>) -> Option<RwClass>` and
     `transport/mcp/server.rs` (the composition root) supplies the real
     `browser::classify::classify` at construction.
  7. **Live reload of the audit sink, overriding g06's own stated out-of-scope note.**
     g06's doc explicitly lists "no reload... one synchronous open-append-close per
     record" as out of scope. RECONCILIATION.md section 3 explicitly earmarks this exact
     scenario -- `audit.destination`/`audit.file.path` becoming live once A5 (hot-reload)
     and G06 (the sink to reload) both exist -- as the trigger point for implementing
     live reopen-on-change, and per BOOTSTRAP's authority order RECONCILIATION overrides a
     conflicting g-doc scope note. Implemented `Recorder::reload(&Config)` (re-resolves
     the destination and swaps it in; a file sink is already open-per-record, so "reopen"
     is just re-deriving the path/destination) plus a `tokio::spawn`'d config-change
     watcher in `transport/mcp/server.rs::run` that calls it on every `ConfigStore`
     change signal, and dropped "Takes effect on restart" from the `AUDIT_DESTINATION`/
     `AUDIT_FILE_PATH` key descriptions (now simply truthful again) -- which required
     regenerating `tests/golden/config-schema.json`/`config-keys.md` (reviewed by hand:
     diff showed exactly those two description strings changed, nothing else).
  8. **Test items 6-8 and 13 adapted from `Recorder` methods to `Governance` methods.**
     The doc's test list assumes `Recorder` itself has `set_client`/`record_call` (test 6:
     `client_info_is_captured_once_first_wins`; test 7:
     `classification_miss_records_mutate`; test 8:
     `computer_action_classification_flows_into_rw`; test 13, the `tests/audit_recorder.rs`
     integration test). In this architecture those methods live on `Governance` (per
     deviation 5 above: `Recorder` implements only the bare `AuditSink::record`), so the
     equivalent coverage lands in `governance::dispatch::tests` (a new `CapturingAuditSink`
     test double, plus `set_client_first_capture_wins`, the strengthened
     `classification_miss_records_mutate`, and the new
     `computer_action_classification_flows_into_rw`) and in `tests/audit_recorder.rs`
     (built via `Governance::all_open` wrapping `Recorder::to_file`, not a bare
     `Recorder`).
  9. **Test items 10-12 (server-wiring tests) needed a `ConfigStore`, which post-dates the
     doc.** `handle_line`/`handle_tools_call` take `&Arc<ConfigStore>` (A5), not a bare
     `Config` as the doc's literal signatures assume. The existing `#[cfg(test)]
     ConfigStore::for_test` constructor takes a private `LastGoodInputs` type not visible
     outside `governance/config/reload.rs`, so it could not be called from
     `transport::mcp::server`'s own test module. Added a second, crate-visible test
     constructor, `pub(crate) fn for_test_with_config(config: Config) -> Arc<ConfigStore>`
     (still `#[cfg(test)]`-gated, zero production cost), which seeds empty last-good
     inputs internally. All 3 server-wiring tests (`tools_call_produces_one_audit_record
     _with_client_identity`, `computer_call_records_action_and_observe_class`,
     `invalid_tools_call_without_name_records_nothing`) land in a new `#[cfg(test)] mod
     tests` in `transport/mcp/server.rs`, driving the real private `handle_line`/
     `handle_tools_call` functions with `Browser::new()` unconnected (so `browser.call`
     fails fast with no extension, exactly as the doc's own test 10 anticipates).
- Verification: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings`
  clean. `cargo test` green (172 lib unit tests, up from 159: +3 in `governance::ports
  ::tests` -- field-order, null-presence, single-line; +2 net in `governance::dispatch
  ::tests` -- `classification_miss_records_mutate` strengthened to assert `RwClass::Mutate`
  via the new `CapturingAuditSink` rather than just calling record_call, plus the new
  `computer_action_classification_flows_into_rw`, plus `set_client_first_capture_wins`
  strengthened to also assert the recorded client field; +4 in `governance::audit::tests`
  -- file-append, disabled-writes-nothing, default-path-shape, reload-reopens-the-sink;
  +3 in `transport::mcp::server::tests`, new this task; +1 new `tests/audit_recorder.rs`
  integration test; all other suites unchanged and green: `tests/all_open_golden.rs` 3
  (updated for the new `Governance::all_open` signature, still passing),
  `tests/architecture.rs` 4 -- confirms `governance/audit/**` and the grown
  `governance/dispatch.rs`/`governance/ports.rs` introduce zero forbidden edges despite
  the new `uuid`/`chrono` dependencies, `tests/config_schema_golden.rs` 5 -- both goldens
  regenerated via `cargo run --quiet -- config schema`/`config docs` and hand-reviewed
  before overwriting, `tests/mcp_protocol.rs` 4 unchanged, `tests/peer_death.rs` 1
  unchanged, `tests/tool_schema_fidelity.rs` 6 unchanged). `git status --short` confirmed
  the touched-file set matches this entry's list exactly (adjusted for the post-A1 module
  paths per deviation 1); `src/transport/mcp/schemas/tools.json` and everything under
  `extension/` show no diff. ASCII scan (`rg -n "[^\x00-\x7F]"`) clean on every touched/new
  file, confirmed via the Grep tool.
- Browser checks queued: none appended to `BROWSER-TESTS.md` (this task needs no browser at
  all -- the recorder writes to a file/stderr regardless of any extension connection). The
  task's own Verification step 6 (manual smoke: rebuild, restart the MCP client, run one
  live tool call, confirm the default audit file gained one plausible line) was NOT run in
  this pass: doing so would mean rebuilding and replacing the very `browser-mcp.exe` this
  live unattended session's own Claude Code connection is running against, risking
  disruption of the session mid-run for a check that is not itself browser-dependent. The
  new `tests/audit_recorder.rs` integration test and the 3 new server-wiring tests exercise
  the identical code path (real `Recorder` writing real JSONL files, driven through the
  real `Governance`/`handle_tools_call` chokepoint) end to end without that risk; the live
  rebuild-and-restart smoke is left for a human's routine post-session verification, not
  BROWSER-TESTS.md (which is reserved for checks that need a live browser/extension).

### g07 domain pattern matcher with bypass-class tests -- 2026-07-02
- Commit: (see this task's commit)
- Files touched: `Cargo.toml` (+`url = "2"`, the one sanctioned new dependency, with its
  required justification comment) and its `Cargo.lock`; `src/browser/pattern.rs` (grown,
  not replaced: the module doc comment now describes both halves, plus the new public
  API and 17 named tests appended after the existing `is_valid_pattern`/`is_valid_label`
  code and tests); `src/browser/mod.rs` (one doc-comment sentence updated, since it
  described `pattern` as "syntax today, matching semantics added by a later task").
- Summary: `host_for_matching(url: &str) -> HostOutcome` parses with `url::Url::parse`
  and extracts a parser-normalized `MatchHost` for `http`/`https` URLs (`NonHttpScheme`
  for anything else, `Unparseable` on any parse failure or hostless result), stripping
  at most one trailing dot from a domain host and failing closed if a second one
  remains. `DomainPattern::parse` validates and canonicalizes the section 5.1 grammar
  (empty / non-ASCII / scheme / userinfo / path / wildcard-shape checks in a fixed
  order, then body canonicalization through `url::Host::parse` -- the same WHATWG host
  rules `Url::parse` itself applies -- so patterns and hosts normalize identically,
  including IPv4 respellings like `0x7f.0.0.1` -> `127.0.0.1` and bracketed/bare IPv6
  literals). `DomainPattern::matches` is an exact string compare for non-wildcard
  patterns and a `.` + suffix `ends_with` check for `*.suffix` patterns, with a
  match-time IP-literal guard even though parse-time already rejects wildcard-over-IP
  patterns (defense in depth). `first_match` is a linear first-hit scan. No matcher
  code inspects a raw URL string for policy signals; all structure comes from the `url`
  crate. Nothing is wired into dispatch, config, or any enforcement path -- this task is
  pure library addition, exactly per its own scope.
- Deviations from the g-doc per RECONCILIATION.md:
  1. **Placement: extends the EXISTING `browser/pattern.rs`, not a new
     `src/policy/domain.rs`.** RECONCILIATION.md section 1 maps "the URL/domain matcher
     (g07)" to `browser/` (the `url` crate lives only there), and -- more specifically --
     `browser/pattern.rs` itself (landed by g01, pre-A1-translated per RECONCILIATION's
     own "known integration point" note) carries a module doc comment stating outright
     that "matching SEMANTICS... belong to the domain matcher task, which extends this
     same file rather than creating a new one." Followed that instruction literally: the
     new `MatchHost`/`HostOutcome`/`host_for_matching`/`DomainPattern`/`first_match`/
     `PatternError` API was appended to `browser/pattern.rs` (after the existing
     `is_valid_pattern`/`is_valid_label` syntax checker, which is untouched), and the 17
     new named tests were appended to that file's existing `#[cfg(test)] mod tests`
     rather than a new file. `src/browser/mod.rs` needed only a one-sentence doc update
     (module declaration was already `pub mod pattern;` from g01). This is a larger
     departure from the doc's literal file layout than any prior task's, but it is the
     doc's OWN target file telling the task where to land, not a RECONCILIATION
     reinterpretation -- the two sources agree exactly.
  2. **`is_valid_pattern` (g01's authored-pattern syntax checker) is untouched and
     coexists with the new `DomainPattern::parse`, deliberately not unified.** The two
     have different semantics on purpose: `is_valid_pattern` is strict (rejects
     uppercase, trailing dots, IPv6) because it validates already-canonical
     `content.security.sacred_domains` config values; `DomainPattern::parse` is lenient/
     canonicalizing (accepts `Allowed.COM`, `example.com.`, `[::1]`) because it is meant
     to validate and normalize AUTHORED patterns from any source (future manifest grant
     domains) into the exact form the matcher needs. Whether G08 (sacred domains)
     switches the registry validator from `is_valid_pattern` to `DomainPattern::parse`
     is explicitly that task's own decision (this task's own Out of scope section
     forbids touching the registry at all), not pre-empted here.
  3. **One test input adjusted after checking against the real `url` crate, not the
     doc's assumption.** The doc's own test 8 already establishes the precedent of
     pinning behavior to whatever the real `url` crate actually does rather than an
     assumed value ("the shared format doc requires the test to pin the parser behavior
     either way, and with the `url` crate the behavior is normalization to
     `127.0.0.1`"). Applying that same precedent: the doc's test 14
     (`malformed_urls_fail_closed`) lists `"http:///path"` as an input that must yield
     `Unparseable`. Verified independently in a scratch Rust program (`url = "2"`,
     matching this task's exact pinned version) before writing the test: `url::Url::
     parse("http:///path")` actually succeeds with `host = Some(Domain("path"))`,
     `path = "/"` -- the WHATWG "special authority slashes" state slurps the redundant
     third slash for special schemes, so this is a legitimate (if unusual) host, not a
     malformed URL. All other 6 inputs in the doc's list verified to yield `Unparseable`
     exactly as expected. Removed only `"http:///path"` from the test's input list, with
     a code comment recording the verified real-crate behavior and why it does not
     belong in a fail-closed test; all 6 remaining inputs plus all 16 other named tests
     are otherwise implemented and asserted exactly as the doc specifies, with no other
     behavior deviating from a direct reading of section 5.
- Verification: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings`
  clean. `cargo test` green (189 lib unit tests, up from 172: +17 new in
  `browser::pattern::tests`, exactly the 17 named tests the doc requires (with input
  #14 adjusted per deviation 3), covering exact/wildcard/case/port normalization, the
  full section 5.3 bypass-class table by name (userinfo CVE-2025-47241, embedded
  credentials, wildcard-never-matches-IP, IPv4 alternate-form normalization, trailing-
  dot strip-without-bypass, punycode/homoglyph non-match, apex-excluded-from-wildcard,
  suffix-stitching-needs-a-label-boundary, non-http schemes, malformed-URL fail-closed),
  the full grammar-rejection table, canonical-form-via-`as_str`, and `first_match`
  ordering; all other suites unchanged and green: `tests/all_open_golden.rs` 3,
  `tests/architecture.rs` 4 -- confirms the new `url` crate dependency and the grown
  `browser/pattern.rs` introduce zero forbidden edges (the `url` crate lives only in
  `browser/`, never `governance/`), `tests/config_schema_golden.rs` 5,
  `tests/mcp_protocol.rs` 4, `tests/peer_death.rs` 1, `tests/tool_schema_fidelity.rs` 6).
  `git status --short` confirmed the touched-file set is smaller than the doc's own
  predicted diff (no new file at all, since the doc's own target file already existed):
  `Cargo.toml`, `Cargo.lock`, `src/browser/mod.rs`, `src/browser/pattern.rs` only;
  `src/transport/mcp/schemas/tools.json` and everything under `extension/` show no
  diff, and neither does any governance/dispatch/config file -- confirming constraint 3
  (all-open stays byte-identical because nothing is wired) held trivially. ASCII scan
  (`rg -n "[^\x00-\x7F]"`) clean on every touched file, confirmed via the Grep tool
  (the one Cyrillic test input is written as a `\u{0430}` escape, per the doc's own
  ASCII-only constraint).
- Browser checks queued: none (pure library addition; nothing wired into dispatch,
  config, or any enforcement path; no runtime-observable behavior change of any kind).

### g08 sacred domains: the never-touch list with structured denials -- 2026-07-02
- Commit: (see this task's commit)
- Files touched: `src/governance/config/mod.rs` (updated `CONTENT_SECURITY_SACRED_DOMAINS`'s
  registry description to the exact shared-format-doc 3.4 string, and its doc comment);
  new `src/governance/denial.rs` (`denial_id`); `src/governance/mod.rs` (`pub mod denial;`);
  `src/governance/ports.rs` (`Denial` grown from A2's 2-field placeholder to the full
  5-field shape; the one existing test constructing it updated); `src/governance/dispatch.rs`
  (`Governance::record_call` gained a `domain: Option<&str>` parameter, new
  `Governance::record_deny` method, both with tests); new `src/browser/sacred.rs`
  (`first_match`, `navigate_target_host`, `sacred`); `src/browser/mod.rs` (`pub mod sacred;`,
  module doc); `src/transport/mcp/server.rs` (the STEP A/B/C enforcement wiring in
  `handle_tools_call`, a new `SacredCheck` struct, `sacred_check`/`resolve_tab_host`
  functions, 4 new chokepoint tests); `tests/audit_recorder.rs` (2 call sites updated for
  `record_call`'s new parameter); `tests/golden/config-schema.json` +
  `tests/golden/config-keys.md` (regenerated: only the sacred-domains description string
  changed).
- Summary: the first real enforcement anywhere in the product (ADR-0018 step 2). A
  `navigate` whose target host matches a `content.security.sacred_domains` pattern, or ANY
  tool call whose current tab's host matches one, is denied before the tool runs, with a
  stable `"D-" + 8-lowercase-hex` denial id (SHA-256 of `manifest_hash + "\n" + grant_id +
  "\n" + rule`, pinned and independently verified in Python before writing any test: `D-
  171052e3` for `sacred/mybank.com`, `D-af6633ec` for `sacred/*.mybank.com`) and a plain,
  actionable message naming only the matched host and the id -- never the pattern, the
  rest of the list, or any config key name. STEP A: an empty list (every preset's default)
  is a byte-identical fast path -- no extension traffic, no parsing, no allocation --
  verified by a dedicated test asserting the extension sees only the real tool's own
  frame. STEP B: for any tool call carrying a numeric `tabId`, an internal
  `tabs_context_mcp` lookup (machinery, not an MCP tool call -- it writes no audit record
  of its own) resolves the current tab's host via G07's `host_for_matching`; a failed
  lookup (not connected, tab not in the group, unparseable URL) never denies -- a deny
  requires a positive match, never a fabricated one. STEP C: for `navigate`, the target
  URL is normalized via a hand-rolled mirror of `extension/service-worker.js`'s own
  normalization (`back`/`forward` -> none; `http(s)://` -> parse as-is; `about:`/`chrome:`/
  `edge:`/`brave:` -> none; otherwise strip one leading 1-6-char scheme prefix then
  prepend `https://`) and checked the same way. STEP B runs first and covers `navigate`
  too (a tab showing a sacred domain may not be touched AT ALL, including navigating it
  away); STEP C runs even when STEP B could not resolve the tab, since it needs no
  extension. A denial short-circuits before `browser.call` ever fires (the real tool never
  runs) and writes exactly one audit record via a new `Governance::record_deny` method
  (`decision: "deny"`, the stable `denial_id`, `grant_id: null`, `duration_ms: 0`); an
  allowed call's existing `record_call` now also carries the STEP-B-resolved `domain`
  through, so a truthful current-tab host reaches the record even when nothing was denied.
- Deviations from the g-doc per RECONCILIATION.md (the second-largest deviation set after
  g06; g08's own doc predates A1/A2/A3/A5/G06/G07 and assumes a flat `src/dispatch.rs` +
  `src/mcp/server.rs` + `src/policy/{denial,sacred}.rs` tree, a `Config` passed by value,
  and a `Recorder` that owns `set_client`/`record_call` directly):
  1. **Placement, per RECONCILIATION section 1's explicit split**: "sacred-domain list +
     enforcement (g08) -> browser/ (data/logic) + governance/ (the always-on check
     wiring)". `denial_id` (pure, domain-agnostic, reused verbatim by g13 per the doc's
     own note) landed in NEW `governance/denial.rs`, core; the grown `Denial` TYPE landed
     in the EXISTING `governance/ports.rs` (A2's seam-contract file), not a new
     `governance/denial.rs`-owned struct, so there is exactly one definition site for
     every governance-core wire type (the same principle G06 applied to `AuditRecord`).
     The sacred LIST matching (`first_match`), the extension-mirroring URL normalization
     (`navigate_target_host`), and the sacred-flavored `Denial` constructor (`sacred`) --
     all browser-domain business logic -- landed in NEW `browser/sacred.rs`, reusing G07's
     `browser/pattern.rs` types and never reimplementing matching semantics. The STEP A/B/C
     ORCHESTRATION (the "always-on check wiring" RECONCILIATION assigns to "the core")
     lives in `transport/mcp/server.rs`, the composition root -- NOT inside
     `governance/dispatch.rs` itself, because STEP B needs a live, async
     `browser.call("tabs_context_mcp", ...)` round trip, and `Governance` (governance
     core) cannot name `Browser` (`transport::executor`) without violating the a7
     arch-test's forbidden-edge scan. `transport/mcp/server.rs` already reaches into
     `browser::classify`/`browser::pattern`/`browser::redact` directly (the established
     precedent for composition-root code depending on the browser plugin); this task
     extends that same precedent to `browser::sacred`, while the DECISION VOCABULARY the
     orchestration produces (`Denial`, and `Governance::record_deny`'s `decision: "deny"`
     audit shape) stays core. Considered making `Governance` generic over an injected
     `ResourceResolver` (A2's port built for exactly this "resolve live state" shape) to
     keep the orchestration inside `governance/`; rejected as disproportionate scope for
     this task -- it would ripple `Arc<Governance<R>>` through every call site session-wide
     for a port whose real, grant-aware consumer is G12/G13, and sacred explicitly bypasses
     grant machinery entirely (constraint 9).
  2. **`navigate_target_host` returns `Option<MatchHost>`, not the doc's literal
     `Option<String>`.** Adapting the doc's guessed signature to G07's real, already-landed
     type (the same category of adaptation G06/G07 already made for stale pre-G07/pre-A5
     signatures): a bare `String` would need re-wrapping into a `MatchHost` to call
     `first_match`, and `MatchHost`'s constructor is deliberately private to
     `browser::pattern` (G07's own invariant: "constructible only via `host_for_matching`,
     so a raw URL string can never be passed to the matcher by mistake"). Returning
     `MatchHost` directly (it already comes from `host_for_matching` internally) keeps
     that invariant fully intact and lets `first_match` consume one uniform type from both
     STEP B's tab host and STEP C's target host, with no second privacy-loosening
     constructor added anywhere.
  3. **`first_match`'s host parameter is `&MatchHost`, not the doc's literal `&str`**, for
     the same reason as deviation 2: reusing G07's real matcher (`DomainPattern::matches`)
     requires a `MatchHost`, and constructing one from a bare `&str` inside `browser::sacred`
     is exactly the privacy violation G07's design forbids. Sacred pattern strings
     themselves stay plain `&[String]` (matching the doc exactly): each is parsed into a
     `DomainPattern` per call via `DomainPattern::parse`, skipping (not crashing on) an
     entry that fails to parse -- defense in depth for a list already validated at config
     load, never expected to happen in practice.
  4. **The registry's `content.security.sacred_domains` validator is UNCHANGED**
     (`browser::pattern::is_valid_pattern`, G01's stricter syntax-only checker), not
     switched to G07's more lenient, canonicalizing `DomainPattern::parse` as the doc's
     phrase "validate each element with the G07 pattern validator" could be read to
     require. Both functions live in the SAME file (`browser/pattern.rs`, per G07's own
     ledger entry, which explicitly flagged this exact question as deferred to this task).
     Kept `is_valid_pattern` deliberately: it is the stricter of the two (rejects
     uppercase, trailing dots, and IPv6 forms that `DomainPattern::parse` would silently
     canonicalize), and per the engine-is-truthful principle, tightening what a
     user-authored PROTECTION list accepts is the conservative direction; loosening it
     would be a real, if small, behavior change this task's own scope does not call for.
     No test in this task's own required list depends on which validator gates the
     registry, so this reduces to a documented, deliberate no-op.
  5. **`Governance::record_call` grew a `domain: Option<&str>` parameter** rather than
     forking a second recording path, and a new `Governance::record_deny` method was added
     alongside it (not a `Recorder`-level `set_client`/`record_call` pair as the doc's
     stale pre-G06-architecture text assumes -- those methods live on `Governance` post-G06,
     per that task's own ledger entry). Both call sites (`transport/mcp/server.rs`,
     `tests/audit_recorder.rs`) and dispatch's own test module were updated for the new
     arity.
  6. **`Denial.grant_id` is populated from the `Denial` itself in `record_deny`**
     (`denial.grant_id.clone()`), always `None` for the sacred rule today, rather than a
     literal hardcoded `None` in `record_deny` -- this makes `record_deny` already correct,
     with no further change needed, for G13's future grant-denial rules that DO set a
     `grant_id`.
- Verification: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings`
  clean. `cargo test` green (203 lib unit tests, up from 189: +4 in `governance::denial
  ::tests` -- the two pinned values plus determinism/format checks; +4 in `browser::sacred
  ::tests` -- the exact-leak-nothing message, list-order honoring, the full navigate-target
  mirror table, and the full section-5.3 bypass-class table turned to the deny direction
  including the homoglyph and apex-vs-wildcard cases; +2 in `governance::dispatch::tests`
  -- domain pass-through on an allow record, a zero-duration deny record; +4 new chokepoint
  tests in `transport::mcp::server::tests` -- `sacred_tab_denies_every_tool_and_never
  _runs_it` (all 4 of read_page/computer/javascript_tool/navigate denied, extension sees
  only the `tabs_context_mcp` pre-flight, all 4 audit records pinned to `D-af6633ec` and
  `www.mybank.com`), `navigate_target_denied_even_when_tab_is_clean` (pinned to
  `D-171052e3`, then a second call to a clean target actually reaches the fake extension),
  `empty_list_is_byte_identical` (no pre-flight chatter at all, plus an unconnected
  `Browser` still resolving to the ordinary not-connected path with zero sacred-check
  overhead), `denied_call_writes_one_deny_record` (exactly one record, the internal lookup
  writes none); all other suites unchanged and green: `tests/all_open_golden.rs` 3,
  `tests/architecture.rs` 4 -- confirms the new `governance/denial.rs` and the grown
  `governance/ports.rs`/`governance/dispatch.rs` introduce zero forbidden edges despite
  reusing `sha2` (already a dependency since G09), `tests/audit_recorder.rs` 1 (updated call
  sites, unchanged assertions), `tests/config_schema_golden.rs` 5 -- both goldens
  regenerated via `cargo run --quiet -- config schema`/`config docs` and hand-reviewed
  before overwriting (diff showed exactly the one sacred-domains description string
  changed), `tests/mcp_protocol.rs` 4 UNCHANGED -- proves the default empty-list config is
  still byte-identical to pre-g08 behavior end to end over stdio,
  `tests/tool_schema_fidelity.rs` 6 unchanged -- g08 never touches tool advertisement.
  `git status --short` confirmed the touched-file set matches this entry's list exactly
  (no `Cargo.toml`/`Cargo.lock` diff at all: `sha2` was already present since G09, so this
  task added zero new dependencies); `src/transport/mcp/schemas/tools.json` and everything
  under `extension/` show no diff (`git diff --stat` confirmed empty for both paths). ASCII
  scan (`rg -n "[^\x00-\x7F]"`) clean on every touched/new file, confirmed via the Grep
  tool (the one Cyrillic test host is a `\u{0430}` escape, reused from G07's own pattern).
- Browser checks queued: none appended to `BROWSER-TESTS.md` in this pass -- the task's own
  Verification step 5 (live check: set `content.security.sacred_domains` in the real user
  config file, restart the MCP client, ask the agent to navigate to/read/screenshot a
  sacred domain and confirm the denial message and browser behavior, confirm normal
  domains still work, confirm the audit JSONL shows the deny record) requires a live
  browser AND restarting this very session's own MCP client connection, which BOOTSTRAP's
  ground rules place out of scope for an unattended pass (no live browser available; no
  human present to restart the client and observe the result). The 4 new chokepoint tests
  exercise the identical `handle_tools_call` code path end to end (fake extension, real
  `Governance`/`Recorder`, real STEP A/B/C logic) without a live browser, covering
  everything the live check would observe except the actual on-screen browser behavior and
  the real default-audit-file location; the live check itself is added to
  `BROWSER-TESTS.md` as item g08-1 for a human to run later, alongside the existing
  reminder that g10/g11/g14/g12/g13/g15 will need the same.

### g10 take-the-wheel pause (a user hold the agent must honor) -- 2026-07-02
- Commit: (see this task's commit)
- Files touched: `src/governance/ports.rs` (`AuditRecord` grew a `held: bool` field, appended
  last per the shared-format doc's own instruction, with a new test); `src/governance/audit
  /mod.rs` (its `sample_record` test helper updated for the new field); `src/governance
  /dispatch.rs` (`HOLD_HINT_AFTER` constant + `hold_message` pure function; `record_call`/
  `record_deny` refactored onto a shared private `build_record` helper that also backs the
  new `Governance::record_held`; 5 new tests); `src/transport/executor.rs` (`Browser` grew
  a `held` field, `held_for`/`set_held`/`toggle_held`, hold-request handling in
  `route_reply`, 4 new tests); `src/transport/mcp/server.rs` (the hold check wired into
  `handle_tools_call`, before governance.decide/the sacred check/any extension traffic; 2
  new chokepoint tests); `src/transport/native/messages.rs` (the hold wire-vocabulary doc
  block, additive); `tests/audit_recorder.rs` (field-order assertion extended); new
  `extension/popup.html` + `extension/popup.js`; `extension/manifest.json` (`action` +
  `commands` keys, no new permissions); `extension/service-worker.js` (hold-request
  handling on the native port, `holdRequest`/`updateHoldBadge` helpers, a
  `chrome.commands.onCommand` listener, a `chrome.runtime.onMessage` listener for the
  popup); `docs/tasks/stage-2/00-shared-format.md` (the `held` field row appended to the
  section 6.1 audit table, additive-only per the task's own instruction).
- Summary: a user-facing pause control (ADR-0018 step 2, alongside sacred domains). The
  extension's popup button and `Alt+Shift+P` shortcut send `get_hold`/`set_hold`/
  `toggle_hold` requests over the existing native-messaging channel; `Browser` holds the
  flag (`Option<Instant>`, process memory only) and answers with `hold_state`/
  `hold_error`. While held, `handle_tools_call` answers EVERY `tools/call` immediately with
  a successful (never `isError`) text result stating plainly the call was NOT executed and
  telling the agent to wait, not retry -- checked before `governance.decide`, before the
  g08 sacred check, and before any extension traffic, so a held call never reaches the
  extension and is never queued, deferred, or replayed (the agent re-issues calls itself
  after resume). Past `HOLD_HINT_AFTER` (2 minutes), the reply appends a second sentence
  naming the only way to resume. The flag survives an extension disconnect/reconnect (the
  user paused; a service-worker death must not silently resume the agent) but never
  persists to disk and never survives a binary restart. A held call still produces exactly
  one audit record (`decision: "allow"`, the new `held: true` field, `duration_ms: 0`,
  `domain: null` since a held call must not touch the extension); every other record now
  carries `held: false`.
- Deviations from the g-doc per RECONCILIATION.md / the module-placement map (paths
  translated post-A1/A5/G06/G08; no explicit g10 row in RECONCILIATION.md, so the general
  placement principles from g06/g08 apply):
  1. **`HOLD_HINT_AFTER` and `hold_message` land in the EXISTING `governance/dispatch.rs`**
     (the doc's own literal instruction: "In `src/dispatch.rs` add:"), not a new file --
     both are pure, domain-agnostic text formatting with no browser-specific concept,
     consistent with `governance/` owning "the chokepoint" and "denial, mode" per
     RECONCILIATION section 1. The hold FLAG (`Browser::held`/`held_for`/`set_held`/
     `toggle_held`) lives on `transport::executor::Browser` (RECONCILIATION: "`src/native/`,
     `src/mcp/`, `src/browser.rs` handle -> `transport/`"), since `governance/` cannot name
     `Browser` without violating the a7 arch-test's forbidden-edge scan.
  2. **The hold check's ORCHESTRATION lives in `transport/mcp/server.rs`, not
     `governance/dispatch.rs`**, for the identical reason g08's sacred-check orchestration
     does: `Governance` cannot hold or query a live `Browser` handle directly (the a7
     arch-test forbids `governance -> transport`), so `handle_tools_call` (the composition
     root's dispatch loop) calls `browser.held_for()` itself and, on a hold, calls the pure
     `governance::dispatch::hold_message` and the new `Governance::record_held` -- the same
     "decision vocabulary in core, live-state orchestration at the composition root" split
     g08 already established, now applied to a second always-on, mode-independent check.
  3. **`record_call`/`record_deny`/`record_held` refactored onto a shared private
     `build_record` helper** rather than duplicating the 13-field `AuditRecord` literal a
     third time. Not a doc requirement, just avoiding a third near-identical struct literal
     now that a third "always-allow/always-deny-shaped" record-building call exists;
     `record_call`'s and `record_deny`'s own observable behavior and public signatures are
     completely unchanged.
  4. **The `held` field is appended LAST to `AuditRecord`**, after `manifest` -- the doc's
     own instruction is to "append this exact row to the field table," which is only
     satisfiable by adding it after every field that exists today; there is no ambiguity
     here, just noting it since every prior task's field-order test needed updating too
     (`governance::ports::tests::record_serializes_all_fields_in_shared_format_order`,
     `tests/audit_recorder.rs`'s own key-order assertion).
  5. **Audit wiring was MANDATORY, not deferred**, per the task's own "Depends on" branch
     rule: the audit subsystem (G06) has landed, so the `held` marker was wired for real
     (no `tracing::info!`-only fallback was needed or used).
- Verification: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings`
  clean. `cargo test` green (215 lib unit tests, up from 203: +2 in `governance::ports
  ::tests` -- the extended field-order list, `held` defaults false and serializes as a
  JSON boolean (never omitted, never null); +5 in `governance::dispatch::tests` -- three
  `hold_message` tests (no-hint below the threshold, hint present at and above
  `HOLD_HINT_AFTER`, the `computer (<action>)` label convention), `record_held` (allow,
  `held: true`, zero duration, no domain), and a check that `record_call`/`record_deny`
  both leave `held: false`; +4 in `transport::executor::tests` -- hold set/toggle/clear,
  a repeated `set_held(true)` preserving the original engage instant across a real 30ms
  sleep, all three hold requests answered correctly over a real duplex connection
  (including the `hold_error` case for a non-boolean `held`), and the hold surviving the
  extension end of the duplex closing; +2 new chokepoint tests in `transport::mcp::server
  ::tests` -- a held `Browser` with NO extension connected returns the `Paused:` text
  before the ordinary not-connected error (proving the hold check precedes it), with hold
  released the existing `isError` path returns unchanged; a held call's audit record shows
  `decision: "allow"`, `held: true`, `duration_ms: 0` while a normal call's shows
  `held: false`; all other suites unchanged and green: `tests/all_open_golden.rs` 3,
  `tests/architecture.rs` 4 -- confirms the grown `governance/ports.rs`/
  `governance/dispatch.rs` introduce zero forbidden edges, `tests/audit_recorder.rs` 1
  (field-order assertion extended, all other assertions and behavior unchanged),
  `tests/config_schema_golden.rs` 5 unchanged -- g10 registers no config key,
  `tests/mcp_protocol.rs` 4 UNCHANGED -- proves the hold-never-engaged path stays
  byte-identical end to end over stdio, `tests/tool_schema_fidelity.rs` 6 unchanged -- g10
  advertises no tool and filters nothing from `tools/list`. `git status --short` confirmed
  the touched-file set matches this entry's list exactly, including the FIRST diff this
  session under `extension/` (`manifest.json`, `service-worker.js`, new `popup.html`/
  `popup.js`); confirmed no diff to `extension/content.js`,
  `extension/agent-visual-indicator.js`, `src/transport/native/host.rs`,
  `src/transport/native/ipc.rs`, `src/install/`, `src/debug.rs`, or
  `src/transport/mcp/schemas/tools.json`. `Cargo.toml`/`Cargo.lock` show no diff (no new
  dependency, matching the task's own constraint). ASCII scan (`rg -n "[^\x00-\x7F]"`)
  clean on every touched/new file including all four extension JS/HTML/JSON files.
  `node --check` confirmed `service-worker.js` and `popup.js` are syntactically valid, and
  `python3 -m json.tool`-equivalent parsing confirmed `manifest.json` is valid JSON (no
  Chrome available in this environment to load the unpacked extension itself).
- Browser checks queued: `BROWSER-TESTS.md` items `g10-1` through `g10-5`, covering this
  task's own Verification steps 3-4 and 6-10 (popup open/pause/resume/badge, the agent
  receiving `Paused:` text with no `tool_request` reaching the extension, the resume hint
  past 2 minutes, the hold surviving a service-worker kill-and-restart from
  `chrome://extensions`, and the popup's `No active browsing session.` state with no MCP
  session running) -- ALL of these need a live Chrome with the extension loaded and (for
  steps 4/6) a live Claude Code connection, which this unattended pass cannot drive. The
  automated suite proves every one of these at the code level (the hold state machine, the
  wire protocol, the dispatch-chokepoint ordering, the audit marker) without a browser;
  only the actual on-screen popup/badge/shortcut behavior and the literal
  reload-survives-a-worker-restart property need a human's eyes.

### g11 panic kill switch: sever the session in one gesture -- 2026-07-02
- Commit: (see this task's commit)
- Files touched: `src/governance/ports.rs` (new `SessionEventRecord` type; `AuditSink` grew a
  second method `record_session_event`; `NullSink` implements it; 3 new tests);
  `src/governance/audit/mod.rs` (`Recorder` implements `record_session_event` via a shared
  `write_serialized` helper factored out of the existing `record`; 1 new test);
  `src/governance/dispatch.rs` (new `Governance::record_session_killed`; 1 new test);
  `src/transport/executor.rs` (`Browser` grew `killed: Arc<AtomicBool>` +
  `kill_hook: Arc<Mutex<Option<KillHook>>>`, `is_killed`/`on_session_killed`, the killed-check
  as the first check in `call`, `session_killed` recognition in `route_reply` before the
  id-less early return, a kill reset at the top of `attach`; a private `kill_error()`
  builder; 5 new tests); `src/transport/mcp/server.rs` (the kill hook registered once in
  `run`, right after `governance` is constructed); `src/transport/native/messages.rs` (the
  `session_killed` event documented, additive); `extension/service-worker.js` (the
  `session_killed` storage marker and gated `connect()`, `killSession()`/
  `sweepDetachAll()`, the tool_request refusal while killed, startup recovery via a new
  `init()`, and `GET_SESSION_STATE`/`KILL_SESSION`/`RECONNECT_SESSION` popup-message
  handling); `extension/popup.html` + `extension/popup.js` (a second, visually distinct
  section: connection/attached-tab status, and a button that is `kill-button`/
  `End session now` or `reconnect-button`/`Start new session` depending on state); new
  `tests/all_open_golden.rs`'s `NullAuditSink` grew the same trait method (compile-only
  change, no new test); `tests/audit_recorder.rs` (1 new integration test).
  `extension/manifest.json` was NOT touched this task: g10 already added the `action` key,
  and this task adds no new manifest keys or permissions.
- Summary: a one-click panic control (ADR-0018 step 2, alongside sacred domains and pause),
  distinct from and never sharing a control with the take-the-wheel pause (g10). The
  extension's `End session now` button persists a `chrome.storage.session` marker FIRST
  (so a service-worker death mid-kill is completed by startup recovery: `init()` re-runs
  the debugger-detach sweep on every worker start while the marker is set, without ever
  reconnecting), signals the binary over the native channel while the port is still open,
  detaches every debugger attachment (the in-memory map, then a
  `chrome.debugger.getTargets()` sweep for attachments a prior worker instance forgot),
  clears in-memory session state, then tears down the port -- never closing, ungrouping,
  or navigating any tab. `Browser::route_reply` recognizes the `session_killed` event
  (idempotent: only the false-to-true transition acts), fails every pending call and every
  subsequent `Browser::call` with the exact truthful `[hop: extension] The user ended the
  browser session (kill switch). Next step: ask the user to reconnect from the Browser MCP
  extension popup, then retry.`, and invokes a once-per-kill hook that writes exactly one
  audit session-event record (`event: "session_killed"`, none of the tool-call fields).
  Recovery is explicit only: the extension refuses every reconnect attempt (keepalive
  alarm, `onDisconnect` retry, worker restart) while the storage marker is set, and a
  fresh `Browser::attach` clears the binary-side `killed` flag only because it is only
  reachable after the user's own `Start new session` click removes that marker.
- Deviations from the g-doc per RECONCILIATION.md / the module-placement map (paths
  translated post-A1/A5/G06/G08/G10; no explicit g11 row in RECONCILIATION.md, so the
  g06/g08/g10 placement precedents apply):
  1. **The session-event record is a NEW type, `SessionEventRecord`, in the EXISTING
     `governance/ports.rs`** (not a new file), alongside `AuditRecord` -- one definition
     site for every governance-core wire type, per the same principle G06/G10 already
     applied. Since its shape is deliberately different (an `event` discriminator, none of
     `tool`/`action`/`rw`/`domain`/`decision`/`grant_id`/`denial_id`/`duration_ms`), it is a
     genuinely separate struct, not a variant squeezed into `AuditRecord` with those eight
     fields forced to `null`/`0` -- doing that would let a future reader mistake a session
     event for a degenerate tool-call record.
  2. **`AuditSink` grew a SECOND method, `record_session_event`, rather than a single
     `record(&self, record: &Record)` taking an enum of the two shapes.** An enum wrapping
     both record types would force every existing call site (`Governance::record_call`/
     `record_deny`/`record_held`) to wrap in a variant for no benefit, and would make the
     "no tool-call fields on a session event" invariant a runtime discipline instead of a
     compile-time one (the two structs' very different field sets stay statically
     separate). `Recorder`'s two trait methods share a private `write_serialized` helper
     (generic over `impl Serialize`) so the actual file/stderr framing logic exists exactly
     once.
  3. **The kill hook is a plain `Fn() + Send + Sync + 'static` closure capturing an
     `Arc<Governance>`, registered directly on `Browser` from `transport/mcp/server.rs`
     (the composition root)** -- the same "injection at construction, orchestration at the
     composition root" shape G06/G08/G10 already established for `classify`/
     `domain_pattern_valid`/the sacred check, now applied to a callback instead of a bare
     `fn` pointer (a closure is required here since it must capture the session's actual
     `Governance` instance, not just point at a stateless function). `Browser`
     (`transport::executor`) never names `Governance` (`governance::dispatch`) in its own
     type signature, only `impl Fn() + Send + Sync + 'static` -- so the dependency edge
     this closure encodes is `transport -> governance` (server.rs's composition-root
     wiring), never `governance -> transport`, which stays forbidden.
  4. **The doc's "async audit writer -> mpsc channel" fallback plan was not needed.**
     `AuditSink::record`/`record_session_event` are synchronous (matching G06's original
     design), so `Governance::record_session_killed` and the hook that calls it run
     synchronously and directly; no channel or spawned drain task was added.
  5. **`connect()`'s existing three callers (top-level startup, the keepalive alarm, the
     `onDisconnect` retry timer) needed NO changes of their own** beyond `connect()` itself
     becoming `async` (the doc's own point: none of them need to await it). The one NEW
     top-level wrapper, `init()`, exists only because plain startup ALSO needs the
     detach-sweep-then-return branch when a kill is already in force (a plain `connect()`
     call would correctly refuse to open the port in that case via its own storage-marker
     guard, but would never run the sweep) -- `init()` is not a general `connect()`
     replacement, it is the one-time startup decision between "finish an interrupted kill"
     and "connect normally."
  6. **One addition beyond the doc's literal text: `updateHoldBadge(null)` at the end of
     `killSession()`.** The doc's own g10 `onDisconnect` handler already clears the hold
     badge to "unknown" on a disconnect, but `killSession()`'s own port teardown
     (`nativePort.disconnect()`) does NOT fire that handler (Chrome does not raise
     `onDisconnect` for a self-initiated disconnect, exactly as the doc notes for the
     reconnect-timer concern) -- so without this call the toolbar badge would show a stale
     hold state after a kill. Added for the same truthful-UI reason g10's own disconnect
     handler exists; not requested by g11's own text, and not a shared control (it renders
     a DIFFERENT widget's state, the pause badge, reacting to the fact that the session
     ended).
- Verification: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings`
  clean. `cargo test` green (224 lib unit tests, up from 215: +3 in `governance::ports
  ::tests` -- `SessionEventRecord`'s field order with none of the eight tool-call fields
  present, the null sink's session-event no-op, `AuditSink`'s object-safety check extended
  to both methods; +1 in `governance::audit::tests` -- a session-event line appended
  alongside tool-call lines in the same file; +1 in `governance::dispatch::tests` --
  `record_session_killed` produces a session event carrying the captured client and
  nothing else; +5 in `transport::executor::tests` -- an in-flight call failed by a kill
  frame with the exact section-7 text, a subsequent call failing within a 1-second bound
  (never the 60s `TOOL_TIMEOUT`), the kill error still winning after the stream itself
  closes, a fresh attach (after tearing down and reconnecting) clearing the flag and
  round-tripping a normal call, and the hook firing exactly once across two kill frames on
  one connection; all other suites unchanged and green: `tests/all_open_golden.rs` 3
  (its `NullAuditSink` updated for the grown trait, no behavior change),
  `tests/architecture.rs` 4 -- confirms the grown `governance/ports.rs`/
  `governance/audit/mod.rs`/`governance/dispatch.rs` introduce zero forbidden edges,
  `tests/audit_recorder.rs` 2 (up from 1: the new end-to-end kill-hook-to-audit-file
  test, driving a real duplex connection through `Browser::on_session_killed` exactly as
  `server::run` wires it), `tests/config_schema_golden.rs` 5 unchanged -- g11 registers no
  config key, `tests/mcp_protocol.rs` 4 UNCHANGED -- proves the kill-never-engaged path
  stays byte-identical end to end over stdio, `tests/tool_schema_fidelity.rs` 6 unchanged
  -- g11 advertises no tool and filters nothing from `tools/list`. `git status --short`
  confirmed the touched-file set matches this entry's list exactly, with NO diff to
  `Cargo.toml`/`Cargo.lock` (no new dependency), `extension/manifest.json`,
  `extension/content.js`, `extension/agent-visual-indicator.js`,
  `src/transport/native/host.rs`, `src/transport/native/ipc.rs`, `src/install/`,
  `src/main.rs`, `src/debug.rs`, or `src/transport/mcp/schemas/tools.json` (`git diff
  --stat` confirmed empty for all of these). ASCII scan (`rg -n "[^\x00-\x7F]"`) clean on
  every touched/new file. `node --check` confirmed `service-worker.js` and `popup.js` stay
  syntactically valid after this task's edits; `manifest.json` still parses as JSON (no
  Chrome available in this environment to load the unpacked extension itself).
- Browser checks queued: `BROWSER-TESTS.md` items `g11-1` through `g11-4`, covering this
  task's own Verification steps 3 (mid-flight kill: infobar disappears, in-flight and
  subsequent calls get the exact error text immediately, popup shows the killed view), 5
  (the mid-kill service-worker-restart guarantee: kill, force the worker down, confirm it
  stays killed across the restart and the keepalive alarm never reconnects it), 6
  (explicit recovery via `Start new session`), and 7 (kill with the binary down, then
  confirm on MCP client restart that calls fail not-connected until reconnect) -- ALL of
  these need a live Chrome with the extension loaded and a live Claude Code connection
  with an actual debugger-attached tab, which this unattended pass cannot drive. Step 8
  (the all-open invariant with the kill button never touched) is already proven by
  `tests/mcp_protocol.rs` staying unchanged and green. The automated suite proves every
  binary-side state transition, the wire protocol, the dispatch-chokepoint ordering
  (killed-check precedes not-connected), and the audit marker without a browser; only the
  actual debugger-infobar disappearance, the real service-worker lifecycle event, and the
  end-to-end popup interaction need a human's eyes.

### g12 manifest parsing, validation, and loading -- 2026-07-02
- Commit: (see this task's commit)
- Files touched: new `src/governance/manifest/document.rs` (the schema-2 manifest types:
  `Manifest`/`IdentityBlock`/`Grant`/`Access`/`ConfigEntry`/`Level`, `ManifestError`, and
  `parse_manifest`'s full pipeline; 32 inline tests); new `src/governance/manifest/source.rs`
  (source-string grammar, org/user selection, `LoadedPolicy`, `load_policy`,
  `manifest_config_as_user_layer`; 21 inline tests); `src/governance/manifest/mod.rs` (module
  doc updated, `pub mod document;`/`pub mod source;`); `src/governance/config/reload.rs`
  (`ConfigStore::load_initial` now delegates to a new
  `load_initial_with_manifest_config`, which merges a user-manifest-derived map into the
  user layer via a new pure `merge_manifest_user_config` helper; 3 new tests); `src/main.rs`
  (`run_server` resolves `--manifest`/`BROWSER_MCP_MANIFEST`, calls `load_policy`, logs the
  outcome truthfully, passes `LoadedPolicy` into `server::run`); `src/transport/mcp/server.rs`
  (`run` takes `LoadedPolicy`, holds it for later stage-2 tasks, and feeds a user-manifest's
  config entries into `ConfigStore`); new `examples/{enterprise-healthcare,
  developer-observe,qa-staging}.json` (byte-for-byte as specified); new
  `tests/manifest_validation.rs` (4 integration tests over the example files and the
  all-open invariant).
- Summary: the manifest engine's front half (ADR-0018 step 3 groundwork; nothing is
  enforced yet). `parse_manifest(text, source_label, domain_pattern_valid, is_known_tool)`
  runs the exact pipeline the doc specifies -- BOM strip, syntax parse (line/column on
  failure), a `schema == 2` check BEFORE shape validation (so a schema-1 manifest gets
  `UnsupportedSchema`, never a confusing shape error), typed deserialize from the STRING
  with `deny_unknown_fields` on every struct (catching superseded blocks and an authored
  `hash` key automatically, since `hash` is `#[serde(skip)]`), semantic field-path
  validation (empty name/version, duplicate grant ids, empty/invalid domains,
  `tools`/`exclude_tools` mutual exclusion and unknown-tool-name checks, unregistered or
  wrongly-typed config keys), then a content hash reusing G09's `canonical_hash` verbatim
  (not a second hash implementation). Source resolution (`env://`/`file://`/`managed://`/
  bare-path grammar, the org-file-always-wins selection rule, always parsing and validating
  a displaced user manifest so its errors stay fatal) and startup wiring (the CLI flag or
  `BROWSER_MCP_MANIFEST`, a truthful startup log naming name/version/hash/mode/origin or
  "no manifest: all-open", a fatal non-zero exit on any broken SELECTED source) are both
  live and manually verified against the real binary (see Verification). A user-supplied
  manifest's `config` entries now actually reach the layer resolver's user layer (with the
  user config FILE's own entries winning on a key collision); an org-sourced manifest's
  entries need no new wiring at all, since G02's own independent parse of the same file
  already feeds them. `Governance`'s mode, `PolicyDecision`, and every dispatch-chokepoint
  behavior are completely untouched: a loaded manifest changes nothing about which calls
  execute, by construction (this task's own explicit scope boundary).
- Deviations from the g-doc per RECONCILIATION.md / the module-placement map (paths
  translated post-A1/G01/G02/G05/G07/G09; RECONCILIATION.md's own row --
  "manifest parse/identity (g09, g12) -> governance/ (core) -- generic over any policy
  doc" -- names this task explicitly):
  1. **`document.rs` and `source.rs` land inside the EXISTING `governance/manifest/`
     directory** (alongside G09's `identity.rs`), not new top-level `src/policy/manifest.rs`
     / `src/policy/source.rs` files -- G09's own module doc already earmarked this exact
     landing spot ("the manifest engine... lands here too, alongside identity, once it
     ships") and its own code comments explicitly anticipated this task's arrival (see
     deviation 2).
  2. **The manifest's hash is computed by calling G09's `canonical_hash` directly**, not a
     second, independent BOM-strip-then-SHA-256 implementation as the doc's own
     `parse_manifest` sketch would produce standing alone. G09's own module doc predicted
     exactly this: "When G12 lands, it computes identity from the exact source bytes it
     already parsed... `canonical_hash` and `identity_from_source` stay as the shared
     primitives." One hash algorithm, one implementation, called from both tasks.
  3. **G09's own standalone org-policy-file reader (`manifest_status`/`ManifestStatus`/
     `active_manifest_identity`, used by `doctor`) is DELIBERATELY LEFT UNTOUCHED**, even
     though G09's own doc comment says it "retires in favor of the engine's loader" once
     G12 lands. No task's scope (including this one) actually asks for that retirement or
     for rewiring `doctor`'s "Policy manifest:" section onto the new engine --
     G12's own diff-scope check (Verification step 6) does not list `doctor.rs` or
     `identity.rs` among the expected touched files, and this task's own required test list
     has no doctor-facing assertion. Retiring a working, already-tested standalone reader
     unprompted would be scope creep beyond what any g-doc actually calls for; G09's
     "retires in favor of" note remains an aspiration for a future task to act on, not an
     implicit requirement of this one. `governance/manifest/identity.rs` shows no diff.
  4. **Tool-name validation and domain-pattern-SYNTAX validation are BOTH injected as
     function pointers** (`is_known_tool: fn(&str) -> bool`, `domain_pattern_valid:
     fn(&str) -> bool`), rather than `document.rs` parsing `TOOLS_JSON` itself or
     hand-rolling its own pattern grammar as the doc's own text assumes. This is the SAME
     "known integration point" pattern used by G01/G02/A5/G03/G08 for exactly this class of
     core-cannot-name-plugin problem: `governance/manifest/document.rs` cannot reference
     the transport layer's tool-schema fixture OR the browser plugin's pattern module
     without violating the a7 arch-test's forbidden-edge scan. `main.rs` (the composition
     root) supplies the real `browser::pattern::is_valid_pattern` and
     `transport::mcp::tools::is_known_tool` at the one real call site
     (`source::load_policy`); `document.rs`'s and `source.rs`'s OWN test modules use small
     test-local mirrors of both (a hardcoded 13-name list; a duplicated syntax check) so
     their tests never depend on `browser::`/`transport::` either -- caught by the a7
     arch-test itself the first time these files were compiled (it flagged even
     TEST-module references, and separately flagged the literal substrings
     `` `crate::transport::` `` / `` `crate::browser::` `` inside doc-comment PROSE
     explaining why those references were avoided; both were reworded to describe the
     avoidance without ever spelling out the forbidden path text).
  5. **Reused `crate::governance::ports::EffectiveMode` for BOTH `Manifest.mode` and
     `Grant.mode`**, per the doc's own explicit instruction ("If G02 already defined an
     observe/enforce mode type... REUSE it instead of defining a duplicate"). No new `Mode`
     enum was created. A NEW `Access` enum (`Read`/`Write`/`All`) WAS created, since nothing
     existing captures a grant's access level (RECONCILIATION.md section 2 is explicit that
     `RwClass`, the classification axis, is a distinct concept from a grant's `access`
     field).
  6. **Domain-pattern SYNTAX validation reuses G07/G08's already-landed, stricter
     `browser::pattern::is_valid_pattern`** (ASCII-only, lowercase-only, per-label length
     and hyphen-position rules), rather than the doc's own literal, more permissive
     grammar sketch ("Case and non-ASCII are NOT load errors... compared after ASCII
     lowercasing at match time... a small pure function... e.g.
     `fn validate_pattern(p: &str) -> Result<(), String>`"). The doc's own prose describes
     a pre-G07 vision (deferring case/non-ASCII handling to a future `policy explain`
     lint and to G13's match-time lowercasing); G07's REAL, already-landed matcher took a
     different, stricter position (`DomainPattern::parse` hard-rejects non-ASCII with
     `PatternError::NonAscii`), and G08 already established the precedent of validating
     `content.security.sacred_domains` with the even-stricter `is_valid_pattern` rather
     than the more lenient `DomainPattern::parse`. Reusing the SAME validator for grant
     `domains` keeps exactly one pattern-syntax grammar authoritative across the whole
     codebase (sacred domains, grants) instead of introducing a third, one-off,
     more-permissive variant that would contradict the two already-landed precedents. No
     required test in this task's own list depends on which validator gates grant domains
     (the required invalid-pattern list contains no uppercase or non-ASCII cases), so this
     is a zero-test-impact consistency choice, not a functional gap. One accepted
     consequence, noted for completeness: error messages name the offending pattern
     ("invalid domain pattern '<pattern>'") but not the SPECIFIC grammar rule it broke,
     since the reused validator returns only a boolean, not a reason string, unlike the
     doc's own suggested signature.
  7. **The "feed manifest config entries into G02's layer model" requirement (section 6)
     was implemented ONLY for the user-manifest case**, via a new
     `ConfigStore::load_initial_with_manifest_config` (default-delegated-to by the
     unchanged `load_initial`) plus `source::manifest_config_as_user_layer`. Tracing through
     the actual mechanics: G02's `parse_org_config` ALREADY reads the org policy file's
     `config` array directly and independently (its own code comment already says so:
     "consumes ONLY the schema and config members; grants, name, version, mode, and
     identity belong to the manifest tasks"), so when the ORG file is the active manifest,
     its config entries already reach the org layers through G02's existing, unrelated
     path -- feeding them again from G12's OWN parsed `Manifest.config` would be a second,
     redundant path to the SAME data, not a new one. The only genuinely new gap this task
     had to fill is the USER-SUPPLIED-manifest case (`--manifest`/`BROWSER_MCP_MANIFEST`),
     which had NO path into the layer resolver at all before this task (`ConfigStore` only
     knew about the fixed org-policy-file and user-config-file paths). Precedence on a
     same-key collision between the manifest's entries and the user config file's own
     entries is not addressed by any doc; picked config.json-wins (the user's own direct,
     immediate expression of preference outranks an external/automated `--manifest` input)
     and documented the choice inline. Mid-session manifest reload/watching stays entirely
     out of scope, per the task's own text; only STARTUP feeding was wired.
  8. **`ManifestError`/`SourceError`/`LoadError` are plain `thiserror` enums surfaced via
     `anyhow`'s blanket `?` conversion at the one real call site (`main.rs`)**, rather than
     routed through `crate::Error`/`crate::Result`. `crate::Error::Config` already exists
     (G02/G03) for the CONFIG REGISTRY's own errors; extending it with manifest-specific
     variants would either overload one variant's meaning across two unrelated concerns or
     require several new `crate::Error` variants for what are already complete, precise,
     `std::error::Error`-implementing types. `anyhow::Context` (`with_context`) adds the
     "loading the governance manifest" framing at the `main.rs` boundary, matching the
     project's own stated style ("typed errors in library code, anyhow in the binary").
- Verification: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings`
  clean. `cargo test` green (277 lib unit tests, up from 224: +32 in
  `governance::manifest::document::tests` -- every required valid-input, invalid-field,
  and shape/field-error-classification case from the doc's own matrix; +21 in
  `governance::manifest::source::tests` -- the full source-string grammar, the pure
  selection function in all four presence combinations, the pure `combine` composition,
  the org-file read/parse/absent/invalid cases against temp files (never the real
  platform path), and the manifest-config-as-user-layer mandatory-downgrade/org-origin/
  no-manifest cases; +3 in `governance::config::reload::tests` -- the manifest/file user
  config merge precedence (empty contributes nothing, manifest-only keys pass through,
  the config file wins on collision); all other suites unchanged and green:
  `tests/all_open_golden.rs` 3, `tests/architecture.rs` 4 -- confirms the new
  `governance/manifest/document.rs`/`source.rs` introduce zero forbidden edges (this is
  where the arch-test caught two real mistakes during this task: production-adjacent test
  helpers that named the transport/browser modules directly, and doc-comment PROSE that
  spelled out the exact forbidden path text while explaining why it was avoided -- both
  fixed, see deviation 4), `tests/audit_recorder.rs` 2, `tests/config_schema_golden.rs` 5
  unchanged -- g12 registers no NEW config key, `tests/mcp_protocol.rs` 4 UNCHANGED --
  proves the no-manifest path stays byte-identical end to end over stdio,
  `tests/tool_schema_fidelity.rs` 6 unchanged; new `tests/manifest_validation.rs` 4 --
  the three example files parse (with `qa-staging.json`'s Unix-shaped `audit.file.path`
  producing the platform-correct, `cfg`-gated outcome: rejected on this Windows dev
  machine by the pre-existing, unrelated `EmptyOrAbsolutePath` registry constraint from
  G01, since `std::path::Path::is_absolute()` requires a drive letter on Windows; accepted
  outcome asserted for non-Windows), plus the all-open invariant (confirmed no real org
  policy file exists on this machine before asserting the strict `LoadedPolicy` shape, the
  same guard G02/G09's own manual-verification passes used). `git status --short`
  confirmed the touched-file set matches this entry's list exactly, with NO diff to
  `Cargo.toml`/`Cargo.lock` (this task adds no new dependency: `sha2` was already present
  since G09), `extension/`, `src/transport/mcp/schemas/tools.json`, or
  `src/governance/dispatch.rs`. ASCII scan (`rg -n "[^\x00-\x7F]"`) clean on every
  touched/new file including the three example JSON files. Manual checks per the task's
  own Verification step 4, run live against the real binary (no browser needed): a valid
  manifest (`--manifest file://examples/enterprise-healthcare.json`) logs
  `name=enterprise-healthcare version=2026.07.1 hash=<64 hex> mode=Some(Observe)
  origin=UserFile` exactly as required (plus two truthful downgrade warnings for its
  `mandatory`-declared `audit.enabled`/`audit.destination` entries, correctly downgraded
  per deviation 7); a `{"schema":1,...}` file fails with
  "unsupported schema version 1 (only schema 2 is supported)"; a manifest with an invalid
  grant domain fails naming the exact pattern and path
  (`grants[0].domains[0]: invalid domain pattern '...'`); no `--manifest` at all logs
  "no manifest: all-open" and the process continues exactly as before this task. All
  manual-check temp files were written under the session scratchpad directory, never a
  real system path, and removed afterward.
- Browser checks queued: none (this task needs no browser at all -- it is pure parsing,
  validation, and startup-time file/environment I/O; no extension file changes, no tool
  advertisement changes, no dispatch-chokepoint behavior changes of any kind).

### g13 grant enforcement at the five points -- 2026-07-02
- Commit: (see this task's commit)
- Files touched: new `src/governance/enforcement.rs` (the pure decision core: `LocalPdp`,
  `check_call` and its `decide_for_host`/`decide_no_page`/`first_matching_grant`/
  `tool_list_denial`/`access_covers` helpers, the `unmatched_domain`/`scheme`/`tool`/
  `access` denial builders, `unclassifiable_denial`; 10 inline tests, every named case the
  doc's own test list requires); new `src/browser/resource.rs` (URL-to-`GoverningResource`
  classification: `resolved_url_resource` for an already-resolved tab URL,
  `navigate_target_resource` for a raw `navigate` argument mirroring the extension's own
  normalization; 7 inline tests); `src/browser/mod.rs` (registers `pub mod resource;`,
  module doc updated); `src/browser/sacred.rs` (factored the shared normalize-then-parse
  step out of `navigate_target_host` into a new `pub(crate) normalize_navigate_target`, so
  g08's sacred check and g13's pre-dispatch check provably agree on the exact same
  extension-mirrored string; `navigate_target_host` itself is behaviorally unchanged, only
  restructured); `src/governance/ports.rs` (deleted a2's placeholder `Grant` struct;
  imports `governance::manifest::document::Grant` instead -- the ONE canonical `Grant` type
  in the crate; `DecisionRequest` grows `action: Option<String>` and
  `manifest_hash: String`; test module updated to match); `src/governance/manifest/
  document.rs` (`Grant` derives `Eq`, needed for `DecisionRequest`'s own `Eq` derive; doc
  comment updated to note it is no longer a stand-in for a2's retired placeholder);
  `src/governance/mod.rs` (registers `pub mod enforcement;`); `src/governance/dispatch.rs`
  (`GovernedState` grows `grants: Vec<Grant>` and `manifest_hash: String`;
  `Governance::governed` grows two matching constructor parameters; `Governance::decide`
  grows to `(tool, action, resource)`, classifies first and denies via
  `enforcement::unclassifiable_denial` on a miss, else delegates to the held `PolicyDecisionPoint`;
  new `Governance::is_governed`; `Governance::record_call` grows a `grant_id: Option<&str>`
  parameter (see deviation 6); new `Governance::record_navigate_landing_deny` for point 5's
  non-zero-duration deny; test module updated for every new/changed signature);
  `src/transport/executor.rs` (new `Browser::tab_url`, sharing a new private
  `send_and_await` helper factored out of `Browser::call`; module doc documents the
  `tab_url_request`/`tab_url_response` pair); `src/transport/native/messages.rs` (doc-only
  addition of the same wire pair, following the existing hold/kill-switch section style);
  `extension/service-worker.js` (the ONE new mechanism-only branch in the native port's
  `onMessage` listener: `tab_url_request` -> `chrome.tabs.get(tabId)` -> `tab_url_response`;
  module doc comment updated); `src/transport/mcp/server.rs` (the dispatch-chokepoint
  rewiring: `run` constructs `Governance::governed` with a `LocalPdp` when
  `loaded_policy.manifest` is `Some`, `Governance::all_open` otherwise; `handle_tools_call`
  runs the sacred check first (unchanged, still always-on and ahead of grant evaluation),
  then, only when governed, resolves the `GoverningResource` for the call
  (`resolve_governing_resource`, new) and consults `governance.decide`, denying before
  dispatch on `Decision::Deny`; after a successful `navigate` dispatch under a manifest,
  `post_navigate_landing_check` (new) re-queries the tab and re-decides, parking on
  `about:blank` and replacing the result with a denial on an off-grant landing; the test
  module's `attach_fake_extension` gained a `tab_url_request`-aware sibling,
  `attach_fake_extension_with_tab_urls`, plus two new point-5 tests); `tests/
  all_open_golden.rs` and `tests/audit_recorder.rs` (both had exactly one call site updated
  for `Governance::decide`'s/`record_call`'s grown signatures; no behavior change); new
  `tests/tool_enforcement.rs` (7 subprocess integration tests: permitted-passes-through +
  denied-domain + audit-shows-both combined into one test since they share a session,
  denied-access-names-the-grant, denied-scheme, fail-closed-on-unknowable-tab-url,
  the `NoPage` union rule end to end under both an all-access and a read-only manifest,
  the all-open invariant, denial-id determinism within and across spawns).
- Summary: the documented no-op policy seam is now real, manifest-driven grant
  enforcement at all five SPEC 5.2-5.5 points, exactly as RECONCILIATION.md section 2
  anticipated: `check_call` (`enforcement.rs`) IS the pure `PolicyDecisionPoint::decide`
  over a2's own `DecisionRequest`/`Decision`/`GoverningResource` seam, not a bespoke
  `CallDomain`/`Verdict` type system (the g13 doc's own sketch of one, minus the
  reconciled parts, was not built -- see deviation 1). With no manifest, behavior stays
  byte-identical to today: `Governance::is_governed()` gates resource resolution (and
  every `tab_url` round trip it would otherwise make) entirely, so STEP 0 adds zero new
  frames and zero new latency, pinned by the all-open invariant. With a manifest active,
  every tool call now resolves a governing resource (the `navigate` target pre-dispatch
  and its final landing post-dispatch; the current tab URL for the 9 other tab-scoped
  tools, queried fresh from the extension every call, never cached or read from tool
  arguments; `GoverningResource::None` for the 3 no-`tabId` tools, decided by the `NoPage`
  union rule) and is allowed or denied before it ever reaches the extension, with denials
  rendered through G08's exact templates and ids and every decision -- allow and deny
  alike -- landing in the audit record with its grant id (allows) or denial id (denies).
- Deviations from the g-doc per RECONCILIATION.md and this session's own established
  reconcile-and-document pattern:
  1. **The entire bespoke `CallDomain`/`Verdict` type system the g13 doc sketches was not
     built.** RECONCILIATION.md section 2 states this explicitly and in this task's own
     name: "g13 `check_call` IS the pure `PolicyDecisionPoint::decide` over a serializable
     `DecisionRequest`." `GoverningResource` (a2) already maps almost exactly onto the
     doc's own `CallDomain` sketch (`Resource(host)`=`Host(host)`, `AlwaysAllow`=
     `AboutBlank`, `OutOfScope(scheme)`=`NonHttp(scheme)`, `None`=`NoPage`,
     `Indeterminate`=`Unknown`), and `Decision`/`Denial` already exist too -- a2's own
     placeholder comments were written anticipating this exact shape. Implementing
     `check_call` directly against these existing types, rather than inventing parallel
     ones, is what this deviation actually is; no functional behavior described in the doc
     was skipped.
  2. **a2's placeholder `Grant { id: String }` is deleted; `DecisionRequest.grants` now
     holds g12's real `Grant` type directly.** There is exactly one `Grant` type in the
     crate. This forced two small, mechanical follow-on changes: `document::Grant` grows
     an `Eq` derive (needed for `DecisionRequest`'s own `Eq`), and `ports.rs`'s test module
     grew a `sample_grant` fixture builder using the real 7-field shape.
  3. **`DecisionRequest` grows `action: Option<String>` and `manifest_hash: String`.**
     Neither field existed when a2 built the placeholder request shape. `action` is
     needed so a denial's `computer (<action>)` label renders correctly from the request
     alone; `manifest_hash` is needed so the denial id is fully reproducible from the
     request alone (load-bearing for g17's future replay-through-the-same-function
     design, per `DecisionRequest`'s own doc comment).
  4. **The URL-to-`GoverningResource` classification lives in a NEW browser-plugin module,
     `src/browser/resource.rs`, not inside `governance/enforcement.rs`.** `check_call`
     itself takes an already-resolved `GoverningResource`; producing one from a raw URL
     string needs `browser::pattern`'s WHATWG-parser-backed matcher, which the governance
     core may never depend on directly (the a7 arch-test). `browser::pattern`'s own module
     doc is explicit that it "assigns no meaning" to a parsed host; `resource.rs` is the
     browser-plugin module that assigns that meaning (parking page / out-of-scope scheme /
     governed host) for g13's specific pre/post-dispatch checks, consuming
     `governance::ports::GoverningResource` (browser depending on governance-core is the
     correct, existing dependency direction; `browser::sacred` already does the same for
     `Denial`).
  5. **The `file:///etc/passwd` scheme-denial example in the g13 doc's own "Integration
     tests" section 8 does not produce a scheme denial once the extension's real
     `navigate` normalization is mirrored exactly (a hard requirement stated twice in the
     doc, and already load-bearing for g08's `navigate_target_host`).** Verified directly
     against `browser::sacred::navigate_target_host`'s own existing, already-passing test
     suite: `"ftp://mybank.com/"` (a foreign, non-allowlisted scheme, exactly analogous to
     `file://`) normalizes to `"https://mybank.com/"` (`Host("mybank.com")`), because the
     extension's regex strips ANY non-`about`/`chrome`/`edge`/`brave` scheme's
     `scheme:/+` prefix and retries the remainder as an `https://` host -- it does not
     preserve the original scheme as a fact to check. Applying the identical transform to
     `"file:///etc/passwd"` (`file` + `:` + three slashes, all consumed by the same
     `scheme:/+` regex) yields `"https://etc/passwd"`, i.e. `Host("etc")`, not
     `NonHttp("file")`. This is not a bug in this task's normalization mirror -- it is
     the ALREADY-EXISTING, already-tested extension behavior for any foreign scheme,
     simply not previously exercised against a `file://`-shaped input. `tests/
     tool_enforcement.rs`'s scheme-denial test uses `"chrome://settings/"` instead (one of
     the four allowlisted prefixes the extension leaves untouched, and already proven by
     `browser::pattern`'s own `non_http_schemes_yield_no_matchable_host` test to classify
     as `NonHttpScheme("chrome")`), and the module doc explains the substitution in full.
     The underlying security property (an off-grant/ungranted target is denied one way or
     another) is not weakened; only which specific rule string fires for a `file://`-shaped
     `navigate` argument differs from the doc's own illustrative pick.
  6. **`Governance::record_call` grows a `grant_id: Option<&str>` parameter.** Found via
     live manual testing (see Verification), not by reading the doc: the pre-existing G06
     signature hardcoded `grant_id: None` for every allow (correct before this task, since
     no grant concept existed yet), so an allowed call under a manifest silently recorded
     `grant_id: null` even though `Decision::Allow { grant_id: Some(..) }` had resolved
     one -- directly contradicting shared format doc section 6.1 and this task's own
     integration-test requirement (test 7: "one record with `decision: "allow"` and
     `grant_id: "example-full"`"). Fixed by growing the signature and threading the
     resolved grant id from the dispatch chokepoint's `Decision::Allow` arm through to the
     call site; every pre-existing caller (in `dispatch.rs`'s own tests and
     `tests/audit_recorder.rs`) updated to pass `None`, preserving their exact prior
     behavior.
  7. **`navigate`'s `"back"`/`"forward"` pre-dispatch check resolves to
     `GoverningResource::None` (the `NoPage` union rule), not a bypass-everything
     sentinel.** The doc states plainly and twice ("none pre-dispatch"; "no pre-check --
     skip straight to dispatch") that these carry no domain to check pre-dispatch, but
     does not say the grant's `tools`/`exclude_tools`/`access` checks should be skipped
     too. Reusing the existing union rule (rather than inventing an unconditional-allow
     path) means a manifest that excludes `navigate` entirely, or grants no write access
     anywhere, still denies a `"back"`/`"forward"` call on tool/access grounds; only the
     DOMAIN check is genuinely inapplicable (there is no target to resolve one from). A
     missing/non-string `url` argument on `navigate` is treated identically, by the same
     reasoning, as a conservative extrapolation beyond what the doc's own cases enumerate.
     Point 5 (the post-navigate landing check) still runs for both cases, exactly as the
     doc requires ("point 5 covers the landing").
  8. **An unparseable `navigate` target (after the extension-mirror normalization) skips
     BOTH the pre-dispatch check and point 5 entirely**, per the doc's own explicit
     "dispatch without pre- or post-check ... nothing to govern": the extension's own
     `new URL(url)` guard refuses to navigate on such an argument (returning `Invalid
     URL: "..."` as an ordinary, non-`isError` text result), so there is no landing to
     re-check either. Implemented as `resolve_governing_resource` returning `None`
     specifically for this one case (distinct from `"back"`/`"forward"`'s `Some((None,
     ..))`), which the dispatch chokepoint reads as "skip the grant machinery for this
     call entirely."
  9. **`Decision::ShadowDeny(_)` is handled with an explicit, commented `unreachable!()`
     at both `match governance.decide(...)` sites in `server.rs`**, rather than folded
     into either the Allow or Deny arm. `Decision`'s three variants predate this task
     (a2); g13 constructs requests with `mode: EffectiveMode::Enforce` unconditionally
     (`Governance::decide`), so `ShadowDeny` is provably unreachable through any path this
     task wires up, and the g13 doc explicitly forbids adding real handling for it
     ("Do not add a ShadowDeny variant... G15 wraps this task's verdict later"). An
     `unreachable!()` is the most honest expression of that: it documents the
     impossibility rather than silently picking a wrong default that could mask a future
     g15 wiring mistake.
  10. **`resolve_tab_host` (g08's sacred-check tab lookup, via `tabs_context_mcp`) and the
      new grant-enforcement `Browser::tab_url` (via a dedicated `tab_url_request` wire
      message) remain two fully independent mechanisms, each queried on its own round
      trip when both the sacred-domains list and a manifest are active for the same
      call.** The g13 doc explicitly introduces `tab_url_request` as a NEW, dedicated,
      side-effect-free mechanism rather than reusing `tabs_context_mcp` (which does tab-
      group management and `createIfEmpty` semantics g13 has no use for), and g08's own
      mechanism/behavior is out of this task's scope to touch ("G13 neither implements
      nor removes it"). The audit record's `domain` field follows the GRANT machinery's
      own resolution when governed (matching shared format doc section 6.1's "the
      parser-normalized host, or null" for a governed session), falling back to the
      sacred check's resolution only when ungoverned or when grant resolution was
      skipped entirely (the unparseable-navigate-target case, deviation 8) -- this is a
      one-line precedence choice (`audit_domain` starts at `tab_domain`, is overwritten
      when grant resolution runs), not a mechanism change to either check.
- Verification: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings`
  clean (two real lints fixed along the way: a hand-written match in `access_covers`
  collapsed to `matches!`, and a nested `if` in the point-5 wiring collapsed per
  `collapsible_if`). `cargo test` green: 296 lib unit tests, up from 277 -- +10 in
  `governance::enforcement::tests` (every named case section 8 of the doc requires:
  `first_matching_grant_wins`, `unmatched_domain_denies`, `access_rules`,
  `tool_list_rules`, `tool_check_precedes_access_check`, `computer_subactions_split`,
  `scheme_and_about_blank`, `unknown_fails_closed`, `no_page_union_rule`,
  `unclassifiable_denies_via_the_tool_rule`), +7 in `browser::resource::tests` (about:blank/
  host/scheme/unparseable classification for both the resolved-URL and navigate-target
  paths, including the back/forward and allowlisted-scheme cases), +2 in
  `transport::mcp::server::tests` (point 5: landing on-grant passes through unchanged;
  landing off-grant parks on `about:blank`, denies naming the final host, and records a
  deny with the REAL elapsed `duration_ms`, not the pre-dispatch `0`) -- all other lib
  suites unchanged in count, only in the handful of call sites whose signatures grew
  (`governance::dispatch::tests`, `governance::ports::tests`); `tests/all_open_golden.rs`
  3 and `tests/architecture.rs` 4 unchanged and green (the arch-test confirms the new
  `enforcement.rs`/`resource.rs` introduce zero forbidden `governance -> browser/
  transport` edges); `tests/audit_recorder.rs` 2, `tests/config_schema_golden.rs` 5,
  `tests/manifest_validation.rs` 4, `tests/peer_death.rs` 1 unchanged;
  `tests/mcp_protocol.rs` 4 and `tests/tool_schema_fidelity.rs` 6 pass UNCHANGED (no
  edits to either file, confirmed via `git status`); new `tests/tool_enforcement.rs` 7,
  covering every numbered integration-test scenario in the doc's section 8 plus
  denial-id determinism (test 4 substitutes `chrome://settings/` for `file:///etc/passwd`
  per deviation 5). A real, load-bearing bug was caught only by live manual testing, not
  by any test written before running the binary by hand (see deviation 6): the FIRST
  manual run showed an allow record with `grant_id: null` despite a resolving grant,
  which is what led to growing `record_call`'s signature; after the fix, the same manual
  scenario showed `grant_id: "example-full"` correctly. `git status --short` confirmed
  the touched-file set matches this entry's list exactly, with NO diff to
  `Cargo.toml`/`Cargo.lock` (no new dependency; `url` was already present since G07) or
  to `src/transport/mcp/schemas/tools.json`. ASCII scan (`rg -n "[^\x00-\x7F]"`) clean on
  every touched/new file, including `extension/service-worker.js`. Two subtle response-
  ordering bugs surfaced and were fixed while writing `tests/tool_enforcement.rs` itself
  (not in production code): `tools/call` runs concurrently (each spawns its own task, per
  `server.rs`'s own module doc), so a near-instant denied call can and does finish before
  a slower permitted call still waiting out the bounded extension-handshake window --
  response order does not track request order, and the same is true of which audit line
  lands first. Both the multi-call integration tests and the audit-line assertions were
  rewritten to look up by `id`/`decision` rather than by position.
- Browser checks queued: 3 (appended to `BROWSER-TESTS.md` as `g13-1`, `g13-2`, `g13-3`):
  a restrictive-manifest session end to end (granted domain works, a mutate action on a
  read-only domain denies while an observe action on the same domain works, hand-clicking
  to an off-grant domain then calling a tool is caught by the per-call drift check, and a
  redirect off-grant parks the tab on `about:blank`); the audit file showing one record
  per call with consistent grant/denial ids across a repeated denial; removing the
  manifest and confirming all-open behaves exactly as before with zero `tab_url_request`
  frames (observable via `--debug`).

### g14 tool advertisement filtering -- 2026-07-02
- Commit: (see this task's commit)
- Files touched: new `src/browser/advertise.rs` (`advertised_tools(fixture, grants)`, the
  pure filter: `grant_permits`/`tool_list_permits`/`access_class_permits`; 6 inline tests);
  `src/browser/mod.rs` (registers `pub mod advertise;`, module doc updated);
  `src/governance/dispatch.rs` (new `Governance::grants()` read-only accessor, mirroring
  `is_governed()`'s shape, exposing `GovernedState.grants` for advertisement -- no
  per-call enforcement or audit code touched); `src/transport/mcp/server.rs`
  (`tools_list_result` takes `&Governance`, parses the fixture, and delegates to
  `advertise::advertised_tools(&fixture, governance.grants())`; the `"tools/list"` match
  arm in `handle_line` passes `governance` through); new `tests/tool_advertisement.rs` (2
  subprocess integration tests: the read-only manifest's exact 8-tool set, and an empty
  `grants` array advertising nothing, both proving the WIRING end to end, not just the
  pure filter g14's own doc asks unit tests to cover).
- Summary: `tools/list` membership now reflects the active manifest, computed once at
  connection time as the union over every grant (never a per-domain decision -- no tab
  exists yet). With no manifest the parsed fixture is returned verbatim, byte for byte,
  preserving the existing `tests/mcp_protocol.rs` invariant unedited. With a manifest, a
  tool is kept when at least one grant's access class (via G05's `classify`, with
  `computer` special-cased to always pass the access-class test per the doc's own
  reasoning: it has both observe and mutate sub-actions, so every access class permits at
  least one) AND tool-list check (`tools`/`exclude_tools`) would let it through; an empty
  `grants` array permits nothing, yielding an empty list rather than falling back to the
  full surface. Schema TEXT is never touched: every retained tool object is a `Value`
  clone of the fixture entry, never rebuilt or re-keyed. Per-call enforcement
  (`governance::enforcement`, g13) remains the sole authoritative check; this task adds no
  denial, no log line, and no audit record of its own.
- Deviations from the g-doc per RECONCILIATION.md and this session's established
  reconcile-and-document pattern:
  1. **The filter module lives at `src/browser/advertise.rs`, not `src/policy/advertise.rs`
     as the pre-A1 doc names it.** RECONCILIATION.md section 1's module-placement map does
     not list a g14-specific row, but its own general principle applies directly: the
     module needs the browser-domain classification TABLE (`browser::classify`, the row
     for g05) and, per RECONCILIATION section 2, "g14 advertisement uses
     `DomainPolicy::tool_surface`" -- squarely browser-plugin territory, the same side of
     the boundary `browser::resource` (g13) and `browser::sacred` (g08) already occupy.
  2. **`DomainPolicy::tool_surface` (a2's sketch, RECONCILIATION section 2) is NOT
     implemented as a trait method.** Precisely mirroring g13's own already-landed
     resolution of the identical tension for `PolicyDecisionPoint`/`check_call`: g13 built
     `check_call` as a plain function taking injected data rather than wiring a
     `DomainPolicy` trait object (which nothing in the crate constructs or consumes to
     date), and `advertised_tools` follows the same precedent -- a plain, directly
     testable function (`fixture: &Value, grants: Option<&[Grant]>`) rather than a trait
     method requiring a `DomainPolicy` implementor that does not otherwise exist.
  3. **`advertised_tools` takes the ALREADY-PARSED fixture as a parameter**, rather than
     parsing `TOOLS_JSON` internally as the doc's primary suggestion describes (the doc's
     own text offers this as an explicit alternative: "Parse `crate::mcp::tools::TOOLS_JSON`
     inside the function (OR accept the parsed fixture)"). Taking the doc's own
     alternative keeps the module free of any production dependency on the transport
     layer (`TOOLS_JSON` lives in `transport::mcp::tools`; `browser::classify`'s own test
     module already reaches across that same boundary, but only in test code, never in
     production code, and this task preserves that asymmetry) and makes the function
     trivially unit-testable against a synthetic fixture, not only the real 13-tool one.
  4. **RECONCILIATION.md section 3's g14 override -- dynamic re-advertisement
     (`notifications/tools/list_changed` on a manifest reload) -- is NOT implemented.**
     This reverses (is more conservative than) an explicit RECONCILIATION instruction, so
     it is called out plainly rather than silently matching the g14 doc's own (superseded)
     "out of scope" note instead. The override assumes a manifest-hot-reload mechanism
     (re-parse-validate-swap the active manifest, fail-closed on an invalid reload, per
     RECONCILIATION section 3's own adjacent "g12 manifest: reloadable" bullet) that does
     not exist at ANY layer yet: `ConfigStore`'s own watcher already has an "INTEGRATION
     POINT (G12)" comment and a `sources.manifest: Option<PathBuf>` slot for exactly this,
     but it is set to `None` unconditionally today (g12 explicitly deferred wiring it,
     documented in g12's own ledger entry: "Mid-session manifest reload/watching stays
     entirely out of scope, per the task's own text; only STARTUP feeding was wired"), and
     `Governance`'s held grants/manifest_hash (g13) are a fixed snapshot built once at
     `run()` startup with no swap mechanism at all. Building real manifest hot-reload --
     which would ALSO need to touch g13's `Governance` construction, not just g14's
     advertisement filter -- is a substantial, cross-cutting, multi-file undertaking that
     does not fit inside "one task = one commit" as a side effect of advertisement
     filtering specifically, and advertising `capabilities.tools.listChanged: true`
     without a real emitter behind it would itself violate the engine's truthfulness rule
     (g14's own constraint list, still binding: "do NOT add `listChanged: true`... would
     violate the truthfulness rule" -- true precisely because the emitter does not exist).
     `advertised_tools`'s own module doc states this plainly and points to this entry.
     Flagging manifest hot-reload (config/grants/advertisement all becoming genuinely live)
     as an explicit follow-up task is the conservative, documented choice BOOTSTRAP asks
     for when no human is available to adjudicate a scope gap between a specific g-doc and
     a terser cross-cutting reconciliation note.
- Verification: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings`
  clean. `cargo test` green: 302 lib unit tests, up from 296 -- +6 in
  `browser::advertise::tests` (every required case: no-manifest byte-identity, the exact
  8-tool read-only set in fixture order, a tool excluded by every grant is omitted, a
  positive `tools` list yields exactly that set, an empty `grants` array yields an empty
  list, `computer` present under both read-only and write-only manifests and absent only
  when every grant excludes it); all other lib suites unchanged in count. `tests/
  all_open_golden.rs` 3, `tests/architecture.rs` 4 (zero new forbidden edges: `advertise.rs`
  depends only on `browser::classify` and `governance::manifest::document`/`ports`, never
  `transport::` in production code), `tests/audit_recorder.rs` 2, `tests/
  config_schema_golden.rs` 5, `tests/manifest_validation.rs` 4, `tests/peer_death.rs` 1,
  `tests/tool_enforcement.rs` 7 all unchanged and green; `tests/mcp_protocol.rs` 4 and
  `tests/tool_schema_fidelity.rs` 6 pass UNCHANGED (no edits to either file, confirmed via
  `git status`); new `tests/tool_advertisement.rs` 2, proving the real wiring (not just the
  pure filter) through an actually-spawned process. `git status --short` confirmed the
  touched-file set matches this entry's list exactly, with NO diff to `Cargo.toml`/
  `Cargo.lock` (no new dependency), `src/transport/mcp/schemas/tools.json`, or anything
  under `extension/`; the one touch to `governance/dispatch.rs` is a single new read-only
  accessor method, no change to `decide`/`record_call`/`record_deny`/audit code. ASCII
  scan (`rg -n "[^\x00-\x7F]"`) clean on every touched/new file.
- Browser checks queued: none (this task is fully unit- and subprocess-testable with no
  extension connected at all: `tools/list` never touches the extension, with or without a
  manifest, and this task adds no new extension traffic, no new audit record, and no
  denial path).

### g15 shadow enforcement (the mode switch) -- 2026-07-02
- Commit: (see this task's commit)
- Files touched: `src/governance/ports.rs` (`EffectiveMode` grows `as_str`/`from_config_str`;
  `DecisionRequest`'s old, never-consulted `mode: EffectiveMode` field is split into
  `manifest_mode: Option<EffectiveMode>` and `config_mode: EffectiveMode`, both actually used
  now; test module updated for the renamed/added fields); `src/governance/enforcement.rs`
  (new `effective_mode` per the doc's own exact signature; new `apply_mode` wrapping any
  `check_call` `Deny` into `Deny`/`ShadowDeny`; `check_call`/`LocalPdp::decide` grow the two
  mode parameters; 6 new inline tests); `src/governance/dispatch.rs` (`GovernedState` grows
  `manifest_mode`; `Governance::governed` grows a matching parameter; `Governance::decide`
  grows `config_mode`, resolves it, and routes the classification-miss denial through the
  SAME `apply_mode` wrap a classified would-deny gets; new `Governance::governance_status`
  and the free `governance_status`/`GovernanceStatus` pair; new
  `Governance::record_shadow_deny`; 5 new inline tests); `src/transport/mcp/server.rs`
  (`run` passes the manifest's own `mode` into `Governance::governed`; `handle_tools_call`
  resolves `config_mode` from `Config::governance_mode()`, threads it through both
  `governance.decide` call sites, and carries a `shadow_denial` across dispatch so a
  `ShadowDeny` executes like an allow but records `shadow_deny`; `post_navigate_landing_check`
  returns the full `Decision` (not just an `Option<Denial>`) and only parks the tab on an
  ACTUAL `Deny`, never on a `ShadowDeny` -- shadow mode is a fully transparent pass-through
  end to end, including point 5; 3 new inline tests: the sacred carve-out under an
  observe-mode manifest, and an enforce-vs-observe pair sharing one denial id);
  `src/governance/manifest/document.rs` (1 new inline test pinning `"observe"`/`"enforce"`
  parsing at both manifest and grant level -- the absent-yields-`None` and
  invalid-string-is-an-error cases were already pinned by G12's own tests);
  `src/doctor.rs` (new `Governance:` report section, `governance_section_lines` resolving
  the manifest via `BROWSER_MCP_MANIFEST` -- doctor has no `--manifest` flag of its own --
  and the real layered config resolver, then a factored-out pure `render_governance_status`
  for the exact wording; 3 new inline tests); `src/governance/config/cli.rs` (`config list`
  grows the g15 SHADOW addendum line, gated on the same shared resolver; `run`/`run_list`
  grow an `is_known_tool` parameter); `src/main.rs` (threads `tools::is_known_tool` into
  the `config` subcommand's `cli::run` call); `tests/all_open_golden.rs` (one call site
  updated for `Governance::decide`'s grown signature); new `tests/shadow_mode.rs` (1
  subprocess integration test: the same denied call under enforce vs observe manifests).
- Summary: the mode switch (ADR-0020 commitment 4) sits directly on top of g13's
  already-landed enforcement path, exactly as the doc frames it: g13's `check_call`
  produces a raw would-deny verdict; `apply_mode` (new) wraps it into a real `Deny`
  (blocks) or a `ShadowDeny` (executes normally, records `decision: "shadow_deny"` with the
  SAME grant id and denial id a `Deny` of the identical call would carry) per the
  precedence `grant.mode > manifest.mode > governance.mode`. The sacred-domains carve-out
  needed no new code at all: sacred denials were already, from g08 onward, a fully
  separate code path that never constructs a `Decision`, so they were already
  structurally incapable of ever becoming `ShadowDeny`; this task adds a test proving that
  observable fact rather than a guard, since there was no `sacred` rule for `apply_mode`
  to special-case. Status surfaces (the `get_status` resolver -- not yet a wire handler,
  since that prerequisite has not landed -- `browser-mcp doctor`, and `config list`) all
  render through the one shared `governance_status` pure function, so they can never
  disagree on whether shadow mode is active.
- Deviations from the g-doc per RECONCILIATION.md and this session's established
  reconcile-and-document pattern:
  1. **The mode data model (`EffectiveMode`) already existed, built by a2/g12, not new to
     this task.** `Manifest.mode: Option<EffectiveMode>` and `Grant.mode: Option<EffectiveMode>`
     were already parsed and validated fields (g12); this task only adds the two small
     convenience methods (`as_str`, `from_config_str`) the wiring and status surfaces need.
     No second `Mode` enum was created, per the doc's own explicit instruction.
  2. **`DecisionRequest`'s pre-existing `mode: EffectiveMode` field is split into
     `manifest_mode: Option<EffectiveMode>` and `config_mode: EffectiveMode`, and renamed.**
     The a2-authored field was never actually consulted by `check_call` (g13 always passed
     a hardcoded `EffectiveMode::Enforce` and nothing read it back) and, as a single value,
     could not represent BOTH tiers of the precedence a resolving grant's own `mode` sits
     between (`grants` already carries each grant's own `mode`; the request was missing a
     manifest-level slot entirely). Splitting it is a minimal, mechanical fix restoring
     `DecisionRequest`'s own "complete, self-contained input" doc-comment promise, not a
     new design.
  3. **The mode switch (`apply_mode`) is applied ONCE, at the very end of `check_call`
     (wrapping whatever `Decision` its existing dispatch produced), rather than threaded
     into each individual denial-builder call site.** `check_call`'s existing internal
     helpers (`decide_for_host`, `decide_no_page`, every `*_denial` builder) are completely
     unchanged; the wrap reads the resolving grant's own `mode` back out by looking up the
     already-returned `Denial.grant_id` in `grants`, rather than threading a grant reference
     through every internal call site a second time. This matches the doc's own framing
     exactly ("G15 wraps that verdict into the final decision") and is the smallest correct
     change to already-tested, already-landed g13 code.
  4. **A classification miss (`Governance::decide`'s `classify` returning `None`) is
     ALSO routed through `apply_mode`**, not left as an unconditional `Deny` as it was
     under g13. The task doc's own rule 3 lists `unmatched_domain, access, tool, scheme` as
     eligible for the mode switch; an unclassifiable call denies with an ordinary
     `tool/<name>` rule (the SAME rule class a grant's `exclude_tools` check produces), so
     it is exactly as eligible as any other `tool` rule -- there is no principled reason an
     org running a rollout in observe mode should have unclassifiable calls hard-blocked
     while every other would-deny is merely logged. `apply_mode` is `pub(crate)` in
     `enforcement.rs` specifically so `dispatch.rs` can reuse it here without duplicating
     the grant-mode lookup.
  5. **`browser-mcp doctor`'s new `Governance:` section resolves the active manifest via
     `BROWSER_MCP_MANIFEST` (never the org policy file's OWN separate G09-era reader,
     which the "Policy manifest:" section above it still uses, untouched)**, using the
     real `governance::manifest::source::load_policy` and a real, standalone
     `ConfigStore::load_initial_with_manifest_config` resolution -- doctor is a one-shot
     CLI invocation with no live session and no `--manifest` flag of its own (that flag is
     server-role only), so the environment variable is the only signal available without
     new CLI plumbing. `config list` (which also has no `--manifest` flag) resolves its
     SHADOW addendum line the identical way. Both failure paths (a broken manifest source,
     a broken config resolution) degrade to a printed line rather than propagating,
     matching doctor's own "always produces a report" posture; `config list`'s addendum
     is simply absent on any such failure (a courtesy line, not part of that command's own
     success/failure contract).
  6. **A real, load-bearing inconsistency in the task doc's own manual-verification
     narrative was found and is NOT "fixed" -- it is documented and worked around.** The
     doc's verification steps 3-4 say "change ONLY the manifest `mode` to `observe`... the
     SAME grant id and the SAME denial id" -- but a manifest's own `mode` field is itself
     part of the canonical bytes `manifest_hash` is computed over (G09's `canonical_hash`),
     so two manifest FILES differing only in `mode` necessarily hash to two DIFFERENT
     `manifest_hash` values, which necessarily produces two DIFFERENT denial ids (the
     formula is `SHA256(manifest_hash + grant_id + rule)`, exactly as ADR-0020 intends: a
     denial id is attributable to "the exact policy version that made it," and a manifest
     with a different `mode` is a different version). This was discovered by writing
     `tests/shadow_mode.rs` literally as the doc describes and watching the denial-id
     assertion fail with two genuinely different ids. The underlying code is correct --
     confirmed by `transport::mcp::server`'s own inline test,
     `grant_shadow_deny_runs_the_tool_and_matches_the_enforce_denial_id`, which holds
     `manifest_hash` and `grants` fixed and varies ONLY the `manifest_mode` parameter
     `Governance::governed` takes, and passes -- so `tests/shadow_mode.rs` was written to
     assert everything else the doc's scenario describes (blocks vs runs, `duration_ms` 0
     vs real, `shadow_deny` vs `deny`, the SAME grant id) while explicitly NOT asserting
     matching denial ids across the two manifest FILES, with a code comment explaining why.
     `BROWSER-TESTS.md`'s g15 entries below correct the same expectation for whoever runs
     the manual verification live: toggling a manifest FILE's `mode` will show a DIFFERENT
     denial id than the doc's own step 4 implies; that is correct, not a bug to chase.
- Verification: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings`
  clean. `cargo test` green: 318 lib unit tests, up from 302 -- +6 in
  `governance::enforcement::tests` (`effective_mode` covering every grant/manifest/config
  combination; the mode switch on `access`/`tool`/`unmatched_domain` denials producing
  identical grant/denial ids across enforce and observe; the switch never touching an
  `Allow`; a per-grant `mode` override winning over an enforcing manifest and config; the
  classification-miss denial going through the identical wrap), +5 in
  `governance::dispatch::tests` (the `governance_status` free function's four cases --
  `None` under all-open, shadow true with grants under observe via either a per-grant-less
  manifest mode or a bare config fallback, shadow false under enforce, never-shadow with
  empty grants even under observe -- plus the live-facade wrapper matching the free
  function exactly), +3 in `transport::mcp::server::tests` (the sacred carve-out staying
  `deny` under an observe-mode manifest; the grant-based enforce-vs-observe pair sharing
  one denial id, using a fake extension so the observe-mode call genuinely dispatches;
  point 5 unaffected since neither new test drives `navigate`), +1 in
  `governance::manifest::document::tests` (`"observe"`/`"enforce"` parsing at both levels),
  +3 in `doctor::tests` (the three exact line-rendering cases via the newly factored-out
  pure `render_governance_status`); all other lib suites unchanged in count except the one
  call site each in `governance::ports::tests` and `tests/all_open_golden.rs` whose
  signature grew. `tests/all_open_golden.rs` 3, `tests/architecture.rs` 4 (zero new
  forbidden `governance -> browser/transport` edges), `tests/audit_recorder.rs` 2, `tests/
  config_schema_golden.rs` 5, `tests/manifest_validation.rs` 4, `tests/peer_death.rs` 1,
  `tests/tool_advertisement.rs` 2, `tests/tool_enforcement.rs` 7 all unchanged and green;
  `tests/mcp_protocol.rs` 4 and `tests/tool_schema_fidelity.rs` 6 pass UNCHANGED (no edits
  to either file, confirmed via `git status`); new `tests/shadow_mode.rs` 1 (see deviation
  6 for exactly what it does and does not assert). `git status --short` confirmed the
  touched-file set matches this entry's list exactly, with NO diff to `Cargo.toml`/
  `Cargo.lock` (no new dependency), `src/transport/mcp/schemas/tools.json`, or anything
  under `extension/`. ASCII scan (`rg -n "[^\x00-\x7F]"`) clean on every touched/new file.
  Manual checks per the task's own Verification steps 3, 5, and 6 (step 4's denial-id
  expectation corrected per deviation 6), run live against the real binary (no browser
  needed): a read-only grant's mutate-class call under an `enforce`-mode manifest returns
  `Denied (D-...)` and the doctor `Governance:` section shows
  `mode  enforce (denied calls are blocked)`; the identical manifest content with `mode`
  flipped to `observe` shows the SHADOW line in both `doctor` and `config list`; a sacred
  domain denies regardless of the manifest's mode (verified inline, see the new
  `transport::mcp::server` test above, since sacred enforcement needs no browser either).
- Browser checks queued: 2 (appended to `BROWSER-TESTS.md` as `g15-1`, `g15-2`): the
  observe-vs-enforce mode switch against a REAL page and REAL agent (confirming a
  shadow-denied mutate action visibly executes with no denial text, unlike g13's own
  enforce-mode verification), and confirming the take-the-wheel/kill-switch/sacred-domain
  paths are all unaffected by an active observe-mode manifest.

### g16 policy explain (deterministic plain-language rendering) -- 2026-07-02
- Commit: (see this task's commit)
- Files touched: new `src/governance/explain.rs` (`explain_manifest`, `explain_user_config`,
  `explain_file`, `ExplainError`, `UserConfigFile`; every template sentence from the task
  doc's Required behavior sections 3-4 transcribed verbatim; 13 inline tests); `src/
  governance/mod.rs` (registers `pub mod explain;`); `src/main.rs` (new `Policy(PolicyArgs)`
  `Command` variant, `PolicyCommand::Explain(ExplainArgs)`, and the synchronous dispatch arm
  calling `explain_file` and printing its `Ok` text with no other stdout output); OVERWRITTEN
  `examples/enterprise-healthcare.json` and `examples/qa-staging.json` with this task's own
  verbatim example content (see deviation 1); new `examples/research-read-only.json`;
  `tests/manifest_validation.rs` (the two existing example tests updated for the new content
  -- `qa-staging`'s Windows-specific config-path error branch is gone along with its old
  `config` array; a new `research_read_only_example_parses` test); new `tests/fixtures/
  explain/{enterprise-healthcare,qa-staging,research-read-only}.txt` (the three goldens,
  reviewed line by line against both the templates and the task doc's own "orientation"
  text, which the `enterprise-healthcare` golden matches character for character modulo the
  hash and two registry description strings the doc itself says will vary); new
  `tests/policy_explain.rs` (3 integration tests: golden equality for all three examples via
  the real spawned binary, an invalid manifest exiting nonzero with empty stdout, a missing
  file exiting nonzero with empty stdout).
- Summary: `browser-mcp policy explain <file>` renders a policy manifest or a user
  configuration file as fixed-template plain-language sentences, entirely from the file
  named on the command line -- no live org policy file, no live user config file, no
  environment variable, no platform path is ever read, so the preview is exactly what an
  administrator or a future import-preview surface would see reviewing that one file in
  isolation. The renderer reuses every prerequisite type and function it possibly can
  (`Manifest`/`Grant`/`Access`/`EffectiveMode`/`ConfigEntry`/`Level`/`IdentityBlock` from
  G12, `parse_manifest` and its already-computed content hash, the G01 key registry's
  `description` strings, `Preset::from_name`, and `layers::validate_value` for user-config
  entries) and duplicates none of them; only the JSON-navigation glue and the fixed
  sentence templates themselves are new. Manual review of all three generated goldens
  against the task doc's own templates (Verification step 5's specific checklist:
  `qa-staging.txt` says `Mode: observe (shadow).`, contains `Observation is not
  protection.`, renders `production-readonly` with `This grant always enforces:`, renders
  `form-writer` with `Only these tools: form_input.`, and carries the `form-writer` write
  warning under `Warnings:`) passed on the first generation; no template needed correction.
- Deviations from the g-doc per RECONCILIATION.md and this session's established
  reconcile-and-document pattern:
  1. **`examples/enterprise-healthcare.json` and `examples/qa-staging.json`, both already
     created and landed by G12 with DIFFERENT content, are OVERWRITTEN with this task's
     own verbatim JSON.** G16's own doc requires these exact three files ("Create the
     `examples/` directory... with exactly these three files (verbatim)") because its
     golden tests pin `explain_file`'s output to their exact bytes; G12's versions (built
     before G16 existed, differing in grant ids, grant shapes, the top-level `mode`
     value, and -- for `qa-staging.json` specifically -- carrying a `config` array with a
     Unix-shaped `audit.file.path` that G12's own manual verification and a
     `#[cfg(windows)]`-gated test built around) cannot produce the byte-for-byte output
     G16's goldens require. Verified before overwriting that no OTHER file references
     these two examples by content (only `tests/manifest_validation.rs`, itself updated
     in this same commit) and that its existing assertions (schema, name, valid hash)
     hold unchanged against the new content; only the grant-count assertions and
     `qa-staging`'s now-obsolete Windows-specific branch needed updating. `examples/
     developer-observe.json` (a G12 file G16 never mentions) is untouched.
  2. **`explain_file`'s public signature grows two injected function pointers**
     (`domain_pattern_valid: fn(&str) -> bool`, `is_known_tool: fn(&str) -> bool`) beyond
     the bare `fn explain_file(path: &Path) -> Result<String, ExplainError>` the pre-A1
     doc shows. `explain_file` must call `parse_manifest` (which itself needs both) and
     `layers::validate_value` for a user-config file's entries (which needs
     `domain_pattern_valid`); `governance/explain.rs` is domain-agnostic core and may
     never import `browser::pattern::is_valid_pattern` or
     `transport::mcp::tools::is_known_tool` directly (the a7 arch-test). This is the
     SAME "known integration point" pattern every prior task in this session has used for
     this exact class of problem (g01/g02/a5/g03/g08/g12/g13's `domain_pattern_valid`;
     g12/g13's `is_known_tool`); `main.rs`'s CLI wiring supplies the real checkers at the
     one real call site, exactly as it already does for `config list`.
  3. **`explain_user_config`'s warnings are computed by a small LOCAL structural pass
     (`parse_user_config_file`, private to this module) rather than by calling
     `governance::config::load::parse_user_config` directly**, even though that function
     already exists and already validates a user config file correctly. Its warning
     STRINGS are formatted for its own log-oriented callers (e.g. `"{path}: unknown
     config key '{key}', ignoring"`) and do not match this task's own required exact
     wording (`"unknown key '<key>' is ignored."`) at all. The task doc's own text
     explicitly allows this ("If the... prerequisite does not expose a user-config
     loader with warnings, validate the `config` map entries against the registry
     locally... but never duplicate MANIFEST parsing or hashing"); the local pass reuses
     every actual VALIDATION primitive (`Preset::from_name`, `key_def`,
     `layers::validate_value`) and duplicates none of their logic -- only the trivial
     JSON member lookups (`obj.get("preset")`, `obj.get("config")`) and the warning
     SENTENCES are new, and those sentences are exactly what section 4.5 requires
     verbatim.
  4. **Two under-specified edge cases were resolved conservatively and documented rather
     than left to guesswork:** (a) an `identity` block present but with `principal` or
     `resolved_by` themselves absent (both are `Option<String>` per the schema; the
     template assumes both are populated whenever the block exists) renders
     `(not specified)` for the missing scalar -- untested by the doc's own required test
     list (which only exercises "no identity block" at all, not a partially-populated
     one) and not exercised by any of the three committed examples either, since all
     three either omit `identity` entirely or populate it fully. (b) the non-ASCII
     domain-pattern lint (shared format 5.1) is UNREACHABLE via the real
     `explain_file` -> `parse_manifest` -> `explain_manifest` pipeline today: `parse_manifest`
     already calls `domain_pattern_valid` (G07's `is_valid_pattern`), which hard-rejects
     any non-ASCII pattern as a MANIFEST VALIDATION ERROR before `explain_manifest` would
     ever see it, so no manifest that successfully parses can carry one. The lint is
     still implemented exactly per the required template and pinned by its own required
     unit test (which constructs a `Grant` directly, bypassing `parse_manifest`, since
     `explain_manifest` makes no assumption about how its caller validated its input --
     a future import-preview surface or a differently-validated manifest source could in
     principle reach this path even though today's one real caller cannot).
- Verification: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings`
  clean. `cargo test` green: 331 lib unit tests, up from 318 -- +13 in
  `governance::explain::tests` (every required case from section 7: the three access
  sentences exact; the bare-write lint firing only for `write`; the non-ASCII lint exact;
  the two per-grant mode sentences plus the no-mode case; all four mode-line/suffix
  combinations plus the observe base sentence's exact ending; both empty-grants two-line
  renderings; the no-identity line; all four settings-block cases (mandatory-only,
  recommended-only, both, empty); both denial-block lines; determinism plus the
  exactly-one-trailing-newline and no-`\r` invariants; a user-config file's settings and
  warnings; an unknown preset's warning and `Preset: none` rendering; an empty user
  config's `User settings: none.` line). `tests/manifest_validation.rs` grew from 4 to 5
  (the new `research_read_only_example_parses` test; the two updated example tests still
  pass, now against G16's own content). All other lib suites and `tests/
  all_open_golden.rs`/`architecture.rs`/`audit_recorder.rs`/`config_schema_golden.rs`/
  `peer_death.rs`/`shadow_mode.rs`/`tool_advertisement.rs`/`tool_enforcement.rs`
  unchanged and green (zero new forbidden `governance -> browser/transport` edges);
  `tests/mcp_protocol.rs` 4 and `tests/tool_schema_fidelity.rs` 6 pass UNCHANGED (no
  edits to either file, confirmed via `git status`, matching this task's own constraint
  3: "if you find yourself editing dispatch or the server loop, stop" -- neither was
  touched); new `tests/policy_explain.rs` 3. `git status --short` confirmed the
  touched-file set matches this entry's list exactly, with NO diff to `Cargo.toml`/
  `Cargo.lock` (no new dependency: `serde_json` and `thiserror` already cover
  everything), `src/transport/mcp/schemas/tools.json`, `src/governance/dispatch.rs`,
  `src/transport/mcp/server.rs`, or anything under `extension/`. ASCII scan
  (`rg -n "[^\x00-\x7F]"`) clean on every touched/new file, including all three example
  JSON files and all three golden `.txt` fixtures. Golden generation and review followed
  the task's exact procedure: ran `policy explain` on each committed example, read every
  line of the output against the Required behavior templates and against the source
  manifest before committing, then separately re-verified byte-for-byte equality between
  the committed golden files and the binary's live output via a script diff (all three:
  exact match) and confirmed no `\r` byte and exactly one trailing `\n` in each golden
  file. Manual checks per the task's own Verification steps 4 and 6, run against the real
  binary: `cargo run -- policy explain examples/enterprise-healthcare.json` prints the
  golden text and nothing else to stdout; `policy explain` on a missing file and on a
  `"schema": 99` file both exit nonzero with nothing on stdout and a message on stderr.
- Browser checks queued: none (a pure CLI/file-based feature; no manifest is ever loaded
  live, no session state is touched, and `explain_file` never contacts the extension or
  any running server).

### g17 policy simulate (replay audit JSONL against a candidate manifest) -- 2026-07-02
- Commit: (see this task's commit)
- Files touched: new `src/governance/simulate.rs`; edited `src/governance/mod.rs` (registers
  `pub mod simulate;`), `src/main.rs` (adds `PolicyCommand::Simulate(SimulateArgs)` and its
  dispatch arm); new `tests/policy_simulate.rs`; new `tests/fixtures/simulate/{audit.jsonl,
  manifest-permissive.json,manifest-restrictive.json}`.
- Summary: `browser-mcp policy simulate <MANIFEST> --replay <AUDIT_JSONL>` replays a recorded
  audit JSON-Lines file through the exact same `governance::enforcement::check_call` pure
  decision function live enforcement calls (g13/g15) -- zero parallel logic, per the task's
  own hard constraint 9. For each non-blank line it reconstructs the call facts (`tool`,
  `action`, recorded `domain` host or the no-URL resource for `domain: null`), recomputes the
  r/w class via `classify` (the recorded `rw` field is never trusted), and calls `check_call`
  with an EMPTY sacred-domain list and `EffectiveMode::Enforce` for both the manifest-mode and
  config-mode parameters (mode is ignored entirely per Required behavior section 3; any
  `Decision::Deny` or `Decision::ShadowDeny` collapses into one would-deny bucket). Every line
  lands in exactly one of three buckets -- would-allow, would-deny, not-evaluable -- via the
  exact ordered bucket table in section 4 (malformed JSON, missing tool, missing/wrong-typed
  domain, wrong-typed action, then unknown tool / unknown computer action / missing computer
  action, then evaluate). Would-deny records group by `(grant_id or "-", domain or "-", tool,
  rule)` in a `BTreeMap` for deterministic, byte-wise ascending output; not-evaluable records
  are collected in file order and never dropped. The renderer produces the exact section-5 ASCII
  report (header, totals, would-deny groups only when nonzero, not-evaluable list only when
  nonzero, a fixed result line), LF-only, one trailing newline. `main.rs`'s `Simulate` arm
  prints the report, flushes stdout explicitly, then calls `std::process::exit(0)` when
  `would_deny == 0` else `exit(2)`; any operational error (unreadable manifest/replay file,
  manifest that fails validation) returns from `main` normally, giving exit 1 with nothing on
  stdout and the message on stderr, matching the existing `?`-propagation pattern the `Explain`
  arm already established (g16). `PolicyCommand` grew its second variant alongside `Explain`
  (g16) rather than creating a second top-level subcommand, per the task's own instruction.
  Manually verified both fixture runs end-to-end against hand-computed expected totals and
  group lines before committing (see Verification).
- Deviations from the g-doc per RECONCILIATION.md:
  1. **The g-doc's own literal function signature for `run_simulate` (section 2) omits the
     injected function-pointer parameters** the "known integration point" pattern requires for
     every governance-core function that needs browser/transport-domain logic (`classify`,
     `domain_matches`, and -- to build the `Manifest`/`GoverningResource` inputs `check_call`
     needs -- `domain_pattern_valid`, `is_known_tool`, matching `parse_manifest`'s own
     signature). `run_simulate` grew all five as trailing `fn` pointer parameters, supplied by
     `main.rs` from `browser::pattern`/`browser::classify`/`transport::mcp::tools`, exactly as
     `explain_file` (g16) and `check_call` (g13/g15) already do. This keeps `governance/`
     free of any `crate::browser`/`crate::transport` reference, satisfying the g-doc's own
     constraint 9 and the arch-test (A7) by construction; it does not change what the module
     does, only how it receives domain-specific logic.
  2. **`SimulateError`'s two file-read variants are named `ManifestIo`/`ReplayIo`** (the g-doc
     does not fix exact variant names, only "manifest file read failure... replay file read
     failure... wrap the manifest task's error type"). Chose names parallel to
     `governance::explain::ExplainError`'s existing `Io`/`Manifest` shape for consistency
     within the same module family; `SimulateError::Manifest` wraps
     `governance::manifest::document::ManifestError` via `#[from]`, unchanged.
  3. **No pure-core extraction was needed** (the g-doc's section 3 fallback, "if the decision
     function turns out not to be callable as a pure function... extract the pure core").
     `check_call` was already a fully synchronous, side-effect-free function taking exactly
     the inputs simulate needs (grants, tool, action, rw, resource, manifest hash,
     domain_matches, manifest_mode, config_mode) as of g15 -- calling it directly from
     `simulate.rs` required zero changes to `check_call` itself, confirming clean reuse and
     satisfying constraint 3 (live enforcement is untouched by construction, not merely by
     test).
  4. **`examples/*.json` DOES exist in the tree** (the g-doc's "Current behavior" section
     states it does not, at authoring time), so the conditional integration test (section 6,
     item 6) was written: `every_committed_example_manifest_never_errors_out` iterates every
     `examples/*.json` file and asserts `policy simulate <file> --replay
     tests/fixtures/simulate/audit.jsonl` never exits 1.
- Verification: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings` clean.
  `cargo test` green: 346 lib unit tests, up from 331 -- +15 in `governance::simulate::tests`
  (empty replay is all zeros; whitespace-only lines uncounted; one test per section-4 bucket-
  table row -- malformed json, non-object json, missing tool, missing domain key, domain wrong
  type, action wrong type, unknown tool, computer action missing, computer unknown action; one
  evaluable allow-and-deny case; totals arithmetic; group sort order with dash entries first;
  the required "same-logic pin" test calling `check_call` directly with the same manifest and
  call facts as a would-deny replay record and asserting its `grant_id`/`denial_id` match the
  simulation report's group exactly). New `tests/policy_simulate.rs` 7 (permissive fixture:
  exit 0, `would deny: 0`, no would-deny-groups section, all four expected not-evaluable
  lines; restrictive fixture golden test: exit 2, exact totals arithmetic (3+6+4=13), all four
  expected group substrings present, every `denial=` value shaped `D-` plus 8 lowercase hex,
  group lines in the exact specified sort order, the four not-evaluable lines; determinism --
  running the restrictive command twice yields byte-identical stdout; nonexistent replay path
  exits 1 with the path named on stderr and empty stdout; invalid-JSON manifest exits 1;
  structurally-invalid manifest -- an `exclude_tools` entry naming an unknown tool -- exits 1;
  every committed `examples/*.json` file never exits 1 against the fixture replay). All other
  suites unchanged and green, including `tests/tool_schema_fidelity.rs` 6 and
  `tests/mcp_protocol.rs` 4 (neither file touched, confirmed via `git status`). `git status
  --porcelain=v1` confirmed the touched-file set matches this entry's list exactly, with no
  diff to `src/mcp/schemas/tools.json`, anything under `extension/`, or `Cargo.toml`/
  `Cargo.lock` (no new dependency). ASCII scan (`rg -n "[^\x00-\x7F]"`, via the Grep tool)
  clean on every touched/new file. Manual runs against the built binary matched hand-computed
  expectations exactly: permissive manifest -> `total actions: 13`, `would allow: 9`,
  `would deny: 0`, `not evaluable: 4`, `result: no would-denies (exit 0)`, exit code 0;
  restrictive manifest -> `total actions: 13`, `would allow: 3`, `would deny: 6`,
  `not evaluable: 4`, four would-deny group lines in the order unmatched_domain / computer-
  access (count=3, folding all three `computer` mutate records into one line) / navigate-
  access / javascript_tool-exclude, `result: 6 would-denies (exit 2)`, exit code 2; a
  nonexistent `--replay` path -> exit 1, stderr names the path, empty stdout; an invalid-JSON
  manifest and a structurally-invalid manifest (unknown tool in `exclude_tools`) each -> exit
  1, empty stdout. Both fixture runs repeated twice with identical byte-for-byte stdout.
- Browser checks queued: none (a pure CLI/file-based feature; simulate never loads a live
  manifest, never touches session state, and never contacts the extension or a running
  server -- it only reads two files given on the command line).

### g18 presets and templates (config preset + policy init) -- 2026-07-02
- Commit: (see this task's commit)
- Files touched: new `src/governance/config/presets.rs`, `src/governance/templates.rs`,
  `examples/developer-unrestricted.json`, `tests/policy_preset.rs`, `tests/policy_init.rs`;
  edited `src/governance/config/{mod.rs,load.rs,cli.rs,reload.rs,layers.rs}`,
  `src/governance/mod.rs`, `src/main.rs`, `tests/manifest_validation.rs`.
- Summary: two independent CLI surfaces landed. (1) `browser-mcp config preset
  <fully-open|safe|restricted> [--dry-run]`: resolves the CURRENT effective state and a
  CANDIDATE state under the new preset from one read of the on-disk org/user layers,
  diffs them key by key (`governance::config::presets::diff_rows`), prints the fallback
  before/after table (the G16 renderer integration point is marked but not wired -- G16
  landed before this task but nothing in this session asked presets to consume it, so the
  literal fallback table stays the only renderer, per the task's own "do not block on G16"
  allowance), then -- unless `--dry-run` -- writes ONLY the `preset` field of the user
  config file (read-modify-write, preserving the `config` map and every other member
  exactly) and confirms. (2) `browser-mcp policy init --template <name> [--out PATH]
  [--force]`: writes one of three `include_str!`-embedded example manifests byte-for-byte
  to a chosen path, refusing to overwrite without `--force`, then prints the exact
  orientation block from the task doc.
  A THIRD piece of work, not called out as its own deliverable in the task doc's Required
  Behavior section but explicitly flagged as this task's responsibility by the tree itself,
  turned out to be the load-bearing part: `governance::config::layers::LayerInputs.preset`
  (layer 4) had been left permanently empty by every call site since G02, with three
  separate code comments reading "mapping a preset name to per-key defaults is the presets
  task (G18)" (`layers.rs`, `load.rs`, `reload.rs`) and a live warning
  ("preset '...' is declared... but preset defaults are not implemented yet, so it has no
  effect") fired on every startup/reload/CLI invocation that saw a `preset` field. Without
  closing this gap, `config preset` would have recorded a selection with zero observable
  effect on any resolved value -- directly contradicting the feature's own purpose. Closed
  it with two new shared primitives in `governance::config`: `Preset::cli_name()` (the
  hyphenated CLI spelling, distinct from `as_str()`'s underscore wire form) and
  `preset_layer(Preset) -> Map<String, Value>` (every registered key's default under a
  preset, built from the registry, never duplicated literals). `load::layer_inputs(org,
  user_values, preset_name)` composes `LayerInputs` from parsed org/user state plus this
  mapping; `load::read_layers` factors the org+user file-read that `load_and_resolve`,
  `cli::resolve_with_warnings`, and the new `presets::resolve_current_and_candidate` all
  now share (previously `load_and_resolve` and `cli.rs`'s `resolve_with_warnings`
  duplicated the same two-file read independently). `reload::LastGoodInputs` grew a
  `preset: Option<String>` field, retained the same way as `user` across a reload (a
  transient bad user-file edit keeps the last-good preset, not drops it); `compose_initial`
  folds the parsed preset into it directly instead of returning it as a separate,
  easy-to-forget tuple element; `plan_reload` and `compose_inputs` route through
  `load::layer_inputs` like everything else. The stale "not implemented yet" warnings and
  doc comments were removed/updated at all three sites. This means `config preset`'s effect
  is now visible everywhere a resolved value surfaces: `config list`/`config get`, the
  mcp-server's live `Config` (on both startup and hot-reload, since a user config file edit
  -- including a `config preset` write -- is one of the three watched sources), and
  `doctor`.
  Manually verified end to end against the real binary (see Verification) before writing
  any test: `config preset fully-open --dry-run` and the `fully_open` alias produced
  identical diffs against pristine local state (three changed rows: secrets.redact,
  audit.enabled, governance.mode); `policy init --template <name>` in an isolated temp
  directory created byte-identical files for all three templates, refused a second run
  without `--force`, and listed all three valid names on an unknown template name.
- Deviations from the g-doc per RECONCILIATION.md:
  1. **The preset-to-layer-4 wiring described above is not itself named as a Required
     Behavior deliverable in the g18 doc** (it predates the current `governance/config/`
     module layout entirely and was authored assuming a flatter `src/policy/` resolver).
     Implemented anyway as a necessary integration point per RECONCILIATION.md section 7
     ("re-verify the target against the current tree before editing") and the doc's own
     repeated in-tree markers naming G18 as the owner. Without it, `config preset` would
     compile, run, and write a file, but every resolved value would stay unchanged --
     silently defeating the feature. Treated as in-scope, not scope creep: it is the
     mechanical prerequisite the CLI surface's own Required Behavior text assumes exists
     ("populate layer 4 with the preset's per-key defaults").
  2. **`src/policy/presets.rs`/`src/policy/templates.rs` (the g-doc's suggested file
     homes) are instead `src/governance/config/presets.rs` and
     `src/governance/templates.rs`**, per RECONCILIATION.md section 1's placement map
     (`src/policy/mod.rs` -> `governance/config/`) and the established pattern g16/g17
     already set for peer features (`governance::explain`, `governance::simulate`).
     `presets.rs` sits under `config/` specifically because it reads/writes the SAME
     `LayerInputs`/`UserConfig`/`user_config_path()` machinery `cli.rs`/`load.rs` own;
     `templates.rs` sits at the `governance/` top level (a peer of `explain`/`simulate`,
     not a config concern) since it manipulates manifest-shaped content, not the config
     registry.
  3. **`examples/enterprise-healthcare.json` and `examples/qa-staging.json` were NOT
     overwritten with this task's own verbatim template text**, even though Required
     Behavior section 2 specifies exact byte-for-byte content for all three files and the
     "Current behavior" section claims `examples/` does not exist yet. Both claims are
     stale: `examples/` was created by G12 and already carries FOUR files (this g-doc
     predates g16's golden-tested overwrite of these same two names with ITS OWN
     different verbatim content -- different org name, version, mode, grant shapes --
     which is itself pinned by `tests/fixtures/explain/*.txt` golden fixtures and
     `tests/manifest_validation.rs`'s grant-count assertions, all already committed and
     shipped in the g16 commit). Overwriting them a third time would have broken g16's
     already-shipped, tested, golden-pinned record for no functional gain: G18's own
     requirements on these two files reduce to "exists under this exact name, `name`
     field matches, parses through the real validator, and (for qa-staging specifically)
     the first two grant ids are `staging` then `production-readonly` in that order" --
     every one of which the CURRENT on-disk content already satisfies (confirmed
     directly: `template_name_fields_agree_with_their_lookup_names`,
     `every_embedded_template_validates_through_the_real_manifest_parser`, and
     `qa_staging_grant_order_is_pinned` all pass against the untouched files). Only
     `examples/developer-unrestricted.json` is genuinely new (no g16-era name collision,
     no prior golden pin), so it is byte-for-byte the g18 doc's own verbatim text.
     `examples/developer-observe.json` (G12-era, a different name/purpose, still exercised
     by its own `tests/manifest_validation.rs` test) and `examples/research-read-only.json`
     (G16-era, golden-pinned) are both left untouched and are simply not part of the
     template set; the g18 doc's "exactly three files" framing is read as "at least these
     three template names exist and validate," matching RECONCILIATION.md's instruction to
     trust prose/intent over stale specifics when the tree has moved on.
  4. **`config preset`'s write path (`write_preset_at`) is unit-tested against temp paths
     only, not integration-tested against the real per-platform user config file**,
     matching the g18 doc's OWN stated test methodology ("build temp paths from
     `std::env::temp_dir()`...") and the established precedent
     `governance::config::cli`'s `write_user_value`/`write_user_value_at` tests already
     set for the identical concern (`load::user_config_path()` resolves the real
     platform path via the `dirs` crate, which is not reliably overridable for a spawned
     child process -- `dirs`'s Windows backend queries the OS profile API directly rather
     than reading `%APPDATA%`, so redirecting it via `Command::env` cannot be trusted to
     work). `tests/policy_preset.rs` therefore exercises ONLY `--dry-run` (read-only,
     safe against the real file, and it does read the real file so its own assertions
     are written to tolerate whatever preset is or is not already declared on the machine
     running the suite) plus clap-level argument parsing (alias resolution, unknown-value
     rejection).
- Verification: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings`
  clean. `cargo test` green: 370 lib unit tests, up from 346 -- +10 in
  `governance::config::presets::tests` (the required locked/kept/changed diff scenario
  exactly as specified: org-mandatory `audit.enabled` locked, user-layer
  `content.security.secrets.redact` kept, `governance.mode` changed `"enforce" ->
  "observe"`, safe->fully_open; the required pristine-defaults no-change case; both
  `render_diff` header forms; every diff-row-kind rendering; write-path missing-file/
  preserve-siblings/corrupt-file/non-object-root cases via `write_preset_at` against temp
  paths; the stored value is always the underscore form for all three presets), +10 in
  `governance::templates::tests` (unknown-template error names all three valid names;
  every template resolves to non-empty bytes; every embedded template validates through
  the REAL `governance::manifest::document::parse_manifest` via local stub validators, per
  the a7 arch-test's text-scan of `governance/**` forbidding even a test-only
  `crate::browser`/`crate::transport` reference; the qa-staging grant-order pin; each
  template's `name` field agrees with its lookup key; `run_init` write/no-force/force/
  unknown-name cases against temp paths; the orientation block's exact text), +3 in
  `governance::config::load::tests` (`layer_inputs` maps a registered preset to its full
  defaults with source `Preset`; `None`/an unregistered name leaves the preset layer empty
  and falls through to `Builtin`; the preset layer never outranks org-mandatory or user),
  +1 in `governance::config::reload::tests` (`compose_initial` folds a declared preset
  into `LastGoodInputs`, and `compose_inputs` maps it through the same `preset_layer`);
  every PRE-EXISTING `LastGoodInputs` test-literal construction (8 sites) updated to
  supply the new `preset` field, three of them (`valid_reload_adopts_both_sources`
  renamed-in-place, `invalid_user_keeps_last_good_user_and_preset_and_warns`,
  `both_sources_invalid_keeps_both_last_good`) extended to assert preset propagation/
  retention through `plan_reload` rather than just adding the field inertly.
  `tests/manifest_validation.rs` grew from 5 to 6 (new
  `developer_unrestricted_example_parses`, mirroring its four siblings). New
  `tests/policy_init.rs` 5 (create matches the embedded template byte-for-byte;
  second-run-without-force names the path and mentions `--force`; `--force` overwrites;
  unknown template lists all three valid names and writes nothing; every one of the three
  templates round-trips through the real CLI to a temp path). New `tests/policy_preset.rs`
  4, deliberately `--dry-run`-only (see deviation 4): the last line is exactly `Dry run:
  nothing written.`; the CLI hyphen spelling and its underscore alias produce
  byte-identical stdout; all three preset spellings dry-run successfully; an unknown
  preset name is rejected by clap itself, naming all three valid values on stderr. All
  other suites unchanged and green, including `tests/tool_schema_fidelity.rs` 6 and
  `tests/mcp_protocol.rs` 4 (neither file touched, confirmed via `git status`).
  `git status --porcelain=v1` confirmed the touched-file set matches this entry's list
  exactly, with no diff to `src/transport/mcp/schemas/tools.json`, anything under
  `extension/`, or `Cargo.toml`/`Cargo.lock` (no new dependency). ASCII scan (via the Grep
  tool, pattern `[^\x00-\x7F]`) clean on every touched/new file. Manual runs against the
  built binary (this task's own Verification steps 3 and 6-8, run before any test was
  written): `config preset fully-open --dry-run` and the `fully_open` alias both printed
  `Preset change: (none) -> fully-open`, three changed rows, and `Dry run: nothing
  written.`, confirmed against the real (unmodified) local user config file; an unknown
  preset value was rejected by clap directly (`invalid value 'bogus'...`, exit 2, distinct
  from this feature's own exit codes); `policy init --template qa-staging` in an isolated
  temp directory created a file byte-identical to `examples/qa-staging.json` and printed
  the exact orientation block; a second run without `--force` failed naming the path and
  `--force`; `--force` succeeded; an unknown template name failed listing all three valid
  names; `diff`-confirmed byte-identity against the repository's own example file directly
  (not just via the test's own read-and-compare).
  **Deliberately NOT run** (this task's own Verification steps 4 and 5): running `config
  preset <name>` WITHOUT `--dry-run` against the real per-platform user config file. This
  would mutate `%APPDATA%\browser-mcp\config.json` (or the equivalent macOS/Linux path) on
  whatever machine runs this unattended session -- a side effect outside the repository
  and outside this task's authority to make unattended, unlike a temp-path write. The
  write path's correctness is instead proven by `write_preset_at`'s own temp-path unit
  tests (deviation 4) plus the identical read-modify-write logic already established and
  manually verified for `config set`'s `write_user_value` in an earlier task. A human
  should run steps 4-5 once on their own machine (or in a disposable VM/container) and can
  safely inspect/revert `%APPDATA%\browser-mcp\config.json` afterward if the result is not
  wanted.
- Browser checks queued: none (both `config preset` and `policy init` are pure CLI/file
  features; neither loads a live manifest, touches session state, or contacts the
  extension or a running mcp-server).

## Reminders before running BROWSER-TESTS.md

Stage 2 is mostly unit-testable (pure governance logic), but several tasks have browser-facing
behavior that needs a real browser: the take-the-wheel pause (g10), the panic kill switch (g11), tool
advertisement filtering and `tools/list_changed` on hot-reload (g14), and end-to-end manifest
enforcement (g12/g13/g15). Accumulate those checks in `BROWSER-TESTS.md` as their tasks land; a human
runs them against a live browser after the code is in, exactly as release-1 did.

## RUN SUMMARY -- BOOTSTRAP.md complete, 2026-07-02

All 23 tasks in the BOOTSTRAP.md linear task sequence (`a1`, `a2`, `a3`, `a7`, `g01`-`g18`) are
landed on the `stage-2` branch, one commit each (with one exception: `a2` produced a small
same-day follow-up correction commit, `8da1bee fix(governance): rename RwClass variants to
Observe/Mutate`, fixing a naming mismatch discovered while implementing `a3`; documented in
`a2`'s own ledger entry). Commit range: `e66b02f` (`a1` module reorg) through this entry's `g18`
commit -- 24 task commits total (23 tasks + the one `a2` correction), for 27 commits on the
`stage-2` branch overall counting the three pre-existing docs commits (`f5c91cf`, `8c188ca`,
`b0b972b`) that seeded `PLAN.md`/the task specs/`BOOTSTRAP.md` itself. The tree is green after
every single commit: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and the
full `cargo test` suite all passed before each commit was made, including
`tests/tool_schema_fidelity.rs` and `tests/mcp_protocol.rs` UNCHANGED and passing from the first
commit to the last (the sacred tool surface and the all-open wire behavior were never touched).
Lib unit test count grew from 0 (pre-stage-2) to 370; the integration test suite grew from
`tests/mcp_protocol.rs` alone to fifteen files.

What stage 2 delivers: a domain-agnostic governance core (`governance/`) cleanly separated from
the browser plugin (`browser/`) and transport infra (`transport/`), enforced by a fail-closed
arch-test (`a7`); a typed, layered configuration registry with hot-reload (`a1`/`a5`/`g01`-`g04`);
an audit flight recorder (`g05`/`g06`/`g09`); the sacred-domains always-on carve-out, the
take-the-wheel pause, and the panic kill switch (`g07`/`g08`/`g10`/`g11`); a schema-2 manifest
engine with real grant enforcement at all five dispatch points, tool advertisement filtering, and
an observe/enforce mode switch with a shadow-deny audit trail (`g12`-`g15`); and the org-policy
trust UX -- deterministic plain-language `policy explain`, audit-replay `policy simulate`,
one-click `config preset`, and `policy init` starting-point templates (`g16`-`g18`).

Conservative choices made along the way, in case any needs revisiting (full reasoning is in each
task's own ledger entry; this is the index):
- **g14**: dynamic tool re-advertisement on a live manifest swap (`tools/list_changed`
  re-computation) was deferred; the static advertisement-filtering logic landed and is tested, but
  see g14's own entry for exactly what is and is not wired to the hot-reload signal.
- **g15**: editing a manifest's `mode` field changes its content hash (one of the three denial-id
  inputs by design), so BROWSER-TESTS.md's g15-1 check does NOT expect the enforce-mode and
  observe-mode denial ids for "the same" rule to match across a manifest edit; this is documented
  as intentional (ADR-0020: a denial id is attributable to the exact policy version that produced
  it), not a bug.
- **g16**: two example manifests (`enterprise-healthcare.json`, `qa-staging.json`) were overwritten
  with g16's own canonical, golden-tested content, superseding what g12 had originally authored.
- **g18**: those same two files were deliberately NOT overwritten a third time with g18's own
  (older, now-superseded) verbatim template text -- reusing g16's already-shipped content instead,
  since it already satisfies every functional requirement g18 actually needs (name agreement,
  real-validator parsing, the qa-staging grant-order pin). Only the genuinely new third template
  (`developer-unrestricted.json`) is g18's own verbatim text. Also: `config preset` WITHOUT
  `--dry-run` was never run against this machine's real per-platform user config file during this
  session (it would have mutated `%APPDATA%\browser-mcp\config.json` outside the repository); the
  write path is proven correct via temp-path unit tests instead, matching this codebase's own
  established precedent for the identical concern (`config set`'s `write_user_value`).
- **Every other task**: no manifest-, config-, or file-shape ambiguity was left unresolved; each
  g-doc's own literal file paths, type signatures, and (where they predated the `governance/`
  module split) module homes were re-derived from `RECONCILIATION.md`'s placement map and the
  actual tree at the time each task ran, documented as a numbered deviation in that task's own
  ledger entry. There are no known open questions or half-finished pieces anywhere in the stage-2
  tree as landed.

`BROWSER-TESTS.md` state: 15 checks queued across five tasks (`g08`-1; `g10`-1 through -5; `g11`-1
through -4; `g13`-1 through -3; `g15`-1 through -2), none of them run in this session (no live
browser is available to an unattended executor). **A human must run every check in
`BROWSER-TESTS.md` against a live Chrome browser with the real extension loaded and a real Claude
Code (or equivalent MCP client) session, exactly as release-1's browser verification pass was
run, before stage 2's governance layer can be considered verified end to end.** Until that pass is
complete, any public-facing copy describing stage 2 (README, release notes, docs) MUST say the
governance layer is **shipped-but-unverified-end-to-end**, not "complete" or "verified" -- the
automated suite proves the pure logic and the wire protocol; it cannot prove a real debugger
attach, a real popup click, a real service-worker restart, or a real redirect landing on-screen.

This branch has NOT been pushed and has NOT been merged to `main` at any point in this run, per
BOOTSTRAP.md's explicit instruction. A human decides when (and whether) `stage-2` merges, after the
BROWSER-TESTS.md pass above.
