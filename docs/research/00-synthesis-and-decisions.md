# Synthesis & Decisions

**Date:** 2026-07-01 · **Status:** discovery complete, fork decisions pending

> **Framing correction (project owner, 2026-07-01): read [NORTH-STAR.md](NORTH-STAR.md) first.**
> The emphasis below over-indexes on enterprise governance and in places conflates the *engine*
> with the *governance overlay*. NORTH-STAR governs. In particular: (1) the **engine is
> unconstrained**; governance is a separable **overlay**, and **"all-open" is a first-class
> mode**, not a degraded one. (2) **Token efficiency is an engine property** (lean refs,
> screenshot discipline) available in all-open mode, *not* a governance side-effect of tool-
> filtering; the "governance as delight" framing in fork #6 is superseded. (3) **The user's real
> browser context is sacred**: anything that moves away from it (cloud/fresh sessions) is
> off-mission. (4) Prior art is a **concern surface, not a feature catalog**: nothing below is a
> recommendation to copy a vendor's paradigm. See NORTH-STAR's "How this re-weights the synthesis"
> table for the engine-vs-overlay split of the eight forks.

This is the cross-cutting read across all eight research reports, organized by the two
premises, then distilled into eight design **forks**: the decisions the discovery surfaced
that change `SPEC.md`. Each fork carries a recommendation and the spec section it touches.

---

## The headline

The field moved in two directions since the spec was drafted, and they point the same way for us:

1. **Semantic primitives + snapshot-refs are displacing screenshot/pixel computer-use.** The
   capability frontier is `act`/`extract`/`observe` (Stagehand) and accessibility-tree
   snapshots with element refs (Playwright MCP, Vercel agent-browser), *not*
   "screenshot → click at (x,y)."
2. **The governance features we're already building double as the two things users most want**:
   lower token cost and prompt-injection safety.

The reframe: our differentiator isn't *only* "enterprise governance." **Governance is the
delight mechanism**: the same manifest that restricts tools also shrinks the context window
and severs the prompt-injection exfiltration path. The current spec sells governance as
compliance; the research says it's also the performance and safety story.

---

## Premise 1: Capabilities (prior-art frontier)

Where the frontier sits vs. our 13 tools (`navigate`, `computer`[screenshot/click/type/key/
scroll/hover/drag/wait], `read_page`, `get_page_text`, `find`, `read_console_messages`,
`read_network_requests`, `tabs_context`, `form_input`, `javascript_tool`, `tabs_create`,
`resize_window`, `update_plan`):

| Frontier capability | Who has it | Do we? | Verdict |
|---|---|---|---|
| Screenshot only on `screenshot`/`scroll` | Official Claude-in-Chrome | ✅ (spec §6.3) | Good instinct: **validated** |
| Accessibility-tree snapshot w/ element **refs** (`@e1`) | Playwright MCP, Vercel agent-browser (claims ~93% less context) | Partial (`read_page`, `find`) | Make refs the lean default, not full-tree dumps |
| Schema-driven **`extract`** → typed/validated data | Stagehand | ❌ | **Biggest single gap**: offloads parsing from the agent's context |
| **`act`/`observe`** NL→action + self-healing action cache | Stagehand (ActCache) | ❌ | Powerful but heavier; v2 candidate |
| Network mocking, save/restore auth state, cookie mgmt | Playwright MCP (40+ tools) | ❌ | Mostly **N/A**: we use the user's *real* session, so persist/restore auth is moot (a quiet advantage) |
| Stealth / CAPTCHA solving / proxies | Browserbase | ❌ | **Deliberately not us**: we govern a real session rather than evade bot protection |

**Central tension:** the "sacred, byte-identical schemas" constraint locks the 13 tools to the
screenshot/computer paradigm the field is moving away from. But the schemas are sacred for
*trained-behavior fidelity*: nothing stops us **adding new governed tools Claude-in-Chrome
never had** (an `extract`, possibly `observe`). Additive tools don't violate the sacred surface.

**On the radar:** WebMCP (W3C, Google+Microsoft) hits a Chrome origin trial mid-2026. It's the
inverse of us (sites expose their *own* tools). Not a competitor; long-term our extension could
*consume* the WebMCP tools a page advertises.

---

## Premise 2: User delight (per persona)

### 🤖 The AI agent: token efficiency is the #1 axis, and it's ours to win
- Tool-schema bloat is a documented crisis: 3 servers / ~40 tools burned **72% of a 200k
  window** before the first query. Our **per-manifest tool filtering means a restricted
  deployment advertises fewer tools → less bloat.** Governance = context savings.
- Full a11y-tree dumps are the other sink; ref-based output is the fix. `read_page` should
  default to lean refs.
- Align with Anthropic's **Tool Search / lazy schema loading** (GA Feb 2026, ~85% reduction).
- Agent-friendly **denials** (spec §5.5 is already good): the denial text is the agent's
  recovery signal.

