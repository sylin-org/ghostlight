# Ghostlight 0.8 reception loop

This is a manual record for learning from release 0.8. It is not analytics and does not create a
new collection path. Fill only dated aggregates, public observations, owner-visible dashboard
totals, support outcomes, and voluntary conversations. Never record credentials or private page
content. Name or quote a person only with explicit permission.

## Checkpoints

| Checkpoint | Due | Status | Recorded by |
| --- | --- | --- | --- |
| Release | 2026-08-07 06:00:52 UTC | recorded 2026-08-07 06:37 UTC | Codex |
| 7 days | 2026-08-14 06:00:52 UTC | pending | - |
| 30 days | 2026-09-06 06:00:52 UTC | pending | - |

At release time, replace the relative due dates with exact UTC dates and times. A missed checkpoint
stays marked missed; do not reconstruct precise historical counters from a later total.

### Snapshot: release

- Observed at: `2026-08-07 06:37 UTC`
- Public product commit: `95468758ab56b38da8b5ea5b717d51642c8cd56d`
- Release tag commit: `993135b048b60622157266b53b21f1719c9df4b3`
- Website commit: `0c801c61f9373fd634bbeeae9438f756d62f30e9`
- Public service: `0.8.0`
- Public Chrome adapter: `0.8.0`
- Surface check: `PASS`; local and live `scripts/check-public-surfaces.ps1 -Online` completed on
  2026-08-07 after website propagation

#### Directional counters

| Signal | Value | Window or denominator | Limits |
| --- | --- | --- | --- |
| npm downloads | 0 | npm point API, 2026-08-07 only, observed 06:37 UTC | Partial publication day; may later include CI, mirrors, retries, and automation |
| GitHub release asset downloads | 28 | 38 assets at tag v0.8.0, observed 06:37 UTC | Release automation and launcher verification account for some or all downloads |
| GitHub stars/forks/open issues | 0 / 0 / 13 | Point in time at 06:37 UTC | Public counters; open issues can include project work and are not satisfaction evidence |
| GitHub views/clones/referrers | not captured | Owner traffic window | No inference |
| Chrome users/ratings/reviews | 2 / no rating / no reviews observed | Public listing at release | Coarse store counters; not completed workflows |
| Glama favorite and grades | 1 favorite; A license; A quality; A maintenance; B `computer` | Point in time after explicit sync | Directory signal and scoring, not adoption |

#### Distribution and cache state

| Surface | Current version/copy | Cache current? | Next check |
| --- | --- | --- | --- |
| GitHub release | v0.8.0, 38 assets | yes | 7-day checkpoint |
| npm | 0.8.0 at `latest`; launcher smoke reached `doctor` | yes | 7-day checkpoint |
| Official MCP Registry | `org.sylin/ghostlight` 0.8.0 | yes | 7-day checkpoint |
| Chrome Web Store | adapter 0.8.0; public CRX matched submitted files | yes | 7-day checkpoint |
| sylin.org | 0.8 fallbacks at website commit `0c801c6` | yes | 7-day checkpoint |
| Scoop | direct manifest 0.8.0; central Extras popularity gate unmet | direct yes; central ineligible | Recheck only after the stated threshold is met |
| Winget | PR #413601 open; CLA green | pending Microsoft review | Check PR state at 7 days |
| Glama | synced to repository commit `9546875` | yes | 7-day checkpoint |
| mcpservers.org | refresh requested | pending directory refresh | Recheck after 48-72 hours |
| Search results | not rechecked after final propagation | unknown | Recheck after 48-72 hours |
| GitHub Discussion | existing welcome thread carries the 0.8 release and feedback prompt | yes | Review voluntary replies at 7 days |

#### Voluntary use and support evidence

No independent user report or permitted quote was available at the release checkpoint.

#### First-use failures

No independent first-use failure was reported at the release checkpoint.

#### Interpretation

- Human evidence available: none yet.
- Distribution evidence available: all eligible core release channels are live; community PRs and
  directory caches have the external states recorded above.
- Directional aggregate evidence available: only bounded public counters at release time.
- Evidence still unavailable: retention, completed-workflow count, satisfaction, and attributable
  human use.
