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
