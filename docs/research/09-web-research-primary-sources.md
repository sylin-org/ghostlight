# Primary Web-Research Notes (inline)

**Date:** 2026-07-01 · **Method:** direct web search from the main loop (used to recover
coverage when the sub-agent fan-out was rate-limited). Findings below are summaries of search
results; sources linked inline.

---

## Playwright MCP: snapshot-first, not screenshot-first

- Uses **accessibility-tree snapshots** by default (the semantic structure screen readers use),
  not screenshots. Structured text output ~**200-400 tokens per snapshot** vs. thousands for
  DOM/screenshots. Snapshot mode is deterministic; a separate **vision/screenshot mode** does
  coordinate-based clicking.
- **40+ tools** covering navigation, forms, **network mocking**, storage, tracing, video,
  console. `browser_snapshot` captures page state on demand; most tools auto-return a snapshot
  after each action.
- **Save/restore state**: persist authentication/session data to a file and restore into a new
  session; list/get/set/delete individual cookies. (Less relevant for us: we ride the user's
  real session.)
- Sources: [Playwright MCP](https://playwright.dev/mcp/introduction) ·
  [microsoft/playwright-mcp](https://github.com/microsoft/playwright-mcp) ·
  [Snapshots](https://playwright.dev/mcp/snapshots)

## WebMCP / MCP-B: the inverse model (sites expose tools to agents)

- **W3C proposed standard**, a joint **Google + Microsoft** initiative; lets websites declare
  capabilities as structured tools agents call directly in-browser via native APIs.
- Timeline: MCP-B origins early 2025 (Alex Nahas @ Amazon) → Google/Microsoft converged Aug 2025
  → **W3C Web ML Community Group accepted the spec Sept 2025** → first browser implementation
  (flagged) **Feb 2026** → **Chrome 149 origin trial (~June 2026)** → native Chrome/Edge support
  expected H2 2026. MCP-B ships a polyfill for browsers without native support.
- Sources: [webmachinelearning/webmcp](https://github.com/webmachinelearning/webmcp) ·
  [webmcp.link](https://webmcp.link/) · [MCP-B docs](https://docs.mcp-b.ai/)

## MCP Authorization spec (2025-11-25): OAuth 2.1 resource server

- MCP servers are **OAuth 2.1 Resource Servers**; clients implement **Resource Indicators
  (RFC 8707)** to audience-scope tokens (prevents a compromised server reusing a token
  elsewhere: the confused-deputy defense). Servers implement **RFC 9728 Protected Resource
  Metadata** (401 + `WWW-Authenticate` → `resource_metadata` URL).
- Formalized **Step-Up Authorization Flow** for insufficient permissions mid-operation; added a
  Scope Selection Strategy, **Client ID Metadata Documents (CIMD)**, and **Enterprise-Managed
  Authorization** (aka Cross-App Access: get MCP-server tokens from the enterprise IdP, no
  redirect).
- Note: **stdio transports SHOULD NOT follow the OAuth flow**, which validates our stdio boundary
  staying manifest-based, not OAuth.
- Sources: [MCP 2025-11-25 authorization](https://modelcontextprotocol.io/specification/2025-11-25/basic/authorization) ·
  [Auth0 analysis](https://auth0.com/blog/mcp-specs-update-all-about-auth/) ·
  [Aaron Parecki, Nov 2025](https://aaronparecki.com/2025/11/25/1/mcp-authorization-spec-update)

## Claude for Chrome: prompt-injection numbers

- Anthropic published measurable rates: **23.6% attack-success before mitigations → 11.2% with
  defenses** (general); browser-specific attacks (hidden form fields, URL manipulation)
  **35.7% → 0**.
- More recent: browser-agent injection **down to ~1% with Opus 4.5**; **Sonnet 4.6 at 1.29%
  scenario ASR** on Best-of-N browser injection (from 49.36% on Sonnet 4.5). Explicitly *not
  solved*: ~1% is still meaningful risk.
- Sources: [Anthropic: prompt-injection defenses](https://www.anthropic.com/research/prompt-injection-defenses) ·
  [VentureBeat: published failure rates](https://venturebeat.com/security/prompt-injection-measurable-security-metric-one-ai-developer-publishes-numbers)

## MCP token/context bloat: the delight battleground

- Playwright MCP exposes **26+ tools and dumps full accessibility trees on every action**:
  thousands of nodes per click. Vercel's **agent-browser uses a "Snapshot + Refs" system**
  (`@e1`, `@e2` instead of full trees) claiming **~93% less context**.
- Enterprise scale: GitHub + Slack + Sentry (3 servers, ~40 tools) consumed **143k of a 200k
  window (72%)** on tool schemas alone before any user query. MCP uses **~35× more tokens than
  CLI** on the same task; reliability drops **100% → 72%** as complexity grows.
- **Tool Search (GA Feb 2026)** delivers ~85% token reductions; lazy schema loading already in
  Claude Code.
- Sources: [The Context Wars (paddo.dev)](https://paddo.dev/blog/agent-browser-context-efficiency/) ·
  [Reduce MCP token bloat (The New Stack)](https://thenewstack.io/how-to-reduce-mcp-token-bloat/)

## Operator / ChatGPT Atlas: human-in-the-loop pattern

- **Approval before significant actions** (submitting an order, sending an email, financial
  transactions, deleting calendar events).
- **"Watch Mode"** on sensitive sites (email, financial): requires the tab active to watch the
  agent; **pauses if the user navigates away**.
- Multiple safeguards: user confirmations for high-impact actions, refusal patterns, prompt-
  injection monitoring.
- Sources: [Introducing Operator](https://openai.com/index/introducing-operator/) ·
  [ChatGPT agent Help Center](https://help.openai.com/en/articles/11752874-chatgpt-agent) ·
  [Simon Willison: OpenAI CISO on Atlas](https://simonwillison.net/2025/Oct/22/openai-ciso-on-atlas/)

## MV3 service-worker lifecycle (extension resilience)

- **Active `chrome.debugger` sessions now keep the service worker alive** (prevents SW timeout
  during CDP calls): a material improvement for our design.
- SW still terminates after **~30s inactivity**; keepalive strategies: port-based
  (`chrome.runtime.connect()`), alarm-based (min 1-min interval), periodic port cycling
  (~250s). `connectNative()` keeps SW alive only while the port is open.
- Sources: [SW lifecycle](https://developer.chrome.com/docs/extensions/develop/concepts/service-workers/lifecycle) ·
  [Chromium issue 40733525](https://issues.chromium.org/issues/40733525)

## MCP gateways: RBAC + audit patterns (governance context)

- Modern gateways offer **tool-level RBAC** (Admin/Owner/Power-User/Auditor/User roles scoped
  per catalog/server/tool), **immutable audit trails** (identity, tool, params, time, outcome),
  **progressive tool disclosure** (a search tool returns definitions on demand instead of
  loading every schema into context), and **default-deny write controls**.
- Players: Docker MCP Gateway, Cloudflare, TrueFoundry, Obot, MintMCP, Lasso, Pomerium. SOC2 is
  the common compliance frame.
- Sources: [Cloudflare enterprise MCP](https://blog.cloudflare.com/enterprise-mcp/) ·
  [Docker MCP Gateway](https://docs.docker.com/ai/mcp-catalog-and-toolkit/mcp-gateway/) ·
  [Obot: 13 best gateways](https://obot.ai/blog/the-13-best-mcp-gateways-for-enterprise-teams/)
