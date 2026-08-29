# Ghostlight plugin

This directory is the plugin distribution member: one package that any plugin-marketplace
client can install to get Ghostlight's browser tools and an agent-facing skill. It carries two
equivalent manifests, and the repository-integrity gate asserts they agree:

- `.claude-plugin/plugin.json` -- canonical for Claude-schema marketplaces (Claude Code
  official, `zai-org/zai-coding-plugins`, and ZCode, which reads the Claude manifest as a
  compatibility name).
- `.zcode-plugin/plugin.json` -- the native twin for ZCode marketplaces; adds a per-server
  `timeoutMs` for first-run download headroom.

The repository root carries the matching catalogs, so this repository is itself a
one-address marketplace in both ecosystems:

- Claude Code: `claude plugin marketplace add sylin-org/ghostlight`, then
  `claude plugin install ghostlight@sylin-org-plugins`.
- ZCode: Settings, Plugin Management, Discover, `+`, then add `sylin-org/ghostlight`.

## What first run does

The plugin's MCP server is `npx -y ghostlight` with no arguments. That is the npm launcher:
it ensures the three checksum-verified Ghostlight binaries exist under
`~/.ghostlight/bin/v<version>/`, downloading them from the official GitHub release only, then
hands its inherited stdio to `ghostlight-mcp-connector`. Requirements: Node.js 18 or newer.
The launcher's download and verification progress goes to stderr and never enters the
protocol stream.

The first real browser call demand-starts the local Ghostlight authority, which walks you
through connecting a browser adapter (the Chrome Web Store extension) if none is connected
yet. Ghostlight is local by construction: no telemetry, no activation service, no hidden
network dependency; the only network traffic is the explicit binary download from the release
channel.

## Notes for specific clients

- ZCode reads `timeoutMs` per server; Claude Code does not, and configures timeouts globally.
- Claude Code on native Windows may need the command wrapped as
  `cmd /c npx -y ghostlight` depending on version; ZCode spawns `npx` directly.
- If Ghostlight is also registered through the installer, the explicit client configuration
  entry wins over the plugin's same-named server. Both can coexist; removing either is safe.

## Versioning

The plugin is its own version space (ADR-0142 model). A plugin release edits four values that
must agree: both manifest versions and both catalog entry versions. The
repository-integrity gate fails otherwise. The skill ships with the plugin; its content is
kept in lockstep with the model-facing language contract in `docs/1.0/LANGUAGE.md`.
