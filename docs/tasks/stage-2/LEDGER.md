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
  `extension/`), `g11` (panic kill switch) landed. Phase C is COMPLETE.
- NEXT TASK: Phase D, task `g12` (`docs/tasks/stage-2/g12-manifest-engine.md`).
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

## Reminders before running BROWSER-TESTS.md

Stage 2 is mostly unit-testable (pure governance logic), but several tasks have browser-facing
behavior that needs a real browser: the take-the-wheel pause (g10), the panic kill switch (g11), tool
advertisement filtering and `tools/list_changed` on hot-reload (g14), and end-to-end manifest
enforcement (g12/g13/g15). Accumulate those checks in `BROWSER-TESTS.md` as their tasks land; a human
runs them against a live browser after the code is in, exactly as release-1 did.
