# Post-evaluation: where Ghostlight stands, 2026-07-07

Date: 2026-07-07. Method: tree-state capture (git history, ADR index, batch ledgers, releases)
plus a fresh cited sweep of every player tracked in docs/research/13 (GitHub API stats and release
notes for the open projects, web sources for the closed ones), plus a scan for entrants and
standards movement the original study did not cover. This is the follow-up to
docs/research/13-competitive-landscape.md (the pre-rename landscape study) and
docs/research/12-official-extension-parity.md (the official-extension baseline).

## Bottom line

The uncontested intersection from research 13 still holds, and it has actually widened on the
open-source side: the closest architectural twins went stale while Ghostlight shipped. But two
things changed shape:

1. **Anthropic now ships the first-party version of our core use case.** Claude Code integrates
   directly with the official Claude in Chrome extension (`claude --chrome`), with site
   permissions, read-vs-write gating in plan mode, and a `browser_batch` composition tool. It is
   single-vendor (direct Anthropic plans only, explicitly not Bedrock/Vertex/Foundry), has no
   audit trail, and no policy-as-code. This both validates the thesis and absorbs the easiest
   slice of the market.
2. **Governance stopped being a whitespace and became a category.** Microsoft released an
   open-source Agent Governance Toolkit (MIT, ~4.7k stars in four months, very active) with a
   policy engine, an MCP security gateway, and OWASP-agentic-top-10 framing. It is generic (no
   browser surface), but it sets the vocabulary enterprises will use to evaluate us.

The defensible position is unchanged but sharper: **the only open, vendor-neutral, self-hostable
fusion of real-session browser automation and governance, with audit as the spine.** The clock
matters more than it did: the window where "Camp A is stale and Camp B has no browser" is a
distribution opportunity, not a permanent fact.

## Part 1: what Ghostlight is now (state capture)

As of dev @ 656259c (2026-07-07), tagged v0.2.0 on main (2026-07-05):

- **Surface:** 17 tools. The 13 trained schemas (reference shape per ADR-0034 D7), `explain`
  (ADR-0022), plus the composition layer: `script` (sequential multi-tool composition with
  `$prev`/`$N` references, dry-run, budgets; ADR-0035), `form_fill` (semantic form interaction by
  label, one Write decision per form; ADR-0036), `wait_for` (condition + adaptive settle
  detector; ADR-0037). `read_page` gained diff mode; mutating actions return consequence
  digests; results carry `structuredContent` + declared `outputSchema` (ADR-0038).
- **Architecture:** Ghostlight Hub (ADR-0030, H0-H9 complete): a persistent per-user service owns
  the one Chrome link; MCP clients are multiplexed adapter sessions; local web API + Console;
  Windows Task Scheduler and Linux systemd user-service integration; inbound/outbound/manage zones
  (ADR-0033); ICapability/ITransport registry with capability manifest at handshake
  (ADR-0034).
- **Governance:** the full overlay is live: RAWX capability classification (read/action/write/
  execute) with per-action requirements and host polarity (ADR-0022), identity-bound grants,
  sacred never-touch domains, observe/shadow/enforce modes, take-the-wheel + panic switch,
  structured audit with orchestration keys (orchestrator, batch_id, step, dry_run), layered
  config with org locks, manifest hot-reload, `explain`, policy CLIs. All-open stays first-class.
- **Onboarding contract:** initialize.instructions workflow preamble, per-tool examples,
  corrective validation errors (ADR-0031); cost-aware coaching in the capability guide
  (ADR-0038 D5).
- **Business posture:** open-core (permissive engine + source-available governance module,
  ADR-0027), tripwire licensing with the Continuity Promise (ADR-0028), public GTM plan drafted.
