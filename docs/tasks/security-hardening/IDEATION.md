# Security-hardening ideation agenda

The owner requires an ideation session before implementing a package whose decisions remain
open. This agenda contains questions, not accepted answers. The [ledger](LEDGER.md) owns state;
the [epic](EPIC.md) maps dependencies and accepted decisions.

## Session contract

Discuss the next affected package with concrete evidence and a small number of meaningful
behavior choices. Start with what is already settled. Bring sample results, failure cases,
policy consequences, and the effect on ordinary allowed work. Read-only investigation and
disposable evidence can inform the session; do not silently implement an undecided policy.

Before that package's implementation cycle:

1. Record the owner's choice, rejected alternatives that explain a tradeoff, and explicit deferrals.
2. Add an ADR or marked amendment when an architecture or product decision changes.
3. Update its ledger entry and draft bounded tasks with expected behavior and acceptance evidence.
4. Proceed within the agreed scope. Do not re-request permission for settled decisions or routine
   implementation choices. Return to ideation if evidence demands a new product tradeoff.

H3's execution-correctness behavior is agreed. Selecting a compatible implementation mechanism
does not by itself require another ideation session. A proposed loss of existing script behavior
or a new tool signature would change that scope and must be discussed first.

## I1: H1 audit confidentiality

- Confirm the proposed correction and its priority using a concrete pair: page content already
  received by a website versus an additional copy persisted in Ghostlight's local audit.
- Settle the permitted failed-operation projection and useful diagnostic detail while preserving
  the existing metadata-only promise and bounded governed host/target-name exceptions.
- Agree the acceptance story for content-bearing exceptions and failed composite results.

Starting recommendation: a closed metadata projection at the existing completion seam. Website
rollback and deletion of historical audit files are not part of that remedy.

## I2: H2b aggregate outcomes and recovery

- What should the client see when continue completes later steps after an earlier failure?
- How should counts distinguish attempted, successful, failed, and unattempted children?
- How should earlier applied effects, later uncertainty, and safe next steps compose?

Bring concrete stop/continue result examples, including a decode failure before dispatch, a
governed refusal after a prior success, and a runtime exception after an effect. H2a's actual
stop behavior and the lack of transactional rollback are already settled.

## I3: H4 child operation history

- What is the useful default parent summary, and what child detail appears on expansion?
- Which child/parent correlation and positive grant attribution belong in the bounded receipt?
- How do refused children and unattempted steps appear without fabricating executed operations?

Starting recommendation: one receipt per attempted child, grouped under the parent, through the
same safe completion seam. Decide after H1/I1 and coordinate with H2b/I2.

## I4: H5 runtime control and workspace scope

- Confirm the scope repair and user-visible recovery from automatic attention in one workspace.
- Agree the observable outcome of a pause arriving between target observation and effect dispatch,
  including work already dispatched and therefore potentially uncertain.
- Specify whether existing denial thresholds/reset behavior suffice before adding any settings.

ADR-0126 already decides that pause refuses subsequent effects and stop is terminal. Its fixed
directives exist. Those decisions are context, not open votes on a new suspended-caller design.

## I5: H6 detailed frame coverage

- Select policy keys, tier resolution details, and what complete operation/page scope requires
  when frames are nested, navigating, unavailable, or outside supported observation mechanisms.
- Choose result coverage fields and authored wording for read, inspect, find, and waits. Separate
  access exclusions, size limits, and unavailable documents; qualify negative answers correctly.
- Decide whether bounded excluded hosts are disclosed, to whom, and under which privacy controls.
  Historical frame-origin hiding needs a deliberate amendment if this changes.
- Settle notice placement and examples for all three preferences without repeated interruptions.
- Choose capture behavior for screenshots, recordings, and derived image artifacts: which
  exclusions can be proven, when to refuse, and what permitted alternatives exist.
- Settle preflight for multi-target effects without promising atomicity during later navigation.

The document boundary, three handling modes, three notice preferences, and starting profile are
already agreed. Use [H6's design](h6-frame-coverage-ux.md) as the discussion artifact. Do not
automatically remove frames, change grants, or treat a presence restriction as network blocking.

## I6: H7 audit availability

- Choose behavior when an audit append fails: how is the failure shown, and can later work run?
- Decide whether policy can require working durable audit, its default, scope, and recovery rule.
- Agree bounded recovery and history wording when the UI has a record that durable storage lacks.

Starting recommendation: visible degradation with continued work by default; explicitly required
audit can refuse later browser work during a known failure. Preserve prior effects and keep
diagnostics/human controls reachable. This recommendation is not yet an accepted policy.

## I7: H8 local resilience

- Choose the concrete failure cases to test: abandoned unauthenticated peers, malformed framing,
  oversized requests, cancellation, and concurrent legitimate callers.
- Use evidence to choose limits, scope, refusal wording, and recovery. Which controls need policy,
  and which are ordinary implementation bounds?
- Identify the supported-platform evidence needed for discovery-file access and peer isolation.

No resource exhaustion exploit or endpoint-containment guarantee is established. Use bounded,
isolated fixtures; do not make the owner's active clients the load-test target.

## I8: C1 provenance reporting details

- Select claimed versus observed fields, verification result vocabulary, and human explanation.
- Decide which details belong in volatile diagnostics versus minimized durable audit.
- Define supported-platform unknown/unavailable behavior and trustworthy cache invalidation.

Reporting first, offline verification, honest uncertainty, and the connector/upstream distinction
are settled in ADR-0105. Extra durable fields depend on the safe audit projection.

## I9: C2/C3 admission integration and lifecycle

- Select the concrete direct peer that justifies admission and a real signed-success fixture.
- Define signer identity, exact-hash representation, rotation, tampering, unavailable evidence,
  and verification cache behavior without hidden network fetches.
- If upstream MCP application admission is desired, choose a participating integration and prove
  how the asserted application identity binds to the actual connection.
- Distinguish binary provenance from credential possession and document offline limitations.

Signer/hash admission is optional and cannot expand browser capabilities. Authenticating
Ghostlight's connector or a parent process does not prove the upstream harness. No handshake,
client credential system, or certificate procurement requirement has been selected.

## Decision record format

For each completed session, append its date, package, concrete agreed behavior, ADR link if
applicable, remaining deferrals, acceptance examples, and next bounded implementation tasks to
the ledger. Leave unresolved questions visible. Prior recommendations remain historical context;
do not silently promote them to decisions.
