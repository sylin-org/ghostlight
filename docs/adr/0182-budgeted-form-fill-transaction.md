# ADR-0182: One Budgeted Physical Form-Fill Transaction

Date: 2026-09-25. Status: Accepted by owner direction.

Amends ADR-0036 Decision 5 and ADR-0169 Decision 2. Builds on ADR-0101, ADR-0138, and
ADR-0181.

## Context

The 1.0 form-fill path correctly made one governance decision and preflighted every field before
editing. Its physical execution still had two unrelated clocks. The orchestrator enforced the
invocation deadline, while the Chromium adapter used fixed retention timers that were absent from
the `Fill` contract. A later correctness fix also waited for stability after every field and then
again for the complete batch. Large ordinary forms could therefore exhaust the service deadline
after browser input even though every effect was progressing normally.

Per-field stability checks did not improve the effect classification. Once native input has been
dispatched, any later failure is already an uncertain partial effect. ADR-0169 also requires input
verification once per composite action rather than once per low-level packet or field.

## Decision

1. `BrowserCommand::Fill` carries the physical `timeout_ms` remaining after the orchestrator
   reserves time for the adapter receipt, relay, executor, and MCP edge. The adapter has no
   independent unbounded clock.
2. One fill is one physical transaction with four phases: preflight every field, dispatch browser
   input, release the debugger lease, then verify the complete batch once. Optional submission
   begins only after that terminal verification succeeds.
3. The adapter checks the shared physical deadline before preflight groups, fields, native text
   packets, verification, and submission. Expiry before input is decisive. Expiry after input keeps
   the existing unknown-effect semantics.
4. Retention polling remains bounded to two seconds but is also capped by the supplied physical
   deadline. It has one stable window for the complete batch. There is no per-field stability
   delay and no second verification path.
5. `browser_fill_form` defaults to 30 seconds because one call may contain 30 fields plus an
   explicit submit and postcondition. Other browser operations keep the eight-second default. An
   explicit caller timeout remains authoritative.

## Consequences

- The service and adapter share one execution budget and one terminal truth boundary.
- Form latency scales with actual input work, not a fixed delay multiplied by field count.
- Known bad controls still refuse during all-field preflight before any edit.
- A page rollback is caught after native input settles, without a second page-local workflow or
  generic transaction framework.
- The adapter remains policy-free. Governance, semantic resolution, and outcome language stay in
  the orchestrator.
