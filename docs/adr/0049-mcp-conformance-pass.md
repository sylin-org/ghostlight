# ADR-0049: MCP conformance pass -- version currency, listChanged, loud parse/batch rejects, and a deliberate no-init-guard

Status: Accepted (2026-07-09). Amends ADR-0041 Decision 5; supersedes the `2025-06-18` target
pinned in `docs/tasks/landscape-1/L2-protocol-version-negotiation.md`.

## Context

A standards-compliance audit this session (external research on the current MCP spec + tooling, and
a read-only audit of `crates/core/src/mcp/`) checked Ghostlight's hand-rolled JSON-RPC 2.0 server
against the current MCP revision (`2025-11-25`) and the official tooling (MCP Inspector `--cli`, the
`@modelcontextprotocol/conformance` suite, and the versioned `schema.json`).

The wire fundamentals were already correct and are NOT changed here: the JSON-RPC envelope, the
typed `content` array on `tools/call`, and -- the commonly-botched one -- the split between
tool-execution errors (a `result` with `isError: true`) and protocol errors (a JSON-RPC `error`
object). The gaps were all in negotiation, capability advertisement, and answering malformed input.

## Decision 1 -- Negotiate `protocolVersion`; latest is `2025-11-25`

`initialize` echoes the client's requested MCP revision when it is one this server supports, and
offers the latest supported revision otherwise (the spec's version-negotiation rule), replacing the
unconditional `"2024-11-05"`. The supported set is `["2024-11-05", "2025-03-26", "2025-06-18",
"2025-11-25"]`; the latest offered is `2025-11-25`.

Chosen through the delight lens: every real client gets ITS requested version echoed back, so no
client is ever surprise-downgraded or sees "why is this server on a 2024 protocol" in its logs; the
fallback for an unknown/future request degrades to the live spec, not a dead pin. It is honest --
the advertised surface is tools-only and uses only features common to the whole range beyond
capability-gated additions (`structuredContent`/`outputSchema`, which entered `2025-06-18` and are
optional), and capabilities gate everything else, so claiming a revision never claims its optional
features. This amends ADR-0041 D5 and supersedes landscape-1 L2's `2025-06-18` target for currency.

The negotiation is a pure function (`negotiate_protocol_version`, unit-tested); `initialize_result`
stays a thin renderer.

## Decision 2 -- Advertise `tools.listChanged`

`initialize` capabilities become `{"tools": {"listChanged": true}}` instead of `{"tools": {}}`. The
server already emits `notifications/tools/list_changed` when the advertised set changes on manifest
hot-reload (ADR-0025), so a capability-strict client must be told to expect the notification it will
otherwise receive unannounced. No other capability is added -- no `resources`/`prompts`/`logging`
are implemented, so none are advertised.

## Decision 3 -- Answer `-32700` on a malformed frame; stay silent on a blank line

An unparseable but NON-empty line now gets an addressable JSON-RPC parse error (`{id: null, error:
{code: -32700}}`, `id: null` because a broken frame carries no recoverable id) instead of being
dropped silently, so a broken client fails fast rather than hanging on a reply that never comes. A
blank / whitespace-only line is a benign keepalive and draws no response.

## Decision 4 -- Reject a JSON-RPC batch loudly, with a teaching message

JSON-RPC batching (a top-level array of requests) was removed from MCP in the `2025-06-18`
revision, so no compliant client sends one. A batch frame is rejected with `-32600` (`id: null`) and
a message that TEACHES the model the two supported ways to do several things: "Send one JSON-RPC
message per line. To run several browser actions in a single call, use the `script` tool." This is
consistent with the rest of Ghostlight's corrective error surface (`ToolError`'s "Next step:",
denials pointing at `explain`).

We deliberately do NOT execute batches. Ghostlight's `tools/call` is asynchronous and streamed (the
handler returns immediately and the response arrives later over a channel; ADR-0045). Honoring a
batch would require buffering the synchronous replies, awaiting each asynchronous one, and joining
them into a single array frame -- reintroducing head-of-line blocking (one slow tool call stalls the
rest of the batch), which the streamed pipeline exists to avoid. That is real cost for a
spec-removed feature with zero demand; the application-level `script` tool (ADR-0035) already serves
multi-step at the right layer.

## Decision 5 -- Deliberately keep NO initialize-before-use guard (non-decision)

The server accepts `tools/call` / `tools/list` without having seen `initialize` first, and this is
INTENTIONAL -- do not "fix" it as a conformance gap. ADR-0045's resilient reconnect replays the
captured `initialize` + `notifications/initialized` to a freshly restarted service and discards the
replayed `initialize` result; a strict initialize-before-use guard would fight that path. The
resilience is worth more than the strictness here, and a hostile client gains nothing by skipping a
handshake that carries no auth. If a guard is ever wanted, it must be gated so the ADR-0045 replay
still satisfies it.

## Consequences

- Friendlier to the official conformance suite and Inspector; the three commonly-botched checks
  (version counter-offer, `isError` vs JSON-RPC error, no reliance on removed batching) all pass.
- No sacred-surface change: `directory.rs` (tool schemas) and the fidelity/golden tests are
  untouched. Only `crates/core/src/mcp/server.rs` (the `initialize` result + the dispatch head) and
  the `tests/mcp_protocol.rs` harness change.
- Clients that send no `protocolVersion` now see `2025-11-25` rather than `2024-11-05`.

## Provenance

- Audit + external research: this session (2026-07-09). Tooling of record: MCP Inspector `--cli`,
  `@modelcontextprotocol/conformance`, versioned `schema.json`.
- D1 amends ADR-0041 D5 and supersedes landscape-1 L2's `2025-06-18` pin (owner-approved bump to
  `2025-11-25` for currency + delight, 2026-07-09).
- D4's teaching-message framing: owner direction ("this is a moment where we can teach the model").
- D5 records an existing intentional behavior so a later reader does not regress ADR-0045.
