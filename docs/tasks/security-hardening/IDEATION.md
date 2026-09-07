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

Accepted 2026-09-06. See [H1](h1-readable-audit.md) and the ADR-0103 amendment for the decision.
The questions below are the preserved agenda, not remaining blockers.

- Confirm the proposed correction and its priority using a concrete pair: page content already
  received by a website versus an additional copy persisted in Ghostlight's local audit.
- Settle the permitted failed-operation projection and useful diagnostic detail while preserving
  the existing metadata-only promise and bounded governed host/target-name exceptions.
- Agree the acceptance story for content-bearing exceptions and failed composite results.

Starting recommendation: a closed metadata projection at the existing completion seam. Website
rollback and deletion of historical audit files are not part of that remedy.

## I2: H2b aggregate outcomes and recovery

Accepted 2026-09-06. See [H2b](h2b-aggregate-outcomes.md) and the ADR-0133 amendment.
The questions below are preserved context, not implementation blockers.

- What should the client see when continue completes later steps after an earlier failure?
- How should counts distinguish attempted, successful, failed, and unattempted children?
- How should earlier applied effects, later uncertainty, and safe next steps compose?

Bring concrete stop/continue result examples, including a decode failure before dispatch, a
governed refusal after a prior success, and a runtime exception after an effect. H2a's actual
stop behavior and the lack of transactional rollback are already settled.

## I3: H4 grouped history and permission explanations

ACCEPTED and implemented locally on 2026-09-06. See [H4](h4-grouped-history.md) and
[ADR-0156](../../adr/0156-grouped-composition-history.md). One collapsed parent groups safe
incremental child receipts. Expansion finds the first problem and preserves view state on updates.
Actual evaluation explains allowed and refused work. Missing receipts stay unconfirmed; wrappers
do not double-count. H1 retention remains intact. Automatic resumption stays a separate decision.

## I4: H5 runtime control and workspace scope

ACCEPTED on 2026-09-06; implemented locally on 2026-09-07. See
[H5](h5-runtime-controls.md) and [ADR-0157](../../adr/0157-session-attention-and-dispatch-control.md).
Request restrictions enforce even when policy observes. Automatic attention belongs to its session;
three matching enforced denials in 60 seconds or five in 120 seconds retain the existing thresholds.
One notice identifies the session and offers history review and explicit scoped recovery. Global
Resume preserves session attention; recovery permits new requests without replay or expanded grants.
Pause/Stop are checked after preparation and writer wait. Already dispatched work retains its facts.
No cooldown, extra setting, automatic replay, or replacement of the fixed human directives.

## I5: H6 detailed frame coverage

Accepted and implemented on 2026-09-07. The owner chose human-only host details, masking excluded
screenshot regions, stopping recordings at the boundary, and refusing unbounded scripts in
excluded cases. ADR-0158 resolves the agenda below. [H6 verification](h6-verification.md) records
the selected contract and actual evidence. These questions are retained as the session's agenda.

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

## I6: H7 audit availability -- accepted (2026-09-07)

The owner accepted the proposed two-mode behavior and directed implementation. Keep working is
preferred by default; Require audit stops subsequent browser work during a known storage failure.
Both preserve actual outcomes, show persistent health and affected receipts without repeated
popups, recover automatically with bounded attempts, and leave gaps explicit without replay.
[ADR-0159](../../adr/0159-audit-health-and-recovery.md) owns the decision;
[H7](h7-audit-health.md) owns implementation evidence. No remaining I6 product choice.

## I7: H8 local resilience -- accepted (2026-09-07)

The owner rejected a refusal-led experience, accepted ordinary burst absorption, quiet cleanup,
visible sustained waiting, responsive other sessions and human controls, and recovery without
replay, then directed implementation. [ADR-0160](../../adr/0160-local-service-continuity.md) owns the
decision; [H8](h8-local-continuity.md) owns its evidence. The original investigation agenda follows.

- Choose the concrete failure cases to test: abandoned unauthenticated peers, malformed framing,
  oversized requests, cancellation, and concurrent legitimate callers.
- Use evidence to choose limits, scope, refusal wording, and recovery. Which controls need policy,
  and which are ordinary implementation bounds?
- Identify the supported-platform evidence needed for discovery-file access and peer isolation.

No resource exhaustion exploit or endpoint-containment guarantee is established. Use bounded,
isolated fixtures; do not make the owner's active clients the load-test target.

## I8: C1 provenance reporting details

Option A accepted on 2026-09-07; reporting foundation is implemented and verified locally under
[ADR-0161](../../adr/0161-connection-bound-provenance.md). See the
[C1 task record](c1-reporting-foundation.md). Every action retains its original connection's
evidence. Familiar labels stay as bounded transient claims; durable records keep only narrow
observed attribution. Plural session connections and action/child details remain quiet and
expandable. Windows corrects its observer's TCP endpoint direction; unsupported and unavailable
evidence remain explicit. Legacy basenames cannot establish the remote executable and are shown
as Not recorded without changing historical files.
Every new connection gets fresh evidence. Signature remains Not checked; no verifier or cache
is added in this cycle. These choices settle the reporting foundation, not all of C1.

The preserved agenda follows. Signature/hash verification fields and result vocabulary, a concrete
verification subject, offline limitations, and trustworthy cache invalidation remain for the next
C1 ideation. They are not blockers for the accepted foundation.

For the remaining verification cycle:

- Select concrete digest/signer evidence, verification result vocabulary, and human explanation.
- Decide which additional verified details belong in transient human views and minimized audit.
  Enabled process diagnostics write to disk; they are not a volatile details surface.
- Select the verifiable subject, supported-platform behavior, and trustworthy cache invalidation.

Reporting first, offline verification, honest uncertainty, and the connector/upstream distinction
are settled in ADR-0105. H1/H4/H7 supply the safe audit projection used by this foundation.

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
