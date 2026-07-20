# Ghostlight public-awareness plan, 2026-07

**Status:** draft for owner discussion; no external publication authorized

**Date:** 2026-07-18

**Input:** [open-source publication-path research](../research/20-open-source-publication-paths-2026-07.md)

## Outcome

Introduce Ghostlight to the people who already want an MCP client to use their real browser, help
them reach a useful result quickly, and turn their evidence into trustworthy word of mouth.

This is not a star campaign. The first success condition is a stranger installing Ghostlight,
using it in a supported MCP client without founder help, understanding why the browser is visibly
controlled, and describing the product accurately afterward.

## Canonical story

The public story should stay practitioner-first:

> Ghostlight lets the MCP client you already use work in your real logged-in Chromium browser.
> The work stays local and visible. You see what it reads and changes. Organizations can add
> identity-bound policy and audit when they need it.

This ordering is deliberate:

1. what the practitioner can now do;
2. why the ordinary logged-in browser matters;
3. how the human remains oriented;
4. local/no-account truth;
5. organizational governance as a later, optional need.

Do not open with licensing, enterprise procurement, "open core," or a comparison to Claude. Do not
call Ghostlight a headless browser, a cloud browser, an autonomous employee, or a security sandbox.

## Launch predicates

### Required for the broad launch

- The Chrome Web Store listing is public and the current release installs successfully from it.
- A clean Windows and Linux user can complete the greenfield install instructions.
- The store path, manual path, npm package, MCP Registry entry, website, and README agree on the
  current version and supported browsers.
- The hero GIF loads from `main` and the live demo remains available.
- The first task works from at least two representative MCP clients without project-owner help.
- `ghostlight doctor` identifies the common broken states in plain language.
- The README and site state the no-account, no-telemetry, local boundary accurately.
- Issue templates or Discussions give new users an obvious place for install friction, bugs, and
  examples.
- The owner can remain available for questions during the anchor launch window.

### Allowed before the broad launch

A small advanced-user proof round may use the manual unpacked extension. It must be described as a
pre-release installation path, sent only to people who knowingly accept the friction, and used to
repair onboarding. It is not the general launch.

## Audiences in order

1. **MCP and coding-agent practitioners.** People using Codex, Claude Code, Cline, Cursor, VS Code,
   Zed, OpenCode, Windsurf, or another local MCP client who need an authenticated browser.
2. **Local-first and browser-automation developers.** People who understand why an ordinary visible
   profile is distinct from Playwright, a hosted browser, or a new isolated profile.
3. **Non-developer MCP users.** People using agent clients for documentation-heavy, authenticated
   workflows who value visible action feedback and no separate Ghostlight account.
4. **Security, platform, and governance practitioners.** People evaluating identity, capability
   control, audit, deployment, and organizational fit.

The fourth audience matters strategically, but it should not define the first screen shown to the
first three.

## Proof stack

Every public explanation should draw from one shared proof stack:

| Proof | Job |
|---|---|
| README hero GIF | Show visible reading, filling, selection, and completion in one short story |
| Live brief demo | Let a reviewer reproduce the hero through the real Ghostlight stack |
| Install page | Convert interest into a correct service plus extension installation |
| First-task recipe | Produce a meaningful result immediately in the user's chosen MCP client |
| Architecture diagram | Explain local process and trust boundaries without marketing shorthand |
| Trust Center | Support exact security, privacy, release, and governance claims |
| Decision aid | Help organizations compare deployment economics and operating assumptions |
| Releases and attestations | Show active maintenance and verifiable artifacts |

The hero proves delight. The install proves accessibility. The architecture and Trust Center prove
restraint. None should try to do all three jobs.

## Publication sequence

### Phase 0: establish the baseline

Before any post:

- archive the current GitHub 14-day traffic, referrers, popular paths, stars, watchers, forks,
  release downloads, and npm downloads;
- record the store-listing status and current release version;
- create a simple campaign ledger with event time, channel, canonical URL, message angle, and the
  same public outcome counters at 24 hours, 72 hours, 7 days, and 14 days;
