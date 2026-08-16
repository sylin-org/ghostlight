# BOOTSTRAP: the reference Ghostlight experience

Read this file, then [PINS.md](PINS.md), then your stage prompt, then the live tree. Assume you have
no memory of any prior session.

## What this epic is for

Ghostlight should feel like one product across every machine a person uses: the same words, the same
controls, and the same truth, shaped to the desktop they are on. Ordinary browser work should
succeed without opening the workbench. Recovery that is safe should happen without ceremony.
Recovery that is not safe should end in an exact statement of what failed and what to do next.

North star:

> Ghostlight is absent when everything works, reassuring when observed, and unusually helpful when
> something breaks.

The progression is the agent's returned sentence first, then `ghostlight` in a terminal, then the
workbench. A person who never opens the window should never be at a disadvantage.

## Who this is for

Five audiences use this product. A change that helps one and harms another is a decision, not a
detail, and belongs in the ledger.

| Audience | What they need |
| --- | --- |
| Windows developer with an MCP harness | Installer, tray, predictable behavior, an honest answer about WSL |
| Linux terminal-first developer | No daemon, XDG correctness, `--json`, provable uninstall |
| Someone moving from Windows to Linux | Both machines agreeing, and familiar shapes where the platform offers them |
| Privacy-driven user | Local audit, revocable authority, no inferred intent, no network |
| Team or organization | Policy, audit, clean lifecycle, no charm required |

macOS is deferred for lack of test hardware, not abandoned. Build platform-dependent behavior as a
table with a row per platform, never as a two-branch conditional, so a third platform is a row and
some evidence rather than a rewrite.

## Authority order

Higher wins. An unanticipated conflict is a STOP condition, not a judgment call.

1. The live tree and its tests, for what is true today.
2. [PINS.md](PINS.md), for exact values this epic fixes.
3. `docs/1.0/INTENT.md`, `LANGUAGE.md`, `ARCHITECTURE.md`, `ACCEPTANCE.md`.
4. Accepted ADRs. Supersede or amend them; never silently contradict one.
5. Your stage prompt.

`LEDGER.md` is the authority on progress. This file is the authority on epic-wide rules.

## Product rules

- **Progressive reveal.** Installation and ordinary use require no workbench ritual. The window,
  controls, preferences, and diagnostics appear in that order, and none of them gate browser work.
- **Adaptive familiarity.** Where a platform offers a familiar shape, use it. A tray where the shell
  has one, an Applications entry everywhere, a notification area on Windows. Never make any single
  one of them the only route to anything. The words follow the same rule: say what is true on the
  desktop the person is actually looking at.
- **Sane defaults.** The common operation succeeds without a question. A setting exists only when a
  reasonable person may reject behavior that changes their attention or environment.
- **Safe self-healing.** Recover automatically only when the recovery is deterministic, bounded,
  idempotent, authority-neutral, and confined to Ghostlight-owned state. Otherwise finish with the
  exact failed seam and the smallest useful next action. Applying this test per platform is expected
  to produce different answers on Windows and Linux. That is the test working.
- **No inferred intent.** A generic click, navigation, or write never becomes a claim about booking,
  buying, sending, or any other task-level meaning.
- **Truth over reassurance.** An already-dispatched effect settles as complete, partial, or
  uncertain. Pause, stop, timeout, and reconnect never fabricate a clean rollback.
- **One voice.** A completed or interrupted operation's sentence, its next step, and its
  measurements come from the orchestrator's typed outcome language. A surface renders that language;
  it does not author it.

## Architecture rules

- Keep the DDD modular monolith. The orchestrator is the sole mutation point for product semantics.
- Add no process, service, daemon, recovery engine, event bus, actor system, workflow framework,
  registry, or second control path without a concrete invariant the monolith cannot hold.
- Centralize each fact at the seam where it becomes knowable. Browser lifecycle recovery, pause
  checks, and display-state inference each have exactly one owner.
- Connectors stay generic: protocol lifecycle, framing, correlation, cancellation forwarding, relay.
  No experience decisions.
- The extension stays policy-free: Chromium APIs, page-local drawing, browser durability. No
  authority, workspace, or product-language decisions.
