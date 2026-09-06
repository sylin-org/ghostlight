# Dependable governance for delightful browser work

Created: 2026-09-06. This is the security-hardening epic, independent of any event or submission.
The owner requested one epic covering the assessment, the subsequent discussion, and its
decisions. Undecided work receives an ideation session before its implementation cycle.

## Purpose

Ghostlight exists to make integrated tooling useful and natural for end users. Its governance
layer lets individuals and organizations choose dependable boundaries that work consistently
whenever work passes through Ghostlight. Permitted work should stay easy. Restrictions, failures,
partial observations, and prior effects should be understandable when they matter.

Success means a person can trust the chosen behavior: one requested script does not unexpectedly
execute twice, a stop actually stops later work, a restricted embedded document follows policy,
and history accurately explains what happened without becoming an unexpected content archive.

## Navigation and authority

- [BOOTSTRAP](BOOTSTRAP.md): execution rules, architecture boundaries, and evidence requirements.
- [LEDGER](LEDGER.md): current state, accepted scope, dependencies, and execution records.
- [IDEATION](IDEATION.md): unresolved decisions to settle before each affected cycle.
- [Assessment](../../design/security-assessment-2026-09-06.md): dated source findings and isolated
  reproductions. Findings are evidence, not automatic approval of every proposed remedy.
- [H2a](h2a-flow-stop.md): existing local flow-stopping fix and its regression evidence.
- [H3](h3-script-effect-truth.md): script-correctness scope and completion evidence.
- [H6](h6-frame-coverage-ux.md): agreed frame policy choices and remaining detailed UX proposals.

ADRs own architecture decisions. The current source, tests, and 1.0 contracts govern code.
The ledger owns progress; this epic is the plan, not another status store.

## Decision index

| Decision | Agreed direction | Durable authority |
| --- | --- | --- |
| Product purpose | Delight through useful integration and dependable chosen boundaries. | [MEMORY](../../MEMORY.md), this charter |
| Host trust | Govern Ghostlight's invocation route; assume host integrity. Independent desktop automation is outside that guarantee. Local execution alone does not prove the upstream application's identity. | [ADR-0105 amendment](../../adr/0105-scripted-intake-channels.md), ledger C1-C3 |
| Provenance | Separate claims from verified observations. Report first; allow optional signer/hash admission only with a verifiable subject and connection binding. Unknown remains unknown, verification stays offline, and no certificate is automatically trusted. | ADR-0105 amendment, ledger C1-C3 |
| Non-atomic effects | Preserve effects that already happened. No general browser rollback or atomic transaction requirement follows from the discussion. | Ledger H1-H3, [ADR-0133](../../adr/0133-behavioral-capability-restoration.md) |
| Readable audit | Readable bounded history, details on demand, and existing target-name controls. Display detail does not authorize more retention. New profiles and richer capture deferred. | [ADR-0103 amendment](../../adr/0103-language-owned-outcome-voice.md), [H1](h1-readable-audit.md) |
| Flow stopping | Stop on the configured failure boundary; do not run later steps after reporting a stop. | ADR-0133 Decision 9, H2a |
| Script execution | Determine a supported form without replaying possibly effectful code; exception text/class is not evidence of safe retry or no effects. Preserve supported REPL behavior and report uncertainty honestly. | ADR-0133 September 6 amendment, H3 |
| Embedded subjects | Apply authored host/RAWX authority to the document actually accessed. A parent's authorization alone does not authorize its embeds. | [ADR-0151 amendments](../../adr/0151-composed-full-page-reading.md), H6 |
| Exclusion handling | Offer permitted content, complete operation access, or complete page access. Notice preferences are separate: on demand, when work is affected, or whenever content is excluded. | ADR-0151 amendments, H6 |
| Starting profile | Use permitted content and notify when work is affected. Structured coverage remains truthful; lower tiers cannot weaken organizational requirements. Read and Write remain separate permissions. | ADR-0151 amendments, H6 |
| Ideation before undecided work | Settle unresolved product choices before their implementation cycle. Accepted decisions stay accepted; routine implementation choices do not require repeated permission. | Owner instruction, BOOTSTRAP, IDEATION |

## Work breakdown

Every package is included in the epic. Inclusion of a proposed remedy does not settle its product
semantics. The ledger labels agreement, conditions, evidence, and actual completion separately.

