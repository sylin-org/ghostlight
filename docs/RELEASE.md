# Releasing Ghostlight

This is the single, complete map of what a Ghostlight release touches, which parts are automated,
and what still needs a human. The driver is `scripts/release.ps1 <version>`, run from `main` after
the `dev -> main` release PR is merged. Everything downstream of the git tag reacts to the assets
the Release workflow builds.

The release spans three first-party repositories plus external registries:

- `sylin-org/ghostlight` (this repo): binaries, the npm launcher, package managers, the browser
  extension, and the MCP registry entry.
- `sylin-org/website` (sylin.org): the install guide and demo pages. It is designed to auto-track
  this repo, so a release touches it only lightly (see "Website" below).
- `sylin-org/homebrew-tap`: the Homebrew formula and its release-asset integrity pins.

## The channel map

| Channel | What ships | Automation | Driven by |
| --- | --- | --- | --- |
| GitHub Release | cross-platform binaries, raw per-target bins, SBOM, the store-ready extension zip, checksums, Sigstore attestations | Automated | tag push -> `.github/workflows/release.yml`; `release.ps1` tags, watches, verifies |
| npm (`ghostlight`) | the launcher that fetches the matching release binary, integrity-pinned | Automated | `release.ps1` (`sums` writes `checksums.json`, `npm` publishes) |
| Homebrew tap (`sylin-org/homebrew-tap`) | the formula (version + macOS/Linux sums) | Automated | `release.ps1` (`tap`) |
| Scoop | in-repo manifest `packaging/scoop/ghostlight.json` | Automated | `release.ps1` (`sums`) |
| Winget | in-repo manifest `packaging/winget/Sylin.Ghostlight.yaml` | Semi: sums filled automatically; a PR to `microsoft/winget-pkgs` is manual, per version | `release.ps1` fills; you open the PR |
| Chrome Web Store | the extension zip (`key` stripped, dev files excluded) | Automated when store creds are set, else printed steps | `release.ps1` (`extension`) -> `publish-extension.ps1` |
| Edge Add-ons | the same zip | Automated when store creds are set, else printed steps | `release.ps1` (`extension`) -> `publish-extension.ps1` |
| Trust center (`docs/trust/`) | "reviewed against vX.Y.Z" footer restamp | Automated | `release.ps1` (`trust`) |
| Website (sylin.org) | refresh the install-guide fallback + trigger a rebuild | Automated | `release.ps1` (`website`) -> `publish-website.ps1` |
| MCP Registry | `server.json` entry | Automated when `MCP_DNS_PRIVATE_KEY` is set, else skipped | `release.ps1` (`registry`) -> `mcp-publisher` (DNS auth) |

The privileged GitHub publisher deliberately has no repository checkout. Every `gh` mutation in
that job must therefore pass an explicit repository identity; do not make it depend on `.git` state.

`release.ps1` runs these as ordered, resumable steps: `preflight, tag, watch, verify, sums, tap,
npm, registry, trust, extension, website, report`. Each step detects whether it is already done and
skips, so the script is safe to re-run; resume at any step with `-From <step>`.

## Prerequisites

Every release needs these (the script checks them in `preflight`):

- `git`, `gh` (authenticated: `gh auth status`), and `npm` (logged in: `npm whoami`) on PATH.
- You are on `main`, the tree is clean, and `main == origin/main`.
- All version files agree on the release version (bump them on `dev` before the release PR; the
  same list `release.ps1` checks: the four `Cargo.toml`s, `extension/manifest.json`,
  `packaging/npm/package.json`, `server.json`, the scoop/winget/homebrew manifests).
- `CHANGELOG.md` has a `## [<version>]` section.

Optional, for the automated extension and website steps (see "One-time credential setup" below).
Without them, the extension step prints exact manual submission instructions instead of failing.

## Cutting a release

1. Land everything on `dev`, bump every version surface, write the CHANGELOG section, and merge the
   `dev -> main` PR (CI green). `main` now carries the release commit.
2. From `main`:

   ```
   pwsh -File scripts/release.ps1 <version> -DryRun   # preview the whole plan, mutate nothing
   pwsh -File scripts/release.ps1 <version>           # live: confirms the two irreversible steps
   ```

   The live run: tags `v<version>` (this fires the Release workflow), watches it green, verifies
   every expected asset, fills and commits the package-manager checksums, updates the homebrew tap,
   publishes npm and smoke-tests the launcher, restamps the trust footers, publishes the extension
   (auto or printed steps), and refreshes the website fallback.

3. Do the one remaining manual channel (the `report` step reminds you):
   - **Winget**: run `scripts/prepare-winget.ps1`. It writes the three-file
     `microsoft/winget-pkgs` submission tree under the system temp directory and runs
     `winget validate`. Copy that version directory into a `winget-pkgs` fork and open the PR
     (needs the one-time CLA). Use `-OutputRoot <fork-root>` to write into a chosen checkout.

