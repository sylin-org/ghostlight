# 1.0-plus BOOTSTRAP

The batch that follows the 1.0 publication (2026-08-26). It holds the deferred engineering debt,
the release-evidence lanes that stayed open at the owner-directed publication, and the owner-action
externals, ordered so the debt ladder runs simplest to most complex first.

Authority order: this BOOTSTRAP, then the ADR named per task, then the current tree. The ledger is
the authority on progress; a task file describes intent, the ledger records what happened.

## Ground rules

- One task = one commit, and every commit leaves a green tree: formatting, warnings-denied Clippy,
  the full Rust suite, the extension suite, npm launcher tests, and the syntax gates its change
  touched.
- The published 1.0.0 channels are immutable. Recovery or follow-up ships as a higher version.
  Extension source changes in this batch make the published 1.0.0 store revision one commit older;
  that is normal forward flow, not a defect, and no store mutation happens in this batch without a
  separate owner authorization.
- Debt tasks repair at the owning seam and add a regression proof or guard where the repair is
  behavioral. A pure move says so and proves equivalence (tests pass unmodified except the
  assertions that pinned the old shape).
- Evidence lanes run their existing runbooks (`docs/testing/linux-live-lifecycle.md`, the release
  checklist rows); the ledger records results, it does not restate them.
- Owner-action externals (Scoop, WinGet, SignPath, registry changes, anything public) stay parked
  until the owner names the action. Draft, then wait.
- Never phone home; never copy from `reference/`; never weaken a trust-doc claim. The standing
  rules in `AGENTS.md` apply in full.
- A task that cannot close honestly is marked BLOCKED in the ledger with the reason. Do not
  improvise around a changed tree; STOP and record.

## Execution order

1. D1 presentation stylesheet module (debt, simplest).
2. D2 GIF palette quality (debt, algorithmic).
3. D3 browser-window attention routing (debt, architectural; needs an ADR before code).
4. E1 store-adapter install check, E2 npm 0.8-to-1.0 upgrade lane, E3 G8 KDE accessibility half,
   E4 G7 public harnesses, E5 G4/G5 when their environments exist. Each lane authors its own dated
   evidence under `docs/testing/` and ticks its checklist row.
5. Externals X1 Scoop, X2 WinGet, X3 SignPath acceptance follow-ups: owner actions, recorded here
   only as state.

RPM and macOS stay outside this batch: RPM waits on a scope decision plus a real lifecycle host,
macOS on test hardware (ADR-0116). They are recorded as parked in `docs/STATUS.md`.
