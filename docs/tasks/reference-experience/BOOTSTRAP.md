# BOOTSTRAP: the reference Ghostlight experience

This epic turns Ghostlight from visible infrastructure into a calm local capability: ordinary MCP
browser work should just work, safe recovery should happen without ceremony, and full control
should remain one deliberate interaction away. The epic is about the lived product, not release
packaging and not a larger feature inventory.

The seven stage prompts are outcome outlines. They do not prescribe files, types, or framework
machinery. Before executing a stage, inspect the live tree and every ADR that owns the affected
subsystem. Record a new ADR or marked amendment before changing an accepted decision.

## North star

> Ghostlight is absent when everything works, reassuring when observed, and unusually helpful when
> something breaks.

A normal user installs Ghostlight and uses browser tools from their chosen harness without first
learning the workbench. A user who wants control opens one calm At a glance surface. A power user
can continue into exact configuration, provenance, diagnostics, policy, audit, and CLI output.

Ghostlight understands canonical browser operations, authority, lifecycle, browser state, and
terminal effects. It does not infer the user's larger task or invent semantic consequences that
the requested operation does not structurally carry.

## Authority order

On conflict, higher wins. An unanticipated conflict is a STOP condition.

1. The live tree and tests for implementation facts.
2. The active request and owner decisions recorded in this epic.
3. `docs/1.0/INTENT.md`, `LANGUAGE.md`, `ARCHITECTURE.md`, and `ACCEPTANCE.md`.
4. Accepted ADRs. Supersede or amend; never silently contradict.
5. The current stage prompt.

The ledger is the authority for execution progress. This bootstrap is the authority for epic-wide
mandates.

## Product mandates

- **Progressive capability reveal.** Installation and ordinary use require no workbench ritual.
  At a glance comes next, controls and preferences after that, and technical depth last.
- **Sane defaults first.** The common operation succeeds without a question. Add a setting only
  when a reasonable user may reject behavior that changes attention or environment, such as
  opening a browser window or drawing on a page.
- **Safe self-healing.** Recover automatically only when the recovery is deterministic, bounded,
  idempotent, authority-neutral, and restricted to Ghostlight-owned state. Otherwise finish with
  the exact failed seam and the smallest useful next action.
- **No invented intent.** Ghostlight never turns a generic click, navigation, or write into a claim
  about booking, purchasing, sending, or another task-level meaning unless the canonical operation
  itself carries that meaning.
- **Human control is runtime truth.** Pause prevents the next browser effect and keeps the caller
  pending for as long as its live transport permits. Resume revalidates transient state. Stop is
  terminal and tells the controller: `The user asked to interrupt the process. Wait for further
  instructions.`
- **One quiet browser doorway.** A small Ghostlight affordance opens the native workbench directly
  on At a glance. It does not duplicate workbench controls in page content and is never required
  for safety or operation.
- **One calm front door.** At a glance answers what is ready, connected, working, paused, repaired,
  or in need of attention. Logs and subsystem archaeology are not the landing experience.
- **Truth over reassurance.** Already-dispatched effects settle as complete, partial, uncertain,
  or otherwise truthful. Pause, stop, timeout, and reconnect never fabricate a clean rollback.

## Architecture mandates

- Keep the DDD modular monolith. The orchestrator remains the product authority and sole mutation
  point for product semantics.
- Use the minimal number of meaningful moving parts. Add no process, service, daemon, generic
  recovery engine, event bus, actor system, workflow framework, registry, or parallel control path
  without a concrete invariant that the existing monolith cannot hold.
- Centralize complexity at the seam where the fact becomes knowable. Do not spread browser
  lifecycle recovery across call sites, pause checks across operations, or display-state inference
  across JavaScript views.
- Keep connectors stable and generic. They own protocol lifecycle, framing, correlation,
  cancellation forwarding, and relay behavior, never the experience decision.
- Keep the extension policy-free. It owns Chromium APIs, page-local drawing and interaction, and
  browser-specific durability. It does not decide authority, workspace behavior, or model-facing
  language.
- Preserve one executor, one workspace aggregate, one governance facade, one browser port, and one
  typed completion path for model-requested work.
- Preserve plural sessions, operations, browser instances, and harness connections. A pleasant
  one-session presentation must not create a singleton domain assumption.
- Keep every new preference a small closed domain choice with one owner and one projection. Do not
  build a general settings framework around two or three deliberate choices.
- A completed or interrupted operation's prose, next step, retry safety, and effect truth come from
  the orchestrator's typed outcome language, never from a UI or adapter literal.
- Preserve all supported MCP protocol eras. Experience work may not make a current harness work by
  breaking an older one.
- Never phone home, overwrite foreign configuration, expand authority through recovery, or make
  presentation success a precondition for browser work.

## Delivery discipline

1. Read the stage prompt, live code, current tests, and owning ADRs.
2. Add or amend the minimum decision record needed for changed behavior.
3. State what the change makes redundant before implementation. Remove duplication in the same
   stage where safe; otherwise record one named follow-up in the ledger.
4. Implement at the owning seam. If the same rule appears in more than one product layer, stop and
   find the missing owner.
5. Add evidence proportional to the user promise: typed/unit tests, process journeys, extension
   tests, and live desktop evaluation as applicable.
6. Run the repository gates from `AGENTS.md` before every commit.
7. Leave every commit coherent and green. If one stage cannot land coherently in one commit,
   record the smallest ordered substeps directly in the ledger; do not create another hierarchy of
   planning documents.
8. Update the ledger after every landed substep, deviation, blocker, and stage close.

## Stage sequence

| Stage | Prompt | Objective | Depends on |
| --- | --- | --- | --- |
| S1 | [Experience contract](S1-experience-contract.md) | Ratify the product boundary, states, defaults, and success measures | -- |
| S2 | [Automatic readiness](S2-automatic-readiness.md) | Make an admitted browser operation recover missing local readiness when safe | S1 |
| S3 | [Human runtime control](S3-human-runtime-control.md) | Give pause, resume, and stop one truthful domain contract | S1 |
| S4 | [At a glance](S4-at-a-glance.md) | Make calm stack truth and control the workbench front door | S2, S3 |
| S5 | [Browser affordance](S5-browser-affordance.md) | Add the small branded doorway from a controlled page to At a glance | S4 |
| S6 | [Progressive depth](S6-progressive-depth.md) | Make preferences, repair, diagnostics, and CLI one coherent deeper layer | S2-S5 |
| S7 | [Reference evaluation](S7-reference-evaluation.md) | Prove the combined experience with Linux users and adversarial journeys | S1-S6 |

Only one stage may be `IN PROGRESS`. Every completed prefix must leave the existing product usable
and the full tree green.

## Epic completion

The epic is complete only when all stage objectives have linked evidence in `LEDGER.md`, no
accepted ADR or active 1.0 contract contradicts the shipped experience, and the S7 evaluation shows
that ordinary users can use and recover Ghostlight without learning its internal topology.

Passing automated tests alone does not complete S7. Delight is an observed user outcome.
