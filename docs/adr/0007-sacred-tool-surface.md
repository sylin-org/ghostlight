# 0007. Sacred tool surface: byte-parity with the official Claude-in-Chrome

- Status: Accepted
- Date: 2026-07

## Context

Claude was trained against the tool schemas the official Claude-in-Chrome
extension advertises. The model's behavior (which tool it reaches for, how it
fills parameters, how it reads a description) is conditioned on those exact
tokens. Any paraphrase, rename, reordered enum, or dropped parameter is a
divergence from the trained surface and degrades behavior in ways that are hard
to see and harder to test. The schemas therefore are not ours to improve; they
are a contract to reproduce. An earlier baseline sourced the schemas from the
community reference (open-claude-in-chrome), but verification showed that
reference is a lossy proxy that carries its own bugs and prose drift
(docs/research/12). The ground truth is the official extension itself.

## Decision

The 13 advertised tool schemas (names, parameter signatures, enum values, and
description strings) are byte-identical to what the official extension
advertises (its `toAnthropicSchema()` output, re-baselined to v1.0.78 in commit
60bf334; SPEC 3, CLAUDE.md). The schemas live as const raw JSON
(`src/mcp/schemas/tools.json`) rather than being built programmatically, so they
cannot drift by accident. A fidelity test (`tests/tool_schema_fidelity.rs`) is
the guard: it pins the exact 13 tools in order, the non-sorted `computer.action`
enum order, the harvested v1.0.78 parameter corrections (navigate `force`,
get_page_text `max_chars`, computer `duration<=10`, javascript_tool without a
`const`), and the rule that descriptions reference the tab tools by their bare
names while the tool `name` fields keep the `_mcp` suffix.

## Consequences

- The model inherits its trained behavior: the surface it sees here matches the
  surface it learned on.
- Descriptions and parameters are frozen. We cannot casually reword a confusing
  description or tidy a schema; changing this array is changing the contract, and
  the test fails loudly if we try.
- We reproduce the official's own inconsistencies verbatim (e.g. `_mcp`-suffixed
  tool names but bare `tabs_context`/`tabs_create` in prose) because those are
  the trained tokens, not mistakes to correct.
- Governance shapes which tools are visible and when they run (SPEC 5), never the
  schema text. Policy is layered on top of a fixed surface.
- The frozen surface is the external contract; internals are free to be leaner
  than any prior art (ADR-0008).
