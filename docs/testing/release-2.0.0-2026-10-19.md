# Ghostlight 2.0.0 publication -- 2026-10-19

The 2.0 cutover of [`docs/RELEASE.md`](../RELEASE.md), run by the coordinator (Claude Code on the
owner's Windows machine) at the owner's direction. Times are UTC; the sitting began on the
evening of 2026-10-19 in the owner's time zone (UTC-4).

## Source

- `v2.0.0`, an annotated tag ("Ghostlight 2.0.0"), on `6a41327f2179d611981294602c408e94c9c99a25`.
- CI run 39346483193 passed on that commit. Release run 39347181245 built the bundle there, and
  `scripts/check-release-candidate.ps1` verified it (2.0.0, 16 artifacts).
- The bundle differs from the one tested on Windows, CachyOS and Bluefin (run 39323041650 on
  `eba78013`) only in docs. Its extension ZIP is the same file
  ([Bluefin record](bluefin-2.0-bundle-2026-10-19.md)).

## Channels, in the order they were published

1. GitHub: the release `v2.0.0`, published at 01:54:51 on 2026-10-20, marked Latest, not a
   prerelease. It holds 18 files: the 16 artifacts, `SHA256SUMS` and `release-candidate.json`.
   Its `SHA256SUMS` (`8a596ea5...`) is byte-identical to the bundle's, listed below.
2. `main`: e3f00d75, a merge whose tree is v2.0.0's. `main` carried one 1.x installer fix that
   `dev` did not (e25a2c05), so a plain fast-forward was not possible. The merge keeps v2.0.0's
   tree. The sylin.org short commands then served `get.ps1` and `get.sh` byte-identical to
   v2.0.0's.
3. npm: `ghostlight@2.0.0`, published at 01:57:26 with the tag `latest`. The registry's tarball
   hashes to `63bde2b5...`, the value `SHA256SUMS` binds.
4. The public install, proven on the owner's Windows machine (step 4 below).
5. Chrome Web Store: item `lejccfmoeogmhemakeknjjdhkfkgncdl`. The 2.0.0 ZIP (`ee224962...`) was
   submitted as STAGED_PUBLISH, approved, and published. The store's status then read 2.0.0
   PUBLISHED at 100%, with nothing pending. The Chrome update feed served 2.0.0 within minutes.
6. MCP Registry: `org.sylin/ghostlight 2.0.0`, published at 02:37:19, active and latest. It went
   out before `main` moved, because the online public-surface check needs it.
7. The public state: e789913b on `dev` and `main`. The website rebuilt from website 969866d,
   which refreshed its install and public-status fallbacks, and
   `scripts/check-public-surfaces.ps1 -Online` then passed: "GitHub, npm, Chrome update feed,
   official MCP Registry, and website agree".
8. WinGet: the 2.0.0 manifests, prepared from the bundle and accepted by `winget validate`, are
   in microsoft/winget-pkgs#456409 (one commit from the owner's fork), pending checks and review.
   Until it merges, `winget install Sylin.Ghostlight` installs 1.3.14.

## Step 4 on Windows

On the owner's machine, outside the Claude app's sandbox, so the registry and AppData were the
real ones:

- `irm https://sylin.org/ghostlight/get.ps1 | iex` in Windows PowerShell 5.1 printed "Ghostlight
  2.0.0 downloaded. Checksums and build provenance verified." Then it refused the development build
  installed there and printed the restore route.
- The restore route's PowerShell line installed the release over the development build. It
  printed the install summary, and `ghostlight status` lost its development channel line.
- Doctor read Ready. Chrome, Edge, Brave and Chromium were registered to this installation, and
  the installed `ghostlight.exe`, `ghostlight-service.exe` and `ghostlight-desktop.exe` were
  byte-identical to the release assets.
- Through the installed release, on the owner's Chrome, the demo desk's bell opened its prompt.
  `browser_dialog` status read it, accept answered, and the page read `the bell says: "hello from
  the desk"`.
- `npx -y ghostlight@2`, the Claude Code plugin's command, gave an MCP client Ghostlight 2.0.0 with
  23 tools. An unversioned `npx ghostlight` fetched 2.0.0 instead of a cached 1.x launcher.
- `npx ghostlight deployment install --restore` downloaded and verified the three images and
  installed them. The plain one-liner in PowerShell 7 then printed "Ghostlight 2.0.0 is installed."
  without restarting the running service.

## Not proven in this sitting

- The public Linux one-liner on test-01. The Linux machines were off for the night, and the owner
  directed the cutover to go on without them. The Linux images are the bundle's, which CachyOS,
  Bluefin and the release workflow's install job on Ubuntu 22.04 and 24.04 ran.
- Step 6, the clean-profile run of the public path on Windows and Linux.

## SHA256SUMS

```text
63bde2b52154f758bdc24e6e31b91d48faa847c8e7456dc0cd3642a1a2ab2b1c  ghostlight-2.0.0.tgz
8708d787bfb07dd8d9f30e3a2def460b85a79e13b4e4327d8677d37a1d873b43  ghostlight-desktop-x86_64-pc-windows-msvc.exe
6f70882ddc380d2b4bed3ef64cd55177f1892dfe50fd3a07c77e65113a74a32c  ghostlight-desktop-x86_64-unknown-linux-gnu
ee224962a56421d1b8d7138f4a0c66c264b1266dfa0227958ab50bbb45f107e0  ghostlight-extension-v2.0.0.zip
5970e409995f47b475df1dede875031638566adf246fe86df77039185e3f9b86  ghostlight-service-x86_64-pc-windows-msvc.exe
381b7aa8b02a70631a9e1097fe5d342df7031b0fb47c4b451486b7cefee0ed5a  ghostlight-service-x86_64-unknown-linux-gnu
0547bcf9de35b9400e20d950eabb6b2f21599bfe89b87fa0059464a87d132db8  ghostlight-v2.0.0-sbom-ghostlight-bridge.cyclonedx.json
d368e8bc87c317fd6c2c0b8760506a2319033ca0992cf4771ce93782eeeea316  ghostlight-v2.0.0-sbom-ghostlight-desktop.cyclonedx.json
eeb7c7f892c4d1d80cb9e9e11077d098e9ca9d46641d26db1949fa00fce60df2  ghostlight-v2.0.0-sbom-ghostlight-relay.cyclonedx.json
dbc6f98e56beb9c80751709c50d0c1ddef3749e9ec243a9364a09ebf32f15b0d  ghostlight-v2.0.0-sbom-ghostlight-service.cyclonedx.json
63987358c9562b8fbe07a2fc2834c9ab274ad915d32f2a826c035e95b278f975  ghostlight-v2.0.0-sbom-ghostlight-win-peer.cyclonedx.json
671ec01877bfadae8128ab371d0b058b41fb84ff18552a2e9b510be6bd43282c  ghostlight-v2.0.0-sbom-ghostlight.cyclonedx.json
230716977c95a9f25042cc974c4aa14ab85048132f0f35c322343de8f9a402c0  ghostlight-v2.0.0-x86_64-pc-windows-msvc.zip
c44e64394af244c2fe6cf2b517e30979df9baa904e1ebdad7b3549f775e8a4ca  ghostlight-v2.0.0.mcpb
8af8bbba398a6a23977b054d7cfbe20aefe9eb491985933d9172fc11a546ab28  ghostlight-x86_64-pc-windows-msvc.exe
fcf48bd16e2c801d628173eb253325305e7495909644c9986d7de807abf37484  ghostlight-x86_64-unknown-linux-gnu
```
