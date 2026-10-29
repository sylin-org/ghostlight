# Ghostlight 2.0.2 publication -- 2026-10-29

The publication sequence of [`docs/RELEASE.md`](../RELEASE.md), run by Claude Code on the owner's
Windows machine at the owner's direction ("Publish 2.0.2"). Times are UTC.

## Source

- `v2.0.2`, an annotated tag ("Ghostlight 2.0.2"), on `00f2bc4301c2cfe264bee35a1c2ef962d6ec98eb`.
- CI run 40888544644 passed on that commit. Release run 40889472200 built the bundle there (2.0.2,
  19 artifacts, all ten jobs green, the installs from the bundle on Ubuntu 22.04, 24.04 and Alpine
  included), `scripts/check-release-candidate.ps1` verified it, and the publish scripts verified it
  again.
- 2.0.2 is a service patch under ADR-0093: the extension contract is unchanged, so the published
  2.0.1 adapter's 2.0 block covers it. The bundle's extension ZIP is the published 2.0.1 adapter
  (`aaa234b1...`), and nothing went to the Chrome Web Store.
- The owner set the candidate's bar at the owner's machine ("no need. as long as they work
  here."). The bundle was installed there over the development build with `get.ps1 -Restore` and
  `GHOSTLIGHT_FROM`, and the ADR-0205 call shapes passed through the installed edge and through a
  fresh headless Claude Code 2.1.311, whose client-side schema check accepted every one. The test
  machines and other harnesses were not run.

## Channels, in the order they were published

1. GitHub: the release `v2.0.2`, published at 14:54:32 with `scripts/publish-github-release.ps1`,
   marked Latest, not a prerelease, with 21 files (the 19 artifacts, `SHA256SUMS` and
   `release-candidate.json`). The script verified every provenance attestation and re-downloaded
   every draft asset for an exact hash comparison before it published.
2. `main`: fast-forwarded to `00f2bc43`, the tagged commit, so the sylin.org short commands serve
   2.0.2's `get.ps1` and `get.sh`.
3. npm: `ghostlight@2.0.2`, published with `scripts/publish-npm.ps1` and served with the tag
   `latest` about five minutes later. The public tarball's SHA-256 is the candidate's
   (`046fb410...`), and the registry's shasum is `ce718939...`.
4. MCP Registry: `org.sylin/ghostlight 2.0.2`, published at 15:27:11 with
   `scripts/publish-mcp-registry.ps1` once npm served the exact version.
5. WinGet: not submitted. The 2.0.1 pull request
   ([microsoft/winget-pkgs#458539](https://github.com/microsoft/winget-pkgs/pull/458539)) still
   waits on Microsoft's review of a Defender false positive; WinGet serves 2.0.0.

## Public install

- Windows, the owner's machine: `npx -y ghostlight@2.0.2 deployment install --restore` downloaded
  the three images from the GitHub release, verified each, installed them and started the service;
  `ghostlight status` read 2.0.2 with three browsers attached. A fresh MCP session then listed tabs
  with `{}`, opened a sylin.org demo stage from an address without a scheme, and waited with `{}`
  and `{"value":"1s"}`.
