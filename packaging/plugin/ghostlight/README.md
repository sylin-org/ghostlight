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
it ensures the three checksum-verified Ghostlight executables (`ghostlight`, `ghostlight-service`
and `ghostlight-desktop`) exist under `~/.ghostlight/bin/`, downloading them from the official
GitHub release only, then hands its inherited stdio to `ghostlight mcp`. Requirements: Node.js 18
or newer. The launcher's download and verification progress goes to stderr and never enters the
protocol stream.

The plugin installs the agent side only. To finish setup, add
[Ghostlight in Browser](https://chromewebstore.google.com/detail/ghostlight-in-browser/lejccfmoeogmhemakeknjjdhkfkgncdl)
from the Chrome Web Store, or open Ghostlight from the tray, the Start menu or Applications to see
the setup steps. Until the first request that reaches a browser succeeds, and while no browser profile holds the
extension, an agent's browser call that finds no browser connected says in its answer where the
extension comes from. Ghostlight
started by an agent shows only its tray icon and opens no window. Ghostlight is local by
construction: no telemetry, no activation service, no hidden network dependency. At each launch,
`npx` asks the npm registry which `ghostlight` package to run, and the launcher downloads the
programs from the GitHub release only when the installed set needs them. The npm launcher row of
[the data flows](../../../docs/trust/data-flows.md) names each case in which it downloads.

## Notes for specific clients

- ZCode reads `timeoutMs` per server; Claude Code does not, and configures timeouts globally.
- Claude Code on native Windows may need the command wrapped as
  `cmd /c npx -y ghostlight` depending on version; ZCode spawns `npx` directly.
- If Ghostlight is also registered through the installer, Claude Code holds two registrations
  named `ghostlight`. Which of them it starts, and whether it starts both, has not been verified.

## Versioning

The plugin is its own version space (ADR-0142 model). A plugin release edits four values that
must agree: both manifest versions and both catalog entry versions. The
repository-integrity gate fails otherwise. The skill ships with the plugin. It carries only the
working habits the tool catalog and the server instructions cannot, so it restates no bound,
default, value list or result field of theirs.