- manually test every link in a logged-out browser.

Snapshot on 2026-07-18:

| Signal | Baseline |
|---|---:|
| GitHub stars | 0 |
| Forks | 0 |
| Watchers/subscribers | 0 |
| GitHub views, previous 14 days | 9 total / 5 unique |
| Visible referrers | Google 5 / 2 unique; github.com 2 / 2 unique |
| npm downloads, 2026-06-18 through 2026-07-17 | 1,153 |
| All GitHub release-asset downloads | 266 |
| v0.6.0 release-asset downloads | 56 |

The 6,019 clone events and 445 unique cloners in the same 14-day GitHub window are anomalous next
to five unique page visitors and are likely dominated by automation. Do not use them as a human
adoption baseline.

### Phase 1: targeted proof users

Goal: find conversion defects, not reach.

- Invite five to ten practitioners across at least three supported clients.
- Give each only the public install page and one first-task prompt.
- Observe whether they need private help; if they do, repair the public path.
- Ask three focused questions: where did you hesitate, what did you think Ghostlight was, and what
  would you try next?
- Capture public quotes only with explicit permission. Anonymous findings can update docs without
  becoming testimonials.

Exit when most testers reach first useful browser work unaided and describe the local/visible
boundary correctly.

### Phase 2: ecosystem seeding

Goal: earn the first relevant users and conversations.

Use the official MCP Registry and npm package as the canonical ecosystem entries. Then select only
directories that can represent a local stdio server honestly. Review every resulting listing for
namespace, version, tools, install command, local execution, and canonical URL.

Participate in MCP-client communities with a client-specific proof, not a generic announcement.
For example: "Here is Ghostlight using the Chromium session already open beside Codex" is more
useful than "new MCP server launched." Ask for one class of feedback per conversation.

Do not post in unrelated issue trackers. A project discussion, showcase area, integration catalog,
or maintainer-approved community channel is the correct surface.

### Phase 3: anchor launch

Preferred anchor: **Show HN**, after the store installation is live.

Why it fits:

- Ghostlight is non-trivial, runnable, local developer software.
- No Ghostlight signup or email is required.
- The architecture invites substantive technical discussion.
- The owner can explain personal motivation and exact tradeoffs.

Candidate title frame, to finalize only after the install is verified:

> Show HN: Ghostlight - let any MCP client use your real logged-in browser

The primary link should go to the repository if the README remains the fastest complete proof. The
owner's opening comment should explain why the project exists, the local visible architecture, the
headless/cloud non-goal, the free core and separate organization boundary in concrete terms, and
the most useful criticism being sought.

Do not ask anyone to upvote or comment. Be present for the full active window. Answer hard
questions directly and turn recurring confusion into same-day documentation fixes when safe.

Fallback anchor: a substantial technical article submitted as a regular Hacker News story if the
project is not ready for frictionless Show HN trial. The article must stand on its own and explain
the design problem, not function as a disguised landing page.

### Phase 4: audience-specific publication

Space these over several days. Each should add a reason to read even if the person has already seen
the repository.

| Channel | Native artifact | Ghostlight angle | Timing |
|---|---|---|---|
| Bluesky | Hero GIF plus concise post/thread; alt text | Visible local browser control from any MCP client | Anchor day or next day |
| LinkedIn | Founder story plus hero | Why a real user context and no separate sign-in changed the experience | Day 2-3 |
| Relevant Reddit community | Text-first post with disclosure and focused question | The exact community problem; technical detail in the post | Only after current rules and account history qualify |
| YouTube | Short install-to-first-task walkthrough | Prove the full greenfield journey and visible feedback | Week 1; name the tested release |
| DEV or project article | Durable technical essay | Why authenticated user-context automation is not a headless-browser problem | Week 1 |
| Security/governance circles | Architecture and Trust Center brief | Agency is local; policy describes allowed intent and records action | After practitioner proof exists |
| Product Hunt | Polished product page and visual proof | Local, visible browser agency with a delightful human feedback layer | Later, if broad-product reach is still useful |

