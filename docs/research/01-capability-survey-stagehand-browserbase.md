# Capability Survey: Browserbase Stagehand, MCP Server, and Platform

**Date:** 2026-07-01 · **Track:** Capabilities · **Source:** research agent (verbatim report)

> Competitive/prior-art report on the semantic-primitive frontier: `act`/`extract`/`observe`,
> self-healing action caching, live view, persistent contexts.

---

## 1. Browserbase Stagehand: the primitives library

**Repo:** github.com/browserbase/stagehand. **~22k GitHub stars**, ~700k+ weekly npm
downloads, MIT license. TypeScript (`@browserbasehq/stagehand`) + Python (`stagehand` module).
Tagline: "The SDK for Browser Agents."

**Execution model.** Playwright-based: Stagehand extends Playwright's `Page` class and adds AI
APIs on top. It can launch via `playwright-core`, `puppeteer-core`, or `patchright-core`. Runs
in two environments: `LOCAL` (any Chromium on your machine) or `BROWSERBASE` (cloud). It does
**not** attach to your everyday logged-in browser profile the way a native-messaging/CDP
extension does. It drives either a locally-launched Chromium or a fresh cloud session.
Persistent auth on the cloud side comes from Browserbase Contexts, not from your real browser
session.

**The four primitives:**

- **`act(instruction)`**: Executes a single natural-language action ("click the login button").
  At runtime the LLM resolves the instruction to a concrete DOM action instead of a hardcoded
  CSS selector, so scripts "survive page redesigns without maintenance."
- **`extract(instruction, schema)`**: Pulls **structured, typed data** validated against a
  **Zod schema** (TS) / Pydantic-style schema (Python). Does not hand the model raw HTML; instead it hands
  a "structured, schema-validated projection of the page, with hidden text stripped, off-screen
  elements de-prioritized, and known injection patterns flagged." Supports iframe/shadow-DOM
  extraction via Deep Locators.
- **`observe(instruction)`**: Returns a **list of actionable/suggested elements** (with
  candidate selectors/actions) *before* you commit to acting. Feeds `act()` for a deterministic
  two-step flow (observe → cache → act).
- **`agent(...)`**: Autonomous **multi-step** workflow execution. Backed by **Computer-Use
  Agent (CUA)** providers: Claude and OpenAI computer-use models that take screenshots and
  reason visually. Multiple tool modes: **DOM, Hybrid, and CUA**.

**Element targeting + self-healing.** Resolves instructions semantically via the LLM; uses
**Deep Locators** traversing iframes and shadow roots (open and closed). "Self-healing" works
through **intelligent cache invalidation**: it caches action results locally (**ActCache**),
replays them deterministically without LLM tokens on subsequent runs, and when it "detects when
a website changes" invalidates affected entries and re-invokes the AI. Core value prop:
"converts AI-driven workflows into deterministic scripts" that skip LLM calls unless the page
structurally changes. (Known open issue #1767: server-side caching not working for
extract/act/observe in some cases; caching is primarily a local/self-hosted feature.)

## 2. Browserbase MCP Server: the MCP wrapper

**Repo:** github.com/browserbase/mcp-server-browserbase. **~3.4k stars**. Package
`@browserbasehq/mcp`. Powered by Stagehand v3.0 (20-40% faster act/extract/observe via caching).

**Tool surface (6 tools, verbatim):**

| Tool | Description | Params |
|---|---|---|
| `start` | "Create a new Browserbase session, or attach to an existing Browserbase session" | none |
| `end` | "Close the current Browserbase session" | none |
| `navigate` | "Navigate to a URL" | `{ url }` |
| `act` | "Perform an action on the page" (natural language) | `{ action }` |
| `observe` | "Observe actionable elements on the page" | `{ instruction }` |
| `extract` | "Extract data from the page" | `{ instruction? }` |

The advertised tool names are the short forms (not `stagehand_`-prefixed). **No dedicated
`screenshot` tool** in the current list.

