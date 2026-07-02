# Stage 2 ledger

Durable, context-wipe-safe record of stage-2 (governance) execution. This file plus
`BROWSER-TESTS.md` are the executor's memory. On every start, after any interruption, and whenever
state is unclear: read the RESUME HERE section first, then `PLAN.md` and `RECONCILIATION.md`, then the
current task prompt, then continue. Never rely on remembering earlier work; re-read files.

## RESUME HERE

- Branch: `stage-2` (off `main`, which has stage 1 merged). Never push, never merge, never commit to
  `main`.
- Progress: tasks `a1` (module reorg), `a2` (governance ports, + RwClass correction), `a3`
  (governance facade), `a7` (arch-test) landed.
- NEXT TASK: Phase A, task `g01` (`docs/tasks/stage-2/g01-typed-key-registry.md`).
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

## Reminders before running BROWSER-TESTS.md

Stage 2 is mostly unit-testable (pure governance logic), but several tasks have browser-facing
behavior that needs a real browser: the take-the-wheel pause (g10), the panic kill switch (g11), tool
advertisement filtering and `tools/list_changed` on hot-reload (g14), and end-to-end manifest
enforcement (g12/g13/g15). Accumulate those checks in `BROWSER-TESTS.md` as their tasks land; a human
runs them against a live browser after the code is in, exactly as release-1 did.