- Preserve one executor, one workspace aggregate, one governance facade, one browser port, and one
  completion path.
- Sessions, operations, browser instances, and harness connections stay plural. A single-session
  presentation must not create a singleton domain assumption.
- Every new preference is a small closed choice with one owner and one projection.
- Preserve every supported MCP protocol era. Making a current harness work by breaking an older one
  is a STOP condition.

## Stage sequence

One task, one commit, one green tree. Every prefix of this list leaves the product usable.

| Stage | Prompt | What a person gets | Depends on |
| --- | --- | --- | --- |
| S1 | [Experience contract](S1-experience-contract.md) | Nothing visible; the decisions and pins | -- |
| S2 | [The second machine](S2-second-machine.md) | A new computer says it is not set up, and shows the way | S1 |
| S3 | [Adaptive familiarity](S3-adaptive-familiarity.md) | Install and doctor speak the local desktop's language | S1 |
| S4 | [Terminal citizenship](S4-terminal-citizenship.md) | PATH, man pages, completions, `--json`, one explanation surface | S1, S3 |
| S5 | [Human runtime control](S5-human-runtime-control.md) | Pause, resume, and stop mean one truthful thing | S1 |
| S6 | [At a glance](S6-at-a-glance.md) | One window that answers whether this is working | S4, S5 |
| S7 | [Readiness recovery](S7-readiness-recovery.md) | Missing readiness is repaired where safe, named where not | S1, S3 |
| S8 | [Evaluation](S8-evaluation.md) | Evidence on real desktops, both platforms | S1-S7 |

At most one stage is `IN PROGRESS`.

## Per-stage procedure

1. Read this file, `PINS.md`, the stage prompt, the ADRs the prompt names, and the live code.
2. Check the prompt's STOP preconditions. If one holds, stop and write it in the ledger.
3. Record any new decision as an ADR or a marked amendment before implementing it.
4. Say what the change makes redundant before you add anything. Remove that duplication in the same
   stage when it is safe, or record one named follow-up in the ledger.
5. Implement at the owning seam. If the same rule needs to exist in two product layers, stop: the
   owner is missing.
6. Add evidence proportional to the promise: unit tests, journeys, extension tests, live checks.
7. Run every gate command in `PINS.md`.
8. Commit once, then update the ledger.

## Failure protocol

If a stage cannot complete: revert to the last green commit, mark the stage `BLOCKED` in the ledger
with the reason and the evidence, and stop. Do not improvise around a broken assumption, do not
widen the stage to make it fit, and do not skip ahead. A blocked stage with a clear reason is a good
outcome; a stage completed by guessing is not.

## Never do these

Each entry names its one sanctioned exception, or states that there is none.

- Never add telemetry, an update ping, an activation call, or any new outbound network behavior.
  ADR-0028 Decision 9 is normative and permanent, and public trust and legal documents depend on it.
  **No exception in this epic.**
- Never copy code from `reference/`. Interface and technique only. **No exception.**
- Never put policy, classification, or audit in the extension. **No exception.**
- Never weaken a claim in `docs/trust/` or `docs/legal/`. **No exception in this epic**; no stage
  here changes a public claim.
- Never edit shell startup files to fix PATH. **No exception**; S4 uses an owned XDG location.
- Never change a `docs/1.0/` contract. **Exception: S1 only**, and only to make it match reality.
- Never edit an existing pin in `PINS.md`. **Exception: S1 only**, and only to append decided values.
- Never change the bridge, MCP connector, or browser connector protocol surface. **Exception: S5**,
  and only if the runtime-control contract provably requires it, with an ADR.
- Never add an extension permission or manifest capability. **No exception in this epic.**
- Never read or modify anything under `local/` or `/private/`. **No exception.**
- Never push, publish, tag, release, or post anything outward-facing. Local commits are expected.
  **No exception.**

## Epic completion

Every stage objective has linked evidence in `LEDGER.md`, no accepted ADR or active 1.0 contract
contradicts the shipped behavior, and S8 shows that a person can install, use, and recover
Ghostlight on both platforms without learning its internal topology. Passing tests is necessary and
not sufficient.