- **In flight / owed:** ADR-0039 (saved scripts as governed artifacts) and ADR-0040 (pipeline
  idempotency gate) are Proposed, not implemented. The composition batch's LIVE-VERIFY.md (13
  pinned observations) has not been run. v0.2.0 is tagged but has no published GitHub release
  (v0.1.0 is still "Latest"). The Chrome Web Store listing is drafted but not submitted
  (screenshots + privacy tab owed). Linux live verification, e2e-smoke unquarantine, and
  the commercial-license legal skim remain open.

Versus the world research 13 described, Ghostlight closed the gaps that study implied: it renamed
(Ghostlight, ADR-0021), led with governance, shipped a real release, and added the composition
layer no Camp A competitor has (script/form_fill/wait_for with governance-aware dry-run and
audit-correlated batches).

## Part 2: how the landscape moved

### The tracked open projects (GitHub API, 2026-07-07)

| Project | Research 13 | Now | Movement |
|---|---|---|---|
| hangwin/mcp-chrome | ~12k stars, "active" | 12,058 stars, last push 2026-01-06 | **Stale six months.** The closest architectural twin stopped moving. 215 open issues. |
| browsermcp.io (BrowserMCP/mcp) | ~6.8k, stale | 6,777, last push 2025-04-24 | Still dead. The namesake risk research 13 flagged is now moot post-rename. |
| Microsoft playwright-mcp | ~34.6k | 34,801, active (v0.0.76, 2026-06-10) | Steady shipping: output-size caps, argument validation, video action-annotation overlays. Still zero governance surface. Extension mode unchanged in kind. |
| ChromeDevTools/chrome-devtools-mcp | ~44.9k | 46,196, very active (v1.5.0, 2026-07-03) | Fastest-growing adjacent. Still debug altitude, still warns it exposes all browser data. |
| vercel-labs/agent-browser | ~37.7k | 38,011, active (v0.31.1, 2026-06-26; 554 open issues) | Still profile-snapshot (not live session), governance-lite allowlists, no audit/identity. |
| browser-use | ~102k | 103,289, very active (0.13.3, 2026-07-01) | **Rebuilt its core agent in Rust** (0.13.0 "Rebuilt in Rust [beta]", 2026-06-08, shipped as a `browser-use-core` binary behind the Python SDK). Governance still lives in the paid cloud. |
| nanobrowser | ~13.4k | 13,436, last push 2025-11-24 | Stale. |
| ofershap/real-browser-mcp | ~35 | 37, active but tiny | No change in kind. |
| stacklok/toolhive | (Camp B) | 1,928, very active | Still a generic MCP gateway, no browser surface. |

Two readings. First, **Camp A is consolidating, not growing**: of the extension-on-real-session
twins, only the giant-backed ones (Playwright MCP, chrome-devtools-mcp) still move; the
independent twins (mcp-chrome, browsermcp.io, nanobrowser) are stale or dead. Second, **the Rust
single-binary secondary differentiator is eroding**: browser-use now ships a Rust core, and
agent-browser always was Rust. Rust is now a credibility marker, not a moat. Governance remains
the moat.

### Anthropic: the origin closes the loop (new since research 13)

- **Claude in Chrome** left research preview: beta for all paid plans since December 2025, with
  admin controls (org-level enable/disable, site allowlists/blocklists) in beta for Team and
  Enterprise plans. Per-user permission modes: "ask before acting" vs "act without asking".