**Deployment modes:** Hosted (Streamable-HTTP at `https://mcp.browserbase.com/mcp`, Bearer
auth); Local (STDIO via `npx @browserbasehq/mcp`, default model **Google Gemini 2.5
Flash-Lite**). Config flags: `--proxies`, `--verified`, `--keepAlive`, `--contextId`,
`--persist` (default true), `--browserWidth/Height`, `--modelName/--modelApiKey`,
`--experimental`. Every tool call drives a Browserbase cloud session (or attaches via `start`);
not a local-real-browser extension.

## 3. Browserbase: the cloud platform

browserbase.com: cloud headless-browser infra for AI agents. Pricing: Free $0, Dev $20/mo,
Startup $99/mo, custom Scale/Enterprise. Spins up **fresh cloud Chromium sessions** (not your
local logged-in browser); persistent auth via the **Contexts API**.

- **Stealth:** Verified/Advanced Stealth (real browser fingerprints Cloudflare etc. recognize);
  managed **CAPTCHA solving** (default on); residential + datacenter **proxy** super-network.
- **Agent Auth ladder:** API keys → Credential vaults (1Password) → Signed agents (Cloudflare
  Web Bot Auth) → Human-verified agents (AgentKit/x402 proof-of-humanity).
- **Contexts API:** stores the Chromium user-data dir across sessions (cookies, localStorage,
  IndexedDB, service workers), encrypted at rest; log in once, reuse contextId.
- **Debugging/observability:** **Live View** (embeddable iFrame with optional user takeover),
  **Session Replay** with full command logging, prompt/action observability.
- **Ecosystem:** Director (NL → Stagehand scripts), Functions, file up/download, custom
  extensions, CUA integration (Claude + OpenAI operator).

## 4. Capability gaps vs. the proposed 13-tool set

Our tool set is a low-level primitive/CDP surface in the Claude-in-Chrome tradition. What
Stagehand/Browserbase have that it structurally lacks:

**A. Semantic/AI primitives (biggest gap):**
- **No `extract`-equivalent**: no schema-driven structured extraction. We can only return raw
  text (`get_page_text`) or the a11y tree (`read_page`); the agent must do all parsing in-context.
- **No `act`-equivalent**: no NL single-action resolver; we require the agent to compute
  coordinates/selectors itself.
- **No `observe`-equivalent**: no "return scored actionable elements before acting." `find` is
  the closest but returns matches, not actionable suggestions to pipe into a cached act.

**B. Self-healing + action caching**: no cache that records resolved actions and replays them
deterministically with automatic re-resolution when the DOM changes.

**C. Autonomous `agent()`/CUA loop**: no built-in multi-step executor; orchestration is on the
calling agent (`update_plan` is only a planning note).

**D. Infra/anti-bot**: no stealth, CAPTCHA solving, proxies, credential vaults. **By design**,
we govern the user's real authenticated browser rather than evade bot protection (a governance
advantage, not a gap).

**E. Persistent cloud auth contexts, live view, session replay**: we have structured audit but
not an embeddable live-view iFrame or visual replay timeline; our persistence model is "the
user's own session," not a reusable Context.

**Net positioning:** Stagehand/Browserbase optimize for *cloud, stealthy, resilient, schema-
structured autonomous automation*. Our project optimizes for *governed, audited, identity-bound
access to the user's own real authenticated browser*. The two most defensible gaps worth
considering: (1) a **schema-driven `extract` primitive**, (2) an **`observe`/`act` self-healing
layer**. Both reduce the calling agent's context burden and brittleness, and neither conflicts
with the governance-first architecture.

## Sources
- https://github.com/browserbase/stagehand · https://stagehand.dev/ ·
  https://deepwiki.com/browserbase/stagehand · https://www.browserbase.com/stagehand
- https://github.com/browserbase/mcp-server-browserbase ·
  https://docs.stagehand.dev/v3/integrations/mcp/setup
- https://docs.browserbase.com/features/stealth-mode ·
  https://docs.browserbase.com/features/contexts ·
  https://www.browserbase.com/blog/ai-web-agent-sdk
