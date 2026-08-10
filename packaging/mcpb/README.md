# Ghostlight MCPB

> Historical 0.8 package documentation. Its launcher and packaging script are not present in the
> 1.0 tree. Preserve this record for future package-design evidence; do not claim or build a 1.0
> MCPB until a new package contract is accepted and implemented.

This directory is the source template for Ghostlight's Claude Desktop MCPB package. Release
packaging adds the signed release binaries for Windows x64, macOS Apple Silicon, and macOS Intel.
The package does not download code or send telemetry at runtime.

The bundle contains the persistent `ghostlight` service, the protocol-versioned
`ghostlight-mcp-connector` stdio edge, and the browser-only `ghostlight-browser-connector` native
host for every packaged platform. The launcher runs `ghostlight install --no-clients --no-open`
before starting `ghostlight-mcp-connector`. That
idempotent setup registers the local browser native host and service, while Claude Desktop remains
the sole owner of its MCP configuration. Users must separately install Ghostlight in Browser from
the Chrome Web Store.

Build a package from downloaded raw release artifacts:

```powershell
pwsh -File scripts/package-mcpb.ps1 -Version 0.8.0 -ArtifactsDir artifacts
```

The result is `dist/ghostlight-v0.8.0.mcpb`.

## Privacy Policy

Ghostlight runs locally and sends no product telemetry or browser data to Sylin. Browser results
go only to the MCP client the user chose. Optional audit records stay on the user's machine under
their retention policy. The full policy covers collection, use, storage, sharing, retention, and
contact details: https://sylin.org/ghostlight/privacy/.
