# Distribution runbook

The distribution push (2026-07-07 session; agentic Tier 0-2 implemented in-repo, external
submissions and Tier 3 are founder actions). Ordered: each step assumes the ones above it.
Check off in place with dates, like FOUNDER-TODO.md. The strategy note behind this list lives
in the session record and docs/research/14 (P1 was "ship the distribution already built").

## Already implemented in-repo (this session; lands with the v0.3.0 release)

- `scripts/get.sh` / `scripts/get.ps1`: one-line installers (download latest release binary,
  run `ghostlight install`). They fetch the RAW per-target binaries release.yml now uploads.
- `packaging/npm/`: the `ghostlight` npm launcher (name verified FREE on npm as of
  2026-07-07). Downloads the version-matched binary on first run; stderr-only chatter.
- cargo-binstall metadata in Cargo.toml.
- `site/`: redirect fallbacks for the canonical `https://sylin.org/ghostlight/` pages;
  the extension opens `https://sylin.org/ghostlight/chromium-extension/post-install/` on first
  install.
- README quick-install block with Cursor/VS Code deeplink buttons and the npx snippet.
- `server.json` (MCP registry descriptor), `packaging/winget/`, `packaging/scoop/`,
  `packaging/homebrew/` (templates; hashes come from release assets).

## Artifact shape (ADR-0046, ADR-0051 Phase 3: two executables)

Every release ships two executables side by side: `ghostlight` (the CLI and the persistent service)
plus the single thin pass-through `ghostlight-relay`, which carries both former roles -- the
MCP-client side (`--role agent`) and the Chrome native-messaging side (browser role, auto-detected
from the extension origin Chrome passes). Each platform archive carries both, and release.yml uploads
both as raw per-target binaries too. The install scripts, the npm launcher, and the
winget/scoop/homebrew templates place the two together in one directory, so `ghostlight install`
resolves the relay as a sibling.

## Founder: accounts and publishes (order matters)

- [x] **npm.** `ghostlight@0.7.3` is live at `latest`. The release pipeline publishes it and smoke
      tests the launcher against the integrity-pinned release binary.
- [x] **Chrome Web Store.** Adapter v0.6.0 is public and v0.7.1 is pending review. Store id:
      `lejccfmoeogmhemakeknjjdhkfkgncdl`.
- [ ] **Edge Add-ons store.** Submit the same packaged extension after configuring the Edge
      publisher credentials.
- [x] **MCP Registry (official).** Published as `org.sylin/ghostlight`; v0.7.3 is active and
      latest. The release pipeline publishes each service version after npm.
- [ ] **GitHub MCP Registry / VS Code `@mcp` discovery.** The founder sent the one-time
      onboarding request to `partnerships@github.com` on 2026-07-31. Initial admission is
      manually curated; later versions sync from the official MCP Registry. Monitor the GitHub
      catalog and VS Code discovery. If Ghostlight is not listed and GitHub has not replied,
      follow up on the same email thread on 2026-08-28.
- [ ] **Cline MCP marketplace.** Submission issue
      [#1989](https://github.com/cline/mcp-marketplace/issues/1989) was updated with the live
      package, extension, and install path on 2026-08-01; awaiting maintainer review.
- [ ] **Directory listings.** Glama indexes Ghostlight, and ownership was verified on 2026-08-01
      through the root `glama.json`. Its Docker introspection configuration is saved; Glama's
      builder is still resolving the base image after the first test exposed the Linux release's
      glibc 2.39 floor. The v0.7.3 tool-definition improvements are on `main` and published; their
      Glama scores depend on its next successful crawl and introspection. Smithery is deferred
      because its local-server path requires a maintained MCPB bundle while its main audience and
      value are hosted integrations; revisit only if MCPB becomes a useful product channel on its
      own. Draft PR
      [#11306](https://github.com/punkpeye/awesome-mcp-servers/pull/11306) adds Ghostlight to
      `punkpeye/awesome-mcp-servers` under Browser Automation; its Glama badge check passes, while
      the quality score waits for introspection. mcp.so and PulseMCP remain open.
- [ ] **Winget.** v0.7.2 PR #410996 merged and is publicly discoverable. The v0.7.3 manifest
      validates locally and PR #411087 is open and mergeable; its CLA check passes while Microsoft
      validation and review remain pending.
- [x] **Homebrew tap.** `sylin-org/homebrew-tap` is live. Release v0.7.3 was published in commit
      `5055db1`; users install it with `brew install sylin-org/tap/ghostlight`.
- [ ] **Scoop.** `packaging/scoop/ghostlight.json` with the sha filled can be installed
      directly by URL (`scoop install <raw-url>`); optionally submit to the scoop `extras`
      bucket later. The manifest carries autoupdate, so it is a one-time fill.
- [x] **Canonical website.** `https://sylin.org/ghostlight/` is live. The old GitHub Pages paths
      are redirect fallbacks, and extension/install entry points use the canonical domain.

## Founder: the launch moment

- [ ] Record the sub-90-second demo (`ghostlight demo` + OBS) using the exact recipe in
      `docs/legal/STORE_LISTING.md`; upload it unlisted to YouTube for CWS and export the README
      hero GIF from the same recording.
- [ ] **Show HN** -- founder-written (HN detects ghostwriting). Lead: "Claude-in-Chrome's
      governance model, open and vendor-neutral"; hooks: the UW study, the honest
      COMPARISON.md, the delight GIF. Stay in the thread all day.
- [ ] Free listings + Discussions welcome thread (already on FOUNDER-TODO.md).

## Founder: Tier 3 homework (compounding loops; reuse existing docs, do not rewrite them)

- [ ] **Client-vendor emails** (Cline, Cursor, Zed): three short founder-voice emails --
      "your users are asking for parity with claude --chrome; here is a vendor-neutral way,
      may we be listed?" Source material: docs/COMPARISON.md (the first-party section) and
      the install page. No new docs needed.
- [ ] **Stranded-user etiquette**: watch hangwin/mcp-chrome and BrowserMCP/mcp issues for
      "is this maintained?" questions; answer honestly with a pointer. Never spam; answer
      questions only.
- [ ] **EU AI Act piece (August 2026)**: already scheduled on FOUNDER-TODO.md; source
      material is docs/guides/compliance-team.md + open-spec/rawx-owasp-agentic-mapping.md.
- [ ] **UW-study post** ("your agent's browser needs an audit trail"): source material is
      docs/research/14 + the mapping doc; publish on the site + dev.to; this is the
      security-narrative hook (owner ruling: origin-flow is the focus).
- [ ] **RAWX at an MCP community call**: present open-spec/rawx-capability-model.md as a
      vendor-neutral proposal; the goal is vocabulary adoption, not product pitching
      (ADR-0041 Decision 1 posture).
- [ ] **Recording-as-growth-loop**: when the session-recording harvest ADR lands (ADR-0041
      D2 candidate), treat shared workflow GIFs as the distribution loop; until then the
      README hero GIF carries it.
- [ ] **GTM sequencing**: docs/business/PLAN.md remains the master GTM doc; fold this
      runbook's outcomes back into it as items close.

## Standing risks this list retires

- The npm name being squatted (step 1).
- The extension dev-mode cliff (CWS submit).
- "Found the extension, no idea what the binary is" (first-run tab -> install page).
- "Found the repo, gave up at step 3" (one-liners + doctor).
