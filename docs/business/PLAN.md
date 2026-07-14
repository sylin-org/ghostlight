# Ghostlight go-to-market and monetization plan

Living document. Ratified decisions live in ADR-0027 (open-core) and ADR-0028 (tripwire
licensing, tiers, Continuity Promise); this file is the execution plan around them.
Authored 2026-07-03 with agentic assistance, at the founder's direction, and kept public
by choice so the reasoning behind the business is as inspectable as the code.

Ground truth at authoring: the product is two days old under its current name, has zero
adopters, zero revenue expected in year one, a zero-dollar infrastructure budget, one
founder, and one domain (sylin.org). Every action below is free unless explicitly
flagged with a dollar amount.

## Positioning

Ghostlight is the governed way to give AI agents a real browser. One Rust binary plus a
thin Chromium extension; schema-exact with the official Claude in Chrome surface; a
governance layer (capability manifests, sacred domains, kill switch, audit) that is OFF
by default and first-class when on. The buyer is a security or compliance team enabling
AI browser automation; the adopter is the individual developer who brings it to them.

Three structural advantages no incumbent can easily copy:

1. Local-only, never phones home, works forever offline (ADR-0028 Decision 9 and the
   Continuity Promise). Most vendors cannot say this; procurement teams notice.
2. Source-available governance: the enforcement and audit code is readable by the
   security team that must trust it.
3. Windows-first enterprise ergonomics (registry install, %ProgramData% org policy,
   PowerShell tooling) in a Mac-first tooling market.

## Principles (fixed)

- Never phone home. No telemetry, no activation servers, no update checks.
- Never gate safety. License state never affects behavior (tripwire, not DRM).
- Always behave and work: the Continuity Promise is permanent and marketing-visible.
- Trust corporate compliance mechanisms (SCA scanners, procurement, audit) to do the
  enforcement; our job is to make license state visible, not to police it.
- All-open personal use is first-class forever.

## Tiers and pricing (initial; ADR-0028 Decision 5)

| Tier | Price | Notes |
|---|---|---|
| development | free, self-signed | evaluation anywhere, never production |
| community | free, self-serve key | production, orgs of 5 or fewer |
| founding | free 12 months, then 50% of list forever | 10 slots, reference + quarterly email questionnaire |
| team | ~$12/user/month, annual, 5-seat minimum | self-serve |
| enterprise | from ~$10k/year | procurement paperwork, SLA, deployment help |

Annual billing only. All early customers grandfathered at signup pricing permanently.
Pricing is published on the site from day one, even while free, so procurement can
budget ahead.

## Phase 0 -- Foundation (weeks 1-3)

Engineering (agentic; execution package prepared at docs/tasks/licensing-1/):

- License verifier, `ghostlight license` CLI, audit stamp, doctor section (l01-l04).
- SECURITY.md and release SBOM (l05); license-ops templates (l06).

Engineering (frontier-session work, queued):

- The three audience cards (see "Audience cards" below).
- docs/SPEC.md rewrite (ADR-0026 Decision 3, already owed).
- managed:// org manifest distribution (ADR-0026 Decision 5): sequence together with
  license infrastructure; the pair makes the enterprise tier real.
- macOS/Linux live verification (owed from t-live-1) before any cross-platform claim.
- v0.1.0 tagged release through the release workflow.

Founder actions: see docs/business/FOUNDER-TODO.md.

## Audience cards (frontier-authored docs, Phase 0-1)

1. Solo developer card: install, all-open in 10 minutes, optional personal governance
   (sacred domains, kill switch), where the free line sits.
2. Compliance team card: the full journey the mode system was designed for -- author a
   manifest, run observe, review shadow denials, tighten, shadow, enforce, org-deploy
   with policy locks, verify with `explain` and `policy simulate`, then distribute via
   managed:// when it lands. This card doubles as the product demo.
3. SIEM card: the syslog audit destination, the JSONL record schema, ready-to-paste
   ingestion snippets and a starter dashboard for Splunk, Microsoft Sentinel, and
   Elastic.

## The procurement document pack (what an org needs before approving a license)

All zero-dollar, all frontier-authored, all public under docs/ or the site:

- Security whitepaper: architecture, trust boundaries, threat model; leads with
  local-only/no-telemetry/extension-is-policy-free.
- Data handling statement: the strongest a vendor can write -- no data leaves the
  environment, no processor relationship, no DPA needed.
- Continuity answer (the "what if the solo dev disappears?" question): binary works
  forever offline, license never disables anything, engine is OSS, governance source is
  readable; source escrow is trivially offerable because the source is already
  available. Almost no vendor can match this; it is a named commitment (the Continuity
  Promise) on the pricing page.
- SBOM per release (CycloneDX, generated in CI; licensing-1 l05).
- SECURITY.md with honest solo-dev SLAs (ack 48h, critical fix target 30 days).
- Pre-filled security questionnaire (CAIQ-lite style): most sections are "N/A,
  local-only tool"; written once so every founding org does not cost a week.
- Compliance mapping page: Ghostlight controls in EU AI Act, NIST AI RMF, ISO 42001,
  and SOC 2 language. The EU AI Act high-risk obligations phase in from August 2026;
  a date-pegged post that month is free, timely reach.
- Standard agreement: adapted from Common Paper's free CC-licensed templates rather
  than a paid first draft. The one real legal task is the LICENSE-GOVERNANCE skim
  (standing flag).

## License operations (ADR-0028 Decision 8)

- Private repo `ghostlight-licensing`: signing tooling + one committed claims JSON per
  issued license. The ledger of record. Production signing seed offline only.
