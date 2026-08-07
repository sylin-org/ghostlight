# Protocol-versioned MCP edge BOOTSTRAP

This batch implements ADR-0096. The target is the smallest complete three-executable design:

```text
MCP client <-> ghostlight-mcp <-> ghostlight <-> ghostlight-relay <-> Chromium
```

Naming amendment (2026-08-04): the implemented shore executables are now
`ghostlight-mcp-connector` and `ghostlight-browser-connector`, with crate directories
`crates/mcp-connector/` and `crates/browser-connector/`. The process topology and responsibilities
below are unchanged; older names in this batch document record the implementation sequence.

## Authority

1. `docs/adr/0096-protocol-versioned-mcp-edge-and-neutral-service.md`
2. Existing accepted ADRs, especially the tool-identity, governance, Hub, executable-split, and
   browser-adapter decisions named by ADR-0096
3. The live tree

## Guardrails

- Keep exactly three executables. Do not add a daemon, broker, event bus, work database, MCP SDK,
  protocol plugin framework, or version crate.
- Keep all MCP JSON-RPC and revision behavior in `ghostlight-mcp`.
- Keep the service protocol-neutral. It owns work, authority, workspaces, scheduling, and browser
  execution.
- Keep `ghostlight-relay` browser-only and policy-free.
- Enforce those roles with executable entry points and dependency direction. Do not add a
  process-global role marker or runtime role-selection framework.
- Preserve the trained tool identity surface. Revision-specific schema changes are projections at
  the MCP shore, never edits to the canonical registry.
- Implement exact revisions `2025-11-25` and `2026-07-28`; use those dates in internal names.
- Route browser work only by `WorkspaceId`, carried in the compatibility `guid` field. A human
  client label is presentation/audit context only and never a routing, scheduling, ownership, or
  authority key. Do not emit the former top-level presentation/routing `clientKey` on current
  tool/group frames. A nested adapter-compatibility scheduler field may carry `WorkspaceId` under
  that old wire spelling; it never carries the human label.
- Preserve all-open behavior, current governance checks, audit finality, fair scheduling, browser
  reconnect behavior, and every shipped distribution channel.
- Cancellation is cooperative. Never abort or replay a browser action that may have started.
- ASCII only. Do not copy code from `reference/` or the downloaded reference implementations.
- Preserve unrelated dirty-tree work, especially the in-flight MCPB distribution changes.

## Verification

Use an isolated `CARGO_TARGET_DIR`. Before completion run formatting, clippy with warnings denied,
workspace tests, Lightbox, extension JavaScript checks, packaging checks, and the revision-specific
edge transcripts added by this batch. Review those transcripts against the immutable official
dated specifications and schemas. Do not claim an official conformance-runner result: its current
server runner accepts an HTTP URL, not Ghostlight's shipping stdio command.
