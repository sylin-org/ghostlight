# 0006. MCP-client-agnostic server

- Status: Accepted
- Date: 2026-07

## Context

AI coding agents already speak the Model Context Protocol. The official
Claude-in-Chrome extension, by contrast, only drives the browser from inside
Anthropic's own surface: its service worker bridges to claude.ai,
api.anthropic.com, and wss://bridge.claudeusercontent.com, so the automation is
usable only through that one product and its cloud. Binding our engine to a
single vendor's app would throw away the main advantage of a protocol that every
serious agent client already implements.

## Decision

The binary is an MCP server that exposes the browser over the MCP protocol
(JSON-RPC 2.0 over stdio). Any MCP client can launch it as a subprocess and
drive it: Claude Code, Cursor, Zed, Cline, and anything else that speaks MCP.
The transport is plain stdio; the client owns the process lifecycle (SPEC 2.1,
2.2). Configuration is the ordinary MCP `mcpServers` block (a `command`, `args`,
and `env`) with nothing Anthropic-specific in it (SPEC 8.3). There is no
required side panel, no vendor account, and no cloud dependency in the path from
agent to browser.

## Consequences

- "Bring your own agent to your own browser": users pick the client; the engine
  is neutral. This is a core strategic differentiator versus the official
  extension, which runs only inside Anthropic's side panel.
- No coupling to a specific product or cloud. The value stays in the local
  binary and the user's real, authenticated session (README).
- We own the MCP protocol layer ourselves (hand-rolled JSON-RPC, per CLAUDE.md),
  so we must track the MCP spec and stay compatible across diverse clients
  rather than leaning on one vendor's app behavior.
- Client-agnosticism is only worth anything if the advertised tools behave as the
  agent expects; that trained contract is fixed by ADR-0007.
