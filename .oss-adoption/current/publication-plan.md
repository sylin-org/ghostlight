Status: complete
Run ID: 20260718T214944Z
Project: Ghostlight
Created: 2026-07-18T21:49:44Z
External publication authorized: no

# Publication and adoption plan

## Strategy and sequencing rationale

Ghostlight should launch as a useful practitioner tool whose responsibility model becomes visible
through use. The message order is: existing authenticated browser, any MCP client, visible work,
local/no account, and optional organization governance. Licensing and procurement belong later.

The plan separates proof from reach. A small cohort can tolerate the manual extension and reveal
onboarding defects. Broad attention waits for store acceptance, reconciled public truth, and a
clean install. One founder-present anchor then supplies evidence for a small number of native
follow-ons. No synchronized blast is needed.

## Phase 0: repair readiness blockers

1. Update the public Ghostlight page from v0.5.7 to v0.6.0 and reconcile the Linux/macOS statement
   with the canonical repository status.
2. Add an automated or release-time drift check covering site version, platform status, npm,
   `server.json`, README, latest release, and store copy.
3. Complete Chrome Web Store review, install the accepted package from a clean Chromium profile,
   and verify that its ID and native messaging path work.
4. Add minimal issue and PR templates and select Discussions or Issues as the visible first-use
   feedback lane.
5. Add a custom social preview and verify the README hero, install page, decision aid, demo, Trust
   Center, release, and privacy pages while logged out.

Exit: every public surface tells the same current story and a clean store install reaches doctor
green plus the first read-only task.

Implementation status: the canonical status, drift check, website fallback, community templates,
decision-aid entry link, social-preview artifact, acceptance recipe, and OSPS draft are complete in
the working tree. Deployment, GitHub settings upload, store acceptance, and cohort evidence remain
closed external gates.

## Phase 1: validate with a small cohort

- Recruit five to ten informed users across at least three MCP clients and Windows/Linux.
- Give them only the public install page and one read-only first task.
- If store review is still open, explicitly call the extension path pre-release and include only
  people who accept Developer mode and unpacked loading.
- Record time to first success, interventions, exact confusion, doctor usefulness, and the user's
  own description of Ghostlight.
- Turn every repeated problem into a public fix. Do not turn private coaching into a successful
  conversion metric.

Exit: most users finish without founder correction; no blocker repeats unresolved; users describe
the local, visible, authenticated-browser boundary accurately.

## Phase 2: seed ecosystem surfaces

- Recheck npm and official MCP Registry metadata after the store-backed release.
- Publish three client-specific first-task pages or Discussions: one for Codex, one for a VS Code
  client such as Cline, and one for OpenCode or another independent client.
- Review a very small number of maintained MCP directories. Add only those that can represent a
  local stdio server, canonical URL, license split, and current version accurately.
- Prepare an OpenSSF assessment. Publish it only when answers have named evidence and an owner for
  maintenance.
- Use RAWX mappings as technical ecosystem material, separate from the product launch pitch.

Exit: high-intent directories lead to one canonical install path and at least two independent
clients have a reproducible workflow.

## Phase 3: anchor launch

Preferred anchor: Show HN.

Candidate title:

> Show HN: Ghostlight - let any MCP client use your real logged-in browser

Primary link: GitHub README, assuming it remains the shortest complete proof.

Founder comment outline:

1. the personal problem that led to Ghostlight;
2. why an existing visible profile is a different job from headless/test automation;
3. the local architecture and managed-tab boundary;
4. how compact model tools and human signage work together;
5. the free core and optional organization governance boundary;
6. explicit non-goals and pre-1.0 status;
7. one specific request for install, client-compatibility, or trust-boundary feedback.

The owner should protect the active discussion window. Do not ask anyone to vote, comment, or
coordinate attention. Answer hard questions directly. Safe documentation corrections can land
quickly; risky code changes stay in the normal review path.

Exit: discussion slows, recurring questions are recorded, and any material misunderstanding has a
canonical correction.

## Phase 4: adapt for high-fit communities

