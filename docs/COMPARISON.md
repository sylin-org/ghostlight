# How Ghostlight compares

Updated 2026-07-07 from the post-evaluation landscape sweep
([docs/research/14](research/14-post-evaluation-2026-07.md); the original study is
[docs/research/13](research/13-competitive-landscape.md)). The honest summary: "an extension
that drives your real, logged-in Chrome from any MCP client" is a crowded idea, and the
strongest version of it now ships first-party from Anthropic. The combination that stays
uncontested is that model PLUS a fused governance layer, open and local-first. This page is a
decision guide, not a scorecard: several of the projects below are excellent, and if one fits
your case better, use it.

The four properties, together, are the product:

1. Automates YOUR authenticated session (real cookies, real SSO, real tabs) via a thin
   extension -- never a fresh profile, a profile copy, or a cloud browser.
2. Client-agnostic MCP server: Claude Code, Cursor, VS Code, anything.
3. Governance fused in: capability classification per action, identity-bound host
   grants, sacred never-touch domains, observe/enforce modes, structured audit -- with
   all-open as a first-class default.
4. Open and local-first: a Rust service with thin relays; the governance module's source is
   readable.

## The first-party path: Claude Code + Claude in Chrome

Anthropic's own integration (`claude --chrome`) connects Claude Code to the official Claude in
Chrome extension: real logged-in session, native messaging, site-level permissions, and
read-vs-write gating of browser calls in plan mode. It is well built, and its permission
design independently converges on the same read/write/action vocabulary Ghostlight formalizes
as RAWX -- which we take as validation, not competition.

**Use the first-party path when** you are on a direct Anthropic plan (Pro/Max/Team/
Enterprise), Claude Code is your only agent, and per-site permissions in the extension are
governance enough.

**Use Ghostlight when** any of these are true: your agent is not Claude Code (Cursor, Zed,
Cline, or anything MCP); your Claude access runs through Bedrock, Vertex, or Foundry (the
first-party path requires a direct plan); you need a structured audit trail of what the agent
did; you want policy as code (grants, capability floors, simulate/shadow/enforce, org locks)
rather than a site list; or you need the whole thing self-hosted and inspectable. The trained
tool schemas are preserved verbatim, so a Claude agent behaves identically on either path.

## Against the closest neighbors

**hangwin/mcp-chrome** (~12k stars, MIT) -- the closest architectural twin: extension +
native messaging, model-agnostic. No access control, no capability classification, no
domain limits, no audit; development has been quiet since January 2026. Its privacy story is
"it runs locally"; Ghostlight's is "it runs locally, and here is the policy engine, the
denial ids, and the audit trail your security team asked for."

**Microsoft Playwright MCP, extension mode** (~35k stars) -- the best-funded project
on the automation axis; its `--extension` mode reuses a real logged-in tab, and it ships
steadily. Node-based, no governance layer, and browser automation is a side feature of a
testing tool. Ghostlight is purpose-built for the governed-agent case: a native Rust runtime with
no Node service, policy and audit at the dispatch chokepoint.

**vercel-labs/agent-browser** (~38k stars) -- a single Rust binary over CDP with domain
allowlists and action policies (governance-lite). But it copies your Chrome profile to a
temporary snapshot: a fresh browser, not your live session, with no extension, no identity
layer, and no audit. Good for sandboxed tasks; not for "act as me in my real tabs, governed."

**browsermcp.io ("Browser MCP", ~7k stars)** -- extension-driven real session, but
unmaintained (last push April 2025) and the extension itself is closed source; only the npm
server is open.

**Google chrome-devtools-mcp** (~46k stars) -- debugging and inspection altitude; can
attach to a running Chrome but defaults to a dedicated profile, and its own docs warn it
exposes all browser data to the client. Different job.

**browser-use** (~103k stars) -- the biggest OSS "make the browser do things" framework,
now with a Rust-backed core agent (0.13.0). Drives its own Playwright browser by default
(real-profile and extension modes exist), and its enterprise controls live in the paid
cloud, not the local server. The one to watch: if it ships local governance it becomes the
most credible neighbor.

**Generic agent-governance layers** (Microsoft Agent Governance Toolkit, Lasso, ToolHive,
MintMCP, and others) -- real policy and audit for ANY agent or MCP tool call, as a gateway or
runtime. We see these as allies that grow the category, not rivals: they set a shared
vocabulary (OWASP agentic risks, policy-as-code) that Ghostlight meets -- see the RAWX
mapping in [open-spec/](../open-spec/). What a generic layer cannot do is make
browser-semantic decisions: it sees `computer(left_click)` as an opaque call, while fused
governance classifies it by intrinsic capability, binds grants to the tab's actual host at
decision time, and filters the advertised tool set. A gateway composes fine in front of
Ghostlight if you already run one.

**Enterprise browsers** (Island, LayerX, Prisma Access Browser, and others) -- they govern
and audit agent activity inside the browser, credibly. They are closed SaaS, several replace
your browser outright, and they oversee agents rather than expose an automation API to your
own MCP client. Different deployment universe (and price class).

## The grid

| | Real session | Any MCP client | Governance + audit | Open + local runtime |
|---|---|---|---|---|
| Ghostlight | yes | yes | yes | yes |
| Claude Code + Claude in Chrome | yes | no (Claude only) | site permissions, no audit | no (closed) |
| mcp-chrome | yes | yes | no | no (Node) |
| Playwright MCP (ext. mode) | yes | yes | no | no (Node) |
| agent-browser | no (profile copy) | yes | partial | yes (Rust) |
| browser-use | opt-in | yes | cloud-only | no (framework) |
| Generic governance layers | n/a (proxy/runtime) | yes | yes (opaque calls) | varies |
| Enterprise browsers | yes | no | yes | no (closed SaaS) |

Star counts and activity are as of 2026-07-07 and will drift; the research notes carry the
sources. Corrections welcome: hello@sylin.org.
