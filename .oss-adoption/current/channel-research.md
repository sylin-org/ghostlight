Status: complete
Run ID: 20260718T214944Z
Project: Ghostlight
Created: 2026-07-18T21:49:44Z
Verified at (UTC): 2026-07-18
Primary source: https://news.ycombinator.com/showhn.html

# Channel research

## Ranked channel portfolio

| Channel | Audience and job | Current rule or mechanism | Native artifact | Effort and support | Risk | Decision |
| --- | --- | --- | --- | --- | --- | --- |
| Owned repository and site | Every evaluator; establish current truth and first success | GitHub topics support discovery and the community profile checks health files | README hero, install path, decision aid, Trust Center, release proof | Medium repair, low ongoing | Current site contradicts v0.6.0 and Linux status | Use now: repair before borrowing attention |
| npm | MCP users installing with `npx` | Search uses title, description, README, and keywords; new packages can take time to appear | Accurate package metadata and runnable one-command install | Low | Downloads include automation and do not prove activation | Maintain now |
| Official MCP Registry | People actively looking for MCP servers | Published server metadata is the canonical directory object | `org.sylin/ghostlight`, npm package, homepage, repository | Low after release automation | Version immutability and metadata drift | Maintain now |
| Chrome Web Store | Chromium users seeking a trusted install path | Listing metadata, privacy fields, icon, and screenshots must be accurate and complete | Accepted extension listing plus current visual assets | High until review clears | Review delay and permission scrutiny | Prepare; broad-launch prerequisite |
| Advanced proof cohort | High-fit users across named MCP clients | Direct invitation with explicit pre-release extension friction | Public install guide plus one read-only first task | High-touch but bounded | Founder help can hide onboarding defects | Use now with 5-10 informed testers |
| Show HN | Technical builders who can run and discuss local software | Must be personally built, non-trivial, runnable, easy to try, and founder-present; no vote solicitation | Repository/hero plus candid founder comment | High for active day | Premature post wastes the one strong first impression | Prepare; preferred anchor after store acceptance |
| Bluesky | OSS, agent, Rust, and local-first networks plus feed-based discovery | Authenticity rules prohibit spam, undisclosed commercial content, and manipulated signals | Hero GIF with alt text, one factual thread, founder replies | Medium | Broadcast without relationships yields shallow reach | Prepare for anchor day or next day |
| Ghostlight GitHub Discussions | Users needing help, ideas, workflows, and show-and-tell | Project-controlled public lane; currently enabled but empty | Welcome/routing post and client-specific workflow threads | Medium | Too many categories create maintenance debt | Use before anchor with a minimal structure |
| Client communities | Users already in Codex, Cline, OpenCode, VS Code, and similar workflows | Rules vary and require fresh per-community review | One client-specific install and first-use proof | Medium | Generic cross-post reads as extractive promotion | Prepare selectively after proof cohort |
| MCP community channels | Protocol contributors and implementers | Official guidance routes support and long-form technical discussion to GitHub Discussions; contribution channels are not general product advertising | RAWX mapping, registry lessons, or concrete interoperability proposal | Medium | Unsolicited launch post would misuse the channel | Use only for relevant technical contribution |
| OpenSSF Best Practices | Security-aware adopters and procurement reviewers | Free voluntary self-certification against published criteria | Honest passing/baseline assessment with evidence | Medium | Badge theater if criteria are not maintained | Prepare after current release consistency repair |
| Reddit communities | Topic-specific browser, MCP, Rust, or local-first practitioners | Site norms plus each community's current self-promotion rules | Text-first technical post with disclosure and focused question | Medium/high | Account-history, spam, and community-fit risk | Watch; use only where owner is a real participant |
| Product Hunt | Broader product and non-developer audience | Product must be live; personal account posts; high craft and useful/novel work influence featuring | Polished page, hero, screenshots, maker comment | High full-day load | Lower intent, launch theater, premature broad reach | Defer until post-anchor evidence says it is useful |
| Curators and newsletters | Readers relying on trusted filters | No universal submission right; individual fit and editorial independence matter | Short factual curator brief with proof and non-claims | Medium | Mass outreach or paid disguised coverage damages trust | Prepare; use only after independent adoption evidence |

## Current source record

### GitHub owned surface

- Verified at (UTC): 2026-07-18.
- GitHub says topics help people find repositories by purpose and subject and allows up to 20
  topics. Ghostlight currently has ten useful topics.
- GitHub's community profile checks README, license, Code of Conduct, contribution, security, and
  intake files. Ghostlight currently reports 75%, with issue and PR templates absent.
- GitHub traffic is a rolling 14-day UTC view available to maintainers; snapshot it before and
  after each campaign.
- Sources:
  - https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/classifying-your-repository-with-topics
  - https://docs.github.com/en/communities/setting-up-your-project-for-healthy-contributions/about-community-profiles-for-public-repositories
  - https://docs.github.com/en/repositories/viewing-activity-and-data-for-your-repository/viewing-traffic-to-a-repository

