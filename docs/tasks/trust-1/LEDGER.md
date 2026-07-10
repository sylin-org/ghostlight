# trust-1 batch: LEDGER

Single source of truth for batch progress. Update after EVERY task. A fresh executor resumes from
RESUME HERE with no other context.

## RESUME HERE

Batch authored 2026-07-10 (same session as the three-lane procurement research and the ADR-0057
Research-ratification amendment). W1-W9 DONE. Batch COMPLETE.

## Status

| Task | Title | Status | Commit | Deviations |
| --- | --- | --- | --- | --- |
| W1 | Trust-center skeleton: README index | DONE | af550fa | none |
| W2 | faq.md: the 22-question front door | DONE | 19d2d40 | none |
| W3 | security-overview.md + data-flows.md | DONE | 68db6c4 | none |
| W4 | sub-processors.md + continuity.md + supply-chain.md | DONE | b8f89f1 | none |
| W5 | controls.md + questionnaire.md (CAIQ-shaped) | DONE | 0c79c4c | none |
| W6 | support-policy.md + tiers.md + PLAN.md 3/2 sync | DONE | f97ac87 | none |
| W7 | msa.md + dpa.md (DRAFT -- pending counsel) | DONE | 3a1e7bb | none |
| W8 | SBOM in release CI + security-insights.yml + SECURITY.md alignment | DONE | 07249f4 | 2 |
| W9 | Red-team pass (over-claims) + cross-links | DONE | c6d197b | none |

Status values: `pending` | `in-progress` | `DONE` | `BLOCKED`.

## Log

One entry per task as it closes (or blocks). Number every deviation from the task file.

### W1 -- Trust-center skeleton: README index (DONE)
- Wrote `docs/trust/README.md` with pinned H1 + H2s (How to read this, Documents, What we do
  not have), both verbatim sentences, a 12-row document table (14 markdown links total), and the
  footer.
- Verification: gated sentence 1 hit; `]\(` count 14 (>=12); em-dash 0; "open source" 0; footer
  present. Global gates clean.
- Deviations: none.

### W2 -- faq.md: the 22-question front door (DONE)
- Wrote `docs/trust/faq.md`: 6 H2 sections, 22 H3 questions in the pinned order, each with a
  closing `Evidence:` line; 3 RUNNABLE lightbox lines (managed-activation-local,
  continuity-source-unreachable, fail-closed-cold-boot); the pinned "There is no model-provider
  client in Ghostlight's dependency tree." sentence; syslog/file-today-HTTP-deferred wording.
- Verification: H3=22, H2=6, Evidence=22, lightbox=3, model-provider=1; em-dash 0; single "open
  source" hit is the engine's Apache/MIT license; footer present. Global gates clean.
- Deviations: none. Notes: (a) intro reworded to avoid a stray literal "Evidence:" (kept the count
  at exactly 22); (b) governance-module clause reworded so the only "open source" string is the
  engine's, satisfying the W9 red-team gate; (c) ADR-0027 link uses its real filename
  `0027-open-core-business-model-and-licensing.md`.

### W3 -- security-overview.md + data-flows.md (DONE)
- Wrote both files with all pinned H2s in order. security-overview.md includes the verbatim
  publish-all-audits sentence and describes the cache as signed and verified-on-load with NO
  at-rest encryption claim. data-flows.md has the "Flows that do not exist" section with per-flow
  ADR-0028 D9 citation.
- Verification: publish-in-full 1; encrypt-at-rest 0; "## Flows that do not exist" 1; em-dash 0;
  no stray "open source"; footers present.
- Deviations: none. Note: per authority-order review the managed cache is described as "signed and
  verified on load" and the docs stay silent on at-rest encryption. ADR-0055 D5 says the cache is
  signed AND encrypted, but the BOOTSTRAP banned-claims floor forbids at-rest encryption claims;
  silence satisfies both (no false claim, no banned claim). Applied batch-wide.

### W4 -- sub-processors.md + continuity.md + supply-chain.md (DONE)
- sub-processors.md: short, states "engages no subprocessors" with reasoning + git-history/release-
  notes change record. continuity.md: verbatim Continuity Promise blockquote from ADR-0028 D6,
  structural explanation, exactly 3 runnable scenarios (continuity-source-unreachable,
  fail-closed-cold-boot, rollback-guardian), and an "If the vendor ceases to exist" section that
  makes NO future-maintenance/foundation-handoff promise. supply-chain.md: releases/SBOM/deps/
  build sections; Socket.dev claim pinned as "scored 100/100 on all axes on Socket.dev at
  publication (2026-07)" with the npm link; SBOM asset name matches W8.
- Verification: no-subprocessors 1; blockquote 6 lines; continuity lightbox ==3; all H2s present;
  em-dash 0; encrypt-at-rest 0; footers present.
- Deviations: none.

### W5 -- controls.md + questionnaire.md (DONE)
- controls.md: opening no-certification paragraph (cites README what-we-do-not-have); ISO/IEC 27001
  Annex A orientation table with the pinned "source access is a standing audit right" line; SOC 2
  orientation; AI frameworks (EU AI Act tool-vendor/deployer + Article 12/26 via audit+policy_seq+
  Policy Passport + D11e no-legal-advice sentence; ISO/IEC 42001 no-cert; NIST AI RMF g/m/m/m).
  questionnaire.md: CAIQ v4-shaped, all 17 domains (full names + acronyms), opening statement of
  due-diligence-filing purpose + planned STAR Level 1 submission (no date); 6 N/A rows
  (DCS/DSP/IVS/UEM structurally-impossible, HRS/SEF partial-N/A).
