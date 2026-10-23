# Ghostlight 2.0.1 publication -- 2026-10-22

The publication sequence of [`docs/RELEASE.md`](../RELEASE.md), run by the coordinator (Claude Code
on the owner's Windows machine) at the owner's direction ("do the full run!"). Times are UTC.

## Source

- `v2.0.1`, an annotated tag ("Ghostlight 2.0.1"), on `8e185d67dceb98257afa85e69b046ee7937898cb`.
- CI passed on that commit. Release run 39715160480 built the bundle there (2.0.1, 19 artifacts),
  and the publish scripts verified it. Its `SHA256SUMS` hashes to `438a6268...`.
- The bundle was checked before publication on Bluefin (588ddd95), Alpine (48ef55fd, 657495a3) and
  leo-desktop-02 (3ff45599, 55cb0756). CachyOS was offline after a power loss.

## Channels, in the order they were published

1. Chrome Web Store: item `lejccfmoeogmhemakeknjjdhkfkgncdl`. The 2.0.1 ZIP (`aaa234b1...`, from
   candidate run 39669867246, the one the freeze record names and byte-identical to the final
   bundle's) was uploaded and submitted as DEFAULT_PUBLISH with `scripts/publish-extension.ps1`,
   approved, and published. The store's status then read 2.0.1 PUBLISHED at 100%, with nothing
   pending, and `scripts/reconcile-chrome-store.ps1 -WriteObservedState` recorded it (5724b03a).
2. GitHub: the release `v2.0.1`, published at 04:39:11 with `scripts/publish-github-release.ps1`,
   marked Latest, not a prerelease. It holds 21 files: the 19 artifacts, `SHA256SUMS` and
   `release-candidate.json`. The script verified every provenance attestation and re-downloaded
   every draft asset for an exact hash comparison before it published.
3. `main`: fast-forwarded to `8e185d67`, the tagged commit, so the sylin.org short commands serve
   2.0.1's `get.ps1` and `get.sh`.
4. npm: `ghostlight@2.0.1`, published with `scripts/publish-npm.ps1` and listed at 04:45:06 with
   the tag `latest`, after a processing delay of about twenty minutes. The registry's shasum
   (`536aa42f...`) is the SHA-1 of the candidate tarball, whose SHA-256 `SHA256SUMS` binds
   (`d79a392a...`).
5. MCP Registry: `org.sylin/ghostlight 2.0.1`, published at 04:46:30 with
   `scripts/publish-mcp-registry.ps1` once npm served the exact version.
6. WinGet: pull request
   [microsoft/winget-pkgs#458539](https://github.com/microsoft/winget-pkgs/pull/458539), with the
   three manifests `scripts/prepare-winget-manifest.ps1 -ValidateWinGet` generated from the
   bundle. Until it merges, WinGet serves 2.0.0.
   Its Installation Validation failed on 2026-10-22 (Validation-Defender-Error): Defender's cloud
   machine-learning verdict Trojan:Script/Wacatac.C!ml blocked the portable zip, while a local scan
   with definitions 1.461.158.0 and cloud protection found nothing. On 2026-10-23 the zip was
   submitted to Microsoft as an incorrect detection (WDSI submission
   d5b9b3cd-7c4f-4b17-a9ed-60fb9b19a0a7, as hello@sylin.org), and the PR asks a moderator to re-run
   validation once the determination clears.

## Public install

- Alpine (musl), test-03, df856a6c: `curl -fsSL https://sylin.org/ghostlight/get.sh | sh` picked the
  musl images; version 2.0.1, doctor Ready; the store extension updated to 2.0.1 by itself and was
  active after a browser restart; the desk bell passed.
- Windows, leo-desktop-02, test-04, 499358bf: `irm https://sylin.org/ghostlight/get.ps1 | iex`
  installed 2.0.1; doctor Ready with the store's 2.0.1 extension; the desk bell passed, its tab in
  the ghost and "Claude Code" group; `npx -y ghostlight@2.0.1 deployment install --restore` from npm
  installed and started the service.
- Bluefin was asked for the same run; its session was offline, and its record is added when it
  reports.
