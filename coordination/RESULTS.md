# Latest result

## [0021] G0 closed; frozen-revision Linux verification lane dispatched

G0 is closed: the 1.0 release candidate is frozen at `08f368606f3deac4115a148f6c20590a7c9afb9b`
(`docs/release/freeze.json`), after the four-task pre-freeze debt batch landed
(`docs/tasks/pre-freeze-debt/LEDGER.md`: shared bridge handshake, amber never-settled rows,
ADR-0136 + the 24th catalog tool `policy_explain`, and ADR-0105 stage 2 observed peer
attribution through the new audited `ghostlight-win-peer` crate; stage 3 re-deferred by owner
decision). The Windows half of G1 passed: full preflight green
(`docs/testing/release-preflight-2026-08-24.md`) plus a live whole-catalog foundry run against
the deployed frozen graph. The Chrome Web Store review already covers the candidate's extension
bytes (unchanged since `70869631`). linux-codex now owns the Linux verification lane for the
same frozen revision: source gates, user-level candidate redeploy, `demo-foundry.sh` green, a
dated CachyOS record, then report back. Freeze rule in force: product defects are documented
BLOCKED-with-evidence, not fixed, unless the owner declares a blocker.
