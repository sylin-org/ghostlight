# BOOTSTRAP -- interface truth polish

Purpose: two polish tasks approved by the owner on 2026-08-24 after the foundry sprint,
both serving one principle -- the interface presents current truth, never remembered state
or swallowed effects.

## Authority order

1. AGENTS.md and the current docs/1.0/ contracts.
2. This file.
3. LEDGER.md (the authority on what is done).

## Ground rules

- One task = one logical commit set, every commit leaves its gates green.
- Rust commits: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --
  -D warnings`, `cargo test --workspace`. Extension commits: `npm test --prefix extension`
  plus `node --check` on changed files.
- Extension behavior changes are not live until the unpacked adapter is reloaded at
  chrome://extensions. Say so explicitly instead of claiming live proof.
- If a task cannot complete, revert, mark BLOCKED in LEDGER.md with the reason, stop.
- Boundaries: no main merge, no tag, no publish/store action, no network behavior added.
- Live authority swaps on Windows go through scripts/dev-loop.ps1 only.

## Task sequence

1. T1 reply-before-dispatch for the remaining dispatch-tail paths (extension).
2. T2 browser_tabs list becomes a current read of real state (bridge + extension +
   orchestrator + LANGUAGE.md). The agreed semantic: enumerate real tabs from the live
   browser and flag workspace binding; list intentionally becomes a dispatching call that
   can refuse when no browser is connected.

Details and running verdicts: LEDGER.md.
