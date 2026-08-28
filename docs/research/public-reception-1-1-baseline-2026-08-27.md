# Public reception snapshot -- 1.1.0 publication day, 2026-08-27

Pre-announcement organic state, taken the day 1.1.0 published. This is the baseline any future
announcement is measured against. Same discipline as the 2026-08 reception loop: a zero counter
is not an absence-of-interest claim, and every number is labeled with what it can and cannot say.

## GitHub (sylin-org/ghostlight)

- 1 star (starred 2026-08-08, during 0.8, by an account that now resolves as hidden), 0 forks,
  0 watchers.
- 0 open issues; 0 organic comments in issues, discussions, or the welcome thread. The only
  discussion posts are the maintainer's own.
- 4 open pull requests, all Dependabot, opened 2026-08-26 (sysinfo, winreg, uuid, the actions
  group); 12 notifications are stale Dependabot mail from 2026-08-13.
- Traffic: 4 views / 3 uniques over 14 days; referrer: Google, 2 views.
- Clones: 1,969 / 144 uniques over 14 days -- dominated by this project's own CI runners, which
  clone per job across dozens of runs. Not a demand signal; recorded to avoid over-reading it
  later.

## npm ghostlight

- 1,507 downloads in the trailing month; 520 in the trailing week.
- Daily shape: roughly 6-35/day through 2026-08-25 (the existing 0.8 user base), then 143 on
  2026-08-26 (1.0.0 publication) and 303 on 2026-08-27 (1.1.0 publication). The publication bump
  is visible; the absolute numbers remain small. Each `npx` install run counts once, and
  publication-day install smokes are included in these counts; per-user attribution does not
  exist and is not invented.

## Chrome Web Store (adapter 1.0.0)

- 11 users; no ratings, no reviews. The listing correctly serves adapter 1.0.0; the 1.1.0
  service needs no adapter update (ADR-0142). The 2026-08 baseline was 2 users.

## Directories and registries

- Official MCP Registry record current at 1.1.0.
- mcpservers.org: fresh text -- 1.0, Apache-2.0 OR MIT, 24 tools, correct platform scope.
- Glama: STALE and now actively wrong -- describes adapter 0.8.0, the retired
  "Ghostlight Commercial License" open-core split (withdrawn by ADR-0140), a macOS claim the
  platform-scope decision never made, and 0.8-era vocabulary. One favorite. Refresh is a
  pre-announcement cleanup item; the canonical repository text it crawls is current.
- A third-party registry mirror (manifest.manifold.security) lists the server with current
  description text.

## Organic mentions

None found on Reddit, Hacker News, Lobsters, YouTube, or X. The only outward post is the
maintainer's own openai/codex discussion (created 2026-08-01; 1 upvote; the only reply is the
maintainer's 0.8.0 update). An unrelated "Ghost browser agent" project by a different author
ranks for generic queries.

## Reading

Consistent with a pre-announcement project that has a small organic 0.8-era user base: real npm
traction, small store adoption, no public feedback pressure anywhere, and one directory carrying
a wrong licensing claim that matters before any announcement goes out.
