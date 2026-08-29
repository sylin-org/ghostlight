# Task batches

A batch is a package of work authored for unattended execution: a `BOOTSTRAP.md` of ground rules, a
task file per step, and a `LEDGER.md` that records what actually happened, one task at a time. The
ledger is the authority on a batch's progress. The decision a batch implements lives in its ADR, and
current implementation state lives in [`../STATUS.md`](../STATUS.md).

This index exists so that finding out where a batch ended does not mean opening twenty-five ledgers.
It is a map, not a source of truth: where this table and a ledger disagree, the ledger wins.

## Read the dates before the descriptions

Ghostlight's internals were rebuilt clean-room on 2026-08-10 in `bf4f4724`, which removed the old
root binary, core, transport, lightbox, and extension mechanisms. **Every batch below except
`reference-experience`, `outcome-language`, and `executor-split` was authored and executed against
those removed internals.** Their file paths,
module names, and code excerpts describe an implementation that no longer exists in the working
tree, though it remains in Git history.

That does not make them worthless, and none of them are going anywhere. They are the evidence of
why the product evolved the way it did, and their ADRs remain in force unless a later ADR supersedes
them. It does mean you should never take a code path from one of these files as current. Check the
tree.

## The batches

| Batch | What it was | Where its ledger stops | Last touched |
| --- | --- | --- | --- |
| [public-distribution](public-distribution/) | ADR-0144: Ghostlight as an installable plugin member (twin manifests, one-address catalogs, bundled skill) and drafted external submissions | P1-P3, X1, D1 complete | 2026-08-29 |
| [1.0-plus](1.0-plus/) | Post-publication batch: deferred debt (simplest to most complex), the release-evidence lanes left open at publication, and owner-action externals | D1 complete; D2 next | 2026-08-26 |
| [demo-press-key-diagnosis](demo-press-key-diagnosis/) | Why the foundry demo's press_key beat failed with a misleading disconnect sentence (diagnosis record, not an execution batch) | Root causes fixed (script ordering; primitive rendering); desk-bell blocking-dialog click defect documented, open | 2026-08-24 |
| [language-delight](language-delight/) | Delight pass over all model-facing sentences: validation messages, tool descriptions, result guidance, live proof | D1-D4 complete; deployed and proven live | 2026-08-24 |
| [evidence-1](evidence-1/) | Blocked integration targets show what Ghostlight found (ADR-0129 Decision 4, ADR-0135) | E1-E3 complete; deployed and proven live | 2026-08-24 |
| [capability-restoration](capability-restoration/) | ADR-0133: restore genuine published 0.8 browser behaviors through the current 1.0 language and typed seams | R1 complete; R2 next | 2026-08-22 |
| [reference-experience](reference-experience/) | One product across every machine: the second-machine state, adaptive familiarity, terminal citizenship, runtime control, At a glance, readiness recovery | S1-S7 and Linux V1-V5 complete; S8 blocked on required real-desktop evidence | 2026-08-17 |
| [executor-split](executor-split/) | Splitting `work/mod.rs` (5824 lines) into per-operation-family files | Complete | 2026-08-15 |
| [outcome-language](outcome-language/) | ADR-0103: one module owns what Ghostlight says happened | Complete | 2026-08-11 |
| [browser-kernel](browser-kernel/) | ADR-0101: canonical browser operations, native surface, compatibility adapter | Open at stage R5 | 2026-08-08 |
| [protocol-versioned-mcp-edge](protocol-versioned-mcp-edge/) | ADR-0096/0098: protocol-versioned MCP edge, neutral service | Implementation and corrections landed | 2026-08-05 |
| [closed-loop-core](closed-loop-core/) | ADR-0078: closed-loop browser core | C1-C6 complete | 2026-07-15 |
| [lightbox-legacy](lightbox-legacy/) | Migrating legacy scenarios onto isolated process orchestration | T1-T4 migrated | 2026-07-14 |
| [experience-closure](experience-closure/) | Closing the gap between the product and its public surfaces | E1-E5 complete; visible-surface follow-ups noted | 2026-07-14 |
| [installer-targets](installer-targets/) | ADR-0071: Windsurf, Zed, OpenCode, and Crush installer targets | Complete | 2026-07-13 |
| [trust-1](trust-1/) | ADR-0057: the open trust center | Authored and red-teamed; see its ledger | 2026-07-10 |
| [managed-5](managed-5/) | Managed authority | Authored and red-teamed; see its ledger | 2026-07-10 |
| [licensing-1](licensing-1/) | The open-core licensing split | Done, implemented directly rather than as the batch | 2026-07-10 |
| [official-rebaseline](official-rebaseline/) | ADR-0050: rebaseline on the official extension's observable interface | Complete | 2026-07-09 |
| [tab-identity](tab-identity/) | Tab identity and continuity | Complete | 2026-07-08 |
| [exe-split](exe-split/) | Splitting the executables | Complete, S1-S10 | 2026-07-08 |
| [dev-override](dev-override/) | Developer override path | Complete | 2026-07-08 |
| [landscape-1](landscape-1/) | ADR-0041/0042: post-evaluation response, origin-flow provenance | Never executed; L1 was next | 2026-07-07 |
| [stage-4](stage-4/) | ADR-0023/0024/0025: registry and pipeline architecture | Staged work on the pre-1.0 line | 2026-07-06 |
| [composition](composition/) | ADR-0035..0038: composition | Complete, C1-C11; operator live-verify remained | 2026-07-06 |
| [onboarding-1](onboarding-1/) | ADR-0031: the agent onboarding contract | Complete | 2026-07-05 |
| [hub](hub/) | ADR-0030: the Ghostlight hub orchestrator | Open at H9, installer auto-start | 2026-07-05 |
| [console](console/) | The Ghostlight console | Complete, K1-K5 | 2026-07-05 |
| [maturity-1](maturity-1/) | Release maturity, m01-m06 | See its ledger | 2026-07-04 |
| [stage-3](stage-3/) | ADR-0022: the capability model | Staged work on the pre-1.0 line | 2026-07-03 |
| [stage-2](stage-2/) | Governance | Staged work on the pre-1.0 line | 2026-07-03 |
| [release-1](release-1/) | The first release package | Complete, all 18 tasks | 2026-07-02 |

An open batch is not a commitment. Several of these describe work that the 1.0 rebuild reached by
another route, or made moot. Deciding whether an open batch still applies is a product decision, and
`STATUS.md` carries the ones that are actually owed.

## Authoring a new batch

The pattern that works, learned across the batches above:

- Compute and pin every expected output yourself. An executor that derives its own expected values
  validates its own bugs.
- Give each task STOP preconditions, so an executor that finds a changed tree halts instead of
  improvising.
- Sequence so that every prefix of the task list leaves a coherent, green tree.
- Say what a change makes redundant, not only what it adds. See
  [outcome-language/LEDGER.md](outcome-language/LEDGER.md) for what that miss costs.
- Treat the ledger's deviations as the feedback channel and read them after the run.