- Product or documentation follow-up: none from reception evidence. Anthropic licensing,
  mcpservers.org refresh, Winget review, and awesome-mcp-servers review are distribution follow-ups.

## Snapshot template

Copy this section for each checkpoint.

### Snapshot: [release | 7 days | 30 days]

- Observed at: `[YYYY-MM-DD HH:MM UTC]`
- Public product commit: `[full hash]`
- Website commit: `[full hash]`
- Public service: `[version]`
- Public Chrome adapter: `[version]`
- Surface check: `[pass/fail plus command and timestamp]`

#### Directional counters

| Signal | Value | Window or denominator | Limits |
| --- | --- | --- | --- |
| npm downloads | - | Exact API start/end dates | May include CI, mirrors, retries, and automation; not people |
| GitHub release asset downloads | - | Release tag and asset count | Mixes binaries, archives, checksums, and automation |
| GitHub stars/forks/open issues | - | Point in time | Small public counters; no claim about satisfaction |
| GitHub views/clones/referrers | - | Exact owner-only traffic window | May include bots, CI, mirrors, and unclassified traffic |
| Chrome users/ratings/reviews | - | Point in time | Coarse store counters; not completed workflows |
| Glama favorite and grades | - | Point in time | Directory signal and scoring, not adoption |

#### Distribution and cache state

| Surface | Current version/copy | Cache current? | Next check |
| --- | --- | --- | --- |
| GitHub release | - | - | - |
| npm | - | - | - |
| Official MCP Registry | - | - | - |
| Chrome Web Store | - | - | - |
| sylin.org | - | - | - |
| Winget | - | - | - |
| Glama | - | - | - |
| mcpservers.org | - | - | - |
| Search results | - | - | - |

#### Voluntary use and support evidence

Keep one row per independent report or conversation. Use a neutral label unless the person has
approved attribution.

| Reference | Client | Task | Outcome | Hesitation | Delight | Recovery unclear | Permission to quote/name |
| --- | --- | --- | --- | --- | --- | --- | --- |
| - | - | - | - | - | - | - | no |

#### First-use failures

Normalize a failure signature by observable symptom and next action, not by the person's wording.

| Signature | Reports this checkpoint | Running total | Resolved? | Follow-up owner |
| --- | --- | --- | --- | --- | --- |
| - | - | - | - | - |

Three independent reports of the same first-use failure stop broader outreach. Open a separate
issue, fix documentation, doctor, or product behavior at its owning layer, verify the recovery,
then decide whether outreach can resume. Do not lower the threshold by merging unrelated symptoms
or raise it by splitting the same symptom into wording variants.

#### Interpretation

- Human evidence available: `[what people voluntarily reported]`
- Distribution evidence available: `[where Ghostlight is listed or linked]`
- Directional aggregate evidence available: `[counters with caveats]`
- Evidence still unavailable: `[for example retention or completed-workflow count]`
- Product or documentation follow-up: `[separate issue/ADR link or none]`

## GitHub Discussion draft

Title:

> What did you try first with Ghostlight 0.8?

Body:

> If you tried Ghostlight 0.8, I would like to learn where the first experience worked and where
> it slowed you down.
>
> - Which MCP client did you use?
> - What task were you trying to complete?
> - Where did you hesitate or need to take over?
> - What felt especially useful or satisfying?
> - Which recovery step was unclear?
>
> Please do not post credentials, private page content, or account details. A short description is
> enough. If you are comfortable with a follow-up question, say so. I will not quote or identify
> you outside this thread without asking first.

Publication amendment: the owner authorized the 0.8 publication sweep on 2026-08-07. Rather than
create a duplicate thread, the release and feedback prompt was added to the existing welcome
Discussion at `https://github.com/sylin-org/ghostlight/discussions/77#discussioncomment-17929817`.
The project-authored prompt is distribution evidence. Replies become reception evidence only
within the limits of what each person voluntarily shares.

## Collection boundary

This loop does not authorize telemetry, analytics, tracking parameters, automatic review prompts,
session uploads, or a vendor-bound reporting path. Ghostlight's runtime remains local and does not
phone home. Evidence gaps remain gaps unless a person chooses to report an experience or a public
surface exposes a bounded aggregate.
