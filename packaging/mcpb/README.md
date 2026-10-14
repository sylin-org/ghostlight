# Ghostlight MCPB

This directory is the source template for Ghostlight's Claude Desktop MCPB package. Release
packaging adds the signed release binaries for Windows x64.
The package does not download code or send telemetry at runtime.

The bundle contains the persistent `ghostlight` service, the protocol-versioned
`ghostlight-mcp-connector` stdio edge, and the browser-only `ghostlight-browser-connector` native
host for every packaged platform. On each start the launcher reads `~/.ghostlight/bin/install.json`.
Only when the installed service is absent or older than the bundle does it run the bundled
`ghostlight deployment install`, which puts the three executables under `~/.ghostlight/bin`
and registers the local browser native host and service. It then starts `ghostlight-mcp-connector`.
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
