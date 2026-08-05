# Public delight 0.8: LEDGER

Durable progress for ADR-0100. A fresh session resumes from this file and does not infer state from
conversation history.

## RESUME HERE

- State: E2 is complete; E3 has not started
- Current pass: E3
- Next action: read `E3-core-public-surfaces.md` and adapt the approved message architecture to the
  product repository front doors and first-success path
- External actions: none authorized beyond preparing website work on a non-publishing branch

## Status

| Pass | Title | Status | Product commit | Website commit | Deviations |
| --- | --- | --- | --- | --- | --- |
| E1 | Truth and reception baseline | DONE | this pass's `docs(public): establish E1 public truth baseline` commit | - | none |
| E2 | Message and content architecture | DONE | this pass's `docs(public): define 0.8 message architecture` commit | - | none |
| E3 | Core public surfaces | pending | - | - | - |
| E4 | Agent guidance and metadata | pending | - | - | - |
| E5 | Website and directory surfaces | pending | - | - | - |
| E6 | Release reconciliation and reception loop | pending | - | - | - |

Status values: `pending`, `in-progress`, `DONE`, or `BLOCKED`.

## Seed observations from 2026-08-05

These observations reduce rediscovery. They are not accepted E1 evidence until reverified and
cited in the dated baseline.

- The official npm API reported `ghostlight` 0.7.3, 538 downloads for 2026-07-29 through
  2026-08-04, and 2,009 downloads for 2026-07-06 through 2026-08-04. Counts are directional and may
  include automation.
- Glama showed A license, A quality, and A maintenance, one favorite, and a B grade for `computer`.
  Its ingested project copy was stale.
- The live Sylin Ghostlight page was indexed with v0.5.7, the older relay topology, and a stale
  Chrome review statement. Other Sylin project routes repeated enough Ghostlight content to create
  search-result confusion.
- Exact Chrome extension-id searches returned no useful indexed listing. Store users and reviews
  were not independently captured.
- Searches found little independent review material. Project-authored Codex and Zed showcases,
  directory submissions, and registry approvals are distribution, not reception.
- Playwright MCP now documents an extension path into existing signed-in tabs. Chrome DevTools MCP
  emphasizes live Chrome debugging and performance. Ghostlight should differentiate through its
  complete local, visible, recoverable, governed, multi-client experience rather than claiming
  signed-in-browser access is unique.
- `docs/public-status.json` and `docs/STATUS.md` say the service candidate and source adapter are
  0.8.0, the public service is 0.7.3, public Chrome adapter is 0.7.1, and Chrome adapter 0.8.0 is
  pending with deferred publication. Re-read them before use.

## Decisions already settled

- ADR-0100 is the public-documentation decision.
- ADR-0094 owns tool guidance: identity stays stable; descriptions and metadata may improve.
- ADR-0093 owns service/browser-adapter compatibility.
- `docs/public-status.json` owns public release and store state.
- `CHANGELOG.md` owns release notes.
- `sylin-org/website` may be changed on a non-publishing branch. Deployment remains owner-gated.

## External gates

- Chrome adapter 0.8.0 approval and intentional publication.
- Ghostlight service 0.8.0 release, registry update, npm publication, and package-manager updates.
- Website push or merge that triggers deployment.
- Store description resubmission while 0.8.0 is under review.
- Directory edits or submissions, GitHub Discussion creation, showcase updates, and social posts.
- Permission to quote any user or identify any proof participant.

## Execution log

Append one section per completed or blocked pass. Include verified facts, files changed, gates run,
commit hashes, external drafts, and numbered deviations.

### E1 -- truth and reception baseline -- 2026-08-05

- Commit: see this pass's `docs(public): establish E1 public truth baseline` commit.
- Baseline: `docs/research/public-reception-2026-08.md`, observed 2026-08-05.
- Verified product facts: public service 0.7.3; source service 0.8.0; public adapter 0.7.1;
  source and pending adapter 0.8.0; 25 registry tools; nine installer clients; Windows and Linux
  live-browser proof; macOS CI proof with live-browser verification still owed; exact 2025-11-25
  and 2026-07-28 source-candidate MCP shores.
