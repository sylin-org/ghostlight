# Pre-freeze debt batch -- ledger

One task = one commit. RESUME HERE is always the first open task.

## RESUME HERE

Next task: T2 (unsettled-row color treatment). T1 is complete.

## Tasks

### T1 -- mcp-connector adopts the shared service handshake

Status: COMPLETE (2026-08-24).

Landed shape: `ghostlight_bridge::client::connect_split` is now the single negotiation home
(runtime read, pre-dial major refusal, dial, hello write, accepted-major check, catalog fetch)
returning a split `Connection { writer, reader, session, server, catalog }`.
`ServiceClient::connect` delegates to it, and the MCP edge's reconnect loop consumes it while
keeping its own reader-thread pump, cancel forwarding, and uuid correlation -- those are edge
concerns ServiceClient does not model. Both major-version checks that only the connector copy
performed now protect every caller, including the CLI edge. Three socket-level unit tests cover
negotiation+invoke, pre-dial runtime refusal, and refused-hello code passthrough.

Gates: fmt; workspace clippy `-D warnings`; 391 Rust tests; 137 extension tests;
process-journey against `.target-t1/debug`.

Design note (verified 2026-08-24): the duplicate is `crates/mcp-connector/src/service_session.rs`
`connect()` (~lines 158-205) versus `crates/bridge/src/client.rs` `ServiceClient::connect`
(~lines 31-93). A whole-session swap is wrong: the edge's reader-thread pump, unsolicited
`CatalogChanged` handling, cancel forwarding, and uuid-keyed out-of-order correlation are
edge concerns ServiceClient does not model. The shared home gains a reusable negotiation step;
both callers use it. The two major-version checks only the connector copy performs must
survive in the shared path. Guard test
`crates/bridge/src/lifecycle.rs::both_connectors_recover_through_the_shared_lifecycle_seam`
pins `mcp-connector/src/service_session.rs` by literal path and asserts
`request_orchestrator_start()` present, `Command::new` absent.

Verification: full gate stack plus `node tests/process-journey.mjs` against the isolated
target directory build.

### T2 -- unsettled readiness rows get color treatment

Status: PENDING.

The duration cell already has running and blocked treatments; an unsettled row reads its
readiness as a parenthetical and should be found while scrolling. Presentation-only; guard
tests in the surface journey must agree.

### T3 -- model-facing policy explain tool

Status: PENDING.

ADR first (next number 0136), citing ADR-0121 Decision 3 (always-available policy explain,
EMPTY requirement class) and closing ADR-0122 Decision 9's deferral. Data source is
`GovernanceFacade::effective_authority()` (`governance/mod.rs:1434`) plus
`capability_map::DIRECTORY`; no browser crossing, host-blind authorize, no workspace lease.
Compile-time arms required: Operation restrictions/name/decode, capability_map requirements,
work run()/activity/timeout/lease, Outcome summary+observed. Pins to move 23 -> 24:
catalog.rs EXPECTED_TOOL_NAMES + annotations table, language/mod.rs count assertions,
capability_map id set + fixtures, desktop medallion test, cli-journey.mjs:121,
process-journey.mjs:464+498, live-journey.mjs:65. Active-truth docs to reconcile:
docs/1.0/LANGUAGE.md (+ new Catalog section), ARCHITECTURE.md:155, ACCEPTANCE.md:11+131+307,
STATUS.md:292+761+Owed bullet, README.md:30 (already over-claims "24 browser tools" against
a 23-tool tree; make it true or precise). RELEASE-CHECKLIST.md:283 says "22-tool" and is
stale against the tree; fix the number while touching claims.

### T4 -- ADR-0105 stages 2 and 3

Status: PENDING.

Owner decision recorded 2026-08-24: relax the invariant for one audited module, implemented
as ONE new workspace member crate whose manifest omits `[lints] workspace = true` (Cargo
applies inherited lints as command-line flags no in-source allow can override; per-crate
override alongside inheritance is rejected by Cargo). Stage 2 observes the socket peer at
the accept site (`service/mod.rs:242-243` currently discards the address): loopback TCP
quadruple -> owning PID via GetExtendedTcpTable, image name only, bounded. Stage 3 adds
`channels.<name>.signers` policy (intersection-composed like other settings), WinVerifyTrust
chain validation without revocation per ADR-0105 lines 92-94, fail-closed where the OS offers
no verification. Default-off when unconfigured: existing admission journeys stay green.
Non-Windows keeps stage-1 behavior; signer-requiring policy denies there.

## Deviations

(none yet)
