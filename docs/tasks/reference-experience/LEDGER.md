# LEDGER: the reference Ghostlight experience

Durable progress for the epic defined by [BOOTSTRAP.md](BOOTSTRAP.md). Update this file before
starting a stage, after every material finding or deviation, after every commit, and when closing
or blocking a stage.

## RESUME HERE

- State: READY; planning only. No production behavior has changed through this epic.
- Current stage: S1 -- experience contract.
- Next action: audit the live user journeys and owning ADRs, then draft the minimum decision record
  and measurable acceptance contract described by `S1-experience-contract.md`.
- Blocking condition: none.
- Source baseline when authored: `dev` at `72402a7d1ed281436877546c0ea797736f25cc3a`.
- Last green evidence: the source baseline's local full gate and successful GitHub CI run
  `31922105213`; no implementation gate is claimed for this docs-only epic authoring change.

## Stage table

| Stage | Status | Closing commit(s) | User-visible checkpoint | Notes |
| --- | --- | --- | --- | --- |
| S1 experience contract | READY | -- | Decisions and measures only | No production behavior change |
| S2 automatic readiness | NOT STARTED | -- | Missing browser readiness recovers safely | Depends on S1 |
| S3 human runtime control | NOT STARTED | -- | Pause, resume, and stop share one truth | Depends on S1 |
| S4 At a glance | NOT STARTED | -- | Workbench opens on calm stack truth | Depends on S2-S3 |
| S5 browser affordance | NOT STARTED | -- | Branded page doorway opens At a glance | Depends on S4 |
| S6 progressive depth | NOT STARTED | -- | Controls and diagnostics stay optional | Depends on S2-S5 |
| S7 reference evaluation | NOT STARTED | -- | Linux users prove the complete journey | Depends on S1-S6 |

Allowed status values are `NOT STARTED`, `READY`, `IN PROGRESS`, `BLOCKED`, and `COMPLETE`. At most
one stage may be `IN PROGRESS`.

## Completion evidence matrix

A prose assertion is not evidence. Link each completed row to a commit, test, fixture, ADR, dated
live record, or consented content-free evaluation note.

| Area | Owning stage | Required evidence | Status | Evidence |
| --- | --- | --- | --- | --- |
| Product boundary and vocabulary | S1 | Accepted decision; reconciled active contracts; closed states and preferences | NOT STARTED | -- |
| User success measures | S1 | Measurable first-use, recovery, control, comprehension, and accessibility criteria | NOT STARTED | -- |
| Browser readiness recovery | S2 | Default and manual modes; single-flight launch; bounded adapter wait; exact failures | NOT STARTED | -- |
| Recovery safety | S2 | Cancellation, ambiguity, foreign-state, authority-neutrality, and no-replay evidence | NOT STARTED | -- |
| Pause and resume | S3 | Safe-boundary hold; pending caller; revalidation; reconnect and timeout behavior | NOT STARTED | -- |
| Stop outcome | S3 | Exact directive; typed terminal state; effect truth; no automatic retry | NOT STARTED | -- |
| At a glance | S4 | All states, plural sessions, controls, keyboard, accessibility, and redundant-UI removal | NOT STARTED | -- |
| Browser affordance | S5 | Approved asset provenance; three modes; native activation; hostile-page and failure tests | NOT STARTED | -- |
| Progressive depth | S6 | One-owner settings; GUI/CLI parity; ownership-safe repair; discoverable diagnostics | NOT STARTED | -- |
| Reference evaluation | S7 | Linux cohort observations, adversarial journeys, before/after findings, final disposition | NOT STARTED | -- |

## Decision register

These are questions for the named stage, not implementation conclusions to assume early.

| Decision | Owning stage | State | Resolution / ADR |
| --- | --- | --- | --- |
| Exact user-visible state vocabulary and aggregate precedence | S1 | OPEN | -- |
| Closed names and defaults for browser startup and page presence | S1 | OPEN | -- |
| Scope and persistence of pause across plural sessions and restart | S1/S3 | OPEN | -- |
| Launcher, tray, notification, idle-lifetime, and window-close behavior | S1 | OPEN | -- |
| Exact conservative browser-selection order | S2 | OPEN | -- |
| Client-timeout behavior while a response is held | S3 | OPEN | -- |
| At a glance information hierarchy and redundant current surfaces | S4 | OPEN | -- |
| Authentic full-vector mascot source and exact affordance sizes | S5 | OPEN | -- |
| Final Linux cohort and measurable thresholds | S1/S7 | OPEN | -- |

## Gate and evaluation log

| Date | Stage | Commit/tree | Automated gates | Live/user evidence | Result and notes |
| --- | --- | --- | --- | --- | --- |
| -- | -- | -- | -- | -- | -- |

## Deviations and findings

Number every deviation or material finding. Record the owning seam, disposition, and evidence. Do
not silently expand a stage or patch around a broken assumption.

None.

## Stage close checklist

- The stage objective is observable and linked to evidence.
- Every changed decision has an ADR or marked amendment.
- The implementation stays inside the DDD modular monolith and at the owning seam.
- No redundant surface or rule remains without one named ledger follow-up.
- All repository gates pass and every commit is green.
- `docs/STATUS.md` and active 1.0 contracts match reality.
- `RESUME HERE`, the stage row, evidence matrix, gate log, and deviations are current.
