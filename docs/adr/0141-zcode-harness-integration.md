# ADR-0141: ZCode harness integration

- Status: Accepted
- Date: 2026-08-27
- Amends: ADR-0071, ADR-0125
- Builds on: ADR-0117, ADR-0135

## Context

ZCode is a third-party coding agent with a desktop shell and a shared CLI configuration. Its MCP
registration lives in `~/.zcode/cli/config.json` under `mcp.servers.<name>` as a plain string
command with `args` and `env`. ZCode was never in the fixed harness roster, so the installer,
doctor, and the workbench could not see it at all.

On 2026-08-27 that invisibility produced a live contradiction: Ghostlight's workbench reported
Zed READY (its own registration was correct and verified working), while ZCode showed "The MCP
process failed to start." The cause was a hand-written ZCode entry pointing at the orchestrator
executable, which has no MCP stdio mode. Ghostlight had no way to know, and the person had no
card to check.

## Decision

- ZCode joins the fixed registry as client id `zcode` with config path `~/.zcode/cli/config.json`
  and a `ZCode` JSON dialect: entry location `mcp.servers.ghostlight`, entry shape
  `{"command": string, "args": [], "env": {}}`, pinned against the shape ZCode itself accepted
  live on 2026-08-27.
- No download destination is pinned. `can_download` stays false until an official, closed, HTTPS
  destination is verified; the Locate and manual-setup routes already cover installation.
- The packaged artwork is a neutral "Zc" monogram in the established icon style, not brand-exact
  artwork.
- Nothing else changes: detection stays parent-directory and PATH based, foreign-entry protection
  applies unchanged, and a hand-registered orchestrator command stays foreign evidence
  (ADR-0135), never overwritten.

## Consequences

- Install, uninstall, doctor, and the workbench operate ZCode like any other harness, and a
  future release build shows its real state instead of the integration failing invisibly.
- A READY card still means only what the roster covers: clients outside the registry remain
  invisible to every surface. That boundary is inherent to the fixed-registry design and is now
  recorded where the next "Zed says fine but X says broken" report can find it.
- The 2026-08-27 live incident is pinned by a test: install and uninstall refuse a foreign
  `ghostlight` command and the inspected state carries bounded evidence.
