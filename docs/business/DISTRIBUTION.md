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
- `site/`: the landing + install pages (GitHub Pages via `.github/workflows/pages.yml`);
  the extension opens `install.html?from=extension` on first install.
- README quick-install block with Cursor/VS Code deeplink buttons and the npx snippet.
- `server.json` (MCP registry descriptor), `packaging/winget/`, `packaging/scoop/`,
  `packaging/homebrew/` (templates; hashes come from release assets).

## Founder: accounts and publishes (order matters)

- [ ] **npm: claim the name.** `npm login` (create the account if needed), then
      `cd packaging/npm && npm publish`. The name `ghostlight` was unclaimed on 2026-07-07;
      it is the single most squattable asset in this plan -- do this the day the v0.3.0
      release is live (the launcher downloads release assets, so the release must exist
      first). Enable 2FA on the npm account.
- [ ] **Chrome Web Store: submit.** Screenshots + Privacy tab per
      docs/legal/STORE_LISTING.md, then submit for review. The extension zip must be the
      v0.3.0 build (it carries the first-run install-page tab). Also submit to the
      **Edge Add-ons store** (free, same zip, far less competition).
- [ ] **MCP Registry (official).** Install `mcp-publisher`, validate `server.json`
      (repo root), authenticate via the GitHub method, publish. The registry feeds client
      UIs; the npm package must be live first.
- [ ] **Cline MCP marketplace.** Submit per their repo's process (issue/PR with the npm
      identifier + logo). Cline users are exactly the persona the first-party path excludes.
- [ ] **Directory listings** (each ~10 minutes, all free): Smithery, Glama, mcp.so,
      PulseMCP. Then a PR to `punkpeye/awesome-mcp-servers` (one line, browser category).
- [ ] **winget PR.** Copy `packaging/winget/Sylin.Ghostlight.yaml` into the three-file
      layout under `manifests/s/Sylin/Ghostlight/0.3.0/` in a fork of microsoft/winget-pkgs,
      fill the sha256 from the release `.sha256` asset, `winget validate`, open the PR.
- [ ] **Homebrew tap.** Create the public repo `sylin-org/homebrew-tap`, add
      `Formula/ghostlight.rb` from `packaging/homebrew/ghostlight.rb` with the three sha256
      values filled. Users then `brew install sylin-org/tap/ghostlight`.
- [ ] **Scoop.** `packaging/scoop/ghostlight.json` with the sha filled can be installed
      directly by URL (`scoop install <raw-url>`); optionally submit to the scoop `extras`
      bucket later. The manifest carries autoupdate, so it is a one-time fill.
- [ ] **Pages custom domain (optional).** Map `ghostlight.sylin.org` (or keep github.io;
      the extension and scripts point at github.io, which redirects automatically once a
      custom domain is set).

## Founder: the launch moment (do these together, after CWS approval)

- [ ] Record the 90-second demo (OBS + scripts/live-demo.ps1), export the README hero GIF.
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