Bluesky copy should include the words people and custom feeds can actually match: MCP, local
browser, Chromium, Codex, Claude Code, Cline, or browser automation as appropriate. Use at most a
few accurate hashtags. Reply to people as a person; do not rely on the trending bot.

For Reddit, inspect the exact subreddit rules immediately before drafting. r/LocalLLaMA's current
self-promotion enforcement, for example, looks at community karma and the share of the account's
activity devoted to its own work. If the owner is not already a participant, skip that channel
rather than manufacturing account history.

### Phase 5: earned amplification

After real use exists, prepare a short factual curator brief:

- one-sentence product truth;
- the ten-to-twelve-second proof;
- supported clients and platforms;
- one install link;
- one architecture link;
- the exact free/separately licensed boundary;
- three explicit non-claims;
- author availability for questions.

Send it individually only to curators or newsletters that demonstrably cover open-source agent
tools, browser automation, Rust tools, or local-first software. Do not buy stars, roundup placement
disguised as editorial, or mass-email scraped lists.

Monitor third-party descriptions. Correct errors such as "headless," "cloud hosted," "fully
autonomous," "Chrome only forever," or "enterprise-only" quickly and politely with a canonical
source.

### Phase 6: maintain the relationship

- Publish substantive releases with concrete user outcomes, not every patch as a launch.
- Turn common support questions into install and doctor improvements.
- Share user-authored workflows with permission.
- Thank contributors and make small contribution opportunities visible.
- Revisit the channel mix every 14 days based on qualified visits and use evidence.
- Preserve a stable canonical explanation so directory and curator copies can be audited.

## Measurement

### Primary outcomes

- clean installs completed without direct help;
- first useful tasks completed in named MCP clients;
- unique repository visitors from relevant referrers;
- current-release asset and npm download change over baseline;
- specific, reproducible issues and documentation feedback;
- users describing Ghostlight accurately in their own words;
- repeat users, outside examples, integrations, and contributors.

### Secondary outcomes

- stars, forks, watchers, post saves, reposts, and newsletter mentions;
- appearance in GitHub Trending or automatic social feeds;
- directory coverage.

### Failure and stop signals

Pause broader publication if:

- more than a small minority of interested users cannot finish installation;
- the same support problem appears three times without a documented recovery path;
- third-party descriptions consistently mistake the product category;
- traffic grows while current-release downloads and first-use reports do not;
- store, registry, package, site, and repository versions disagree;
- a security or privacy claim cannot be defended from the current tree;
- the owner cannot remain present to answer the anchor launch discussion.

## Campaign ledger template

```text
Event:
UTC date/time:
Channel and community:
Public URL:
Audience:
Problem angle:
Proof artifact:
Specific ask:

Baseline:
24 hours:
72 hours:
7 days:
14 days:

Questions received:
Install failures:
Message misunderstandings:
Documentation or product changes:
Organic follow-on links:
Decision: continue / adapt / pause
```

Do not add tracking code, product telemetry, activation, or phone-home behavior for this ledger.
Use public platform counters, GitHub's short-lived traffic report, package/release counters, and
voluntary conversations.

## Preparation work that can happen before publication

- Re-run the complete clean-install path when the store listing clears.
- Draft the first-task prompt for each of the top three clients.
- Prepare a logged-out link and social-preview review.
- Draft, but do not post, the Show HN title and founder comment.
- Draft native Bluesky, LinkedIn, and one relevant Reddit version from the same facts.
- Create the campaign ledger and baseline snapshot procedure.
- Identify five to ten advanced proof users and the exact feedback question for each.
- Select a very small set of MCP directories and define an owner/review date for every listing.
- Prepare a curator fact sheet with explicit non-claims.

## Decisions still owed from the owner

Before external publication, decide:

1. whether to run a manual-extension proof cohort before the Chrome Web Store clears;
2. which personal account will be the durable founder voice on each social channel;
3. whether GitHub Discussions or Issues is the preferred public first-use feedback surface;
4. whether Product Hunt is worth the later attention cost;
5. which date can be protected for founder presence during the anchor launch.