- Daily scheduled Action (template: docs/business/templates/expiry-reminder-workflow.yml)
  opens an issue at T-30 and T-7 per expiry. The founder sends the renewal email
  personally (templates: renewal-t30.md, renewal-t0.md). Tone is fixed: nothing stops
  working; renew when procurement is ready.
- Stripe payment links (no fixed cost); manual key issuance within 24 hours until
  volume forces automation.
- Email: Cloudflare Email Routing on sylin.org (receive) + Gmail send-as (send).
  Single sink address: hello@sylin.org for everything (founding applications, security
  reports, licensing); per-purpose aliases can come later without changing anything.

## Phase 1 -- Visibility (weeks 2-6)

- Site on GitHub Pages with the sylin.org domain: landing, the three cards, pricing,
  the Continuity Promise, the comparison page. README overhaul as the storefront with a
  90-second demo GIF (the visual FX -- ripples, ghost tab group -- were accidentally
  built for this; record the built-in `ghostlight demo` tour with OBS).
- Listings, all free: the official MCP servers directory, Smithery, Glama, mcp.so,
  PulseMCP, Awesome-MCP lists. Chrome Web Store listing ($5 one-time developer fee,
  flagged; CWS package already prepared).
- Launch: Show HN, founder-written ("Show HN: governed browser automation for AI
  agents -- single Rust binary, open-core"). Also r/rust (hand-rolled JSON-RPC, single
  binary, no SDK), r/ClaudeAI, the MCP Discord, lobste.rs.
- Comparison page: vs the official Claude in Chrome extension, the community reference
  implementation, Playwright MCP, and the Browser MCP namesake. Raw material exists in
  docs/research/13.
- Content cadence, one post every 1-2 weeks, mined from the ADRs (each is 80% written):
  the capability model (read/action/write/execute as an open vocabulary for agent
  governance), the kill switch, the sacred surface (why trained tool schemas are
  preserved byte-for-byte), the EU AI Act piece in August.

## Phase 2 -- Founding organizations (months 2-6)

- Publish the offer: 10 slots, 12 months enterprise-equivalent free, a short quarterly
  email questionnaire (5-8 topics; template in docs/business/templates/) plus a
  reference (named preferred, anonymized accepted), post-year price locked at 50% of
  list forever. No calls, no meetings. One-page agreement
  (docs/business/templates/founding-org-agreement.md). Applications: hello@sylin.org.
- Sourcing: Show HN reply threads ("we need this at work" commenters), org-affiliated
  stargazers, the MCP Discord, LinkedIn, Anthropic community channels.
- Each onboarding yields an anonymized policy pattern for an examples/ policy cookbook
  (docs and marketing in one artifact).
- No telemetry means success is measured in what founding orgs tell you: the quarterly
  questionnaire asks for their own numbers. Free instrumentation that respects the
  principles: GitHub stars/traffic, CWS install counts, listing referrers.

## Phase 3 -- Revenue (months 9-18)

- Founding conversions begin at month 12-14 at the locked rate.
- Open self-serve team tier once at least two founding orgs are in production
  (social proof exists).
- Honest math: ceiling if all 10 founding orgs convert at enterprise-ish rates is
  roughly $50k ARR; realistic at 30-50% conversion plus self-serve trickle is
  **$15-30k ARR entering year two**. That is side-income, not salary. Year three is
  where compounding content, references, and the product family (desktop-mcp on the
  same policy engine and license schema) change the slope. This paragraph exists so
  nobody is surprised at month 14.

## Sustainability (solo-dev upkeep)

- Support SLAs, honest: free tier = GitHub issues best-effort; team = email,
  3-business-day acknowledgment; enterprise/founding = 2-business-day acknowledgment.
  Never promise 24/7.
- Release cadence: monthly patch releases; the recurring maintenance reality (Chrome,
  CDP, and the official extension's schema evolution) is ALSO the renewal value story:
  the license pays for keeping pace.
- Security response per SECURITY.md. Public roadmap via GitHub milestones.
- Founder time budget during Phase 2: roughly 60% product, 20% content, 20% founding
  org support.

## Opportunity backlog (unscheduled, revisit quarterly)

- Package managers: winget, scoop bucket, homebrew tap (free PRs; winget doubles down
  on Windows-first).
- GitHub Sponsors on the engine as a passive lane separate from commercial licensing.
- crates.io publication of the engine (needs the workspace split into engine +
  governance crates; revisit when there is a reason).
- The capability-manifest format as a published open spec ("agent governance
  manifest"); Ghostlight as reference implementation.
- Anthropic ecosystem: MCP directory presence now; a single well-written email to the
  enterprise solutions team when there are two reference customers.
- Conference CFPs (BSides, Rust meetups, MCP/agent meetups) from month 4, only if the
  content flywheel is already running.
- Cyber-insurance/AI-governance questionnaires increasingly ask about AI agent
  controls; the compliance mapping page should rank for those searches.

## Risks, named

- Obscurity is the real competitor, not piracy. Every week of Phase 1 not done is the
  actual cost.
- Solo bus factor: mitigated commercially by the Continuity Promise and source
  availability; not mitigable for velocity. Do not promise enterprise SLAs the founder
  cannot keep.
- The two non-zero spends: CWS developer fee ($5, unavoidable for the store listing);
  trademark filing (~$250-350, deferred by decision to first paid license; use (TM)
  meanwhile).
- LICENSE-GOVERNANCE still needs its legal skim (standing flag since ADR-0027).
- The Node/CLI quirk noted in the maturity-1 ledger (extension-unit CI job) is
  unverified on CI until the first push with Actions enabled.
