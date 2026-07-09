# ADR-0051 Phase 4: in-process re-tier of the test suite

Normative source: `docs/adr/0051-verification-topology-fewer-moving-parts.md` (Accepted) and the eval
in `docs/design/verification-topology-evaluation.md`. Phases 1-3 are LANDED on `dev`
(`fc363e3`..`d1a3f9f`); this doc is the P4 execution plan. Re-read the live tree before acting -- the
counts below are as-of-authoring (2026-07-09).

## Goal

Move the tests that spawn OS processes ONLY to prove wiring onto an in-process fixture, so `cargo
test` rarely builds or spawns a service; leave the genuinely end-to-end tests as a small, explicitly
quarantined tier. This is the biggest flakiness win and is fully internal (no install/shipping
impact). Each sub-step leaves a green tree and its own commit.

## The four in-process seams that already exist (generalize these)

1. `Browser` over `tokio::io::duplex` + a fake extension task -- a REAL `Browser` on a fake pipe.
   Pattern lives in `tests/hub_multiplex.rs` (~lines 52-58). The most generalizable transport seam.
2. `Governance::all_open(Arc<sink>)` + `.decide(...)` / `.record_call(...)` -- the whole decision +
   audit chokepoint with no server, no IPC. Pattern in `tests/all_open_golden.rs` (~line 78,
   `NullAuditSink`), `tests/audit_recorder.rs`, `tests/inbound_*`.
3. `StepRunner` trait + `StubRunner` -- the script interpreter over stub dispatch
   (`crates/core/src/mcp/script.rs`, ~lines 579-591). Zero processes.
4. The pure code-declared surface: `advertised_tools_json()`, `browser::directory::REGISTRY`,
   `render_config_schema()`, and source-text scans.

## Sub-steps (each green, one commit)

- P4.1: promote the `Browser`-over-`duplex` + `Governance`-with-fake-sink pattern into a documented,
  reusable `tests/support` in-process fixture (a helper that returns an attached `Browser` + a
  drivable fake-extension handle, and an all-open `Governance`). Optionally introduce a real
  `Listener`/`Stream` trait so the fake transport is not ad hoc (ADR-0051 mentions this; weigh
  whether it earns its keep vs. the duplex helper alone -- prefer the smaller change).
- P4.2: migrate the INCIDENTALLY-E2E wiring tests onto the fixture, a few files per commit, each
  green. Candidates (spawn only to prove wiring; the pure logic is already unit-tested):
  `tool_enforcement` (12), `tool_advertisement` (3), `shadow_mode` (1), `script_tool` (2), most of
  `mcp_protocol` (8; the late-extension test stays E2E), the `manage_web_*` HTTP tests (12, against an
  in-process router if one is reachable), and the CLI-plan subprocess tests (`install_instance`,
  `policy_init`/`policy_preset`/`policy_explain`/`policy_simulate` ~24 -- their plan/render cores are
  pure; test those directly). Do NOT change what a test PROVES, only how it reaches the code.
