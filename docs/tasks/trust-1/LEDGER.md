# trust-1 batch: LEDGER

Single source of truth for batch progress. Update after EVERY task. A fresh executor resumes from
RESUME HERE with no other context.

## RESUME HERE

Batch authored 2026-07-10 (same session as the three-lane procurement research and the ADR-0057
Research-ratification amendment). W1-W3 DONE. Next task: W4.

## Status

| Task | Title | Status | Commit | Deviations |
| --- | --- | --- | --- | --- |
| W1 | Trust-center skeleton: README index | DONE | af550fa | none |
| W2 | faq.md: the 22-question front door | DONE | 19d2d40 | none |
| W3 | security-overview.md + data-flows.md | DONE | (pending) | none |
| W4 | sub-processors.md + continuity.md + supply-chain.md | pending | - | - |
| W5 | controls.md + questionnaire.md (CAIQ-shaped) | pending | - | - |
| W6 | support-policy.md + tiers.md + PLAN.md 3/2 sync | pending | - | - |
| W7 | msa.md + dpa.md (DRAFT -- pending counsel) | pending | - | - |
| W8 | SBOM in release CI + security-insights.yml + SECURITY.md alignment | pending | - | - |
| W9 | Red-team pass (over-claims) + cross-links | pending | - | - |

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