- **Claude Code now drives Chrome first-party** (`claude --chrome`, or ambient in the VS Code
  extension): same native-messaging shape as ours, the official extension as the executor,
  login-state sharing, pause-on-login/CAPTCHA. Notable convergences with our design vocabulary:
  plan mode gates browser calls by read vs state-changing classification (their in-product
  echo of RAWX's first axis); a `browser_batch` tool runs promptless only when every inner
  action is read-only (their echo of `script` + capability floors); flags like `save_to_disk`
  and `createIfEmpty` escalate an otherwise-read call (their echo of per-action classification).
  Also new on the official surface: GIF session recording and scheduled tasks.
- **What it does not have:** any MCP surface for other clients; availability on Bedrock, Vertex,
  or Foundry (explicitly unsupported -- enterprise shops that consume Claude only through a cloud
  provider cannot use it without a separate claude.ai account); structured audit; policy-as-code,
  simulate/shadow, or explain; self-hosting.

This is the most consequential delta. The "developer on a direct Anthropic plan who wants Claude
Code to drive their logged-in Chrome" persona is now served in-box. Ghostlight's buyer is
everyone that first-party path excludes: other MCP clients (Cursor, Zed, Cline), cloud-provider
Claude deployments, mixed-model shops, and any org that needs audit and policy-as-code rather
than a site list in a SaaS admin panel.

### Governance became a category (new since research 13)

- **Microsoft Agent Governance Toolkit** (github.com/microsoft/agent-governance-toolkit, created
  2026-03-02, MIT, 4,670 stars, pushed daily): runtime policy enforcement for agents (YAML/OPA
  Rego/Cedar policies, sub-millisecond decisions), an MCP security gateway, identity and trust
  scoring, execution sandboxing, and explicit mapping to all 10 OWASP agentic risks. Generic:
  no browser of its own, so it is Camp B, but an open-source, vendor-blessed Camp B that will
  shape enterprise evaluation checklists.
- The regulatory tailwind is now concrete: EU AI Act high-risk obligations take effect August
  2026; a March 2026 EY/AIUC-1 survey found only 38% of organizations monitor AI traffic
  end-to-end across prompts, tool calls, and outputs. "Structured audit of what the agent did in
  the browser" is a compliance line item now, not a nice-to-have.

### The agentic browser wave and its security reckoning (context shift)

- Consumer agentic browsers went mainstream-ish: OpenAI Atlas (Oct 2025, ~10-15M MAU claimed),
  Perplexity Comet (cross-platform + enterprise since March 2026), Gemini in Chrome auto-browse
  (shipping at OS level on Pixel 10 / Galaxy S26). Combined share still ~1-3% of the browser
  market.
- A University of Washington study (July 2026) found four of seven agentic browsers tested,
  including ChatGPT Atlas, Chrome with Gemini, Perplexity Comet, **and Claude for Chrome**,
  create same-origin-policy bypass conditions; a proof-of-concept exfiltrated data cross-origin
  on Atlas. Prompt injection and memory poisoning are the named mechanisms.
- This is the strongest external validation yet of the north-star framing: an unconstrained
  agent in a real session is a blast-radius problem, and the mitigation people can actually
  evaluate is a governance layer with origin-aware rules and audit.

### Standards movement

- **MCP spec 2026-07-28** (release candidate locked 2026-05-21, final ships in three weeks):
  stateless protocol core (protocol-level sessions and `Mcp-Session-Id` removed from Streamable
  HTTP), Multi Round-Trip Requests replacing server-initiated sampling/elicitation, a Tasks
  primitive for long-running work, an Extensions framework, authorization hardening (six OAuth
  alignment SEPs), and a formal 12-month deprecation policy. Ghostlight hand-rolls MCP over
  stdio, which insulates most of it, but the hub's web adapter and any future Streamable HTTP
  ambitions must be built against the stateless model, and `script`/saved-scripts map naturally
  onto Tasks.
- **WebMCP** graduated from flag to public origin trial (Chrome 149 through 156); the API moved
  from `navigator.modelContext` to `document.modelContext` in Chrome 150. Today only Gemini in
  Chrome consumes site-declared tools; the mainstream agents still scrape and screenshot. Still
  a watch item, but it now has a shipping vehicle and a timeline.

## Part 3: positioning verdict

Re-testing research 13's four-way intersection (real-session thin-extension automation +
client-agnostic MCP + built-in governance + open/local/single-binary):

- **Still uncontested.** No project found combines all four. The nearest miss is still Claude in
  Chrome (real session + governance, but single-vendor, closed, no MCP surface), and it got
  nearer by wiring into Claude Code.