Useful flags: `-From <step>` resumes after a partial run; `-SkipTap`, `-SkipNpm`, `-SkipExtension`,
`-SkipWebsite`, `-SkipRegistry` skip a channel; `-Yes` skips the interactive confirmations.

### MCP registry (`MCP_DNS_PRIVATE_KEY`)

The `registry` step publishes `server.json` to registry.modelcontextprotocol.io via `mcp-publisher`
(downloaded on demand, pinned). Authentication is DNS ownership of the namespace's domain
(`org.sylin/...` -> `sylin.org`): a one-time apex TXT proof record, `v=MCPv1; k=ed25519; p=<pubkey>`,
must stay in place. Generate the ed25519 key with `openssl` (see `local/RELEASE-CREDENTIALS.md` / the
audit log), store the private hex as `MCP_DNS_PRIVATE_KEY`, and the step logs in and publishes. The
registry is immutable per version, so re-running the same version is a no-op. If the key is unset,
the step skips (not fatal).

## Extension stores

The store zip is one artifact, produced by `scripts/package-extension.ps1`: it stages `extension/`,
excludes dev-only files, and STRIPS the manifest `key` field (the Chrome Web Store rejects a `key`
on upload). The Release workflow builds this exact zip as the `ghostlight-extension-v<version>.zip`
release asset (via `pwsh` on the runner), so the shipped asset is directly submittable.

`scripts/publish-extension.ps1` publishes it. For each store, if the credentials are present in the
environment it uploads and publishes via the store's API; otherwise it prints the manual dashboard
steps (pointing at the built zip). It never fails a release for a missing credential. The
`release.ps1` `extension` step runs it ONLY when `extension/` actually changed since the previous
tag (a Rust-only release needs no store resubmission).

Run it standalone any time:

```
pwsh -File scripts/publish-extension.ps1 -DryRun          # show the plan + which creds are set
pwsh -File scripts/publish-extension.ps1                  # publish where creds exist, else steps
pwsh -File scripts/publish-extension.ps1 -Target trustedTesters -SkipEdge
```

## Website (sylin.org)

The website (`sylin-org/website`, an Eleventy site deployed by an external host that builds on push
to its `main`) is built to auto-track this repo: `src/_data/ghostlightInstall.js` fetches
`llms-install.md` from ghostlight's `main` at build time and republishes it at
`sylin.org/ghostlight/install.md`, with a committed fallback snapshot as a safety net. The install
guide is version-agnostic and the demo pages are static, so there are NO version or download strings
to bump.

`scripts/publish-website.ps1` therefore does the one thing a release needs: clone the website repo,
copy this repo's `llms-install.md` over the committed fallback, and push if it changed (which
triggers the host's rebuild, and the rebuild re-fetches the live guide). If the guide is unchanged,
the live site already serves it and nothing is pushed; `-ForceRebuild` pushes an empty commit to
rebuild anyway.

## One-time credential setup (for the automated store/website steps)

Store these as environment variables in your release shell or a secret manager. NEVER commit them.

### Chrome Web Store (`CWS_*`)

Uses the Chrome Web Store API v1.1 (OAuth2). One-time setup:

1. In a Google Cloud project, enable the "Chrome Web Store API".
2. Create an OAuth client of type "Desktop app"; note the client id and secret.
3. Obtain a refresh token once via the OAuth consent flow for scope
   `https://www.googleapis.com/auth/chromewebstore`. `scripts/get-cws-refresh-token.ps1` runs the
   loopback flow end to end (opens the consent page, catches the redirect, prints the refresh
   token); authorize as the account that owns the listing.
4. The item id is the extension's Web Store id (in the dashboard URL).

Set: `CWS_CLIENT_ID`, `CWS_CLIENT_SECRET`, `CWS_REFRESH_TOKEN`, `CWS_ITEM_ID`. The full click-by-click
walkthrough (kept machine-local) is `local/RELEASE-CREDENTIALS.md`.

### Edge Add-ons (`EDGE_*`)

Uses the Edge Add-ons API v1.1 (Partner Center). In Partner Center -> the extension ->
Publish API, create an API credential: note the product id (the extension's Partner Center product
id), the client id, and the API key.

Set: `EDGE_PRODUCT_ID`, `EDGE_CLIENT_ID`, `EDGE_API_KEY`.

### Website

No extra credentials: `publish-website.ps1` uses your existing `gh` auth to clone and push the
website repo.

## After a release

- **CHANGELOG date / next cycle**: start the next `## [Unreleased]` section on `dev` as work lands.
- **Chrome/Edge review latency**: store publishing is queued for review (hours to a few days); the
  automated step returns as soon as the store accepts the submission, not when it goes live.
- **Verify**: the GitHub release page, `npm view ghostlight version`, `brew info sylin-org/tap/ghostlight`,
  and `sylin.org/ghostlight/install.md`.
