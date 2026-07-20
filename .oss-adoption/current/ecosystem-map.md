Status: complete
Run ID: 20260718T214944Z
Project: Ghostlight
Created: 2026-07-18T21:49:44Z

# Ecosystem map

## User workflow and adjacent systems

```text
MCP client -> Ghostlight relay/service -> Chromium extension -> managed tab in user's profile
     |                 |                         |
  agent UX       policy + audit            visible human feedback
```

Discovery starts in MCP clients, the MCP Registry, npm, GitHub, Chromium extension surfaces, and
browser-automation discussions. Adoption depends on all of them resolving to one current install
story. Organization adoption adds local identity sources, managed policy, SIEM destinations,
procurement review, and the RAWX capability vocabulary.

## Comparable projects

Counts below are point-in-time context from 2026-07-18, not product-quality scores.

| Project | Current public center | Overlap | Structural difference relevant to users | Strategic response |
| --- | --- | --- | --- | --- |
| vercel-labs/agent-browser, 38,711 stars, v0.32.2 | Fast Rust CLI/daemon, direct CDP, persistent automation, broad installation | Navigation, interaction, screenshots, page state, scripting, MCP/agent use | Default product model owns an automation browser; user-session attachment is optional. It is broader for testing and automation but lacks Ghostlight's managed-tab, visible-intent, RAWX, and identity-bound audit model. | Keep a current mutual capability map. Borrow compact action ergonomics where they improve model delight without importing headless/cloud scope. |
| microsoft/playwright-mcp, 35,238 stars, v0.0.78 | Playwright MCP with isolated, persistent, and extension modes | Broad browser actions, accessibility, console/network, existing-session extension mode | Test/automation framework with an optional existing-browser path, not a fused local governance and visible-human product | Position by job, not superiority: testing/isolated sessions versus responsible user-context work. |
| ChromeDevTools/chrome-devtools-mcp, 47,149 stars, v1.6.0 | Chrome debugging, performance, inspection, and automation | Existing Chrome attachment, CDP, screenshots, console/network, action tools | Debugging/performance altitude; telemetry and update checks default on, with opt-outs; no Ghostlight managed-tab governance model | Treat as a specialist complement and use its inspection breadth to identify non-core feature opportunities. |
| browser-use, 105,418 stars, v0.13.6 | Browser agent framework, local and hosted/cloud paths, broad integrations | Agent-driven browser tasks, forms, persistent sessions, custom tools | Framework/cloud-agent model with headless, stealth, proxy, and scale paths that Ghostlight deliberately excludes | Watch local governance development, but do not chase cloud or stealth parity. |
| Anthropic first-party browser integration | Claude-native real-browser experience | Existing user session and visual user experience | Tied to Anthropic clients; closed product surface | Not a target concern. Ghostlight serves other MCP clients and organizations wanting local RAWX-style governance. |

Sources:

- https://github.com/vercel-labs/agent-browser
- https://github.com/microsoft/playwright-mcp
- https://github.com/ChromeDevTools/chrome-devtools-mcp
- https://github.com/browser-use/browser-use
- `../../docs/COMPARISON.md`

## Complements and integrations

- **MCP clients:** Codex, Claude Code, Cline, Cursor, VS Code, Zed, OpenCode, Windsurf, Crush, and
  any stdio-capable client are direct adoption hosts.
- **Official MCP Registry:** a high-intent canonical server directory already carrying
  `org.sylin/ghostlight`.
- **npm and package managers:** low-friction service installation plus discoverable metadata.
- **Chromium browsers:** Chrome, Edge, Brave, and Chromium are supported installation targets.
- **Generic agent governance:** gateways and policy runtimes can compose outside Ghostlight;
  Ghostlight adds browser-semantic classification and actual-host binding.
- **SIEM and local audit workflows:** organization value appears when the audit can join existing
  operational evidence without sending data to Ghostlight.
- **RAWX spec:** can become a vendor-neutral artifact that makes the governance model easier to
  discuss beyond Ghostlight.

## Registries, catalogs, and host ecosystems

Use now or maintain:

- GitHub repository, Releases, topics, and Discussions;
- npm package `ghostlight`;
- Official MCP Registry `org.sylin/ghostlight`;
- Homebrew tap and release artifacts;
- Chrome Web Store after review, then Edge Add-ons if the accepted artifact remains equivalent.

Prepare selectively:

- a small number of maintained MCP directories that accept local stdio servers and link to the
  canonical registry entry;
- client-specific showcase or community surfaces where a working proof can be demonstrated;
- OpenSSF Best Practices or OSPS Baseline self-assessment for security-aware adopters.

Avoid treating generic AI directories, paid roundup placement, or high-volume startup catalogs as
primary adoption surfaces.

## Communities and trusted curators

- Hacker News is a plausible anchor because Ghostlight is non-trivial, locally runnable, and
  technically discussable. It is only appropriate after the frictionless install path works.
- MCP GitHub Discussions is for protocol questions, proposals, and community help, not unsolicited
  product promotion. Participate when Ghostlight has a relevant technical contribution or
  integration question.
- Client communities are high-fit only when the message shows that client's exact install and
  first-use path.
- Local-first, Rust, browser automation, and agent-security writers are potential earned channels
  after independent use exists.
- Bluesky and similar social feeds can carry the hero and founder voice but should amplify a
  canonical artifact, not substitute for it.

## Strategic openings

1. **Own "responsible user-context browser automation."** Competitors cluster around test
   automation, agent frameworks, debugging, and cloud scale. Ghostlight can make visible local
   agency plus inspectable governance legible as a distinct job.
2. **Make model delight measurable.** Compact results, semantic one-call actions, recovery
   guidance, and stable schemas can be shown with before/after task traces, not vague efficiency
   claims.
3. **Turn visual delight into trust evidence.** The same border, scan, typing, click, camera, and
   JavaScript signatures that delight users make agency legible to non-developers.
4. **Use the decision aid as a choice tool.** It can route readers to Ghostlight, isolated
   automation, cloud browsers, or first-party integrations without pretending every user should
   choose Ghostlight.
5. **Publish RAWX as a category artifact.** Useful mappings, policy examples, and integrations can
   earn security/governance discovery separately from product promotion.
6. **Create client-native first-task recipes.** One verified recipe per major MCP client is a
   durable ecosystem object and lowers support cost.

## Risks and dependencies

- Store review and extension permissions create a trust and conversion bottleneck.
- Product-site version drift can invalidate otherwise careful claims.
- Competitors ship quickly; comparison facts must be date-stamped and rechecked.
- A solo founder can be overwhelmed by simultaneous attention and security questions.
- Governance language can make the free core look paid or enterprise-first if it appears too early.
- Trending bots, stars, clone counts, and generic directories can create misleading demand signals.
- A real authenticated browser is powerful. Promotion that hides this responsibility would attract
  the wrong users and increase risk.
