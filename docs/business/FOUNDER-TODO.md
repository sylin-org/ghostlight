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

- [ ] GitHub Pages site skeleton + sylin.org DNS (half a day; content arrives from the
      frontier queue).
- [ ] Stripe account; draft (unpublished) payment links for team and enterprise.
- [ ] Chrome Web Store developer account ($5 one-time) and submit the prepared CWS
      package (scripts/package-extension.ps1 output).
- [ ] Record the demo: scripts/live-demo.ps1 under OBS, cut to ~90 seconds, export GIF
      for the README and MP4 for the site. (1 evening.)
- [ ] Write the Show HN post yourself (founder voice; HN detects ghostwriting). Post
      it; stay in the thread all day; DM the "we need this at work" commenters.
- [ ] Submit the free listings: official MCP servers directory, Smithery, Glama,
      mcp.so, PulseMCP, relevant Awesome lists.

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
