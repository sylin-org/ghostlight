# MCP spec currency: MCP 2026-07-28 and the Ghostlight tree

Last reviewed: 2026-08-04 against the published `2026-07-28` revision and the unreleased
ADR-0096 working tree.

This note originally concluded that version negotiation was the only required change. The final
revision invalidated that conclusion. [ADR-0096](../adr/0096-protocol-versioned-mcp-edge-and-neutral-service.md)
is authoritative and records the resulting architecture replacement. Final implementation gates
remain in progress; this note does not claim a published release or a conformance-runner result.

## Useful protocol delta

The published revision differs materially from `2025-11-25`:

1. The base protocol is request-stateless. It removes initialize and protocol sessions, and each
   request carries its protocol version, client capabilities, and client information.
2. `server/discover` exposes server identity and capabilities before ordinary requests.
3. Results carry explicit result types, while list caching and change delivery use the revision's
   cache fields and `subscriptions/listen` model.
4. Cross-call application state requires explicit handles. A stdio connection or process is not
   an application-state identity.
5. Multi Round-Trip Requests replace the older sampling/elicitation shape. Tasks are a separate
   optional extension rather than an implication of long-running internal work.
6. The authorization changes target HTTP transports. They do not create a reason to add remote
   browser-control ingress to Ghostlight's local stdio product.

## Current Ghostlight disposition

- `crates/mcp-connector/` owns hand-rolled JSON-RPC over stdio. Its exact handlers are named
  `mcp_2025_11_25` and `mcp_2026_07_28`; older compatibility revisions are intentionally removed.
- `mcp_2025_11_25` owns initialize/initialized lifecycle state and an implicit service-minted
  `WorkspaceId`. `mcp_2026_07_28` validates metadata on every request and requires explicit
  workspace continuity for stateful work.
- The persistent `ghostlight` service receives typed product work, not raw MCP. Protocol dates,
  JSON-RPC ids, lifecycle, envelopes, and connection metadata stay at the MCP shore.
- Browser routing uses only `WorkspaceId` in the compatibility browser-wire field `guid`. Human
  client labels are presentation/audit context and never routing or authority.
- Multi Round-Trip `input_required` flows and MCP Tasks remain unadvertised. Either needs a
  concrete product flow and its own accepted decision before implementation.
- Structured content and output schemas remain part of the canonical tool projection, rendered
  into the exact response shape by each dated handler.
- Ghostlight still has no HTTP or WebSocket browser-control transport. The local management HTTP
  surface is separate, loopback-only, read-only, and not an MCP endpoint under ADR-0077.

## Evidence posture

Current protocol evidence is an immutable dated-schema/spec-driven review plus exact stdio
transcript tests, neutral-service tests, and real-process tests. The official conformance server
runner currently accepts an HTTP URL, not a stdio command, so it was not run against Ghostlight's
shipping transport. Use it as an additional gate if compatible transport support is added; do not
describe the present evidence as a conformance-runner pass.

## Sources

- [MCP 2026-07-28 changelog](https://modelcontextprotocol.io/specification/2026-07-28/changelog)
- [MCP 2026-07-28 base protocol](https://modelcontextprotocol.io/specification/2026-07-28/basic)
- [MCP 2026-07-28 server discovery](https://modelcontextprotocol.io/specification/2026-07-28/server/discover)
- [Immutable MCP 2025-11-25 schema](https://raw.githubusercontent.com/modelcontextprotocol/modelcontextprotocol/2025-11-25/schema/2025-11-25/schema.json)
- [Immutable MCP 2026-07-28 schema](https://raw.githubusercontent.com/modelcontextprotocol/modelcontextprotocol/2026-07-28/schema/2026-07-28/schema.json)
- [Official TypeScript SDK migration notes](https://github.com/modelcontextprotocol/typescript-sdk/blob/main/docs/migration/support-2026-07-28.md)
- [Official MCP conformance suite](https://github.com/modelcontextprotocol/conformance)
