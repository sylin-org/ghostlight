# Ghostlight 2.0.3 publication -- 2026-11-14

The publication sequence of [`docs/RELEASE.md`](../RELEASE.md), run by Claude Code on the owner's
Windows machine at the owner's direction ("Proceed with process to publish the service."). Times
are UTC.

## Source

- `v2.0.3`, an annotated tag ("Ghostlight 2.0.3"), on `93c3c14d7ce5672fd0732dd475c519849915a7e1`.
- CI run 43560608441 passed on that commit, every job on Windows and Linux, the browser journeys
  included. Release run 43563505093 built the bundle there (2.0.3, 19 artifacts, all ten jobs
  green, the installs from the bundle on Ubuntu 22.04, 24.04 and Alpine included),
  `scripts/check-release-candidate.ps1` verified it, and the publish scripts verified it again.
- 2.0.3 is a service patch under ADR-0093 that carries ADR-0207 and the ADR-0196 amendment of
  2026-11-13. There is no wire change: the published 2.0.1 adapter's 2.0 block covers it, and so
  does adapter 2.0.2.
- The bundle's extension ZIP is Ghostlight in Browser 2.0.2 with SHA-256 `bca8719d...`, the same
  bytes submitted to the Chrome Web Store from a local package the day before
  ([submission record](extension-2.0.2-store-submission-2026-11-14.md)). The workflow and the
  local packager agreed.
- Before the tag, the bundle was installed on the owner's machine over the development build with
  `get.ps1 -Restore` and `GHOSTLIGHT_FROM`. Through the installed edge a read, a text wait that
  ran out, and a page scroll behaved as 2.0.3 should. The same source had been checked live on
  that machine as a development build against the published 2.0.1 extension and the unpacked
  2.0.2 one ([field session](office-web-field-session-2026-11-13.md)). The test machines and
  other harnesses were not run.

## Channels, in the order they were published

1. GitHub: the release `v2.0.3`, published at 06:04:13 with `scripts/publish-github-release.ps1`,
   marked Latest, not a prerelease, with 21 files (the 19 artifacts, `SHA256SUMS` and
   `release-candidate.json`). The script verified every provenance attestation and re-downloaded
   every draft asset for an exact hash comparison before it published.
2. `main`: fast-forwarded to `93c3c14d`, the tagged commit, so the sylin.org short commands serve
   2.0.3's `get.ps1` and `get.sh`.
3. npm: `ghostlight@2.0.3`, published with `scripts/publish-npm.ps1` and served with the tag
   `latest` within two minutes. The public tarball's SHA-256 is the candidate's (`ff5dc41b...`),
   and the registry's shasum is `6cb64375...`.
4. Chrome Web Store: the owner published Ghostlight in Browser 2.0.2 after its review. The store
   API reported it published at 100 percent, and `scripts/reconcile-chrome-store.ps1
   -WriteObservedState` observed public adapter 2.0.2 and recorded it.
5. MCP Registry: `org.sylin/ghostlight 2.0.3`, published with `scripts/publish-mcp-registry.ps1`
   once npm served the exact version.
6. WinGet: at publication WinGet served 2.0.2 (observed with `winget show`). On the owner's go the
   same day, the 2.0.3 manifests went to
   [microsoft/winget-pkgs#479983](https://github.com/microsoft/winget-pkgs/pull/479983), generated
   by `scripts/prepare-winget-manifest.ps1 -ValidateWinGet` from this release's own assets. The
   installer hash in the manifest (`d3ce038f...`) is the one in the public release's `SHA256SUMS`.
   WinGet serves 2.0.2 until Microsoft merges it.
7. Website: `scripts/publish-website.ps1` pushed the refreshed public status to `sylin-org/website`
   (`c44f4bf`). `scripts/check-public-surfaces.ps1 -Online` then passed: GitHub, npm, the Chrome
   update feed, the MCP Registry and the website agree on 2.0.3 with adapter 2.0.2.

## Public install

- Windows, the owner's machine: `npx -y ghostlight@2.0.3 deployment install --restore` downloaded
  the three images from the GitHub release, verified each, installed them and started the service;
  `ghostlight status` read 2.0.3 on the release channel with a browser attached.
