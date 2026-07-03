# Tool Schemas: the sacred surface

`tools.json` is the advertised MCP `tools/list` surface: the **13 tools** Ghostlight preserves at
**byte-parity with the official Claude-in-Chrome extension** (the exact tool names, descriptions,
parameter names/types/enums/constraints, and required sets the model's trained behavior depends on).
This is the **one** thing we preserve verbatim; everything else is a clean, lean re-design (Browser
MCP is not a port). See [ADR-0007](../../../docs/adr/0007-sacred-tool-surface.md) and
[ADR-0008](../../../docs/adr/0008-not-a-port.md).

## Provenance and fidelity

- The surface is byte-faithful to the official extension's advertised `toAnthropicSchema()` output,
  harvested in [docs/research/12-official-extension-parity.md](../../../docs/research/12-official-extension-parity.md).
- Schemas are stored as const raw JSON, never built programmatically, so they cannot drift.
- [tests/tool_schema_fidelity.rs](../../../tests/tool_schema_fidelity.rs) guards this file: the tool
  set and order, the `computer` action enum order, the harvested parameter corrections (`navigate`
  `force`, `get_page_text`/`read_page` `max_chars`, `computer.duration` max 10, `javascript_tool`
  without a `const`), and the bare-name (`tabs_context`/`tabs_create`) description convention.

## Tools (13)

`tabs_context_mcp`, `tabs_create_mcp`, `navigate`, `computer` (13 actions), `find`, `form_input`,
`get_page_text`, `javascript_tool`, `read_console_messages`, `read_network_requests`, `read_page`,
`resize_window`, `update_plan`.

## Not advertised (excluded from v1)

`gif_creator`, `shortcuts_list`, `shortcuts_execute`, `switch_browser`, `upload_image`. See
[ADR-0014](../../../docs/adr/0014-v1-scope-exclusions.md). (`gif_creator` and `upload_image` are
implemented in the official extension but deferred as niche; `shortcuts_*` are product-bound to the
official's side-panel agent.)
