Ghostlight ${VERSION} -- governed browser automation for AI agents.

A native Rust service, protocol-versioned MCP edge, and thin Chromium adapter that give an AI
agent controlled access to your real, authenticated browser session, with an opt-in governance
layer (capability grants, sacred domains, audit). See the README and docs/guides/ for the
full walkthrough.

## Install

1. Install the service and register detected MCP clients: `npx -y ghostlight install`.
2. Install [Ghostlight in Browser](https://chromewebstore.google.com/detail/ghostlight-in-browser/lejccfmoeogmhemakeknjjdhkfkgncdl)
   from the Chrome Web Store.
3. Restart your MCP client, then verify the whole chain with `npx ghostlight doctor`.

Each platform archive contains the persistent `ghostlight` service, the protocol-versioned
`ghostlight-mcp-connector` stdio edge, and the browser-only `ghostlight-browser-connector` native host. MCP clients
must start `ghostlight-mcp-connector`; the removed `ghostlight-relay --role agent` path is not a fallback.

End-to-end verified on Windows and Linux. macOS builds and passes the full test suite in CI;
live-browser verification there is still owed.

## Verify

Every archive carries a signed build-provenance attestation (GitHub Artifact
Attestations / Sigstore). Prove an artifact was built by this repo's release workflow,
not a mirror or a tampered copy:

```
gh attestation verify <archive> --repo sylin-org/ghostlight
```

The SHA-256 checksums are below (the attestation is the stronger, signed check).

## Downloads

| Platform | Architecture | Download |
|---|---|---|
| Windows | x86_64 | `ghostlight-${VERSION}-x86_64-pc-windows-msvc.zip` |
| macOS | Apple Silicon | `ghostlight-${VERSION}-aarch64-apple-darwin.tar.gz` |
| macOS | Intel | `ghostlight-${VERSION}-x86_64-apple-darwin.tar.gz` |
| Linux | x86_64 | `ghostlight-${VERSION}-x86_64-unknown-linux-gnu.tar.gz` |
| Chrome adapter | any | `ghostlight-extension-v${ADAPTER_VERSION}.zip` |
| Claude Desktop MCPB | Windows/macOS | `ghostlight-${VERSION}.mcpb` |

## Checksums (SHA-256)

```
${CHECKSUMS}
```
