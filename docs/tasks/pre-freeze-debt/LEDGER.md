# Pre-freeze debt batch -- ledger

One task = one commit. RESUME HERE is always the first open task.

## RESUME HERE

All four tasks are closed. Next: the G0 freeze, then the G1 pipeline (see
docs/RELEASE-CHECKLIST.md).

## Tasks

### T4 -- ADR-0105 stages 2 and 3

Status: COMPLETE as a scoped landing -- stage 2 shipped; stage 3 re-deferred by owner decision
during implementation (2026-08-24). Recorded as the ADR-0105 amendment of the same date.

What landed:

- New workspace member crate `crates/win-peer` (`ghostlight-win-peer`), the one audited home of
  raw memory access. Its manifest deliberately omits `[lints] workspace = true` because Cargo
  applies inherited workspace lints as command-line flags no in-source allow can override; every
  foreign function is hand-declared against system libraries with `// SAFETY:` notes; no new
  third-party dependency was added.
- Stage 2 end to end: at hello the orchestrator captures the connection quadruple, resolves the
  owning process through `GetExtendedTcpTable` plus a bounded image-name read, and records only
  that name beside the claimed channel in every audit record (new optional `peer_image` field;
  name only, never the path; never an authority input). Non-Windows builds return absence and
  behave exactly as before.
- A repository guard test proves raw memory access stays confined to that crate across the whole
  workspace, forever.
- Live proof on this Windows host: the crate's loopback test resolves an in-process pair to its
  own pid and image; process-journey and cli-journey pass with attribution intact.

Why stage 3 did not land (owner decision, mid-task): Ghostlight has no signed Windows artifact,
so no allowlist entry could ever admit anything and the verification success path could not be
exercised by any test or live lane in this tree. The practical boundary is already covered by
the runtime token plus the stage-1 channel switch. The implementation attempt confirmed real
integration cost (a correct-looking hand-rolled WinVerifyTrust call returned
TRUST_E_PROVIDER_UNKNOWN against signed system binaries while platform tooling verified them)
and was withdrawn rather than half-shipped. Revisit trigger: Ghostlight's first signed release
artifact.

Deviation D1: the task title said "stages 2 and 3"; the landing is stage 2 plus an explicit
re-deferral of stage 3. The ADR amendment carries the decision, not this ledger alone.

Gates: fmt; clippy `-D warnings`; full workspace suite (393 Rust incl. guard + win-peer);
process-journey and cli-journey against fresh `.target-t1/debug` binaries.

### T3 -- model-facing policy explain tool

Status: COMPLETE (2026-08-24).

Landed shape: [ADR-0136](../../adr/0136-model-facing-policy-explain.md) closes the ADR-0122
Decision 9 deferral. `policy_explain` joins the catalog as its twenty-fourth tool with an EMPTY
requirement set, local-read annotations (open_world false), no browser crossing, and no workspace
lease. The handler serves `GovernanceFacade::effective_authority()` -- the same compilation the
workbench renders -- with layer document texts and filesystem paths withheld from model results
per ADR-0136 Decision 2. New `Outcome::PolicyExplained { capabilities, layers }` carries the
sentence and measurement; oracle pins cover singular/plural and sentence/measurement agreement.
Every 23-pin moved to 24: EXPECTED_TOOL_NAMES, annotations table, language count assertions,
capability-map id set plus decode fixture, desktop medallion test, cli/process/live journey
counts. Active-truth docs reconciled: LANGUAGE (new section), ARCHITECTURE, ACCEPTANCE,
STATUS, README (its stale "24 browser tools" claim is now precise), RELEASE-CHECKLIST G7 row
(was a stale "22-tool").

Live proof on the deployed Windows authority after the sanctioned dev-loop deploy:
`ghostlight call policy_explain "{}"` returned status succeeded, effect none, repeat-safe,
summary "Explained current authority across 4 capability areas over 0 layers." on the all-open
machine, with no path fields in the payload. `tests/live-journey.mjs` served the exact 24-tool
catalog against real attached Chrome and completed open/read/screenshot/region work.

Gates: fmt; clippy `-D warnings`; full workspace suite (391 Rust); 137 extension tests;
node --check on changed scripts; process-journey and cli-journey against `.target-t1/debug`;
live-journey against the deployed stack.

### T2 -- unsettled readiness rows get color treatment

Status: COMPLETE (2026-08-24).

Landed shape: `words.js` gains `READINESS_ATTENTION` (loading, unknown) and
`readinessNeedsAttention(entry)` beside the existing `READINESS_NOTE` map; "interactive" is
informational and stays neutral. `view.js` applies the predicate to the row duration cell as an
`unsettled` class; `styles.css` tones that cell amber (`--amber`, the established caution color)
while the parenthetical words stay in the activity cell. The workbench surface journey gained a
43rd assertion pinning the predicate's truth table and the view/stylesheet halves by source --
the same text-guard practice the window already uses.

Gates: node --check on changed modules; workspace tests (391 Rust); workbench-surface journey.

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

Status: CLOSED (scoped; see RESUME HERE above for the landing summary).

Original design note (kept for provenance):

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

D1: the task title said "stages 2 and 3"; the landing is stage 2 plus an explicit re-deferral of
stage 3. The ADR amendment carries the decision, not this ledger alone.

D2: ordinary CI on the pushed freeze exposed a Linux-only Clippy failure in the new crate's test
module (`use super::*` unused where the Windows-only test is cfg-gated out). Repaired at the seam
in `e7d8986b` with a cross-platform negative-control pin, proven against the installed
`x86_64-unknown-linux-gnu` target locally, and the candidate re-frozen at `e7d8986b` per
declare-freeze's deliberate `-Force` path. Lesson carried forward: run the Linux-target Clippy
check on any cfg-split module before pushing, since Windows-local gates cannot see it.
