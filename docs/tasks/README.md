# Task batches

A batch is optional. It is meant for parallel-lane or unattended multi-session work: a
`BOOTSTRAP.md` of ground rules, a task file per step, and a `LEDGER.md` (or one ledger file per
lane) that records what actually happened, one task at a time. The ledger is the authority on a
batch's progress. The decision a batch implements lives in its ADR, and current implementation
state lives in [`../STATUS.md`](../STATUS.md).

This index is a map, not a source of truth: where this table and a ledger disagree, the ledger wins.

## Read the dates before the descriptions

- Batch files describe the tree as it was when written; check the current tree before trusting any
  path or code excerpt in them.
- The internals were rebuilt clean-room on 2026-08-10 in `bf4f4724`.
- Batches are never removed, and their ADRs stay in force unless a later ADR supersedes them.

A finished batch's BOOTSTRAP ground rules bound that batch only while it ran; they are not
standing rules. That covers NEVER-touch fences, sacred-file lists, pins, stage orders and
verification blocks; per-batch branches (release-1-hardening, stage-2, stage-3, stage-4,
maturity-1, licensing-1: work on dev, and leave merges to main to the owner); overnight-mode
rules ("no human available, never ask", deferring browser checks to BROWSER-TESTS.md); push
steps (language-delight, evidence-1 and experience-closure grant no push authority now; push
authority is whatever AGENTS.md and docs/MEMORY.md say); manual ASCII-scan one-liners and any
`git add -A` step (superseded by `pwsh -NoProfile -File scripts/check-repository-integrity.ps1`
before committing text changes, and by staging only your own paths with `git add -- <paths>`);
dependency bans; and trained-tool-surface fences (tools.json, tests/tool_schema_fidelity.rs,
EXPECTED_TRAINED and hand-bumped tool counts name files that no longer exist; the 1.0 catalog
lives in crates/orchestrator/src/language/ and is pinned by EXPECTED_TOOL_NAMES and
CATALOG_TOOL_COUNT; catalog wording is delegated to the agent and recorded in an ADR per
docs/MEMORY.md, while adding, removing or reclassifying a tool still changes governance, which
stays the owner's). The ADRs those batches implement stay in force. The license is declared per
package (Cargo workspace license, packaging/npm/package.json, LICENSE), and
package-extension.ps1, package-mcpb.ps1 and prepare-npm-package.ps1 copy the license files into
every artifact; per-file SPDX headers are not required, and existing headers stay (keep them on
standalone downloaded scripts such as scripts/get.sh and scripts/get.ps1). If you resume a
finished batch, keep its task content and follow current process. The BOOTSTRAP of a batch that
is still open (for example break-2.0) stays binding for that batch's work; its lane, ownership
and commit-safety rules add to AGENTS.md and take precedence for that batch.

## The batches

| Batch | Opened | Note |
| --- | --- | --- |
| [break-2.0](break-2.0/BOOTSTRAP.md) | 2026-10-14 | ADR-0197 and ADR-0199: the 2.0 break set built by service, extension and connectors lanes in one checkout, then a switch. Lanes landed; the switch (see [switch.md](break-2.0/switch.md)) continues in the seams ledger, which carries the 2.0 decisions of ADR-0199 (see [seams.md](break-2.0/seams.md)). |
| [security-hardening](security-hardening/EPIC.md) | 2026-09-06 | Dependable governance for delightful browser work: assessment, decisions, bounded cycles, and ideation for unresolved choices |
| [process-diagnostics](process-diagnostics/) | 2026-08-29 | ADR-0145: one shared local diagnostics directory all three executables write bounded, content-free operational logs into; watch-driven marker, CLI, popup, and workbench surfaces |
| [public-distribution](public-distribution/) | 2026-08-29 | ADR-0144: Ghostlight as an installable plugin member (twin manifests, one-address catalogs, bundled skill) and drafted external submissions |
| [1.0-plus](1.0-plus/) | 2026-08-26 | Post-publication batch: deferred debt (simplest to most complex), the release-evidence lanes left open at publication, and owner-action externals |
| [demo-press-key-diagnosis](demo-press-key-diagnosis/) | 2026-08-24 | Why the foundry demo's press_key beat failed with a misleading disconnect sentence (diagnosis record, not an execution batch) |
| [evidence-1](evidence-1/) | 2026-08-24 | Blocked integration targets show what Ghostlight found (ADR-0129 Decision 4, ADR-0135) |
| [handle-continuity](handle-continuity/) | 2026-08-24 | Three owner-approved adjustments: continuation for tab identity, immediate recovery for transient states, one-call recreation for dead tabs. Complete, T1-T9. |
| [interface-truth-polish](interface-truth-polish/) | 2026-08-24 | Two polish tasks so the interface presents current truth, never remembered state or swallowed effects. Complete. |
| [pre-freeze-debt](pre-freeze-debt/) | 2026-08-24 | Closed 2026-08-26; its extension byte floor expired with the staged review. |
| [release-streamlining](release-streamlining/) | 2026-08-24 | Closed, all five tasks complete. |
| [language-delight](language-delight/) | 2026-08-23 | Delight pass over all model-facing sentences: validation messages, tool descriptions, result guidance, live proof |
| [capability-restoration](capability-restoration/) | 2026-08-22 | ADR-0133: restore genuine published 0.8 browser behaviors through the current 1.0 language and typed seams |
| [reference-experience](reference-experience/) | 2026-08-15 | One product across every machine: the second-machine state, adaptive familiarity, terminal citizenship, runtime control, At a glance, readiness recovery |
| [executor-split](executor-split/) | 2026-08-15 | Splitting `work/mod.rs` (5824 lines) into per-operation-family files |
| [outcome-language](outcome-language/) | 2026-08-11 | ADR-0103: one module owns what Ghostlight says happened |
| [browser-kernel](browser-kernel/) | 2026-08-08 | Closed at R5: catalog, profiles and rollout superseded by ADR-0107; separation of language, semantics and mechanisms remains in ADR-0101 and ADR-0096. Its stage and release rules are not in force. |
| [protocol-versioned-mcp-edge](protocol-versioned-mcp-edge/) | 2026-08-05 | ADR-0096/0098: protocol-versioned MCP edge, neutral service |
| [closed-loop-core](closed-loop-core/) | 2026-07-14 | ADR-0078: closed-loop browser core |
| [lightbox-legacy](lightbox-legacy/) | 2026-07-14 | Closed: T1-T5 migrated, all 27 rows DONE, legacy ignored tests, wrappers and CI job retired 2026-07-14; the harness itself was later retired with the 0.8 layer (ADR-0143). Rules not in force. |
| [experience-closure](experience-closure/) | 2026-07-14 | Closing the gap between the product and its public surfaces |
| [installer-targets](installer-targets/) | 2026-07-13 | ADR-0071: Windsurf, Zed, OpenCode, and Crush installer targets |
| [trust-1](trust-1/) | 2026-07-10 | ADR-0057: the open trust center |
| [managed-5](managed-5/) | 2026-07-10 | Managed authority |
| [official-rebaseline](official-rebaseline/) | 2026-07-09 | ADR-0050: rebaseline on the official extension's observable interface |
| [tab-identity](tab-identity/) | 2026-07-08 | Tab identity and continuity |
| [exe-split](exe-split/) | 2026-07-08 | Splitting the executables |
| [dev-override](dev-override/) | 2026-07-08 | Developer override path |
| [landscape-1](landscape-1/) | 2026-07-07 | ADR-0041/0042: post-evaluation response, origin-flow provenance |
| [composition](composition/) | 2026-07-06 | ADR-0035..0038: composition |
| [onboarding-1](onboarding-1/) | 2026-07-05 | ADR-0031: the agent onboarding contract |
| [console](console/) | 2026-07-05 | The Ghostlight console |
| [hub](hub/) | 2026-07-04 | ADR-0030: the Ghostlight hub orchestrator. Complete, H0-H9 (H9 375810a). |
| [licensing-1](licensing-1/) | 2026-07-03 | The open-core licensing split |
| [stage-4](stage-4/) | 2026-07-03 | ADR-0023/0024/0025: registry and pipeline architecture |
| [maturity-1](maturity-1/) | 2026-07-03 | Release maturity, m01-m06 |
| [stage-3](stage-3/) | 2026-07-03 | ADR-0022: the capability model |
| [stage-2](stage-2/) | 2026-07-02 | Governance |
| [release-1](release-1/) | 2026-07-01 | The first release package |

An open batch is not a commitment. Several of these describe work that the 1.0 rebuild reached by
another route, or made moot. Deciding whether an open batch still applies is a product decision, and
`STATUS.md` carries the ones that are actually owed.

## Authoring a new batch

The pattern that works, learned across the batches above:

- Take expected values from the contract or spec (wire fixtures, standards vectors, decided
  wording), never from the output of the code under test. If an expectation is wrong, correct it and
  record why in the ledger. When wording changes on purpose, re-pin the new exact text in the same
  commit; do not turn an exact assertion into a looser one or delete it to get green.
- On a failure, commit nothing red or partial, record the blocker and the failing command in the
  ledger, and continue with work that does not depend on it. Never weaken or skip a pinned assertion
  to get green. If a pinned requirement conflicts with an ADR or an owner decision, record both sides
  for the owner instead of choosing; that task waits, independent work continues.
- Give a task a STOP precondition only for a load-bearing safety or invariant assumption: an
  isolation mechanism that must be proven before any write to a real user or machine path, or a
  conflict between an ADR invariant and a test the task protects. Do not pin signatures, line
  numbers, file shapes or test counts as STOP conditions; an executor adapts to that drift and
  records it as a deviation. Ledgers need no "no STOP precondition fired" lines.
- Do not restate a general authority order; AGENTS.md "Authority and historical continuity"
  applies. State only the batch-specific precedence AGENTS.md does not cover, in one line (for
  example: the named ADR, then PINS or the design file, then the task file). Drift that does not
  touch a load-bearing assumption is resolved with judgment and noted in the ledger. A tree fact that
  contradicts a task's load-bearing assumption, or two pinned requirements that cannot both hold, is
  a STOP: write no code, record BLOCKED and what the author must decide (see hub H3 and H6 and the
  ADR-0030 always-ready-service amendment).
- Do not restate the live-tree rule in a new BOOTSTRAP; AGENTS.md states it. Cite code by name,
  signature or quoted anchor text, and treat line numbers as hints only.
- Sequence so that every prefix of the task list leaves a coherent, green tree.
- Say what a change makes redundant, not only what it adds. See
  [outcome-language/LEDGER.md](outcome-language/LEDGER.md) for what that miss costs.
- Record a deviation in the ledger when it changes scope, skips or weakens a check, contradicts the
  task or an ADR, changes a contract or expectation a later task depends on, or exposes a bug or a
  wrong check. Say its impact on later tasks. Give it a short ID (D1, D2, ...) only when something
  else will refer to it. Do not log formatting, line-ending or naming trivia. Read the deviations
  when the run ends.
- Update the ledger in the same commit as the task it records, and give each task commit a unique
  subject so git log finds it. Do not back-fill commit hashes. A blocked task gets a ledger-only
  commit that records the blocker; never commit partial code.
- Add a row to the table above when you create a batch; progress lives only in its ledger.
