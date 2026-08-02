# Founder to-do (personal actions only)

Personal checklist for actions only the founder can take. Agentic work is NOT tracked
here (it lives in docs/tasks/licensing-1/ and the frontier queue in
docs/business/PLAN.md). Check items off in place; add dates.

Last reconciled: 2026-08-01. This checklist is current through Ghostlight v0.7.3.

## Now (owner-only foundations)

- [ ] Verify Gmail send-as for `hello@sylin.org`. Cloudflare Email Routing, MX, and SPF are live;
      inbound delivery is no longer a blocker.
- [ ] Review and approve the five license-operations templates after the agentic `l06` task is
      refreshed and landed. The preserved task predates ADR-0028 Decision 10 and still names
      Stripe, so do not transcribe it verbatim.
- [ ] After those templates exist, create the PRIVATE `ghostlight-licensing` GitHub repo. Copy the
      expiry-reminder workflow into `.github/workflows/` and create `issued/`. Commit only claims
      JSON inside `issued/` -- never seeds or signed license files.
- [ ] Create production key generation 1 on an air-gapped machine by following
      `docs/business/issuing-licenses.md`. Production signing requires TWO offline 32-byte seeds:
      Ed25519 and ML-DSA-65. Back both up offline, print both public keys with `--seed` and
      `--mldsa-seed`, then approve the public-key commit in
      `crates/core/src/governance/license/crypto.rs`. Never put either seed in a repo, CI, synced
      folder, or online service.
- [ ] Legal review: `docs/licenses/LicenseRef-Ghostlight-Commercial.txt`, the MSA, DPA, and the
      founding-organization agreement after its template lands. Resolve the vendor entity name and
      cyber-insurance yes/no line before the first commercial execution.
- [x] Pricing, tier names, and the Continuity Promise are published and frozen as the initial
      offer (2026-07-04). Future changes require a new explicit decision.
- [ ] Verify privileged-account MFA. Decide and configure branch/direct-push protection, GitHub
      secret scanning and push protection, and Dependabot security updates.
- [ ] Add a second trusted npm publisher and verify recovery for the GitHub, npm, and CWS publisher
      accounts. Define offline recovery for any future Verified CRX private key before opting in.
      This is separate from the air-gapped production signing-seed backup above.

## Public launch and distribution

- [x] The canonical `https://sylin.org/ghostlight/` site, DNS, install routes, privacy route, and
      GitHub description, homepage, and discovery topics are live.
- [ ] Upload `docs/assets/social-preview.png` as the repository's custom social preview. GitHub
      still reports the generated default image as of 2026-08-01.
- [x] Ghostlight v0.7.3 is published on GitHub, npm, Homebrew, the official MCP Registry, and the
      website. The release and live-install checks passed on 2026-08-01.
- [x] **Chrome Web Store is public.** Adapter v0.6.0 is live under store id
      `lejccfmoeogmhemakeknjjdhkfkgncdl`; the v0.7.1 adapter is pending review.
- [ ] Monitor the v0.7.1 Chrome review and answer reviewer questions. Keep Verified CRX uploads
      deferred until release cadence and offline key recovery are stable; losing that key would
      prevent future updates.