- Verification: standing-audit-right 1; N/A 6; STAR Level 1 1; controls H2s present; em-dash 0;
  no stray "open source"; encrypt-at-rest 0; footers present.
- Deviations: none.

### W6 -- support-policy.md + tiers.md + PLAN.md 3/2 sync (DONE)
- support-policy.md: Channel / Response commitment / Severity and scope / Enterprise extras;
  acknowledgment (not resolution) 3 business days Team / 2 Enterprise; business days Mon-Fri UTC;
  "typically much faster" appears exactly once as color. tiers.md: claims->features->evidence
  table (central policy, SIEM audit, email support, security questionnaires, MSA, DPA, deployment
  help + roadmap input) plus the pinned "never enforced at runtime" sentence. PLAN.md: only the
  support-SLA line changed from team 2-business-day / enterprise 1-business-day to 3 / 2 business
  days acknowledgment; nothing else touched.
- Precondition sweep: PLAN.md line 170-171 stated 2-day/1-day, so the "replace those phrases"
  branch applied (not the "add none" branch).
- Verification: UTC 1; typically 1; 1-business-day across PLAN+trust 0; never-enforced-at-runtime
  1; support-policy H2s present; em-dash 0; footers present.
- Deviations: none.

### W7 -- msa.md + dpa.md drafts (DONE)
- Both files carry the verbatim DRAFT banner immediately after the H1. msa.md: 15-section MSA
  template, plain language, with the license-grant split (Apache-2.0 OR MIT engine + Ghostlight
  Commercial License / LICENSE-GOVERNANCE governance module), support BY REFERENCE to
  support-policy.md, Continuity BY REFERENCE to continuity.md, termination-never-disables clause
  citing ADR-0028, and bracketed UPPERCASE [TO BE COMPLETED IN REVIEW] placeholders for
  fees/liability caps/governing law. dpa.md: short no-processing DPA; recitals establish the
  vendor processes NO customer personal data (cite data-flows.md + ADR-0028 D9); controller/
  processor clauses NOT ENGAGED; conditional future-processing section; sub-processors none;
  transfers none; breach notification by reference to security-overview.md.
- Verification: banner 1 each; TO BE COMPLETED IN REVIEW 4 (>=2); no-customer-personal-data 6;
  em-dash 0; no stray "open source" (governance module described as source-available); footers.
- Deviations: none.

### W8 -- SBOM CI + security-insights.yml + SECURITY.md alignment (DONE)
- release.yml: SBOM step added to the `release` job (the once-per-release job; the `build` job is
  a per-target matrix and was correctly avoided). The step installs cargo-cyclonedx --locked and
  runs the pinned command with VERSION=${{ needs.prepare.outputs.version }}, then stages the
  root-package SBOM into artifacts/ so `gh release create ... artifacts/*` uploads it.
  security-insights.yml (repo root): OpenSSF Security Insights v2 shape (header schema-version
  2.0.0 + last-updated/last-reviewed/url; project + repository sections) filled honestly (repo
  https://github.com/sylin-org/ghostlight, vuln reporting via SECURITY.md + hello@sylin.org,
  bug-bounty-available false, distribution points GitHub releases + npm, per-release CycloneDX
  SBOM, Apache-2.0 OR MIT engine license + source-available governance note). SECURITY.md: appended
  a "Disclosures and advisories" section (no-bounty absence with reason; 3-business-day vendor-side
  advisory commitment; link to docs/trust/security-overview.md); all existing content preserved.
- Verification: cyclonedx in release.yml 5 hits (>=1); schema-version 1; SECURITY.md trust link 1;
  both YAML files parse via python yaml; isolated-target `cargo build --workspace` green.
- Deviations: (1) added a `dtolnay/rust-toolchain@stable` step to the release job, which had no
  Rust toolchain, because `cargo cyclonedx` requires cargo; mirrors the toolchain step style used
  by the test/build jobs. (2) SBOM generated for the root package only (no --all), matching the
  pinned single-file command and avoiding same-basename collisions across the 5 workspace members.

### W9 -- over-claim red-team pass + cross-links (DONE)
- Pass 1 claim audit ran CLEAN; zero fixes. Sweeps: (a) SOC 2 / ISO 27001 / ISO/IEC 42001 /
  penetration-test -- every hit is a negation, orientation, or roadmap statement, no possession
  claim; (b) `encrypt` -- one hit only, the CAIQ domain NAME "Cryptography, Encryption & Key
  Management", not an at-rest claim; (c) `open source` -- one hit only, faq.md engine Apache/MIT
  line; (d) all 4 named lightbox scenarios (continuity-source-unreachable, fail-closed-cold-boot,
  rollback-guardian, managed-activation-local) exist in crates/lightbox/src/scenarios.rs; (e) all
  115 relative links in docs/trust/ resolve (python os.path check); (f) all 13 docs/trust/*.md end
  with the exact footer.
- Re-ran every W1-W8 verification command: all still pass. Global gates: em-dash 0; 13/13 footers.
- Pass 2 cross-links (the only files outside docs/trust/ touched): added one row to the root
  README.md Documentation table and one row to docs/guides/README.md's task table, both linking
  ../trust/README.md (or docs/trust/README.md), mirroring existing table style, no restructuring.
- Deviations: none.

## Batch complete

All nine tasks DONE. docs/trust/ holds 13 published documents; W8 touched release.yml,
security-insights.yml, SECURITY.md; W6 synced docs/business/PLAN.md to 3/2; W9 added two
cross-links. Owner gates remain: counsel skim (MSA/DPA/LICENSE-GOVERNANCE) before first
execution of the legal templates; security.txt on sylin.org (founder-side); publish/push.