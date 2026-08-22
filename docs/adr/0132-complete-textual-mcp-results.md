# ADR-0132: Complete textual MCP results

- Status: Accepted
- Date: 2026-08-22
- Amends: ADR-0096 result rendering and the 1.0 result-envelope contract
- Builds on: ADR-0038, ADR-0096, and ADR-0103

## Context

Ghostlight returned each invocation as an authored summary in MCP `content` and the complete
canonical envelope in `structuredContent`. Some otherwise capable MCP clients expose ordinary
content to the model but do not surface `structuredContent`. Those clients could see that
`browser_execute` ran or that `browser_find` found a match, but not the returned JavaScript value
or the match handles. The product result was correct at the service boundary and incomplete at
the client compatibility boundary.

Teaching each tool to copy selected facts into its summary would duplicate product vocabulary,
make completeness depend on individual call sites, and eventually drift. Teaching the connector
about `facts`, `matches`, or `value` would move product meaning into the protocol edge.

## Decision

### 1. Ordinary MCP text carries the complete result

The connector renders one text block containing the orchestrator-authored summary and safe next
steps, followed by a compact JSON serialization of the complete opaque result envelope. It also
retains the same envelope in `structuredContent`.

Clients with structured-result support keep their machine-readable contract. Clients that expose
only ordinary text still receive status, effect, readiness, repeat safety, facts, and next steps.
The duplicate representation is intentional compatibility, not a second result vocabulary.

### 2. The edge remains generic

The connector does not inspect the tool name, result keys, status, or facts. It serializes the
opaque JSON value it already receives. The orchestrator remains the only owner of product
sentences, tool-specific facts, and safe recovery decisions. Rich image content retains its
existing separate blocks and ordering.

### 3. Existing bounds govern both representations

The textual projection introduces no new unbounded input. Page text and script values retain
their existing 20,000-character maximums, target collections retain their existing item and label
bounds, and image bytes remain absent from the JSON envelope. Audit and presentation continue to
use their content-minimized projections rather than either MCP representation.

## Consequences

- `browser_execute`, `browser_find`, `browser_read`, and every future tool remain usable through
  clients that ignore `structuredContent`.
- Clients that consume both channels may observe the same envelope twice.
- The MCP connector changes once for all current and future tools without tool-specific dispatch.
- Unit and process journeys must prove nested values and match handles are present in ordinary
  content while `structuredContent`, `isError`, and rich content remain intact.
