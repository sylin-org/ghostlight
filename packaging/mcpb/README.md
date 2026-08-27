# Ghostlight MCPB

This directory is the source template for Ghostlight's Claude Desktop MCPB package. Release
packaging adds the signed release binaries for Windows x64.
The package does not download code or send telemetry at runtime.

The bundle contains the persistent `ghostlight` service, the protocol-versioned
`ghostlight-mcp-connector` stdio edge, and the browser-only `ghostlight-browser-connector` native
host for every packaged platform. The launcher runs `ghostlight install --no-clients --no-open`
before starting `ghostlight-mcp-connector`. That
idempotent setup registers the local browser native host and service, while Claude Desktop remains
the sole owner of its MCP configuration. Users must separately install Ghostlight in Browser from
the Chrome Web Store.

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