- [x] The CWS video evidence is present in the listing and the README hero GIF is published.
- [x] Client showcase posts are live in
      [Codex Show and tell](https://github.com/openai/codex/discussions/36424) and
      [Zed Show and tell](https://github.com/zed-industries/zed/discussions/62035).
- [ ] Reuse the existing recordings and GIFs as native proof in suitable public posts. The
      recording prerequisite shipped in v0.5.7; this is a distribution loop, not a product task.
- [ ] Write and post the Show HN entry in founder voice. Protect the active window, stay in the
      thread, and follow up personally with serious users.
- [x] GitHub Discussions, Q&A, and Ideas are enabled for Ghostlight.
- [ ] Post and pin the first Ghostlight welcome Discussion, pointing to CONTRIBUTING.md's three
      participation lanes.
- [x] The official MCP Registry and Glama listings are live. Glama recognizes all 25 tools and
      scores Ghostlight A for license, A for quality, and B for maintenance.
- [ ] Submit the remaining directory entries to mcp.so and PulseMCP.
- [ ] Monitor the open external submissions: Winget
      [#411087](https://github.com/microsoft/winget-pkgs/pull/411087), Cline Marketplace
      [#1989](https://github.com/cline/mcp-marketplace/issues/1989), and
      `awesome-mcp-servers` [#11306](https://github.com/punkpeye/awesome-mcp-servers/pull/11306).
- [ ] Follow up on the GitHub MCP Registry / VS Code discovery email on 2026-08-28 if there is no
      reply and Ghostlight remains absent from the catalog.
- [ ] Configure Edge Add-ons publisher credentials and submit the current compatible adapter.
- [ ] Optionally submit the already working v0.7.3 Scoop manifest to Scoop Extras.
- [ ] Send client-specific founder outreach to Cursor, Zed, and Cline using a concrete proof, not
      a generic launch announcement.
- [ ] Watch relevant `hangwin/mcp-chrome` and `BrowserMCP/mcp` discussions for genuine
      "is this maintained?" questions. Reply only when useful; never seed or spam threads.
- [ ] Recruit first users through public channels and collect consented first-use evidence. A
      private greenfield cohort is not expected.
- [ ] Present RAWX as a vendor-neutral capability vocabulary when an appropriate MCP or agent
      community call appears; the goal is vocabulary adoption, not a product pitch.

## Owner-assisted verification

- [ ] After an agent refreshes `docs/tasks/composition/LIVE-VERIFY.md` for the current 25-tool
      surface and green e2e baseline, supervise the updated 13-observation run. Do not execute the
      preserved checklist verbatim; it still assumes quarantined e2e and the old tool set.
- [ ] Run the ADR-0047 stage-2 supervised real-browser verification.
- [ ] Complete the repeated-model visible baseline evidence.
- [ ] Arrange and capture a consented follow-up non-author review.
- [ ] Live-verify macOS when suitable hardware is available. Linux verification and release CI are
      complete.
- [x] The official-extension rebaseline completed against v1.0.80; the resulting fidelity work is
      shipped.
- [ ] Decide whether to approve the drafted WebMCP response, join Chrome's early preview, and choose
      a controlled experiment origin. This would authorize feedback and a bounded experiment, not
      product support.

## Commercial setup and ongoing work

- [ ] Create a Polar.sh account and draft unpublished team and enterprise checkout links. Polar is
      the merchant of record; Lemon Squeezy is the fallback. Do not use Stripe as the primary
      checkout and never use a vendor's online license-key validation.
- [ ] Decide the funding-link recipient, entity, provider, and accounting/tax treatment. Keep
      repository funding links unset until all four are clear.
- [ ] Respond to founding applications after the agreement and production generation are ready;
      issue licenses offline and commit only claims JSON to `ghostlight-licensing`.
- [ ] Once founding organizations exist, send the quarterly questionnaire and harvest a consented,
      anonymized policy pattern into the examples cookbook.
- [ ] Send renewal mail when the private ledger opens T-30/T-7 reminders. Lead with the Continuity
      Promise every time.
- [ ] Continue monthly tagged releases and inspect every new or changed workflow's first live run.
- [ ] Use `Ghostlight (TM)` now; file the trademark when the first paid license lands.
- [ ] Publish the EU AI Act piece in August 2026.
- [ ] Publish the UW-study audit-trail article on the planned content cadence.

## Decision log (fill in as items close)

- 2026-07-03: ADR-0028 accepted; plan persisted publicly; licensing-1 batch prepared
  (not yet executed).
- 2026-07-04: hello@sylin.org chosen as the single sink address (changeable later).
  Public content pass landed: PRICING.md (pricing numbers now PUBLISHED, freezing the
  ADR-0028 initial prices as list), SECURITY.md, three guides, COMPARISON.md, README
  refresh. First live CI run: core suite green on all three OSes; extension-unit fixed
  forward; e2e-smoke quarantined (continue-on-error) pending log access via gh auth.
- 2026-07-04: **v0.1.0 SHIPPED.** gh CLI authenticated (via Ghostlight dogfooding the
  browser to mint a token). CI hardened (least-privilege, --locked, cargo-audit, per-OS
  cache) + dependabot; release.yml overhauled (dry-run, archives, checksums, provenance,
  GitHub Release) -- all harvested from Koi. main reconciled with dev and is now the
  release branch; dev is trunk. e2e-smoke stays quarantined (native-messaging in headless
  Playwright, a design question). NEXT: Chrome Web Store listing, step by step.
- 2026-07-04: CWS dashboard account created; draft item created; store id
  lejccfmoeogmhemakeknjjdhkfkgncdl. Extension renamed **"Ghostlight Browser" ->
  "Ghostlight in Browser"** (it read like a browser). Decided to DEFER "Verified CRX uploads"
  (launch-day complexity + key-loss lockout) and to leave it as a hardening objective. Also
  shipped (dev): ADR-0029 process-lifecycle fix, the extension SW-registration fix +
  lib/constants.js, the per-action visual feedback vocabulary, the options page + captions.
  Product and project name simplified from "Ghostlight Browser" to **"Ghostlight"** (extension
  stays "Ghostlight in Browser"); README / CLAUDE.md / extension README / script synopses swept and
  ADR-0021 amended. The Visual Feedback Dictionary design artifact was preserved verbatim into
  docs/design/visual-feedback-dictionary.html.
- 2026-07-08: **v0.3.0 SHIPPED** (GitHub Release + Pages site live; distribution Tier 0-2 in-repo).
- 2026-07-09: **v0.4.0 RELEASED + npm PUBLISHED.** dev->main PR merged, tag `v0.4.0` cut; the
  GitHub Release published 34 assets and `ghostlight@0.4.0` went live on npm (unscoped, public,
  under `lbotinelly`) -- `npx -y ghostlight@0.4.0` verified end to end. A latent CI/release bug
  was fixed en route: the test jobs ran `cargo test --workspace` with no preceding
  `cargo build --workspace`, so spawn-based integration tests could not find the adapter
  deliverable binaries (rust-cache masked it until the version bump rotated the cache); the tag
  was moved to the fixed commit and re-run. Publishing used a classic npm **Automation token**
  because the account's 2FA is Windows Hello/WebAuthn (no CLI `--otp`). This release carried
  ADR-0044 (named instances), ADR-0045 (resilient reconnect), ADR-0046 (three role executables),
  ADR-0047 (tab identity), ADR-0048 (development override + the per-user hub-key fix), and
  ADR-0049 (the MCP protocol-conformance pass).
- 2026-08-01: **v0.7.3 SHIPPED.** GitHub, npm, Homebrew, the official MCP Registry, and the
  canonical website agree on the service release. Chrome Web Store v0.6.0 is public; adapter
  v0.7.1 remains under review. Codex and Zed Show and tell posts are live, Glama scores Ghostlight
  A/A/B across all 25 tools. The awesome-mcp, Cline, and Winget paths are in external review; the
  GitHub catalog request was submitted and is pending a response, with Ghostlight still absent.
  The founder checklist was reconciled to current composite signing, Polar.sh, public-channel
  recruitment, and the remaining owner-only gates.