### npm

- Verified at (UTC): 2026-07-18.
- npm says description and keywords support discovery. Search matches title, description, README,
  and keywords. Ghostlight's package contains all four plus repository, homepage, bugs, version,
  license, and `mcpName` metadata.
- Sources:
  - https://docs.npmjs.com/files/package.json/
  - https://docs.npmjs.com/searching-for-and-choosing-packages-to-download/

### Official MCP Registry

- Verified at (UTC): 2026-07-18.
- The official registry describes itself as a community-driven MCP server registry and exposes a
  publish API. Current publisher releases are active. Ghostlight already has a version-aligned
  `server.json` and automated DNS-authenticated publication.
- Sources:
  - https://registry.modelcontextprotocol.io/docs
  - https://github.com/modelcontextprotocol/registry/releases

### Chrome Web Store

- Verified at (UTC): 2026-07-18.
- Chrome requires a description, icon, screenshots, accurate comprehensive metadata, and privacy
  fields consistent with behavior. It rejects misleading, incomplete, stale, or keyword-spam
  listings.
- Source: https://developer.chrome.com/docs/webstore/program-policies/listing-requirements

### Show HN and Hacker News

- Verified at (UTC): 2026-07-18.
- Show HN accepts non-trivial work that people can try and whose maker is present to discuss it.
  It favors low-friction access and excludes landing pages or fundraisers. Minor release notices
  are generally insufficient. Vote or comment solicitation is prohibited.
- General HN guidance says self-posting is acceptable only as part of genuine curiosity-driven
  participation and asks submitters to use the original source without promotional title tricks.
- Sources:
  - https://news.ycombinator.com/showhn.html
  - https://news.ycombinator.com/newsguidelines.html

### Bluesky

- Verified at (UTC): 2026-07-18.
- Bluesky's current guidelines prohibit spam, repeated disruptive posting, manipulated social
  signals, deceptive accounts, and undisclosed commercial content. A founder-authored factual post
  with clear affiliation and normal replies fits; coordinated engagement does not.
- Source: https://bsky.social/about/support/community-guidelines

### MCP community

- Verified at (UTC): 2026-07-18.
- Official MCP guidance distinguishes Discord coordination from durable GitHub Discussions and
  routes user support and proposals to the appropriate public records. Use those spaces for a real
  protocol, registry, or governance contribution, not a generic product announcement.
- Sources:
  - https://modelcontextprotocol.io/community/contributing
  - https://modelcontextprotocol.io/community/communication

### OpenSSF

- Verified at (UTC): 2026-07-18.
- The Best Practices program is a free voluntary self-certification and now supports both its
  traditional criteria and OSPS Baseline series.
- Source: https://openssf.org/projects/best-practices-badge/

### Reddit and Product Hunt

- Verified at (UTC): 2026-07-18.
- Reddit's site-wide etiquette is only a baseline; exact subreddit rules still need review on the
  day of use. Ghostlight should skip a community where the owner lacks genuine participation.
- Product Hunt requires a personal posting account and a live product. Current featuring guidance
  emphasizes utility, novelty, craft, and creativity. It can fit Ghostlight's visual delight later,
  but is not the highest-intent first channel.
- Sources:
  - https://support.reddithelp.com/hc/en-us/articles/205926439-Reddiquette
  - https://help.producthunt.com/en/articles/9883485-product-hunt-featuring-guidelines
  - https://help.producthunt.com/en/articles/479557-how-to-post-a-product

## Facts, inferences, and hypotheses

Facts:

- GitHub metadata, npm, v0.6.0 Releases, and MCP Registry distribution are live.
- The Chrome Web Store path is not yet public.
- The website is stale relative to the repository.
- The repository has no stars, forks, watchers, public issues, or Discussions at this snapshot.
- npm recorded 461 downloads during 2026-07-11 through 2026-07-17.

Inferences:

- Ghostlight has distribution plumbing but essentially no deliberate public awareness yet.
- The 6,019 clone events and 445 unique cloners against only five unique repository visitors are
  dominated by automation, mirrors, release work, or other non-human activity and should not be
  treated as adoption.
- A Show HN anchor has better problem and proof fit than a Product Hunt-first launch.

Hypotheses to test:

- The phrase "your real logged-in browser" produces the fastest accurate category recognition.
- Visible action feedback attracts non-developers without weakening the developer proposition.
- The Chrome Web Store path materially improves trust and completion versus manual unpacked load.
- Client-specific examples convert more useful users than a broad generic MCP post.

Owner choices:

- Which founder accounts will be durable public voices.
- Whether Discussions or Issues is the primary first-use feedback lane.
- Whether to run a manual-extension proof cohort before store acceptance.
- Whether Product Hunt earns the later support cost.
