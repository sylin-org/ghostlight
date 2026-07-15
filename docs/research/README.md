# Browser MCP: Discovery Research

**Date:** 2026-07-01
**Phase:** Pre-Phase 0 (before any implementation)
**Premises:** *User delight* (across four personas) and *Capabilities* (prior-art frontier)

This folder captures the prior-art and user-delight discovery conducted before writing any
Rust. It exists to make the design decisions in `SPEC.md` traceable to evidence, and to
preserve the raw research so later revisions don't have to re-derive it.

## Personas weighted (all four requested)

1. **The AI agent**: Claude Code / Cursor as the tool consumer (ergonomics, token/context
   efficiency, error/denial quality, recovery from service-worker death).
2. **The developer**: personal/dev deployment (install friction, single-binary distribution,
   using the real authenticated session, DX).
3. **The enterprise admin**: governance operator (manifest push via Intune/GPO, audit UX,
   compliance, default-deny).
4. **The end user watching**: the human whose browser is driven (trust, transparency,
   human-in-the-loop, prompt-injection safety).

## How this research was produced

A fan-out of parallel research agents mined primary sources (GitHub issues via web, vendor
docs, IETF drafts, blog/forum discussion) across four tracks: capability survey, governance &
enterprise, delight & pain, and security & trust.

**Methodology note (honesty):** partway through, the Anthropic backend hit a sustained
transient rate-limit event ("Server is temporarily limiting requests") that killed several
sub-agents mid-flight. The work was recovered by (a) letting the surviving top-level agents
finish, and (b) running the remaining research **inline via sequential/parallel web search**
from the main loop, which slipped through the throttle. A few sub-agent tracks returned only
rate-limit errors and produced no content; where that happened, the parent track's report
re-derived the substance from primary sources (noted in-file). No track was silently dropped.

## Index

| File | Track | What it covers |
|---|---|---|
| [NORTH-STAR.md](NORTH-STAR.md) | **Governing** | **Read first.** Project-owner design principles: unconstrained engine + governance-as-overlay, "all-open" as a first-class mode, user-context is sacred, prior-art-as-concern-surface (not feature-copy). Governs how everything below is used. |
| [00-synthesis-and-decisions.md](00-synthesis-and-decisions.md) | Synthesis | Cross-cutting findings + the 8 design forks. **Read through the lens of NORTH-STAR**. Its emphasis is corrected there. |
| [10-forks-decided.md](10-forks-decided.md) | **Decisions** | The eight forks re-lensed and **decided** (via adversarial workflow + owner review): per-fork calls, postures matrix, v1 scope, and the consolidated SPEC-change map. `extract` deferred to v2. |
| [01-capability-survey-stagehand-browserbase.md](01-capability-survey-stagehand-browserbase.md) | Capabilities | Stagehand / Browserbase: `act`/`extract`/`observe`, self-healing action cache, live view, contexts. The semantic-primitive frontier. |
| [02-install-and-onboarding-friction.md](02-install-and-onboarding-friction.md) | Delight (dev) | Real install pain across the ecosystem; the single-binary differentiator and the native-messaging landmine. |
| [03-governance-enterprise-prior-art.md](03-governance-enterprise-prior-art.md) | Governance | Policy-resolution semantics, manifest ergonomics, audit standards (OCSF/CEF/RFC 5424), enterprise deployment mechanics. |
| [04-mcp-gateway-lasso-deep-dive.md](04-mcp-gateway-lasso-deep-dive.md) | Governance | Lasso MCP Gateway source-level analysis: a content-sanitization proxy pattern, and what it lacks vs our model. |
| [05-identity-xaa-idjag-rfc8693.md](05-identity-xaa-idjag-rfc8693.md) | Governance | Okta Cross-App Access, ID-JAG, RFC 8693 token exchange / delegation (`act`/`may_act`). |
| [06-agentic-identity-standards.md](06-agentic-identity-standards.md) | Governance | The broader "OAuth for agents" landscape: ID-JAG, WIMSE, SPIFFE, Transaction Tokens, MCP Enterprise-Managed Authorization. |
| [07-hitl-and-stepup-auth.md](07-hitl-and-stepup-auth.md) | Security/Delight | Human-in-the-loop approval (MCP elicitation/MRTR), RFC 9470 step-up, MCP tool-annotation risk vocabulary. |
| [08-cdp-origin-verification-extension-trust.md](08-cdp-origin-verification-extension-trust.md) | Security | **Load-bearing.** How to trust "current URL" at the CDP layer; the tampered-extension trust boundary; per-frame enforcement. |
| [09-web-research-primary-sources.md](09-web-research-primary-sources.md) | All | Inline web-search findings: Playwright MCP, WebMCP, MCP auth spec, Claude-in-Chrome injection numbers, token bloat, Operator/Atlas, MV3. |
| [11-install-detection.md](11-install-detection.md) | Delight / Fork 4 | Install-flow auto-detection: per-OS native-messaging registration (Windows registry / macOS + Linux dirs) for every Chromium browser, MCP-client config detection + add-server mechanics, and the deterministic-extension-ID trick. Feeds the self-registering installer. |

## Later research (post-implementation)

These were written after the engine and governance layers shipped; they follow the same
cited-sources discipline but are not part of the pre-Phase 0 discovery above.

| File | What it covers |
|---|---|
| [12-official-extension-parity.md](12-official-extension-parity.md) | Re-baseline of the tool surface against the official Claude-in-Chrome extension (v1.0.78), which the community reference only approximates. |
| [13-competitive-landscape.md](13-competitive-landscape.md) | Four-lens competitive sweep (2026-07): Camp A (real-session automation, no governance) vs Camp B (governance, no browser); the uncontested intersection; the rename recommendation. |
| [14-post-evaluation-2026-07.md](14-post-evaluation-2026-07.md) | Post-evaluation (2026-07-07): Ghostlight state capture, how every tracked player moved (first-party Claude Code + Chrome, Microsoft Agent Governance Toolkit, browser-use's Rust core, MCP 2026-07-28, WebMCP origin trial), positioning re-test, and the ranked P1-P10 proposal list. |
| [15-webmcp-governance-participation.md](15-webmcp-governance-participation.md) | WebMCP origin-trial participation brief: runtime consequence classification, origin/frame identity, dynamic-registration races, private reads, prepare-versus-commit, provenance, a bounded experiment, and a draft standards response. No product implementation. |
| [16-denial-burst-circuit-breaker.md](16-denial-burst-circuit-breaker.md) | Prior art and ADR input for quiet isolated denials plus a real session-scoped pause after a denial burst. Defines counter scope, recovery semantics, threat boundary, audit facts, and evaluation gates. No implementation. |
| [17-agent-browser-overlap-2026-07.md](17-agent-browser-overlap-2026-07.md) | Mutual capability map against agent-browser v0.31.2 and strategic opportunities through model, user, and governance delight. |
| [18-free-surface-evaluation-plan-2026-07.md](18-free-surface-evaluation-plan-2026-07.md) | Measurement plan and acceptance gates for ref-linked annotated screenshots and optional owned-tab labels. No public schema change. |
| [19-firefox-browser-adapter-dossier-2026-07.md](19-firefox-browser-adapter-dossier-2026-07.md) | Firefox extension, Marionette, and WebDriver BiDi capability map; adapter boundary; dynamic explanation; multi-browser selection; security posture; and bounded proof-of-concept gates. No product decision. |

## Status

Discovery **complete**; the eight forks are **decided**. See
[10-forks-decided.md](10-forks-decided.md) (`extract` deferred to v2). No code written yet.