| Package | Intended outcome | Dependencies and decision work |
| --- | --- | --- |
| H2a: Flow stopping | A failed child under stop prevents subsequent dispatch and preserves earlier effects. | Separate implementation record; broader aggregate behavior is H2b. |
| H3: Script effect truth | A runtime exception cannot cause automatic duplicate execution or a false no-effect result. | Selected first new cycle; preserve script compatibility and prove the mechanism in Chromium. |
| H1: Audit confidentiality | Operation metadata stays useful without copying page results, values, scripts, or arbitrary errors into durable audit. | I1 accepted: readable bounded history and existing name control; profiles and richer capture deferred. See [H1](h1-readable-audit.md). |
| H2b: Aggregate reporting | Completed, failed, unattempted, and uncertain work has one truthful aggregate account. | Ideation I2; build on H2a and coordinate effect semantics with H3. |
| H4: Child receipts | Each attempted child gets a safe receipt grouped under its parent. | Ideation I3; safe projection from H1 and agreed aggregate semantics from H2b. |
| H5: Runtime control | Automatic attention belongs to its workspace; explicit human controls act at the intended boundary. | Ideation I4 for unresolved details; existing pause/stop decisions remain authoritative. Direct scope proof can precede H4; composed proof follows it. |
| H6: Embedded-document policy | Chosen exclusion handling and notice preferences work consistently across supported observations and actions. | Ideation I5 settles remaining schema, scope, disclosure, and capture choices; agreed modes are not reopened. |
| H7: Audit availability | A recording failure has truthful visibility, recovery, and any explicitly chosen admission consequence. | Ideation I6; coordinate with H1/H4 for consistent records and prior effects. |
| H8: Local resilience | Malformed or abandoned clients cannot consume unbounded local resources or silently strand unrelated work. | Ideation I7 chooses bounded evidence and response; do not infer a demonstrated exploit. |
| C1: Provenance reporting | Claimed client identity and observed/verified peer evidence remain distinct and useful. | Ideation I8 for field/privacy/platform details; safe audit projection before extending durable records. |
| C2: Direct-peer admission | Optional signer/hash rules work for an actual verifiable connecting subject. | C1 and ideation I9 selecting a real integration and verification contract. |
| C3: Upstream identity binding | Application-specific admission rests on evidence bound to the real connection. | Conditional on pursuing that admission. I9 chooses the integration and proof before enforcement claims. |

## Implementation cycles

1. Finish the existing H2a checkpoint and implement H3 as the first new cycle. Recheck the tree,
   preserve the separate logical changes, and prove script compatibility alongside the defects.
2. H1 is accepted and complete locally. Settle H2b in ideation, then implement aggregate behavior. H4
   follows their shared completion semantics. These packages establish safe evidence for later work.
3. Run targeted ideation for H5-H7 and implement bounded tasks. H6's agreed choices shape its
   evidence contract; do not build a generic policy framework while details remain unresolved.
4. Complete C1 after safe durable reporting is available. C2/C3 follow only the agreed concrete
   identity proof. H8 adds focused resilience work justified by its investigation.
5. Reconcile active claims and run the composed acceptance journey across delivered packages.

H3 was selected as the first new cycle; the ledger records its completion evidence. The order
after H3 remains a working recommendation and may be
revised during ideation based on evidence and dependencies. An undecided package holds its own
implementation; independent agreed work need not wait for every epic discussion to finish.

## Evidence and acceptance

- Translate reproduced assessment cases into meaningful regression tests at the current seams.
- Prove both refusal and useful permitted work, including all-open behavior and allowed targets
  alongside policy-excluded embeds under the chosen mode.
- Check positive observations and scoped negative results; no excluded area can appear inspected.
- Prove one attempted script does not repeat after the two SA-04 exceptions, while ordinary
  expressions, supported bare returns, and top-level await remain usable.
- Preserve prior effects and unknown outcomes across flow stopping, cancellation, and audit failure.
- Demonstrate content-minimized receipts for actual attempts after H1/H4; distinguish durable
  logging success from a workbench-only projection after H7.
- Exercise relevant real process and Chromium boundaries in addition to deterministic unit tests.
  Record untested platforms and unavailable lanes honestly.
- Meet the repository's commit gates and update current contracts and active trust claims with
  the evidence. No claim is strengthened merely because a decision or test plan exists.

Epic completion requires a disposition for every package: delivered with evidence, or explicitly
deferred by the owner with its condition and remaining obligation recorded. Unit success, local
completion, deployment, and publication remain separate states. This planning request does not
publish, deploy, or make an event submission.

## Scope limits

Keep the local product and existing process boundaries. Do not add host containment, browser-wide
network filtering, semantic business-intent authorization, a vendor telemetry service, a SIEM
integration, cryptographic audit chaining, a universal client credential protocol, or a signing
certificate procurement requirement merely to strengthen a submission narrative. Such work has
not been selected. Protect the existing all-open experience and avoid blanket page blocking unless
the configured exclusion mode requests it.
