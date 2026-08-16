# LEDGER: the reference Ghostlight experience

Durable progress for the epic defined by [BOOTSTRAP.md](BOOTSTRAP.md), with exact values in
[PINS.md](PINS.md). Update this file before starting a stage, after every material finding, after
every commit, and when closing or blocking a stage.

## RESUME HERE

- State: READY. No production behavior has changed through this epic.
- Current stage: S1, the experience contract.
- Next action: read the ADRs S1 names, audit the journeys it lists, write the decision record, and
  append the decided values to `PINS.md`.
- Blocking condition: none.
- Source baseline: `dev` at `2f24943fa125d952fce9e4f11086aada762e4cad`.
- Last green evidence: the baseline's recorded Windows current-source pass
  (`docs/testing/windows-current-source-pass-2026-08-15.md`) and CI run `31920645118`. No
  implementation gate is claimed for the authoring commits themselves.

## Provenance

This epic was first authored on 2026-08-15 in `83ca2ba9` as seven outcome-outline stages. It was
reworked on 2026-08-16 after a review against the live tree and the prior Linux research. The
reasons are recorded here so they are not re-argued:

1. The original stages cited no ADRs. This tree has 125 of them and a standing rule to read the
   owning ADR before touching a subsystem.
2. The original S7 required a recruited user cohort. That process was already rejected as a release
   gate in `docs/testing/greenfield-first-success.md` and in `docs/business/FOUNDER-TODO.md`. The
   evaluation now uses the Ubuntu GNOME Wayland lifecycle that ADR-0123 already made
   release-blocking, a Windows lane, and consented public feedback.
3. The original S5 required an authentic full-vector mascot. No such asset exists in the tree; every
   mascot file is raster and the only vector is `extension/icons/ghost-mark.svg`. The in-page
   affordance is deferred to `docs/design/in-page-affordance-deferred-2026-08.md`.
4. The original S2 made automatic browser launch the default recovery. On Linux that inherits the
   session-environment problem ADR-0082 exists for, plus keyring prompts, profile locks, and
   sandboxed browser packages. Recovery is now platform-honest and late in the sequence.
5. The review found four verified gaps that no stage covered, all in the path of a person moving
   between machines. They are now S2 and S3.
6. `docs/MEMORY.md` had recorded the human-stop directive as a durable fact. It does not exist in
   the tree. That entry was corrected to name it as this epic's intent.

The 2026-08-15 stage files remain in Git history. Do not take a path or excerpt from them as current.

## Stage table

| Stage | Status | Closing commit | Checkpoint | Notes |
| --- | --- | --- | --- | --- |
| S1 experience contract | READY | -- | Decisions and pins only | No behavior change |
| S2 the second machine | NOT STARTED | -- | A new computer explains itself | Extension only |
| S3 adaptive familiarity | NOT STARTED | -- | Local desktop language | Includes WSL |
| S4 terminal citizenship | NOT STARTED | -- | PATH, man, completions, `--json` | Depends on S3 |
| S5 human runtime control | NOT STARTED | -- | Pause, resume, stop | Semantic change |
| S6 At a glance | NOT STARTED | -- | One calm window | Depends on S4, S5 |
| S7 readiness recovery | NOT STARTED | -- | Safe repair, exact refusal | Platform-asymmetric |
| S8 evaluation | NOT STARTED | -- | Evidence on real desktops | Depends on S1-S7 |

Allowed values: `NOT STARTED`, `READY`, `IN PROGRESS`, `BLOCKED`, `COMPLETE`. At most one stage is
`IN PROGRESS`.

## Completion evidence matrix

A prose assertion is not evidence. Link a commit, test, fixture, ADR, or dated record.

| Area | Stage | Required evidence | Status | Evidence |
| --- | --- | --- | --- | --- |
| Vocabulary and measures | S1 | Accepted ADR; reconciled 1.0 contracts; appended pins | NOT STARTED | -- |
| Host-absent state | S2 | Distinguished state, both surfaces, offline route, tests | NOT STARTED | -- |
| Platform and desktop table | S3 | One owner, closed set, WSL case, consumer parity, tests | NOT STARTED | -- |
| Terminal citizenship | S4 | PATH ownership, man pages, completions, `--json`, doctor parity guard | NOT STARTED | -- |
| Runtime control | S5 | One state machine, effect truth, deadline interaction, plural scopes | NOT STARTED | -- |
| At a glance | S6 | All states, controls, keyboard, accessibility, redundant surface removed | NOT STARTED | -- |
| Readiness recovery | S7 | Per-platform posture, single flight, bounded waits, exact failures | NOT STARTED | -- |
| Evaluation | S8 | Ubuntu GNOME lifecycle, Windows lane, migration cases, dispositions | NOT STARTED | -- |

## Decision register

Open questions, owned by a stage. Not conclusions to assume early.

| Decision | Stage | State | Resolution |
| --- | --- | --- | --- |
| Does a hold keep the caller pending or keep refusing, and what happens at caller timeout | S1/S5 | OPEN | -- |
| How `Attention` and `StartSession` map onto pause, resume, and stop | S1/S5 | OPEN | -- |
| How a held operation interacts with the ADR-0113 deadline and quarantine | S1/S5 | OPEN | -- |
| Whether a held state survives workbench close, reconnect, and restart | S1/S5 | OPEN | -- |
| Owner and default of the browser-startup preference, and whether it joins registered policy settings | S1/S7 | OPEN | -- |
| Whether the per-user route owns `~/.local/bin/ghostlight` or only reports the path | S1/S4 | OPEN | -- |
| Whether At a glance replaces Monitor or becomes a new destination | S1/S6 | OPEN | -- |
| Acceptance thresholds for first use, recovery, and comprehension | S1/S8 | OPEN | -- |
| Whether 1.0 publishes before this epic lands | owner | PROVISIONAL: yes | -- |
| Whether the in-page affordance returns | owner | PROVISIONAL: deferred | -- |

## Gate and evaluation log

| Date | Stage | Commit | Automated gates | Live evidence | Result |
| --- | --- | --- | --- | --- | --- |
| -- | -- | -- | -- | -- | -- |

## Deviations and findings

Number every deviation. Record the owning seam, the disposition, and the evidence. Do not silently
widen a stage or work around a broken assumption.

None.

## Stage close checklist

- The objective is observable and linked to evidence.
- Every changed decision has an ADR or a marked amendment.
- The change sits at one owning seam inside the modular monolith.
- No redundant surface or duplicated rule remains without a named follow-up here.
- Every gate command in `PINS.md` passed, and the counts are in the gate log.
- `docs/STATUS.md` and the active 1.0 contracts match reality.
- `RESUME HERE`, the stage row, the evidence matrix, and this file's deviations are current.
