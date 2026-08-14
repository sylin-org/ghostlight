# Ghostlight public truth and reception baseline -- 2026-08-05

This is the evidence baseline for the public-documentation 0.8 work. It records what the product
can support, what public distribution surfaces say, and what evidence exists that somebody chose
or used it. It is a dated snapshot, not a live dashboard.

## Evidence labels

- **Product evidence** proves what the current source, release, or operating boundary does.
- **Distribution evidence** proves that Ghostlight is available or described somewhere. It does
  not prove that an independent person noticed or used it.
- **Reception evidence** records an independent action or aggregate audience signal. Aggregate
  traffic and download counts are directional, not user counts.

All external observations below were rechecked on 2026-08-05 unless a narrower date is shown.

## Canonical public truth

| Fact | Current truth | Evidence | Owner of the fact |
| --- | --- | --- | --- |
| Public service | 0.7.3 | Product: [npm latest](https://registry.npmjs.org/ghostlight/latest), [official MCP Registry](https://registry.modelcontextprotocol.io/v0.1/servers?search=org.sylin%2Fghostlight), and [repository status](../public-status.json) | Release pipeline and `docs/public-status.json` |
| Public platform proof | Windows and Linux verified end to end with live browsers | Product: [repository status](../public-status.json) | `docs/public-status.json` and release verification |
| Source service candidate | 0.8.0, not released | Product: [Cargo workspace](../../Cargo.toml), [changelog](../../CHANGELOG.md), and [repository status](../public-status.json) | Source tree and changelog |
| Public Chrome adapter | 0.7.1, updated 2026-08-02 | Product and distribution: [Chrome Web Store listing](https://chromewebstore.google.com/detail/ghostlight-in-browser/lejccfmoeogmhemakeknjjdhkfkgncdl) | Chrome Web Store public listing |
| Source Chrome adapter | 0.8.0; covers 0.8.x services | Product: [extension manifest](../../extension/manifest.json) and [compatibility contract](../../compatibility.json) | Source tree and compatibility contract |
| Pending Chrome adapter | 0.8.0 accepted for review with deferred publication; public listing does not expose this state | Product: [repository status](../public-status.json) and [project status](../STATUS.md) | Owner dashboard, reconciled into `docs/public-status.json` |
| Runtime shape | Three sibling executables: MCP connector, persistent service, and browser connector | Product: [0.8 changelog](../../CHANGELOG.md) and [ADR-0096](../adr/0096-protocol-versioned-mcp-edge-and-neutral-service.md) | Release artifacts and ADR-0096 |
| Source-candidate MCP revisions | Exact 2025-11-25 and 2026-07-28 shores | Product: [0.8 changelog](../../CHANGELOG.md) and [MCP connector source](../../crates/mcp-connector/src) | MCP connector source |
| Source-candidate conformance | Live official-runner checks passed initialize, ping, tools/list, safe `explain`, and dated wire-schema validation for both supported revisions | Product: [project status](../STATUS.md) and [conformance ADR](../adr/0049-mcp-conformance-pass.md) | Local conformance fork evidence recorded in project status |
| Declared tools | 25 additive registry entries; the original 13 trained identities remain stable | Product: [0.8 tool registry](https://github.com/sylin-org/ghostlight/blob/c01cc3276102471f3e18de2ae90cb90abf98ed88/crates/core/src/browser/directory.rs), [0.8 schema fidelity test](https://github.com/sylin-org/ghostlight/blob/c01cc3276102471f3e18de2ae90cb90abf98ed88/tests/tool_schema_fidelity.rs), and [ADR-0094](../adr/0094-agent-readable-tool-definitions.md) | Registry and fidelity test |
| Installer clients | Claude Code, Claude Desktop, Cursor, VS Code, Codex, Windsurf, Zed, OpenCode, and Crush | Product: [0.8 installer client definitions](https://github.com/sylin-org/ghostlight/blob/c01cc3276102471f3e18de2ae90cb90abf98ed88/crates/core/src/install/clients.rs) | Installer source |
| Runtime trust boundary | Local runtime, no vendor service or telemetry; optional managed policy may fetch from the user's organization | Product: [Trust Center](../trust/README.md), [data flows](../trust/data-flows.md), and [Continuity Promise](../trust/continuity.md) | Trust Center and implementation |

The pending Chrome submission is the canonical product-state item above that requires the owner
dashboard. Recheck it and reconcile `docs/public-status.json` before any 0.8 release or store
claim.

## Capability map by user outcome

This map describes the present source candidate. It does not imply that every public directory
has ingested the candidate metadata.

| User outcome | Current capability | Product evidence |
| --- | --- | --- |
| Start in the browser already in use | Work in visible Chromium tabs, preserve signed-in state, create or select a workspace, and recover a stale workspace through explicit tab creation | [0.8 tab registry entries](https://github.com/sylin-org/ghostlight/blob/c01cc3276102471f3e18de2ae90cb90abf98ed88/crates/core/src/browser/directory.rs), [ADR-0090](../adr/0090-explicit-stale-workspace-recovery.md) |
| Understand a page | Read page structure or text, find matching content, inspect console and network buffers, and request an explained state summary | [0.8 tool registry](https://github.com/sylin-org/ghostlight/blob/c01cc3276102471f3e18de2ae90cb90abf98ed88/crates/core/src/browser/directory.rs) |
| Act with the right level of control | Choose semantic actions, form filling, direct input, low-level computer actions, scripts, or bounded batches | [0.8 tool registry](https://github.com/sylin-org/ghostlight/blob/c01cc3276102471f3e18de2ae90cb90abf98ed88/crates/core/src/browser/directory.rs) |
| Work across browser transitions | Keep exact workspace identity, adopt an unambiguous browser-created child tab, and resume after explicit context refresh or recovery | [ADR-0099](../adr/0099-browser-created-tab-continuity.md), [0.8 tool registry](https://github.com/sylin-org/ghostlight/blob/c01cc3276102471f3e18de2ae90cb90abf98ed88/crates/core/src/browser/directory.rs) |
| Handle browser side effects | Upload files or images, handle dialogs, create GIFs, resize the window, and wait for observable page state | [0.8 tool registry](https://github.com/sylin-org/ghostlight/blob/c01cc3276102471f3e18de2ae90cb90abf98ed88/crates/core/src/browser/directory.rs) |
| Keep control visible | Use a normal local browser window where the person can observe, interrupt, or take over the session | [Trust Center](../trust/README.md), [data flows](../trust/data-flows.md) |
| Add governance without changing the browser workflow | Apply capability and domain grants, local identity, and structured audit while leaving the all-open engine first-class | [Governance guide](../guides/governance-configuration.md), [ADR-0013](../adr/0013-governance-overlay-all-open.md) |
| Use the same product from several MCP clients | Install client-specific stdio entries while sharing one protocol-neutral local service and one browser endpoint | [0.8 installer client definitions](https://github.com/sylin-org/ghostlight/blob/c01cc3276102471f3e18de2ae90cb90abf98ed88/crates/core/src/install/clients.rs), [ADR-0096](../adr/0096-protocol-versioned-mcp-edge-and-neutral-service.md) |
| Operate without a vendor dependency | Run locally without a Ghostlight account, hosted control plane, telemetry, or activation service | [Continuity Promise](../trust/continuity.md), [FAQ](../trust/faq.md) |

## Public and reception measurements

### Package and store

| Signal | Observation | Label and limits | Direct source |
| --- | --- | --- | --- |
| npm current version | 0.7.3 on 2026-08-05 | Product and distribution | [npm package metadata](https://registry.npmjs.org/ghostlight/latest) |
| npm downloads, last week | 538 for 2026-07-29 through 2026-08-04 | Reception, directional. npm counts may include CI, automation, mirrors, repeat downloads, and launcher retries; they are not people or active users. | [npm downloads API](https://api.npmjs.org/downloads/point/last-week/ghostlight) |
| npm downloads, last month | 2,009 for 2026-07-06 through 2026-08-04 | Reception, with the same automation caveat | [npm downloads API](https://api.npmjs.org/downloads/point/last-month/ghostlight) |
| Chrome adapter version | 0.7.1, updated 2026-08-02 | Product and distribution | [Chrome Web Store listing](https://chromewebstore.google.com/detail/ghostlight-in-browser/lejccfmoeogmhemakeknjjdhkfkgncdl) |
| Chrome users | 2 on 2026-08-05 | Reception. Store user count is a coarse public metric, not proof of a completed workflow. | [Chrome Web Store listing](https://chromewebstore.google.com/detail/ghostlight-in-browser/lejccfmoeogmhemakeknjjdhkfkgncdl) |
| Chrome ratings and written reviews | `No ratings`; the reviews view showed no reviews on 2026-08-05 | Reception. This describes the store surface only; it is not evidence that nobody has an opinion. | [Chrome Web Store reviews](https://chromewebstore.google.com/detail/ghostlight-in-browser/lejccfmoeogmhemakeknjjdhkfkgncdl/reviews) |

### GitHub

| Signal | Observation | Label and limits | Direct source |
| --- | --- | --- | --- |
| Stars and forks | 0 stars and 0 forks on 2026-08-05 | Reception. A zero public counter is not an absence-of-interest claim. | [GitHub repository](https://github.com/sylin-org/ghostlight) |
| Open issues | 0 open issues excluding pull requests on 2026-08-05 | Reception. This is not a support-volume or satisfaction measure. | [GitHub issues](https://github.com/sylin-org/ghostlight/issues) |
| Discussions | 1 discussion on 2026-08-05, the project-authored welcome thread created 2026-08-02 | Distribution, not reception, because the project created it. | [Discussion 77](https://github.com/sylin-org/ghostlight/discussions/77) |
| v0.7.3 release assets | 62 aggregate asset downloads as of 2026-08-05 across 28 assets | Reception, directional. The sum mixes binaries, archives, checksum files, and automation; it is not an install or user count. | [GitHub release API](https://api.github.com/repos/sylin-org/ghostlight/releases/tags/v0.7.3) |
| Repository views | 13 views from 10 unique visitors in GitHub's owner-only 14-day window ending 2026-08-04 | Reception, directional and owner-only. The endpoint requires repository traffic permission. | [GitHub traffic views API](https://api.github.com/repos/sylin-org/ghostlight/traffic/views) |
| Repository clones | 848 clones from 157 unique cloners in GitHub's owner-only 14-day window ending 2026-08-04 | Reception, directional and owner-only. Bots, CI, mirrors, and repeat fetch behavior may contribute. | [GitHub traffic clones API](https://api.github.com/repos/sylin-org/ghostlight/traffic/clones) |
| Referrers | Google 1 view/1 unique and github.com 1 view/1 unique in the owner-only traffic response on 2026-08-05 | Reception, directional. GitHub reports only qualifying referrers and omits direct or unclassified traffic. | [GitHub popular referrers API](https://api.github.com/repos/sylin-org/ghostlight/traffic/popular/referrers) |

The GitHub traffic endpoints and pending Chrome review state require owner access. Preserve only
dated aggregate observations in public planning documents; do not expose credentials, visitor
identity, or private proof-participant details.

### Registries and directories

| Surface | Observation | Label and limits | Direct source |
| --- | --- | --- | --- |
| Official MCP Registry | `org.sylin/ghostlight` 0.7.3 was latest and active on 2026-08-05; seven version records were returned | Distribution. The latest record still carries the old `one portable binary` description. | [Registry API](https://registry.modelcontextprotocol.io/v0.1/servers?search=org.sylin%2Fghostlight) |
| Glama | A license, A quality, A maintenance, 1 favorite, and B for `computer` on 2026-08-05 | Distribution; the favorite is a small reception signal without identity or workflow context. The ingested repository snapshot is stale and still describes the old relay topology and older store state. | [Glama listing](https://glama.ai/mcp/servers/sylin-org/ghostlight), [Glama API](https://glama.ai/api/mcp/v1/servers/sylin-org/ghostlight) |
| mcpservers.org | Development listing live on 2026-08-05 | Distribution. Its copied project text is stale and describes the old relay topology and development extension path. | [mcpservers.org listing](https://mcpservers.org/servers/sylin-org/ghostlight) |
| Cline marketplace | Submission issue 1989 open on 2026-08-05 | Distribution, not reception | [Cline issue 1989](https://github.com/cline/mcp-marketplace/issues/1989) |
| awesome-mcp-servers | PR 11306 open, ready, and mergeable on 2026-08-05 | Distribution, not reception | [awesome-mcp-servers PR 11306](https://github.com/punkpeye/awesome-mcp-servers/pull/11306) |
| Winget | v0.7.3 PR 411087 merged on 2026-08-02 | Distribution. The merge proves catalog ingestion work completed; it does not prove installs. | [winget-pkgs PR 411087](https://github.com/microsoft/winget-pkgs/pull/411087) |
| GitHub MCP Registry | Owner records say approval completed 2026-08-03; no public catalog record was independently located on 2026-08-05 | Distribution with uncertainty. Keep the owner-recorded approval, but do not claim public discoverability until a public catalog entry is located. | [Project status](../STATUS.md), [GitHub MCP Registry](https://github.com/mcp) |
| Claude directory | Not submitted as of 2026-08-05; eligibility and released-MCPB gates remain | Distribution gate, not reception | [Local submission runbook](../business/DIRECTORY-SUBMISSIONS.md), [Anthropic submission guide](https://support.anthropic.com/en/articles/11175166-publishing-remote-and-local-mcp-servers-for-claude) |
| OpenAI plugin directory | Not submitted as of 2026-08-05; the public HTTPS requirement conflicts with Ghostlight's local-only boundary | Distribution gate, not reception | [Local submission runbook](../business/DIRECTORY-SUBMISSIONS.md) |

## Discovery and stale public surfaces

The following are bounded search and page checks, not exhaustive indexing audits.

| Check | Observation on 2026-08-05 | Evidence and owner |
| --- | --- | --- |
| Canonical Sylin page | The live page still describes adapter 0.6.0 with 0.7.1 pending and the old relay topology. | [Live Ghostlight page](https://sylin.org/ghostlight/). Website source owns the fix; `docs/public-status.json` and ADR-0096 own the correct facts. |
| Other Sylin routes | `/agyo/` and `/zen-garden/` repeat enough stale Ghostlight fallback content to create version and product confusion in search results. | [Agyo route](https://sylin.org/agyo/), [Zen Garden route](https://sylin.org/zen-garden/). Website routing/source owns the fix. |
| `Ghostlight MCP` | Results were weak and mixed stale Sylin pages, old package mirrors, and unrelated products sharing the name. | [Google query](https://www.google.com/search?q=%22Ghostlight+MCP%22). This is an engine- and location-dependent snapshot. |
| `Ghostlight browser automation` | Results did not establish a strong independent review or workflow trail. | [Google query](https://www.google.com/search?q=%22Ghostlight+browser+automation%22). This is not proof that no mention exists. |
| Extension name and id | Exact searches did not surface a useful indexed store result, although the direct official listing is live. | [Google name query](https://www.google.com/search?q=%22Ghostlight+in+Browser%22), [Google extension-id query](https://www.google.com/search?q=lejccfmoeogmhemakeknjjdhkfkgncdl), [official listing](https://chromewebstore.google.com/detail/ghostlight-in-browser/lejccfmoeogmhemakeknjjdhkfkgncdl). |
| Glama and mcpservers.org copy | Both public listings are live but ingest stale project text. | [Glama](https://glama.ai/mcp/servers/sylin-org/ghostlight), [mcpservers.org](https://mcpservers.org/servers/sylin-org/ghostlight). Each directory owns crawl timing; canonical repository and website text own the source material. |

## Independent reception

The bounded searches above did not locate an independent written review, a user-authored workflow,
or a public third-party case study. That is an unavailable evidence category, not a zero metric and
not evidence of dissatisfaction. The project-authored GitHub welcome discussion, Codex showcase,
Zed showcase, directory submissions, and registry approvals remain distribution evidence.

Reception evidence that is available today is limited to npm download counts, GitHub aggregate
release and traffic counters, two Chrome Web Store users, one Glama favorite, and the public
zero-valued GitHub/store counters reported above. None of those establishes retention, successful
task completion, governed usage, or an active-user count. Any future quote, named workflow, or
proof-participant story requires the participant's permission.

## Closest alternatives and defensible distinctions

| Alternative | What its primary source now supports | Defensible Ghostlight distinction |
| --- | --- | --- |
| Playwright MCP | Its Chrome extension can connect to existing tabs in the default browser profile and use logged-in state; the first connection asks the user to select and approve tabs. | Do not claim existing signed-in tabs are unique. Ghostlight combines visible local work with exact workspace identity, browser-created tab continuity, a stable multi-client surface, and optional capability/domain governance plus local audit. |
| Chrome DevTools MCP | It focuses on browser automation, debugging, console/network inspection, and performance analysis, and can connect to a running Chrome instance. Its docs disclose default usage statistics with an opt-out and optional CrUX URL sharing during performance work. | Ghostlight is not a performance-analysis replacement. Its defensible boundary is a local browser-workspace product with no vendor telemetry, optional local governance, and structured audit. |
| Claude in Chrome | It works in the user's signed-in visible browser, can fill forms, debug sites, and record GIFs, and is tied to Anthropic's supported Claude plans and surfaces. | Do not claim visibility, signed-in access, forms, debugging, or GIFs are unique. Ghostlight's distinction is vendor-neutral MCP client support plus its local governance and continuity model. |
| Browser Bridge | It advertises any MCP client, real Chrome tabs and logins, a local native host, per-site approval, high-risk confirmation, and no analytics. | Do not claim that local signed-in multi-client browser access is unique. Ghostlight can defend the completeness of its stable tool surface, workspace recovery and child-tab continuity, capability/domain grants, and structured local audit. |

Primary sources checked on 2026-08-05:

- [Playwright MCP repository](https://github.com/microsoft/playwright-mcp) and
  [Playwright extension guide](https://github.com/microsoft/playwright/blob/main/packages/extension/README.md)
- [Chrome DevTools MCP repository](https://github.com/ChromeDevTools/chrome-devtools-mcp)
- [Claude in Chrome documentation](https://code.claude.com/docs/en/chrome) and
  [Anthropic help article](https://support.claude.com/en/articles/12012173-get-started-with-claude-in-chrome)
- [Browser Bridge Chrome listing](https://chromewebstore.google.com/detail/browser-bridge/dgccjfjjilfpkbdllclmkiicajndkfcd)
  and [Browser Bridge repository](https://github.com/whg517/browser-bridge)

The claim to carry forward is the combined experience, not uniqueness of any primitive:
Ghostlight gives MCP clients a visible and interruptible local browser workspace, stable client
integration, explicit recovery and tab continuity, and optional capability/domain governance with
local audit, without a Ghostlight-hosted service or telemetry.

## Claims this baseline does not support

- Do not claim Ghostlight is the only MCP path to existing signed-in browser tabs.
- Do not convert downloads, clones, store users, or favorites into active users.
- Do not call project-authored showcases or submissions independent adoption.
- Do not claim the public website, Glama copy, mcpservers.org copy, or registry description is
  current until each surface is reconciled and rechecked.
- Do not claim GitHub MCP catalog discoverability from owner-recorded approval alone.
- Do not claim independent reviews or workflows were absent; say none were located in the dated,
  bounded searches.
- Do not publish a quote, name a proof participant, or describe a private workflow without explicit
  permission.

## Required rechecks before later passes publish anything

1. Recheck the Chrome owner dashboard and `docs/public-status.json` before store or 0.8 release
   language changes.
2. Recheck the live website after E5; remove the repeated stale fallbacks on other project routes.
3. Recheck directory crawls after canonical source text changes. Treat lag as a directory issue,
   not permission to put conflicting claims in the repository.
4. Refresh package, release, traffic, store, and reception measurements in E6. Keep their evidence
   labels and caveats.
5. Ask for explicit owner confirmation before any external submission, directory edit, store
   publication, review request, or named participant story.

## E6 pre-publication recheck -- 2026-08-05 15:14 -04:00

E6 repeated the version, store, release, traffic, directory, discovery, and comparison checks. The
full owner-gated action order is in
[`PUBLICATION-PACKET-0.8.md`](../business/PUBLICATION-PACKET-0.8.md). This section records what
changed after the E1 snapshot without rewriting that earlier observation.

- The live canonical website changed. Website `main` and the E5 work branch both resolve to
  `1568538ba5ca217e46b917688b41d17b7e672488`. The Ghostlight, install, privacy, brief, foundry,
  Agyo, and Zen Garden routes return HTTP 200. The Ghostlight page now shows public service 0.7.3,
  public adapter 0.7.1, pending adapter 0.8.0, current platform proof, the three executable roles,
  the read-only first task, and the four bounded recipes. It no longer exposes the old relay copy.
- Search caches have not caught up. Bounded searches still returned old Sylin snippets and did not
  locate an independent review, user-authored workflow, useful extension-id result, or GitHub MCP
  catalog entry. This remains unavailable evidence, not a zero metric.
- npm stayed at 0.7.3 with 538 downloads for 2026-07-29 through 2026-08-04 and 2,009 for
  2026-07-06 through 2026-08-04. GitHub stayed at 0 stars, 0 forks, 0 open issues excluding pull
  requests, one project-authored Discussion, and 62 downloads across 28 v0.7.3 assets. The
  owner-only traffic window stayed at 13 views/10 unique and 848 clones/157 unique, with Google
  and github.com at one view/one unique each.
- The public Chrome update feed stayed at adapter 0.7.1. The public reviews HTML still showed two
  users and `No ratings`. After the owner selected the private source, Ghostlight opened the Chrome
  Developer Dashboard through the signed-in browser and adopted its exact browser-created child.
  Chrome blocks extension inspection and debugger attachment on Web Store pages, so the owner
  supplied a screenshot instead. It showed publisher `sylin.org`, `Ghostlight in Browser` 0.8.0,
  two users, no rating, and `Pending review`. Recheck after that status changes and before any
  store or service publication.
- The official MCP Registry stayed at seven active version records with 0.7.3 latest and the old
  `one portable binary` description. Cline issue 1989 and awesome-mcp-servers PR 11306 stayed open;
  PR 11306 remained non-draft, mergeable, and clean. Winget 0.7.3 remained merged.
- Glama stayed A/A/A with one favorite and `computer` B. Glama and mcpservers.org still ingest the
  former relay copy. Their lag is downstream of the now-current canonical website and local 0.8
  source.
- The primary comparison sources still support E1's distinctions: Playwright MCP documents its
  existing-browser extension and signed-in state; Chrome DevTools MCP documents performance work
  and usage-statistics controls; Claude in Chrome remains first-party and signed-in; Browser
  Bridge remains a local real-Chrome alternative. No uniqueness claim changed.
- `scripts/check-public-surfaces.ps1 -Online` passed the current public 0.7.3 service, 0.7.1
  adapter, source 0.8.0 service and adapter, platform, and entry-path claims.

The release, 7-day, and 30-day manual record is
[`public-reception-loop-0.8.md`](public-reception-loop-0.8.md). It keeps public counters,
owner-visible aggregates, voluntary human reports, and project-authored distribution in separate
evidence categories.
