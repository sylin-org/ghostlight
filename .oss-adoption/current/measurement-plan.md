Status: complete
Run ID: 20260718T214944Z
Project: Ghostlight
Created: 2026-07-18T21:49:44Z

# Measurement plan

## Decisions this measurement should support

- Is the public install path self-sufficient?
- Which MCP clients and user jobs produce real first success?
- Does the message attract people who want visible authenticated-browser work rather than headless
  or cloud automation?
- Which channel produces useful users without exceeding solo-maintainer capacity?
- Which product, documentation, or trust defect should be repaired next?
- Is there enough independent use to justify broader publication or organization outreach?

## Baseline at 2026-07-18 UTC

| Signal | Baseline | Interpretation limit |
| --- | --- | --- |
| GitHub stars / forks / watchers | 0 / 0 / 0 | Discovery bookmarks only, not adoption |
| Public issues / Discussions | 0 / 0 | No public feedback history yet |
| Open pull requests | 7 | Development activity, not outside adoption |
| GitHub views, prior 14 days | 9 total / 5 unique | Rolling 14-day window; very small sample |
| GitHub visible referrers | Google 5 / 2 unique; github.com 2 / 2 unique | Search engines and GitHub reporting are incomplete |
| GitHub clones, prior 14 days | 6,019 / 445 unique | Anomalous next to 5 unique visitors; likely automation-heavy |
| npm downloads, 2026-06-18 through 2026-07-17 | 1,153 | Includes CI, repeat installs, crawlers, and failed first use |
| npm downloads, 2026-07-11 through 2026-07-17 | 461 | Does not identify people or activation |
| All GitHub release-asset downloads | 266 | Includes checksums, relays, and repeated/automated fetches |
| v0.6.0 release-asset downloads | 56 | Asset-level total, not unique installations |
| Latest release | v0.6.0 on 2026-07-15 | Distribution fact only |
| GitHub community profile | 75% | Issue and PR intake templates absent |
| Chrome Web Store | Under review | Broad-install prerequisite not met |

Data sources:

- GitHub repository, traffic, and release APIs read on 2026-07-18.
- https://api.npmjs.org/downloads/point/2026-06-18:2026-07-17/ghostlight
- https://api.npmjs.org/downloads/point/2026-07-11:2026-07-17/ghostlight
- https://docs.github.com/en/repositories/viewing-activity-and-data-for-your-repository/viewing-traffic-to-a-repository

## Funnel signals

| Stage | Signal | Source | Review window | Decision threshold | Privacy note |
| --- | --- | --- | --- | --- | --- |
| Discovery | Qualified repository visitors by referrer | GitHub traffic | 24h, 72h, 7d, 14d | Compare channel mix, not absolute fame | Aggregate platform data |
| Evaluation | README/install/decision-aid questions and link use | Public conversation plus site aggregate if already available | 72h and 7d | Repeated category confusion triggers copy repair | Add no new tracking |
| Trial | npm and current-release download change | npm/GitHub public counters | 24h to 14d | Useful only with first-success evidence | Counts include automation |
| First success | User completed install, doctor green, and one real task | Voluntary cohort or public report | Per user | Most cohort users finish without private correction | Record no browser content |
| Return | User reports another task or uses a later release | Voluntary follow-up, issues, Discussions | 14d to 30d | Several independent repeat uses justify next channel | Do not fingerprint users |
| Advocacy | Independent workflow, post, recommendation, or integration | Public links with author consent | 14d to 60d | Accurate independent explanation is stronger than star count | Credit and do not scrape profiles |
| Contribution | Reproducible issue, docs fix, code PR, review help | GitHub | 14d to 90d | Quality and repeat participation matter | Public contribution data only |
| Sustainability | Support hours, security load, release burden | Owner time ledger | Weekly | Pause broadening above roughly 8 focused hours/week | Private aggregate only |

## Proof-cohort record

For each participant, record only what is needed:

```text
Anonymous participant id:
Date and release:
OS, browser, and MCP client:
Extension path: store or informed manual preview
Time to doctor green:
Time to first useful task:
Founder interventions:
First hesitation:
User's own product description:
Next task they wanted:
Outcome: success / partial / blocked
Documentation or product action:
Quote permission: none / anonymous / attributed
```

Never record pages visited, browser content, account identity, or form data merely to measure
adoption.

## Campaign ledger

```text
Event and UTC time:
Channel/community and public URL:
Audience hypothesis:
Problem angle and proof artifact:
Specific ask:
Owner effort:

Baseline:
24 hours:
72 hours:
7 days:
14 days:

Qualified questions:
Install and first-success failures:
Category misunderstandings:
Independent workflows or recommendations:
Support and moderation hours:
Product or documentation changes:
Decision: continue / adapt / pause / stop
```

## Continue, adapt, pause, and stop rules

Continue when qualified users reach first success, describe the product accurately, and support
load remains manageable.

Adapt when discovery rises but trials do not, trials rise but first success does not, or users
consistently expect headless/cloud behavior. Fix the relevant layer before adding another channel.

Pause when the same install problem appears three times, public version/platform truth diverges,
the store or release path becomes uncertain, the owner cannot participate, or support exceeds the
weekly budget.

Stop a channel when its rules conflict with the proposed post, the audience cannot use the product,
the response becomes harmful or spam-like, or it produces attention without relevant evaluation.

## Review cadence and owner

- Before each event: snapshot the 14-day GitHub window, npm periods, release counts, store status,
  public-surface versions, and owner support capacity.
- At 24 and 72 hours: triage blockers, misunderstandings, and security reports.
- At 7 days: decide whether to adapt the message or proceed to another channel.
- At 14 days: close the event ledger and compare first success, return, advocacy, and support cost.
- Monthly during the 90-day focus period: review ecosystem listings, claims, maintainer load, and
  whether broader organizational work has earned priority.

The owner maintains the ledger. No telemetry, activation, fingerprinting, or cross-site identity
linking may be added for this plan.
