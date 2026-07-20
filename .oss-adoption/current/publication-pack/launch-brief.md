Status: complete
Run ID: 20260718T214944Z
Project: Ghostlight
Created: 2026-07-18T21:49:44Z

# Launch brief

## Headline and one-sentence positioning

Candidate headline:

> Your real browser, for any MCP agent - visible, local, and yours.

Positioning:

> Ghostlight lets the MCP client you already use work in your existing logged-in Chromium browser.
> The work stays local and visible, and organizations can add inspectable policy and audit when
> they need it.

## Audience, trigger problem, and fit

Primary audience: MCP users whose client lacks a capable real-browser integration or whose browser
tool is tied to a different client.

Trigger: the agent needs cookies, SSO, or authenticated application state already present in the
user's browser, and the user wants to see and remain responsible for the work.

Good fit: visible browser work, existing Chromium session, multiple MCP clients, local-only runtime,
compact model tools, and optional local governance.

Bad fit: headless tests, scraping farms, stealth, isolated profiles, remote cloud browsers, bulk
parallel sessions, or a Claude-only workflow already fully served by Anthropic.

## Proof and evidence links

- Hero and product truth: https://github.com/sylin-org/ghostlight
- Install: https://sylin.org/ghostlight/install.md
- Safe demo: https://sylin.org/ghostlight/demo/brief/
- Browser-control decision aid: https://sylin.org/ghostlight/decision-aid/
- Latest release: https://github.com/sylin-org/ghostlight/releases/latest
- Trust Center: https://github.com/sylin-org/ghostlight/tree/main/docs/trust
- Candid comparison: https://github.com/sylin-org/ghostlight/blob/main/docs/COMPARISON.md

## Maturity, limitations, and non-claims

- Pre-1.0 at v0.6.0.
- Chromium only for v1; Firefox and multi-browser adapters are v2 research.
- The Chrome Web Store listing is still under review.
- macOS live-browser verification remains owed.
- Governance constrains capability and destination; it does not infer semantic user intent or
  eliminate in-domain prompt injection.
- The tab group is visible organization, not a security sandbox.
- No SOC 2/ISO certification, completed third-party penetration test, bug bounty, or maintainer
  team exists today.

## Demo and first-success path

```text
npx -y ghostlight install
```

Complete the current extension walkthrough, restart the MCP client, then ask:

> In my current browser, summarize the active page and tell me which tab you used. Do not click or
> change anything.

If the chain is incomplete:

```text
npx -y ghostlight doctor
```

The anchor launch must wait until this works through the accepted store package on clean Windows
and Linux machines.

## Likely questions and response owner

| Question | Short answer | Evidence owner |
| --- | --- | --- |
| Why not Playwright or agent-browser? | They are excellent for testing and owned automation sessions; Ghostlight is for the user's visible authenticated context plus optional fused governance. | Owner, comparison guide |
| Why no Ghostlight account? | No vendor service is in the runtime path. The service and extension pair as the local OS user. | Owner, architecture and data flows |
| Can it touch ordinary tabs? | Ghostlight works in its managed tab surface; the tab group shows scope but does not create authority. | Owner, ADR-0066 and tests |
| Is it actually free? | The automation core is Apache-2.0 OR MIT. Organization governance has separate licensing terms. | Owner, LICENSE/PRICING |
| Does governance stop prompt injection? | No. It constrains host and capability and records actions; semantic intent belongs with the client and user. | Owner, SECURITY |
| Why an extension? | It reaches the user's existing Chromium session and renders truthful visible feedback. | Owner, architecture |

The solo owner should be present for the active launch window. No outreach is authorized by this
brief.