- **Camp A got weaker** (independent twins stale) **and stronger** (the giants ship faster). The
  competition for "automation that works" is now Microsoft, Google, and Anthropic; competing on
  automation alone was always losing ground, and that is truer now.
- **Camp B got a heavyweight** (Microsoft AGT), which raises the bar for what "governance" must
  mean to an enterprise evaluator (policy language, OWASP mapping, identity story) but also
  educates the market Ghostlight sells into. A generic gateway still cannot make browser-semantic
  decisions (it sees `form_input` as an opaque tool call; Ghostlight sees a Write against a
  host with polarity and a grant).
- **New axis where Ghostlight is ahead of everyone:** the governed composition layer. Nobody
  else has script-with-dry-run, semantic form filling as a single governed Write, settle
  detection, consequence digests, and audit-correlated batches on a real session. The official
  extension's `browser_batch` is the closest and is read-only-gated, not governed.
- **New axis where nobody is credible yet, including us:** origin-flow governance. The UW
  findings define the attack class (cross-origin data movement by an injected agent); no
  shipping product governs it. Ghostlight's per-call domain grants are adjacent but do not
  track data provenance across calls.

Positioning sentence that survives this re-test: "Ghostlight is the governed way to let any
agent use your real browser: open, self-hostable, audited, and vendor-neutral." Lead with
governance and audit; treat Rust and single-binary as credibility, not headline.

## Part 4: proposals and gap closures

Ranked by leverage. Effort: S (hours), M (days), L (a batch).

**P1. Ship the distribution that is already built (S).** Publish the v0.2.0 GitHub release (the
tag exists; v0.1.0 is still shown as Latest), and finish the Chrome Web Store submission
(screenshots + privacy tab). Camp A's independent twins are stale right now; every week the CWS
listing is unpublished is free mindshare left on the table. This is the highest
leverage-per-hour item on the list.

**P2. Publish the comparison positioning against first-party Claude Code + Chrome (S/M).** A
README section and a short doc: when to use the first-party path (direct Anthropic plan, one
client, no audit needs) and when Ghostlight (Cursor/Zed/Cline or any MCP client, Bedrock/Vertex/
Foundry deployments, audit + policy-as-code + self-hosting, org rollout without a SaaS admin
panel). Honest, specific, and it inoculates against the obvious "doesn't Claude do this now?"
objection every prospect will raise.

**P3. Map RAWX + grants + sacred domains to the OWASP agentic top 10 and the UW findings (M).**
One document ("what a governed session actually prevents") that walks the injection blast-radius
story: host polarity, write floors, sacred domains, take-the-wheel, audit. Microsoft AGT made
OWASP-agentic the evaluation vocabulary; meeting it on that field costs a doc, not a feature.
Cite the UW study; note honestly which findings governance mitigates (action gating, domain
confinement, audit) and which it does not (in-context prompt injection itself).

**P4. MCP 2026-07-28 currency audit (M).** The final spec ships in three weeks. Audit the
hand-rolled MCP layer against the RC: confirm stdio behavior is unaffected, make sure the hub
web adapter's session model does not assume protocol-level sessions, check
structuredContent/outputSchema conformance (already shipped per ADR-0038, verify field-level
agreement), and record an ADR stance on Tasks (natural fit for `script` and ADR-0039 saved
scripts) and on the Extensions framework. Deliverable: a spec-currency note + any small fixes,
not a rewrite.

**P5. Ratify and land ADR-0039 saved scripts (L).** The landscape shifted toward repeatable,
schedulable agent tasks (official extension scheduling, Comet/Atlas task automation). Named,
parameterized, hash-bound approved workflows are the governed answer to that trend and deepen
the composition lead nobody else has. ADR-0040 (idempotency gate) rides along per its batch.

