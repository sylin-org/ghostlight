# Ghostlight ${VERSION}

Ghostlight gives an AI agent governed access to the Chromium session where you are already signed
in. A release is three executables that ship as one tested set: `ghostlight`, the edge that MCP
clients and browsers start; `ghostlight-service`, the one persistent service that owns every
product decision; and `ghostlight-desktop`, the window and the tray. The Chromium extension,
`Ghostlight in Browser`, is delivered separately through the Chrome Web Store.

## What changed

${CHANGELOG}

## Install

Install takes three steps: install Ghostlight, add Ghostlight in Browser, then ask your agent to
do something. The shortest route is the npm launcher:

```sh
npx -y ghostlight@${VERSION} install
```

The one-line installers need no Node.js. On Windows, in PowerShell:

```powershell
irm https://sylin.org/ghostlight/get.ps1 | iex
```

On Linux:

```sh
curl -fsSL https://sylin.org/ghostlight/get.sh | sh
```

Every route installs into `~/.ghostlight/bin`, connects detected MCP clients and opens the
Ghostlight window on the setup steps. The window's Browser step adds `Ghostlight in Browser`
${ADAPTER_VERSION} from the Chrome Web Store.

If anything does not connect, run `npx -y ghostlight@${VERSION} doctor` for an ownership-safe
checkup.

WinGet is a carrier for the same install: run `ghostlight install` once through the WinGet alias.
The raw binaries below are the images the launchers and installers download and verify; most
people do not need them.

If you are upgrading from 1.x, run the install again and update the extension once. The service
and the extension are updated together; Ghostlight says which side to update.

## Platform evidence

${PLATFORM_EVIDENCE}

This section must name only clean-install, upgrade, demand-start, visible-browser, and uninstall
journeys actually completed with these exact artifacts. Do not turn a CI build into a live-platform
claim.

## Verify

`SHA256SUMS` binds every uploaded artifact. GitHub build provenance binds those bytes to this
repository's release workflow and source revision:

```sh
gh attestation verify <downloaded-file> \
  --repo sylin-org/ghostlight \
  --signer-workflow sylin-org/ghostlight/.github/workflows/release.yml
```

Ghostlight does not Authenticode-sign the Windows executables. `SHA256SUMS` and GitHub's keyless
build-provenance attestations are the artifact-integrity controls.

## Downloads

| Platform or evidence | Architecture | Asset |
| --- | --- | --- |
| Raw image: edge `ghostlight` | Windows x86_64 | `ghostlight-x86_64-pc-windows-msvc.exe` |
| Raw image: edge `ghostlight` | Linux x86_64 | `ghostlight-x86_64-unknown-linux-gnu` |
| Raw image: `ghostlight-service` | Windows x86_64 | `ghostlight-service-x86_64-pc-windows-msvc.exe` |
| Raw image: `ghostlight-service` | Linux x86_64 | `ghostlight-service-x86_64-unknown-linux-gnu` |
| Raw image: `ghostlight-desktop` | Windows x86_64 | `ghostlight-desktop-x86_64-pc-windows-msvc.exe` |
| Raw image: `ghostlight-desktop` | Linux x86_64 | `ghostlight-desktop-x86_64-unknown-linux-gnu` |
| Raw image: edge `ghostlight` | Linux x86_64, musl (Alpine) | `ghostlight-x86_64-unknown-linux-musl` |
| Raw image: `ghostlight-service` | Linux x86_64, musl (Alpine) | `ghostlight-service-x86_64-unknown-linux-musl` |
| Raw image: `ghostlight-desktop` | Linux x86_64, musl (Alpine) | `ghostlight-desktop-x86_64-unknown-linux-musl` |
| WinGet archive | Windows x86_64 | `ghostlight-v${VERSION}-x86_64-pc-windows-msvc.zip` |
| Chromium extension (store ZIP) | Any supported desktop | `ghostlight-extension-v${ADAPTER_VERSION}.zip` |
| npm launcher tarball | Supported desktops | `ghostlight-${VERSION}.tgz` |
| Claude Desktop MCPB (final release) | Windows | `ghostlight-v${VERSION}.mcpb` |
| Component SBOMs | All | `ghostlight-v${VERSION}-sbom-*.cyclonedx.json` |
| Checksums | All | `SHA256SUMS` |

The service and the window are version ${VERSION}. The edge and the relay carry the relay
revision, 4.0.0, and change only with their protocol.

The MCPB is the last one: this release retires it as a channel. After it, install with the npm
launcher or the one-line installers.

GitHub also provides source archives for the exact release tag. Ghostlight is Apache-2.0 OR MIT;
see `LICENSING.md` in the source archive.

## Checksums

```text
${CHECKSUMS}
```