Space adaptations over several days and make each useful on its own:

- Bluesky: hero GIF with alt text, one-sentence problem, local/no-account truth, and a short thread
  explaining fit and non-fit.
- Client communities: exact client configuration, first task, and observed result. Ask one focused
  compatibility question.
- Technical article: "Why authenticated user-context browser automation is not a headless-browser
  problem," including the architecture and risks.
- Local-first/Rust audience: local process design, restart recovery, no phone home, and native
  service tradeoffs.
- Security/governance audience: RAWX, actual-host binding, audit, residual in-domain prompt
  injection, and the public Trust Center.

Do not reuse identical copy. Do not enter a subreddit or community solely to post the project.

## Phase 5: enable earned discovery

After independent use exists, prepare individual curator briefs for writers or maintainers who
cover MCP tools, local-first software, browser automation, Rust tooling, or agent governance. Give
them the project truth, proof, install path, current maturity, and non-claims. Never ask for or imply
guaranteed coverage. Correct third-party category errors politely with canonical sources.

Product Hunt is a later option only if the user-delight story has independent evidence and the
owner wants a broader non-developer audience enough to staff the full launch day.

## Phase 6: maintain and learn

- Announce substantive user outcomes, not every patch.
- Convert repeated install and doctor questions into owned documentation.
- Share permissioned user workflows and credit contributors.
- Snapshot outcomes at 24 hours, 72 hours, 7 days, and 14 days.
- Recheck old directory and curator descriptions for version drift.
- Review support hours and explicitly pause when attention exceeds capacity.

## Activity contract

| Activity | Purpose | Audience | Artifact | Owner | Prerequisite | Success signal | Stop condition | Follow-up |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Website truth repair | Restore one canonical story | All evaluators | v0.6.0 page and platform wording | Owner | Canonical status chosen | Site and repo agree | Any claim lacks current proof | Add drift check |
| Store acceptance test | Remove major install friction | Chromium users | Accepted listing and clean install record | Owner | Review clears | Doctor green and first task works | Package/permission mismatch | Fix before promotion |
| Proof cohort | Find conversion defects | 5-10 high-fit users | Install guide plus first task | Owner | Honest extension status | Most finish unaided | Same blocker appears 3 times | Repair and rerun |
| Client recipes | Seed durable ecosystem proof | MCP client users | Three client-specific recipes | Owner/testers | Verified client runs | Outside user reproduces | Recipe needs private knowledge | Correct docs |
| Show HN | Anchor technical discovery | Builders | README, hero, founder comment | Owner | All launch predicates | Qualified questions, trials, accurate retelling | Security issue or owner unavailable | Publish findings and fixes |
| Bluesky adaptation | Show visible delight | OSS/agent networks | GIF, alt text, short thread | Owner | Anchor/canonical URL live | Relevant replies and saves | Spam-like repetition or poor-fit traffic | Answer and archive signal |
| Technical article | Create durable search and understanding | Browser/local-first developers | Standalone architecture essay | Owner | Launch questions known | Qualified referrals and citations | Becomes disguised product page | Strengthen educational value |
| Curator outreach | Enable earned amplification | High-fit readers | Individual factual brief | Owner | Independent use evidence | Accurate voluntary coverage | Mass-list behavior or paid ambiguity | Thank, correct, do not pressure |
| OpenSSF assessment | Add bounded trust evidence | Security reviewers | Maintained assessment | Owner | Current controls documented | Reviewers use it accurately | Unsupported answer or stale evidence | Assign review cadence |

## Decisions requiring owner approval

1. Manual-extension proof cohort before store acceptance: yes or no.
2. Primary first-use lane: Discussions or Issues.
3. Founder account for Bluesky, Hacker News, and any later Product Hunt launch.
4. Protected date and time window for Show HN participation.
5. Whether Product Hunt belongs in this cycle at all.
6. Whether a non-author tester's feedback may become a public quote or only anonymous learning.

No activity above grants authorization to publish, post, message, submit, or change public metadata.