**P6. ADR the origin-flow governance direction (M for the ADR; L to build).** The genuinely new
whitespace: track data provenance across calls (page content read from host A feeding a write on
host B) and let policy express cross-origin flow rules; even audit-only visibility of
cross-origin flows would be unique in the market. Start with an ADR + an audit-field design, not
enforcement. This is the feature that turns the UW headline into a Ghostlight demo.

**P7. Re-baseline against the official extension (S/M).** Research 12 pinned v1.0.78. The
official surface has since added `browser_batch`, GIF recording, scheduled tasks, and
escalation-relevant flags (`save_to_disk`, `createIfEmpty`, `clear`). Re-run the parity harvest:
confirm no trained-tool drift (fidelity snapshot should catch it), and decide deliberately which
new official behaviors to harvest (GIF recording is a strong delight candidate; scheduling folds
into ADR-0039).

**P8. Enterprise proof pack (M).** Admin controls upstream are "beta"; EU AI Act high-risk
obligations land August 2026; 38% end-to-end monitoring is the stat to quote. Package the
org-policy quickstart (Intune/GPO push, org locks, observe-then-enforce rollout) plus a
one-page audit/compliance story. Mostly assembling what exists.

**P9. WebMCP stance ADR (S).** Upgrade the watch item to a recorded position: Ghostlight as a
*governed WebMCP consumer* (site-declared tools surfaced through the capability registry, gated
by the same grants) is a coherent future that no one else can offer, because consuming WebMCP
safely is exactly a governance problem. No implementation now; Chrome 150 already renamed the
API once, it is too early to build against.

**P10. Close the standing verification debts (S/M).** Run composition LIVE-VERIFY.md (13
observations), Linux live verification, e2e-smoke unquarantine, commercial-license legal
skim. None are landscape-driven; all of them gate the credibility of P1/P8 claims.

## Outcome (added 2026-07-07)

The owner reviewed this report on 2026-07-07 and accepted all ten proposals, with three
rulings on the deltas: convert the vocabulary validation into capability onboarding; meet
generic governance players with alternatives and standards, not competition; and make
origin-flow governance the focus. The response is recorded in ADR-0041 (dispositions for
P1-P10), ADR-0042 (origin-flow provenance), and ADR-0043 (WebMCP stance); ADR-0039 was
ratified. Executable work lives in docs/tasks/landscape-1/; operator items moved to
docs/business/FOUNDER-TODO.md.

## Key sources

- Claude Code + Chrome: https://code.claude.com/docs/en/chrome
- Claude in Chrome admin controls: https://support.claude.com/en/articles/13065128-claude-in-chrome-admin-controls
- Claude in Chrome permissions: https://support.claude.com/en/articles/12902446-claude-in-chrome-permissions-guide
- Microsoft Agent Governance Toolkit: https://github.com/microsoft/agent-governance-toolkit and https://opensource.microsoft.com/blog/2026/04/02/introducing-the-agent-governance-toolkit-open-source-runtime-security-for-ai-agents/
- browser-use 0.13.0 (Rust core): https://github.com/browser-use/browser-use/releases/tag/0.13.0
- Playwright MCP v0.0.76: https://github.com/microsoft/playwright-mcp/releases/tag/v0.0.76
- chrome-devtools-mcp v1.5.0: https://github.com/ChromeDevTools/chrome-devtools-mcp/releases
- MCP 2026-07-28 release candidate: https://blog.modelcontextprotocol.io/posts/2026-07-28-release-candidate/
- WebMCP origin trial: https://developer.chrome.com/blog/ai-webmcp-origin-trial and https://developer.chrome.com/docs/ai/webmcp
- UW agentic-browser study coverage: https://www.technology.org/2026/07/03/some-agentic-ai-browsers-come-with-major-cybersecurity-risks-uw-study-finds/
- Agentic traffic share: https://www.humansecurity.com/learn/blog/state-of-agentic-traffic-april-26/
- EU AI Act / monitoring stat context: https://obot.ai/blog/ai-governance-trends-2026/
- Repo stats: GitHub API, retrieved 2026-07-07.
