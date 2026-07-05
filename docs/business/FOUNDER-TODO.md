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
- [ ] **Version bump + re-release (0.1.1/0.2.0).** GitHub v0.1.0 shipped the pre-fix extension
      (SW registration failed) and pre-ADR-0029 binary. Bump both artifacts, re-cut the GitHub
      release with the fixes + FX + options page, and keep the CWS package in step.
- [ ] Record the demo: scripts/live-demo.ps1 under OBS, cut to ~90 seconds, export GIF
      for the README and MP4 for the site. (1 evening.)
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
