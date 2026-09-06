# security-hardening BOOTSTRAP

This epic hardens Ghostlight's own browser-operation boundary and the evidence it produces.
It is independent of any event or submission. The [epic](EPIC.md) records the purpose, complete
work breakdown, decision index, dependencies, and acceptance. The [ledger](LEDGER.md) distinguishes
agreed direction, conditional work, proposals, and completed implementation; it owns progress.
The owner selected H3 as the next implementation cycle and requires ideation before any cycle
whose product decisions remain unresolved. H3's first cycle is complete locally; read the ledger
for current readiness before implementing any further package.

## Read first

1. Root [AGENTS.md](../../../AGENTS.md), [MEMORY](../../MEMORY.md), and [STATUS](../../STATUS.md).
2. The [dated security assessment](../../design/security-assessment-2026-09-06.md), which records
   source findings, isolated reproductions, and the limits of those checks.
3. The four [1.0 contracts](../../1.0/) and the ADRs for the subsystem being considered.
4. [ADR-0105](../../adr/0105-scripted-intake-channels.md), including its September 6 amendment,
   for the agreed client-provenance scope.
5. [ADR-0133](../../adr/0133-behavioral-capability-restoration.md) and
   [ADR-0151](../../adr/0151-composed-full-page-reading.md), including their September 6 amendments,
   for accepted script correctness and frame policy choices.
6. [IDEATION](IDEATION.md) for unresolved decisions in the package about to start.
7. [DEV-LOOP](../../DEV-LOOP.md) before any build, process journey, or deployment.

## Scope and architecture

- Ghostlight governs operations submitted through it. It assumes host integrity and does not
  contain actors with independent control of the user's desktop or alternate browser controllers.
  A model limited to approved tools is a different attacker capability from unrestricted native
  execution as the same OS user.
- The orchestrator remains the sole product and policy authority. Existing connectors carry
  typed evidence; the extension owns browser mechanisms and observed facts, never policy.
- Preserve one immutable authority snapshot, one workspace lease, and one truthful completion
  for each operation. Composition must share those seams without reacquiring a parent's lease
  or taking a different snapshot for each child.
- Preserve all-open operation, including local HTTP(S) destinations under ADR-0155. No new
  default address ban, host-containment claim, telemetry, or vendor service follows from this epic.
- Content-minimized audit is a structural requirement. Typed audit projection precedes adding
  more result-bearing records. Raw page errors and nested results are not safe audit metadata.
- Pause refusal, the pinned stop directive, and the distinction between human hold and automatic
  attention are already decided in ADR-0126. Test enforcement timing and scope; do not reintroduce
  a suspended-caller design or duplicate those language decisions.

## From discussion to implementation

The owner's instruction is: "the ones without decisions, we'll discuss prior to implementation
cycle in an ideation session." Apply it to each affected package, using IDEATION's concrete
agenda. Work included in this epic does not acquire accepted product semantics merely by being
listed. Record the session's decisions and remaining deferrals before implementing dependent work.

Settled decisions remain settled. H3's behavior is accepted; routine technical exploration and
implementation selection within it do not require another permission request. An unresolved
package does not block independent agreed work. If new evidence requires a product tradeoff,
bring that tradeoff to ideation before changing the contract.

The ledger entries are work packages; H3 has a bounded preparation brief. Before implementing
a package, map the current owning seams, read its ADRs, and write one-commit tasks with explicit
expected behavior, prerequisites, and what each task removes or replaces. Use the explore skill
before production changes. Record architecture decisions in an ADR or marked amendment rather
than treating this plan as a second architecture contract.

Use the current source to recheck each finding. The assessment baseline is
`48ef29ece1ff0f1633daba62a03932a666f00ca5`; a later revision is a reason to inspect its changes,
not to assume either that every finding persists or that the old implementation should return.
If a finding no longer reproduces, record the fixing source and the evidence.

## Evidence and completion

- Use deterministic fixtures and unique synthetic sentinels. Do not require real secrets,
  production sites, or machine-local notes to demonstrate a failure.
- Distinguish a fake browser-port test, an evaluator test, a process journey, and a real Chromium
  journey. Name the lane that proved each result and any untested platform.
- Turn the assessment's external exploratory probes into repository regression tests at the
  relevant implementation seams. Do not mark a fix complete from the old passing suites alone.
- One implementation task is one logical commit, with the AGENTS.md gates green. Process and
  browser journeys apply when their boundaries change; point `GHOSTLIGHT_BIN_DIR` at the actual
  fresh build under test. Live deployment uses the existing dev-loop only.
- Reconcile affected active contracts and trust claims with each completed fix. Historic records
  remain intact. A documentation correction must not imply an unfixed behavior is now enforced.
- Record implementation, tests, commit, deviations, and remaining proof in the ledger. A scope
  agreement, a drafted task, or a completed unit test alone is not a shipped capability.
