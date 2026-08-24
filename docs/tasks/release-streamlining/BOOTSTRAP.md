# BOOTSTRAP -- release streamlining

Purpose: five owner-approved improvements (2026-08-24) to the release process. Automate the
deterministic middle of the existing G-gates; never remove a human gate or a stage that
exists because it caught something real.

## Authority order

1. AGENTS.md, RELEASE-CHECKLIST.md, DEV-LOOP.md.
2. This file; LEDGER.md records progress.

## Ground rules

- One task = one logical commit set, gates green per commit.
- New scripts follow house style: SPDX header, param block, strict mode, stop on error,
  ASCII only.
- The preflight runner is validated by running it end to end at least once fully green
  before its own commit.
- Boundaries: no main merge, no tag, no publish/store action, no credential handling beyond
  what already exists.

## Task sequence

1. P1 `scripts/release-preflight.ps1`: one-command ordered G1 gate runner with per-stage
   PASS/FAIL/SKIP, generated dated evidence skeleton under docs/testing/ on full pass.
2. P2 machine-readable freeze: `docs/release/freeze.json`, `scripts/declare-freeze.ps1`,
   `scripts/assert-freeze.ps1`; optional stage wired into the preflight.
3. P3 `scripts/verify-custody.ps1 <candidate-dir>`: freeze binding + deep candidate checks +
   SHA256SUMS recomputation + optional provenance verify.
4. P4 RELEASE-CHECKLIST ordering rule: no store submission before G2 custody.
5. P5 journey-artifact hygiene: sweep tests/ stragglers, pin the ignore pattern.

Details and running verdicts: LEDGER.md.
