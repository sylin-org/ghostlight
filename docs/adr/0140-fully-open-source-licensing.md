# ADR-0140: Fully open-source licensing

Date: 2026-08-25. Status: Accepted.
Supersedes ADR-0027's commercial split and ADR-0028's tiers. Amends ADR-0021 (license stance
returns to whole-repo permissive), ADR-0055, and ADR-0057 where they describe the commercial
governance license. The Continuity Promise from ADR-0028 Decision 6 is retained without change.

## Context

Ghostlight shipped as open-core: the engine was Apache-2.0 OR MIT while
`crates/orchestrator/src/governance/` was source-available under the Ghostlight Commercial
License, free for individuals, small teams, evaluation, and all-open operation, with paid tiers
for larger organizations running governance operationally. The runtime never enforced any of it:
no key, activation, status command, or behavior gate ever existed, exactly as ADR-0028 promised.

The owner has now decided the business model changes: Ghostlight becomes fully free and open
source, with every paid option removed. The one situation that paid (larger organizations using
configured governance operationally) no longer exists. The governance module has always been a
pass-through in all-open mode and an optional overlay otherwise; nothing about its engineering
depends on being commercially licensed.

## Decision

1. **One license for the whole product.** Everything in the repository, including
   `crates/orchestrator/src/governance/`, is offered under Apache-2.0 OR MIT, at the recipient's
   option. The engine/governance seam remains an architecture boundary; it is no longer a license
   boundary.
2. **No paid options remain.** The tier table, founding program, prices, and procurement path are
   withdrawn entirely. `PRICING.md` is deleted rather than stubbed: git history preserves the
   former page, and no active surface may link to it. No price, seat count, or upgrade exists
   anywhere in active surfaces.
3. **The commercial license text is retired.** `docs/licenses/LicenseRef-Ghostlight-Commercial.txt`
   is removed from the tree and from every package payload. Grants already made under it are
   unaffected: a later decision does not retroactively narrow rights granted by the version a
   recipient received. The exact retired text remains recoverable from git history at that path.
4. **Runtime stays gate-free.** The Continuity Promise holds verbatim in spirit: no activation,
   license check, behavior gate, telemetry, or audit license marker exists or may be added to
   enforce anything. License terms and technical enforcement were already separate; now there is
   also nothing to enforce.
5. **Contributions unify on the DCO.** Every part of the repository accepts contributions under
   the Developer Certificate of Origin with inbound equals outbound under Apache-2.0 OR MIT. The
   governance-module CLA requirement is dropped because there is no commercial license to protect.

## Consequences

- Support becomes community support: GitHub Issues and Discussions first, best-effort email for
  what cannot be public. The Team/Enterprise acknowledgment windows are withdrawn with their tiers;
  the support policy states best-effort response honestly instead of pretending a promise survived.
- Procurement artifacts built for paying customers (the MSA template and the tier claim map) are
  retired as templates for a relationship that no longer exists. The no-processing DPA facts they
  recorded stay available through the trust center's data-flow evidence.
- Packaging, installers, manifests, and the workbench About view drop the second license row and
  ship only Apache-2.0 and MIT texts.
- Historical documents (ADRs 0026 through 0030 and others, SPEC, 0.8 records, business planning)
  keep describing open-core as history. They are superseded here, not rewritten. Active surfaces
  must not quote them as current.
- The OpenSSF self-assessment rows that were "Not met" solely because of the mixed license become
  satisfiable; the assessment gains a dated revision note rather than a silent rewrite.

## Acceptance

- No active surface (README, guides, trust center, packaging, workbench UI, manifests) names a
  paid tier, price, commercial license, or CLA-for-governance path.
- `grep` for `LicenseRef-Ghostlight-Commercial`, `open-core`, `PRICING`, and "Commercial" over
  tracked files returns only historical records (ADRs, CHANGELOG, 0.8 material, dated design and
  research notes, superseded business planning) and this document's own references.
- All four crates still build and test green with the relicensed headers; packaging scripts pass
  their legal-payload checks against the reduced file set.
