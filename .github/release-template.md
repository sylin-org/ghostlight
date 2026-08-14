# Ghostlight ${VERSION}

Ghostlight gives an AI agent governed access to the Chromium session where you are already signed
in. The 1.0 release is a native desktop product: one persistent Rust authority, a stable MCP stdio
connector, a browser-only native connector, a thin Chromium adapter, and a local tray workbench.

## What changed

${CHANGELOG}

## Install

The shortest path is the npm launcher:

```sh
npx -y ghostlight@${VERSION} install
```

Then install `Ghostlight in Browser` ${ADAPTER_VERSION} from the Chrome Web Store and reconnect
your MCP client. The installer connects detected clients directly to the cached native connector;
Node is not a resident service.

If anything does not connect, run `npx -y ghostlight@${VERSION} doctor` for an ownership-safe
checkup.

The signed Windows setup and Debian package below are equivalent native-package routes. Start
Ghostlight, open **MCP integrations** to connect a client, and use **Status** to check the whole
chain.

Use the Windows setup executable or install the Debian package with
`sudo apt install ./ghostlight-v${VERSION}-x86_64-unknown-linux-gnu.deb`. The package owns the three
Ghostlight executables, native-messaging registration, and exact license texts. It does not install
a resident supervisor or phone home; either connector demand-starts the one local authority.

If you are upgrading from 0.8, rerun `npx -y ghostlight@${VERSION} install` or install the native
package. Ghostlight recognizes owned old registrations and preserves unrelated client
configuration. The removed `ghostlight-relay --role agent` command is not a compatibility fallback.

## Platform evidence

${PLATFORM_EVIDENCE}

This section must name only clean-install, upgrade, demand-start, visible-browser, and uninstall
journeys actually completed with these signed artifacts. Do not turn a CI build into a live-platform
claim.

## Verify

`SHA256SUMS` binds every uploaded artifact. GitHub build provenance binds those bytes to this
repository's release workflow and source revision:

```sh
gh attestation verify <downloaded-file> \
  --repo sylin-org/ghostlight \
  --signer-workflow sylin-org/ghostlight/.github/workflows/release.yml
```

${SIGNATURE_NOTES}

## Downloads

| Platform or evidence | Architecture | Asset |
| --- | --- | --- |
| Windows setup | x86_64 | `ghostlight-v${VERSION}-x86_64-pc-windows-msvc-setup.exe` |
| Debian package | x86_64 | `ghostlight-v${VERSION}-x86_64-unknown-linux-gnu.deb` |
| Portable archive | Windows x86_64 | `ghostlight-v${VERSION}-x86_64-pc-windows-msvc.zip` |
| Portable archive | Linux x86_64 | `ghostlight-v${VERSION}-x86_64-unknown-linux-gnu.tar.gz` |
| Claude Desktop MCPB | Windows | `ghostlight-v${VERSION}.mcpb` |
| npm launcher source tarball | Supported desktops | `ghostlight-${VERSION}.tgz` |
| Raw launcher binaries | Two target triples | `ghostlight*-<target>[.exe]` |
| Chromium adapter | Any supported desktop | `ghostlight-extension-v${ADAPTER_VERSION}.zip` |
| Component SBOMs | All | `ghostlight-v${VERSION}-sbom-*.cyclonedx.json` |
| Checksums | All | `SHA256SUMS` |

GitHub also provides source archives for the exact release tag. Ghostlight is open-core; see
`LICENSING.md` in the source archive and the exact license texts installed with each native
package.

## Checksums

```text
${CHECKSUMS}
```