- Verified reception measurements: npm 538 downloads for 2026-07-29 through 2026-08-04 and 2,009
  for 2026-07-06 through 2026-08-04; Chrome Web Store two users with no ratings or written
  reviews; GitHub 0 stars, 0 forks, 0 open issues, 62 aggregate v0.7.3 asset downloads, and one
  project-authored discussion; Glama one favorite. The baseline records the required automation,
  aggregation, and absence-of-evidence caveats.
- Owner-access measurements: Chrome adapter 0.8.0 is accepted for review with deferred
  publication. GitHub's 14-day owner-only window ending 2026-08-04 reported 13 views from 10
  unique visitors, 848 clones from 157 unique cloners, and one view/one unique each from Google and
  github.com in qualifying referrers. Recheck the owner dashboard and traffic APIs before reuse.
- Distribution corrections: Glama is A/A/A with `computer` B but has stale copy; mcpservers.org is
  live but stale; Winget v0.7.3 merged on 2026-08-02; Cline issue 1989 and awesome-mcp-servers PR
  11306 remain open; GitHub MCP Registry approval is owner-recorded but public catalog visibility
  was not independently located.
- Discovery result: the canonical Sylin page and the `/agyo/` and `/zen-garden/` routes expose old
  adapter or topology text. Bounded searches located no independent written review,
  user-authored workflow, or case study. This was recorded as unavailable evidence, not zero.
- Defensible comparison: existing signed-in tabs, visible browser work, form actions, debugging,
  GIFs, and local multi-client access are not unique primitives. The supported distinction is the
  combined visible, recoverable, governed, audited, multi-client local experience.
- Files changed: the dated baseline, both distribution runbooks, this ledger, and `docs/STATUS.md`.
  No README, website, tool definition, or external surface changed.
- Gates: `cargo fmt --all -- --check`; strict workspace Clippy; full fast-tier workspace tests;
  ASCII scan; relative-link validation; `git diff --check`; and live external evidence-link fetches
  all passed. The three owner-only GitHub traffic links returned the expected anonymous 401 after
  their authenticated observations were captured. The pre-existing future `main` MCPB form URL
  remains 404 until that release gate lands.
- Deviation 1: the first Clippy attempt could not create Cargo's temporary sibling beside
  `F:\tmp\ghostlight-e1-target`. It was rerun successfully in the isolated workspace target
  `.target-e1`; this was an environment-path failure, not a code finding.

### E2 -- message and content architecture -- 2026-08-05

- Commit: see this pass's `docs(public): define 0.8 message architecture` commit.
- Copy kit: `docs/design/public-message-0.8.md`.
- Canonical story: one-sentence, short, medium, and long descriptions follow the ADR-0100 order of
  useful work, visibility, continuity, local ownership, optional governance, then technical depth.
- Architecture: six outcome pillars, four audience routes, fit and anti-fit language, a 13-claim
  evidence matrix, and one primary job for each public surface keep later adaptations coherent
  without making every page comprehensive.
- First-success contract: reusable blocks cover roughly 15-second recognition, 2-minute fit,
  5-minute first success, and symptom-led recovery. Exact 0.8 local stdio wording names protocol
  revisions `2025-11-25` and `2026-07-28` below the product story.
- Proof recipes: safe synthetic form, user-chosen authenticated read, exact browser-created child
  continuity, and page/console/network diagnosis each include a copyable prompt, visible result,
  success boundary, and evidence owner. The two Sylin demo routes and both example domains returned
  HTTP 200; the live brief labels and success text matched the recipe.
- Discovery copy: page title is 59 characters, search description is 147 characters, and the
  compact directory draft is 230 characters. The fuller directory draft and banned-claim list are
  internal drafts only; no external edit or submission occurred.
- Better-fit guidance: Playwright MCP, Chrome DevTools MCP, first-party browser integration, and
  hosted/headless browser use cases are stated directly. Signed-in access is not treated as a
  unique primitive.
- Files changed: the new copy kit, this ledger, and `docs/STATUS.md`. No public front door, package
  metadata, tool definition, website, store, directory, or other external surface changed.
- Gates: `cargo fmt --all -- --check`; strict workspace Clippy; full fast-tier workspace tests;
  ASCII scan; relative-link validation; external proof-page and exact-marker checks; and
  `git diff --check` all passed.
- Deviations: none.
