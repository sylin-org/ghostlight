# exe-split LEDGER

Durable batch progress. One task = one commit = one log entry. Update after EVERY task, before
starting the next.

## RESUME HERE

- Next task: **S1** (`S1-workspace-and-transport-skeleton.md`)
- Base commit: `fccca60` on `dev` (tree green at batch authoring; later docs-only commits carry
  the batch itself)
- Batch state: NOT STARTED

## Task table

| Task | Title | Status | Commit |
|---|---|---|---|
| S1 | Workspace + transport crate skeleton | pending | - |
| S2 | Move leaf utilities to transport | pending | - |
| S3 | Move wire + handshake to transport | pending | - |
| S4 | Create ghostlight-core; root becomes facade | pending | - |
| S5 | ghostlight-adapter-agent bin + rewire clients + test harness | pending | - |
| S6 | ghostlight-adapter-browser bin + host install rework | pending | - |
| S7 | Retire roles from the ghostlight bin | pending | - |
| S8 | Reconnect patience (120s) + ADR-0045 amendment | pending | - |
| S9 | --no-supervisor + DEV-LOOP.md | pending | - |
| S10 | Packaging + distribution sweep | pending | - |

## Log

(Append one entry per finished task:)

```
### S<n> -- <title>
- Commit: <hash>
- Verification: fmt OK / clippy OK / test --workspace OK / linux cross-check OK
- Deviations:
  1. <none | numbered list, one line each>
```

## Blocked

(Only if the failure protocol fired: task id, exact failing step/error text, one-paragraph
diagnosis. The batch HALTS here.)
