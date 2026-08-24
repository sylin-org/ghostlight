# BOOTSTRAP: blocked-target evidence re-land

Read this file, [LEDGER.md](LEDGER.md), the live tree, and [ADR-0135](../../adr/0135-blocked-target-evidence.md).
Assume no memory of earlier sessions. `LEDGER.md` is the authority on progress; its `RESUME HERE`
names the only next task.

## Objective

Close ADR-0129 Decision 4: a blocked integration target shows what Ghostlight found instead of
only asserting that something is wrong. One optional orchestrator-authored `evidence` sentence on
`HarnessSummary`, rendered by the card the destination already has. No layout, action, or
ownership changes.

## Authority order

1. This file and ADR-0135.
2. `AGENTS.md`, `docs/MEMORY.md`.
3. Current source and tests.

## Ground rules

- The WebView renders the evidence string verbatim; it authors no words of its own.
- Disclosed commands and reasons are whitespace-normalized, stripped of control and bidi
  characters, and capped at 200 visible characters before composition.
- Never change ownership behavior: foreign entries are still never overwritten or removed;
  `can_install` stays false while a target is blocked; actions stay as they are.
- Every edited assertion keeps pinning equivalent strength; add pins for each new evidence
  family (foreign across JSON/TOML/YAML, malformed, unblocked-has-none).
- ASCII only. Person-plain sentences. No "simply", no blame.
- Each slice is one green commit; every prefix stays usable.

## Task sequence

| Task | Scope | Commit subject |
| --- | --- | --- |
| E1 | Projection in `crates/orchestrator/src/install/mod.rs`: capture the found command on foreign entries and the parse reason on malformed files; compose distinct plain-word details plus the bounded optional `evidence` field (serde skip when absent); unit pins for every family. | `feat(install): carry blocked-target evidence` |
| E2 | Surface in `crates/orchestrator/ui/`: render the evidence paragraph inside blocked cards only; styles treatment; `tests/workbench-surface.mjs` assertions; preview fixture rows gain evidence strings. | `feat(workbench): show blocked-target evidence` |
| E3 | Deploy the release orchestrator by exact-path swap per DEV-LOOP; prove the evidence paragraph in the real workbench against an isolated seeded foreign fixture if reachable without touching live configs, otherwise record exactly what was and was not proven; full common gate; STATUS reconciliation; ledger close. | `test(integrations): prove blocked-target evidence live` |

## Per-task procedure

1. Read LEDGER; confirm the task is next; mark IN PROGRESS.
2. Implement within the named files; keep a running count of changed messages.
3. Update pinned assertions in the same commit; add pins named above.
4. Common gate: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`, `npm test` from `extension/`,
   `pwsh scripts/check-repository-integrity.ps1`; E2 adds `node tests/workbench-surface.mjs`.
5. Update ledger evidence + RESUME HERE; commit with the pinned subject; push.

## Failure protocol

Anything that cannot be done without changing behavior beyond this ADR: record it in the ledger
deviations table and move on. Anything else that fails: mark BLOCKED with the exact command and
output, set RESUME HERE, stop.

## Never do these

- Never write to any real harness configuration outside a test-owned temporary directory.
- Never weaken an ownership refusal or an existing assertion to make new code pass.
- Never touch `docs/trust/` claims.
- Never upload, publish, push tags, or mutate anything external.