### 🧑‍💻 The developer: single binary is validated hard, with one landmine
- npx-on-Windows is *fundamentally broken* for MCP stdio (pipes don't connect); a Rust static
  binary erases that whole class of failure.
- **The landmine:** the binary does *not* solve native-messaging host registration +
  extension-ID/`allowed_origins` wiring, and Anthropic's own Claude-in-Chrome ships broken
  here on Windows (claude-code #21426) and has a host-name collision when Desktop+Code coexist
  (#20887). Currently a Phase-6 afterthought.
- **Delight opportunity:** a self-registering installer that writes the registry key/manifest
  with absolute paths *and* auto-discovers + injects the unpacked extension ID into
  `allowed_origins`. "Zero-runtime binary + self-registering native-messaging installer" is
  genuinely novel against the field.
- The real magic: **it just works with my already-logged-in browser**: no re-auth, no separate
  profile.

### 🏢 The enterprise admin: differentiated model; align to emerging standards
- No prior-art tool combines identity-bound + per-domain r/w tier + tool mask + audit for the
  user's *own authenticated browser*. The gap holds up.
- Standards to align with: MCP Authorization spec (2025-11-25) now defines OAuth 2.1
  resource-server, Step-Up Authorization, and Enterprise-Managed Authorization / Cross-App
  Access. Our "manifest = deployment-channel identity" is deliberately simpler (no runtime IdP).
  That is good, but our "request elevated access" denials could later map onto step-up.
- Audit: gateways center on immutable, SIEM-ready trails for SOC2. Adding **OCSF/CEF** output is
  a concrete delighter. Reusable from Lasso: `@register_plugin` auto-discovery for audit
  destinations; a secret-masking regex table for PHI-aware param redaction.

### 👁️ The end user watching: under-weighted in the current spec
- **Prompt injection is the defining browser-agent risk and is not solved:** even Opus 4.5 sits
  at ~1% attack-success; Anthropic's browser-specific mitigations drove hidden-form/URL attacks
  35.7%→0. **Our default-deny domain allowlist already breaks the "exfiltration" leg of the
  lethal trifecta**: an injected "go to evil.com and paste the data" is denied by policy. The
  spec barely mentions this.
- Industry HITL pattern is settled: approval before significant actions + "Watch Mode" on
  sensitive sites (Operator/Atlas pause if you leave the tab).
- **Transparency gap:** our audit is for the SIEM, not the human at the keyboard. A local,
  real-time "agent is controlling this tab" indicator + activity feed is cheap in the extension
  and a big trust win, a *different artifact* from the audit log.

---

## The eight forks

| # | Fork | Recommendation | SPEC section |
|---|---|---|---|
| 1 | **Semantic tools** | Add `extract` (schema-driven) as a *new additive* governed tool in v1; `observe`/`act` self-healing in v2. Doesn't touch the sacred 13. | §3 (taxonomy) |
| 2 | **Snapshot-first posture** | Default to `read_page`/`find` refs (lean, token-cheap); screenshots as fallback, within the sacred schema. | §3.1, §6.3 |
| 3 | **Risk tier + HITL** | Annotate tools with MCP risk hints (`readOnlyHint`/`destructiveHint`) in v1 (near-free); elicitation/MRTR-based approval on high-risk as fast-follow. Complement the client's approval layer, don't duplicate it. | §3.3, §11 |
| 4 | **Self-registering installer** | Promote native-messaging auto-registration from Phase 6 → first-class Phase 1/2. Write registry key/manifest w/ absolute paths; auto-discover + inject extension ID into `allowed_origins`. | §8, Phase plan |
| 5 | **Audit standards** | Keep flat JSON canonical; add **OCSF 6003 (aligned to OWASP AOS)** + **CEF** output modes; default-redact URL query strings (PHI); add hash-chaining for tamper-evidence; model identity as delegation (`act`/`may_act`). | §7 |
| 6 | **Positioning** | Adopt "governance as delight" (token efficiency + injection safety, not just compliance) as first-class framing. | §1 |
| 7 | **Extension trust + enforcement correctness** | Accept residual tamper risk (pin/force-install, document it); enforce on `Page.frameNavigated.securityOrigin`, **per-frame**, with canonicalized hosts (NFKC+IDNA+`inet_aton`); enforce at `Target.createTarget`; opaque-origin deny-by-default. Optional paranoid mode: binary launches Chrome in `--remote-debugging-pipe` for an independent oracle. | §5, §9.2 |
| 8 | **Policy resolution semantics** | **first-match-wins → explicit-deny-wins + most-specific-match**; add a top-level always-wins `deny` block; separate domain-scope from capability-scope; adopt Chrome's `[.]host[:port][/path]` domain grammar + a defined specificity metric. (Our twin agent-browser already did all of this.) | §4.2, §4.3 |

### Recommended v1 vs v2 split

- **Cheap v1 wins:** 1 (`extract` only), 2, 4, 6, 7 (correctness rules), 8 (policy semantics).
  Fork 8 in particular is a *correctness/safety* fix, not a feature: highest priority.
- **Time-box / fast-follow:** 3 (annotations in v1, elicitation approval after), 5 (JSON+syslog
  v1; OCSF/CEF/hash-chaining as the enterprise-hardening pass).

### Forks that *challenge* the current spec (not just extend it)

- **#8** contradicts §4.3 ("first matching domain pattern wins"): the research shows this is the
  outlier on the dangerous side.
- **#7** sharpens §9.2: message-signing between binary and extension does **not** rescue the
  tampered-extension threat (the extension is the *source* of the CDP data, not just the pipe).
- **#5** changes §7.2's "url always logged" → default-redact query strings (PHI leak).
- **#3** promotes §11's "conditional HITL" toward v1, now that the mechanism can be protocol-
  native (MCP elicitation) rather than custom extension UX.

---

## Provenance

Derived from reports 01-09 in this folder. Each report is independently cited; this synthesis
does not re-cite but every claim above traces to one of them.
