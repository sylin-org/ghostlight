# Distribution runbook

Last checked: 2026-08-05.

The distribution push (2026-07-07 session; agentic Tier 0-2 implemented in-repo, external
submissions and Tier 3 are founder actions). Ordered: each step assumes the ones above it.
Check off in place with dates, like FOUNDER-TODO.md. The strategy note behind this list lives
in the session record and docs/research/14 (P1 was "ship the distribution already built").

## Already implemented in-repo (this session; lands with the v0.3.0 release)

- `scripts/get.sh` / `scripts/get.ps1`: one-line installers (download the three latest release
  binaries, run `ghostlight install`). They fetch the raw per-target binaries release.yml uploads.
- `packaging/npm/`: the `ghostlight` npm launcher (name verified FREE on npm as of
  2026-07-07). Downloads the three version-matched binaries on first run; stderr-only chatter.
- The root Cargo workspace defaults build all three product executables together for source users.
- `site/`: redirect fallbacks for the canonical `https://sylin.org/ghostlight/` pages;
  the extension opens `https://sylin.org/ghostlight/chromium-extension/post-install/` on first
  install.
- README quick-install block with Cursor/VS Code deeplink buttons and the npx snippet.
- `server.json` (MCP registry descriptor), `packaging/winget/`, `packaging/scoop/`,
  `packaging/homebrew/` (templates; hashes come from release assets).

## Artifact shape after the ADR-0096 cutover release

Every post-cutover release ships three executables side by side: `ghostlight-mcp-connector` (the exact-revision MCP stdio
edge), `ghostlight` (the CLI and persistent protocol-neutral service), and `ghostlight-browser-connector` (the
browser-only native-messaging host). Each platform archive carries all three, and release.yml
uploads all three as raw per-target binaries too. The install scripts, npm launcher, MCPB, and
winget/scoop/homebrew templates place them together in one directory. MCP-client entries launch
`ghostlight-mcp-connector`; the Chromium native-host manifest independently launches `ghostlight-browser-connector`.

## Founder: accounts and publishes (order matters)

- [x] **npm.** `ghostlight@0.7.3` is live at `latest`. The release pipeline publishes it and smoke
      tests the launcher against the integrity-pinned release binaries.
- [x] **Chrome Web Store.** Adapter v0.7.1 is public. Adapter v0.8.0 is accepted for review with
      deferred publication according to the owner dashboard and `docs/public-status.json`.
      Recheck both before any release or store claim. Store id:
      `lejccfmoeogmhemakeknjjdhkfkgncdl`.
- [ ] **Edge Add-ons store.** Submit the same packaged extension after configuring the Edge
      publisher credentials.
- [x] **MCP Registry (official).** Published as `org.sylin/ghostlight`; v0.7.3 is active and
      latest. The release pipeline publishes each service version after npm.
- [x] **GitHub MCP Registry / VS Code `@mcp` discovery.** The founder sent the one-time
      onboarding request to `partnerships@github.com` on 2026-07-31. GitHub completed its review
      and approved `org.sylin/ghostlight` for inclusion on 2026-08-03. GitHub will add the server
      to the catalog; no further founder action is required.
- [ ] **Cline MCP marketplace.** Submission issue
      [#1989](https://github.com/cline/mcp-marketplace/issues/1989) was refreshed in place on
      2026-08-05 with the current 0.7.3 package, 25-tool surface, extension, platform proof,
      install path, and Trust Center link; awaiting maintainer review.
- [ ] **Directory listings.** Glama indexes Ghostlight, and ownership was verified on 2026-08-01
      through the root `glama.json`. On 2026-08-05 its card scored Ghostlight A for license, A for
      quality, and A for maintenance, showed one favorite, and graded `computer` B. Its ingested
      project copy was manually synchronized to public README commit `f1423bae`; the editable
      profile description was also shortened to one accurate sentence. Glama's overview renders
      the repository README and has no separate overview field in the maintainer profile. The free
      mcpservers.org Development listing is live at
      `https://mcpservers.org/servers/sylin-org/ghostlight`. One refresh request was accepted on
      2026-08-05, but the copied project text remained stale on later checks. A self-contained
      Windows/macOS MCPB source, launcher, release packager, and validation gate now prepare the
      Claude Desktop path. Anthropic submission remains gated on a released asset and clarification
      of the live form's MIT-only requirement because Ghostlight is open-core. OpenAI's public
      plugin form currently requires a public production HTTPS MCP endpoint and is incompatible
      with ADR-0077's local-only boundary. The exact packets and inquiry drafts are in
      `docs/business/DIRECTORY-SUBMISSIONS.md`. Smithery can be reconsidered after the MCPB ships,
      while its main audience remains hosted integrations. PR
      [#11306](https://github.com/punkpeye/awesome-mcp-servers/pull/11306) adds Ghostlight to
      `punkpeye/awesome-mcp-servers` under Browser Automation. Its Glama badge check passes, and the
      PR was marked ready for maintainer review on 2026-08-01. mcp.so and PulseMCP remain open.
- [x] **Winget.** v0.7.2 PR #410996 merged and is publicly discoverable. The v0.7.3 manifest
      validates locally, and PR [#411087](https://github.com/microsoft/winget-pkgs/pull/411087)
      merged on 2026-08-02. The merge proves catalog ingestion, not installs.
- [x] **Homebrew tap.** `sylin-org/homebrew-tap` is live. Release v0.7.3 was published in commit
      `5055db1`; users install it with `brew install sylin-org/tap/ghostlight`. Formula metadata
      was refreshed through PR #1 and merged as `f60cdd1c` on 2026-08-05 without changing the
      package version, archives, or checksums.
- [ ] **Scoop.** `packaging/scoop/ghostlight.json` with the sha filled can be installed
      directly by URL (`scoop install <raw-url>`); optionally submit to the scoop `extras`
      bucket later. The manifest carries autoupdate, so it is a one-time fill.
- [x] **Canonical website.** `https://sylin.org/ghostlight/` is live. The old GitHub Pages paths
      are redirect fallbacks, and extension/install entry points use the canonical domain.
- [x] **GitHub front doors.** The repository About description and Sylin organization profile
      carry the current delight-led 0.7.3 positioning. Organization-profile PR #1 merged as
      `64f763cb` on 2026-08-05.

## Founder: the launch moment

- [ ] Record the sub-90-second demo (`ghostlight demo` + OBS) using the exact recipe in
      `docs/legal/STORE_LISTING.md`; upload it unlisted to YouTube for CWS and export the README
      hero GIF from the same recording. The existing 1:44 unlisted product tour at
      `https://www.youtube.com/watch?v=Xk3L4jACgmk` now has a useful title, description, and
      canonical product, source, and privacy links; it does not retire the sub-90-second task.
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