- P4.3: mark the IRREDUCIBLE E2E tier and gate it as its own CI job. Keep as spawn tests:
  `adapter_reconnect` (2), `adapter_override` (1), `hub_lifecycle` (2), `peer_death` (1),
  `hot_reload` (1), `hub_completion_criteria` (1), `bare_invocation` (1), the redaction spawn test in
  `all_open_golden` (1), and the `mcp_protocol` late-extension test (1). Split `ci.yml` so the fast
  tiers gate every push and the E2E tier runs as a separate (still-required) job; document the split.
  FOLD former-P1.3 here: where an E2E test that STAYS asserts on debug-LOG-TEXT (adapter_reconnect
  "session identity minted..."/"service restart detected; reconnected"; adapter_override "override
  resolution: connected to candidate 1/2"), replace the log-text scrape with a STRUCTURED debug-state
  signal (add a reconnect counter / resolved-candidate index / mint-once field to the debug snapshot
  in `crates/transport/src/observability.rs` -- an observability improvement, NOT a poll-hack, per
  [[prefer-sustainable-architecture]]).

## Progress (as executed)

- P4.1 DONE (`aa182ce`): `tests/support/inproc.rs` -- `Harness::all_open()` / `Harness::governed(manifest)`
  build a real `ServiceContext` via `from_startup`; `drive()` / `drive_raw()` run a fresh
  `serve_session` per call over `tokio::io::duplex`; `attach_fake_extension()` supplies the `Browser`
  leg. The process role is set to `Service` once per binary via a `Once` guard. Chose the duplex
  helper alone over a new `Listener`/`Stream` trait (the smaller change earned its keep). `tests/
  inproc_fixture.rs` is the green self-test. GOTCHA (documented in the fixture): tools that
  orchestrate internal sub-calls (`script`, non-denied `form_fill`) re-enter the runtime via
  `block_in_place`, which panics on the default current-thread test runtime and only shows up as a
  `drive()` hang -- those tests need `#[tokio::test(flavor = "multi_thread")]`.
- P4.2 DONE -- the incidentally-E2E serve_session-WIRING files migrated: `tool_advertisement` +
  `shadow_mode` (`b1faa49`), `tool_enforcement` 11/12 (`4af9300`), `script_tool` + `mcp_protocol`
  7/8 (`c68f4e5`). Three tests that need timing/layers the in-process shape can't reproduce stay
  spawn tests and join the P4.3 quarantine tier: `tool_enforcement::form_fill_without_extension_...`
  (all-open + USER-CONFIG-FILE audit layer, which would need a process-global `GHOSTLIGHT_USER_CONFIG_DIR`
  env racing parallel tests), `mcp_protocol::tools_call_waits_for_a_late_extension_...` (late IPC
  connect), and the `all_open_golden` redaction spawn test.
- P4.2 SCOPE CORRECTION (honest, per [[prefer-sustainable-architecture]]): `manage_web_*` (real
  HTTP/1.1 over a real TCP listener) and the CLI-plan subprocess tests (assert the real `ghostlight`
  CLI's stdout; their render cores are already unit-tested inline) are NOT incidentally-E2E -- they
  test genuinely-external surfaces. Bending them onto the fixture would fabricate an in-process
  router / test a different thing and LOSE coverage. They are reclassified into the P4.3 quarantine
  tier, not migrated.
- P4.3a DONE (CI tier split): the discriminator is "spawns a real SERVICE/ADAPTER" (the slow,
  exe-lock-prone, stdio-relay tests) = the `spawn_service*`/`spawn_adapter` set -- 27 tests across 15
  files, each marked `#[ignore = "e2e: spawns a real ghostlight service/adapter; run via the e2e
  tier -- cargo test -- --ignored"]`. The MIXED files got only their spawn test(s) tagged
  (all_open_golden 1, hub_lifecycle 2, hub_multiplex 2, manifest_validation 1, mcp_protocol 1,
  tool_enforcement 1); the rest are whole-file. NOTE the CLI-command tests (policy_*, bare_invocation,
  install_instance plan tests) spawn only a one-shot `ghostlight` CLI that exits immediately -- fast
  and reliable, so they STAY in the fast tier (not ignored). `ci.yml`: the `test` job runs
  `cargo test --workspace` (skips ignored, spawn-free feedback on every push) + a new still-required
  `e2e` job runs `... -- --ignored`; both across the 3-OS matrix. `scripts/test-e2e.{sh,ps1}` now run
  `-- --include-ignored` (the FULL local pass). Verified: fast tier green with exactly 27 ignored
  (44/44 binaries); e2e tier (`-- --ignored`) green.
- P4.3b NOT STARTED (the remaining focused follow-up; polish on PASSING tests, not load-bearing):
  fold former-P1.3 structured observability. `adapter_reconnect` / `adapter_override` currently scrape
  the debug-EVENTS log text (`observability.rs` Event.summary: "session identity minted (stable for
  this adapter process)", "service restart detected; reconnected", "override resolution: connected to
  candidate N/M"). Replace with STRUCTURED debug-STATE fields on the `Snapshot` (mint-once bool /
  reconnect counter / resolved-candidate index) that the relay updates alongside the event, and have
  the two tests read them via `support::newest_state` + JSON field access. Spans transport
  observability + the relay reconnect/override emit sites + the 2 (slow, spawn) tests -- do it focused
  with its own verify cycle.

## Guardrails

- Local runs: use `scripts/test-e2e.{sh,ps1}` (isolated CARGO_TARGET_DIR + closed stdin) or
  `cargo test ... -- --test-threads=1 < /dev/null` with no live `ghostlight service` running; a live
  dev service + Chrome lock `target/debug/*.exe`.
- Do NOT touch the sacred tool surface, `EXPECTED_TRAINED`, or the ADR-0050 T1 pins.
- The advertised-surface oracle is `directory::advertised_tool_names()/advertised_tool_count()`
  (P1.1) -- behavior tests derive from it; the 2 fidelity guards + `pipeline.rs` explain literal stay
  hand-pinned.

## After P4

Re-pin the ADR-0050 T2-T5 prompts to the post-ADR-0051 surface (the count-bump instructions for the
now-DERIVED sites -- mcp_protocol/tool_enforcement/hub-outbound/adapter/hot_reload -- are obsolete;
an additive tool now touches REGISTRY + its directory.rs pin tables + the 2 fidelity guards +
`pipeline.rs` explain literal only), then resume the batch at T2 (browser_batch).
