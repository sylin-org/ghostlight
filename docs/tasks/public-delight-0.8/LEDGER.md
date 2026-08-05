# Public delight 0.8: LEDGER

Durable progress for ADR-0100. A fresh session resumes from this file and does not infer state from
conversation history.

## RESUME HERE

- State: execution package authored; no execution pass has started
- Current pass: E1
- Next action: read `E1-truth-and-reception-baseline.md`, reverify its seeded external observations,
  and create the dated evidence baseline
- External actions: none authorized beyond preparing website work on a non-publishing branch

## Status

| Pass | Title | Status | Product commit | Website commit | Deviations |
| --- | --- | --- | --- | --- | --- |
| E1 | Truth and reception baseline | pending | - | - | - |
| E2 | Message and content architecture | pending | - | - | - |
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
