# Browser MCP

**Governed access to your *own* browser, for AI agents.**

Browser MCP is a single Rust binary plus a thin Chromium (Manifest V3) extension that gives AI
coding agents — Claude Code, Cursor, and any other MCP client — controlled access to **your
real, authenticated browser session**. It drives the browser you're already logged in to, so an
agent can observe and act on the web apps you already use, under access control you can turn all
the way off, dial in yourself, or have set for you by policy.

> **Status: design phase.** There is no implementation yet. This repository currently holds the
> authoritative specification and the discovery research behind it. See
> [docs/SPEC.md](docs/SPEC.md).

---

## What makes it different

- **It's your session, not a clean-room browser.** The value *is* the user's own authenticated
  context — real cookies, real SSO, real tabs. We never relocate your work to a cloud or
  freshly-launched browser to gain a technical property. (See
  [NORTH-STAR.md](docs/research/NORTH-STAR.md), Principle 4.)
- **Unconstrained engine, optional governance overlay.** The engine exposes the full
  browser-automation capability surface with no built-in limits. Governance is a *separable*
  layer that can gate it — or be absent entirely.
- **"All-open" is a first-class mode.** For personal use, zero restrictions is a valid, supported
  default — not a stripped-down enterprise build. Governance is additive, never required.
- **Single portable binary, zero runtime dependencies.** No Node.js, no `npx`, no separate
  servers to babysit — the class of install failures that plagues Node-based browser MCPs simply
  doesn't exist.

## Operating postures

Three postures, one engine, no code changes:

| Posture | Who sets limits | Stance |
|---|---|---|
| **All-open** (personal default) | nobody | a first-class unrestricted browser-automation MCP |
| **User-chosen** | you | opt into whatever limits *you* want ("keep the agent to these sites") |
| **Policy-enforced** (enterprise) | deployment channel (Intune/GPO) | default-deny, audited, identity-bound |

## Architecture

```
MCP Client ──stdio──▶ Rust Binary ──native messaging──▶ Extension ──CDP──▶ Browser
  (agent)              (engine + optional              (thin CDP           (your real
                        governance overlay)             executor)           session)
```

Three processes, two protocol boundaries. The binary is simultaneously the MCP server (stdio) and
the browser's native-messaging host — one process at the center. The extension is a deliberately
thin, dumb CDP executor; **all** capability, policy, and audit live in the binary.

## Documentation

| Doc | What it is |
|---|---|
| [docs/SPEC.md](docs/SPEC.md) | The authoritative design specification. Start with §1. |
| [docs/research/NORTH-STAR.md](docs/research/NORTH-STAR.md) | Governing design principles (engine-vs-overlay, layered delight, user-context sacred). |
| [docs/research/](docs/research/) | Pre-implementation discovery: prior-art and user-delight research feeding the spec. |

## Positioning & prior art

This is a clean-room Rust rewrite informed by
[open-claude-in-chrome](https://github.com/noemica-io/open-claude-in-chrome) (a Node.js
reimplementation of the Claude-in-Chrome extension). We study prior art as a *concern surface* —
the hazards and questions others hit — not as a feature catalog to copy. What no existing project
combines: extension-based automation of the user's own authenticated session, an unconstrained
engine with a composable governance overlay, tool-level read/write classification, and structured
audit — in a single deployable artifact.

## License

TBD (intended open-source).
