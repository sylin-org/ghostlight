# ADR-0148: Windsurf and Devin harness continuity

- Status: Accepted
- Date: 2026-08-30
- Amends: ADR-0125 Decision 2
- Builds on: ADR-0117 and ADR-0146

## Context

Windsurf's official Linux download now serves a product named Devin. The current application uses
`devin-desktop`, advertises `surf` as its command alias, and reads the user MCP configuration from
`devin/mcp_config.json` below the platform configuration root. Ghostlight's existing Windsurf row
instead recognizes `windsurf` and maintains the historical
`~/.codeium/windsurf/mcp_config.json` file.

Treating the new executable as evidence for the old row would make setup appear successful while
writing a file the current application does not use. Replacing the old row would stop Ghostlight
from maintaining an installed historical Windsurf client.

## Decision

The fixed harness registry keeps the historical `windsurf` target unchanged and adds one concrete
`devin` target. Both share the `windsurf` product id, download destination, and packaged visual
identity, so the workbench presents one product card with only the installed generation visible.

The Devin target:

- reads and writes `devin/mcp_config.json` under the effective platform configuration root;
- recognizes `devin`, `devin-desktop`, and `surf` executables;
- uses the established `mcpServers` JSON dialect and ownership-safe writer; and
- never redirects a historical Windsurf configuration to the new location.

On Linux the effective root is `XDG_CONFIG_HOME` or `~/.config`. On Windows it is `APPDATA`, as
already decided by ADR-0117.

## Consequences

- Current Devin and historical Windsurf installations can coexist and be maintained independently.
- `Set up everything` writes only the target proved by its own executable or product-specific
  configuration evidence.
- A vendor rename does not become a silent configuration migration or an executable alias hack.
- Live acceptance uses the current official Linux build and its own MCP listing or process chain;
  registry inspection alone is not a full integration claim.

## Acceptance evidence

1. The fixed registry contains both `windsurf` and `devin` under one product id.
2. Devin resolves its platform-relative config path and all three current executable names in a
   pure registry test.
3. The complete preview fixture pins all 23 registry targets and 19 product cards.
4. Automatic setup writes the exact current connector into Devin's configuration without changing
   the historical Windsurf file.
5. The installed current Linux application observes the registration through its own MCP surface
   or starts the exact connector.
