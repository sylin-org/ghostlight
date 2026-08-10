# Choosing a browser-control approach

Updated 2026-08-10 for the Ghostlight 1.0 contract, retaining the
[public truth and reception baseline](research/public-reception-2026-08.md) and the linked primary
sources below. This is a decision guide, not a scorecard. Existing signed-in browser access is a
shared capability, not a Ghostlight uniqueness claim.

## The short answer

Choose Ghostlight when an agent should work in the person's visible signed-in Chromium browser,
the same local browser capability should serve compatible MCP clients, and explicit recovery or
optional capability, domain, and audit boundaries matter.

Choose another approach when its primary job matches yours more closely:

- [Playwright MCP](https://github.com/microsoft/playwright-mcp) for deterministic browser testing,
  isolated contexts, and Playwright-native automation. Its extension can also connect to selected
  existing tabs in the default signed-in profile.
- [Chrome DevTools MCP](https://github.com/ChromeDevTools/chrome-devtools-mcp) for Chrome debugging,
  performance analysis, console/network inspection, and DevTools workflows.
- [Claude in Chrome](https://code.claude.com/docs/en/chrome) when Claude is the supported
  environment and Anthropic's first-party browser workflow already meets the need.
- A hosted or headless browser product when work must run away from the person's visible local
  browser, at scale, or inside disposable test sessions.
- A generic governance gateway when the organization needs one policy layer across many tools and
  browser-specific intent is not required.

## Where Ghostlight is narrower

Ghostlight is a local Chromium workspace product. It does not target Firefox, stealth automation,
scraping farms, remote browser hosting, or a shared multi-tenant browser service. It is also not a
replacement for Playwright test authoring or Chrome performance tooling.

Its supported combination is:

1. a dedicated visible workspace in the Chromium profile the person already uses;
2. a stable tool surface for supported and compatible local stdio MCP clients;
3. coherent browser work with explicit stale-workspace, child-tab, and transport recovery;
4. optional monotonic capability, host, and tab-close policy plus payload-free audit; and
5. a local runtime with no Ghostlight account, hosted control plane, or telemetry.

Personal and all-open operation is complete without governance. Ghostlight is open-core: the
engine is Apache-2.0 OR MIT, while the governance module is source-available under the Ghostlight
Commercial License. See [LICENSING.md](../LICENSING.md) for the exact boundary.

## Closest approaches

### Playwright MCP

Playwright MCP is a strong fit when the browser is part of a repeatable test or automation system.
Its official Chrome extension path can connect to existing selected tabs and use the default
profile's logged-in state. Do not choose Ghostlight merely because a job needs an authenticated
tab.

Choose Ghostlight instead when the primary product is the person's visible browser workspace and
you need Ghostlight's exact continuity, local policy, or audit model across compatible MCP clients.
Choose Playwright MCP when Playwright contexts, selectors, test tooling, or deterministic browser
ownership are the better abstraction.

Sources: [Playwright MCP](https://github.com/microsoft/playwright-mcp) and the
[Playwright extension guide](https://github.com/microsoft/playwright/blob/main/packages/extension/README.md).

### Chrome DevTools MCP

Chrome DevTools MCP focuses on automation plus browser debugging and performance analysis. It can
connect to a running Chrome instance and is the natural choice when traces, DevTools diagnostics,
or performance evidence are the result you need. Its documentation also explains its usage
statistics and performance-data controls; review those settings against your environment.

Choose Ghostlight when the central job is a visible user-session workflow with optional local
capability/host policy, payload-free audit, and explicit recovery. Ghostlight 1.0 does not expose
console traces, network capture, or performance tooling.

Source: [Chrome DevTools MCP](https://github.com/ChromeDevTools/chrome-devtools-mcp).

### Claude in Chrome

Anthropic's first-party integration works in a signed-in visible browser and supports forms,
debugging, and GIF capture from supported Claude surfaces. It is the simplest choice when Claude
is the only assistant, its plan and platform requirements fit, and its browser permission model is
enough.

Choose Ghostlight when the same browser capability must work through compatible non-Anthropic MCP
clients, or when local capability/host authority and payload-free audit are part of the operating
model.

Sources: [Claude in Chrome documentation](https://code.claude.com/docs/en/chrome) and
[Anthropic's setup guide](https://support.claude.com/en/articles/12012173-get-started-with-claude-in-chrome).

### Browser Bridge

Browser Bridge is another local native-host and extension approach. Its public material advertises
compatible MCP clients, real Chrome tabs and logins, per-site approval, high-risk confirmation,
and no analytics. That means local signed-in multi-client access is not a Ghostlight-only idea.

Compare the control model you need. Ghostlight's defensible distinction is its complete stable
browser job language, explicit workspace and child-tab recovery, monotonic capability/host
authority, and payload-free local audit. Browser Bridge may be the simpler fit when its per-site and confirmation model is
enough.

Sources: [Browser Bridge repository](https://github.com/whg517/browser-bridge) and
[Chrome Web Store listing](https://chromewebstore.google.com/detail/browser-bridge/dgccjfjjilfpkbdllclmkiicajndkfcd).

### agent-browser and browser frameworks

[agent-browser](https://github.com/vercel-labs/agent-browser) and frameworks such as
[browser-use](https://github.com/browser-use/browser-use) cover broader browser ownership,
isolated sessions, testing controls, cloud providers, and specialist automation. They are better
fits when the agent should own a separate browser lifecycle or the project needs a framework for
building a browser agent.

Ghostlight stays narrower: use the person's local visible Chromium profile from compatible MCP
clients, keep exact workspace authority, and optionally govern each browser intent at dispatch.

## Governance depth

Generic MCP and agent gateways can provide valuable organization-wide policy, identity, and audit.
They compose in front of Ghostlight. Their tradeoff is browser semantics: a generic gateway sees a
tool call, while Ghostlight classifies the requested browser job, checks its current landing at the
final boundary, and records one content-free decision and terminal outcome.

Enterprise browsers can govern agent activity inside a managed browser, often with deeper fleet
administration. They are a different deployment choice: the browser or enterprise service owns
the environment, while Ghostlight exposes a local automation capability to the person's chosen
MCP client.

See the [RAWX capability model](../open-spec/rawx-capability-model.md),
[governance configuration guide](guides/governance-configuration.md), and
[Trust Center](trust/README.md) for Ghostlight's exact claims and limits.

## Questions to decide with

1. Must the work happen in this person's existing visible Chromium profile?
2. Should the browser be user-owned, test-owned, or remotely hosted?
3. Is the primary result task completion, deterministic testing, or browser diagnosis?
4. Which clients must connect, and which have actually been verified?
5. Are per-site prompts enough, or are capability, host, tab-close, and payload-free audit records
   required?
6. What should happen when a tab, popup, window, or MCP connection changes?
7. Does the runtime's telemetry, cloud, licensing, and continuity boundary fit the environment?

The [interactive decision aid](https://sylin.org/ghostlight/decision-aid/) applies the same
operating-model questions. Corrections are welcome through
[GitHub Discussions](https://github.com/sylin-org/ghostlight/discussions) or hello@sylin.org.
