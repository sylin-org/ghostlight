# Ghostlight MCPB

This directory is the source template for Ghostlight's Claude Desktop MCPB package. Release
packaging adds the signed release binaries for Windows x64.
The package does not download code or send telemetry at runtime.

The bundle contains the three images of one release: the edge `ghostlight` (the MCP relay and
the browser native host), the persistent service `ghostlight-service` and the window
`ghostlight-desktop`. On each start the launcher reads `~/.ghostlight/bin/install.json`. Only when
the installed service is absent or older than the bundle, the installed edge or service is
missing, or the installed window of the bundle's own version is missing, does it run the bundled
`ghostlight-service deployment install`, which puts the three images under `~/.ghostlight/bin`
and registers the local browser native host. An install of the same version puts a missing or
changed window back. It then runs the installed `~/.ghostlight/bin/ghostlight mcp`, never the
copy in the bundle, so a newer service installed by npm always meets an edge of its own
revision. When `install.json` records the development channel, the launcher never installs: it
runs the installed edge, and if that edge is gone it names the way back to a release and exits.
Claude Desktop remains the sole owner of its MCP configuration. Users must separately install
Ghostlight in Browser from the Chrome Web Store.

Version 2.0.0 is the final release of this channel (ADR-0197 Decision 16). MCPB runs its bundled
installer on every Claude Desktop start, so the final bundle carries 2.0.

Build a package from one checked release candidate:

```powershell
pwsh -File scripts/package-mcpb.ps1 -CandidateDirectory dist/release-candidate
```

The result is `dist/ghostlight-v<version>.mcpb`.

## Privacy Policy

Ghostlight runs locally and sends no product telemetry or browser data to Sylin. Browser results
go only to the MCP client the user chose. Optional audit records stay on the user's machine under
their retention policy. The full policy covers collection, use, storage, sharing, retention, and
contact details: https://sylin.org/ghostlight/privacy/.
