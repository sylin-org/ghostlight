# Founder to-do (personal actions only)

Personal checklist for actions only the founder can take. Agentic work is NOT tracked
here (it lives in docs/tasks/licensing-1/ and the frontier queue in
docs/business/PLAN.md). Check items off in place; add dates.

## Now (Phase 0, unblocks everything else)

- [ ] Cloudflare Email Routing on sylin.org; single sink address hello@sylin.org
      (decided 2026-07-04; aliases can come later); Gmail send-as for outbound.
      (~1 hour. Blocks: founding applications, the SECURITY.md contact going live,
      renewal emails. PRICING.md and SECURITY.md already publish hello@sylin.org.)
- [ ] Create the PRIVATE GitHub repo `ghostlight-licensing`; copy
      docs/business/templates/expiry-reminder-workflow.yml into
      .github/workflows/ there; create the issued/ directory.
- [ ] Generate the production signing seed OFFLINE:
      `openssl rand -out ghostlight-signing-gen1.bin 32`; store the file offline with
      one encrypted backup (never in any repo, never in CI). After licensing-1 lands,
      print its verifying key with
      `cargo run --features license-admin -- license pubkey --key ghostlight-signing-gen1.bin`
      and add it as keygen 1 in src/governance/license.rs (one-line constant; commit).
- [ ] Legal skim of LICENSE-GOVERNANCE (standing flag since ADR-0027). Free options:
      careful self-review against the EE-template family it derives from; a startup
      legal clinic if available.
- [ ] Approve or amend: pricing numbers, tier names, the Continuity Promise wording
      (ADR-0028 Decisions 5-6), and the founding agreement template (after l06 lands).

## Launch window (Phase 1)

- [x] First tagged release -- **v0.1.0 shipped 2026-07-04**
      (https://github.com/sylin-org/ghostlight/releases/tag/v0.1.0): 4 platform binaries
      + extension zip, checksums, signed build-provenance. CI/release infra harvested
      from Koi. (Agent-driven; listed here only as the milestone marker.)
- [ ] GitHub Pages site skeleton + sylin.org DNS (half a day; content arrives from the
      frontier queue).
- [ ] Stripe account; draft (unpublished) payment links for team and enterprise.
- [ ] **Chrome Web Store listing (IN PROGRESS, step by step).** Account created
      (hello@sylin.org, non-trader, $5 paid, individual identity verification pending). Item
      created as a DRAFT named **"Ghostlight in Browser"**; store-assigned id
      **lejccfmoeogmhemakeknjjdhkfkgncdl**. Renamed package uploaded and its name verified in the
      dashboard. Remaining founder steps:
      >=1 screenshot at exactly 1280x800 (capture recipe + shot list now in
      docs/legal/STORE_LISTING.md "Graphic assets checklist": Chrome DevTools device mode, NOT the
      agent screenshot tool, which hides the effects and mis-sizes); fill the Privacy tab (also
      paste-ready in that file); submit. Screenshots must be the REAL extension, not the
      FX-dictionary Artifact.
- [ ] **Verified CRX uploads -- future hardening, NOT at launch.** The CWS "Verified CRX
      uploads" opt-in ties item updates to a private signing key you hold (blocks account-
      takeover -> poisoned update). Deferred deliberately: it forces signed-CRX uploads from
      the next upload on, and losing the key locks you out of updating your own item. Turn it
      on once the release cadence is stable, paired with the same offline key-management
      discipline as the licensing signing seed above.
- [x] **v0.3.0 release (supersedes the v0.2.0-release item, post-eval P1).** The v0.2.0 tag's
      release run FAILED on a known flaky test (rewritten under ADR-0032; the tag predates the
      fix) and the tag also predates the 17-tool composition surface the docs advertise. The
      2026-07-07 distribution session fixed dev CI (proc.rs/supervisor.rs cross-platform
      compile, console truncation lingering-close, e2e-smoke 6h-hang cap) and cut v0.3.0 from
      dev instead. Verify the release run went green end to end, then work
      **docs/business/DISTRIBUTION.md** top to bottom (npm name claim FIRST -- `ghostlight`
      was unclaimed on npm as of 2026-07-07).
      DONE: v0.3.0 shipped 2026-07-08; v0.4.0 released + `ghostlight` published to npm 2026-07-09
      (see the DISTRIBUTION.md npm step and the 2026-07-09 decision-log entry below).
- [ ] **Post-eval verification debts (P10; these gate the credibility of the claims above).**
      Run docs/tasks/composition/LIVE-VERIFY.md (13 pinned observations); live-verify macOS and
      Linux; unquarantine e2e-smoke or record the design decision; the LICENSE-GOVERNANCE
      legal skim is already listed under Phase 0 above.
- [ ] **Official-extension re-baseline (P7, operator-assisted).** Research 12 pinned
      v1.0.78; the official surface has since added browser_batch, GIF recording, scheduled
      tasks, and escalating flags. Update the installed extension, let an agent session re-run
      the research-12 harvest against it, and confirm the fidelity snapshot still passes.
      Output: a delta note in docs/research/, before any harvest ADR (ADR-0041 D2).
- [ ] Record the demo: run `ghostlight demo` under OBS using `docs/legal/STORE_LISTING.md`, keep it
      below 90 seconds, upload the MP4 unlisted to YouTube for CWS, and export a GIF for the README.
      (One recording, three uses.)
- [ ] Write the Show HN post yourself (founder voice; HN detects ghostwriting). Post
      it; stay in the thread all day; DM the "we need this at work" commenters.
- [ ] Submit the free listings: official MCP servers directory, Smithery, Glama,
      mcp.so, PulseMCP, relevant Awesome lists.
- [ ] Enable GitHub Discussions on the repo (Settings -> Features) and post a short
      welcome pinned thread pointing at CONTRIBUTING.md's three lanes; enable the
      Q&A and Ideas categories.

## Ongoing (Phase 2 onward)

- [ ] Respond to hello@ founding applications; sign the one-page agreement; issue founding
      licenses (sign with the gen-1 seed; commit claims JSON to ghostlight-licensing).
- [ ] Quarterly: email the founding questionnaire (docs/business/templates/
      founding-questionnaire.md) to each founding org; harvest an anonymized policy
      pattern from the replies into the examples/ cookbook.
- [ ] Renewal emails when the private repo's Action opens a T-30/T-7 issue (templates:
      docs/business/templates/). Lead with the Continuity Promise, always.
- [ ] Monthly tagged release; watch the first CI run of every new workflow
      (ci.yml jobs, release.yml SBOM) since none has executed on GitHub yet.
- [ ] Trademark: use (TM) on "Ghostlight" now; file (~$250-350) when the first paid
      license lands.
- [ ] August 2026: publish the EU AI Act piece (date-pegged; high-risk obligations
      phase in that month).

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
