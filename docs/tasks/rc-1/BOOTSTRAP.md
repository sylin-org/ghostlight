# rc-1: ground rules

The owner asked on 2026-10-18 for a product-owner pass and a visual coherence pass (a UX
designer and a UX specialist) before the 2.0 release candidate. Both ran; a skeptic checked every
finding. This batch carries the confirmed ones that belong in 2.0. The `v2.0.0` tag is cut from
the same commit as `v2.0.0-rc.1` (docs/RELEASE.md, The 2.0 cutover), so what 2.0 should carry
lands before rc.1. [LEDGER.md](LEDGER.md) is the record.

## Lanes

Four lanes run in parallel in one checkout, then the docs lane:

- **frontdoor**: the install scripts, the installer, uninstall and doctor.
- **agent**: what agents see (tab defaults, page-side refusals).
- **window**: the desktop window's page and words.
- **native**: `crates/desktop` (the frame and the tray).
- **docs**: after the four, CHANGELOG, notes, README, trust pages, release steps.

The extension (`extension/`) and its freeze golden are out of this batch until the owner rules on
folding the popup fixes into 2.0.

## Rules

- AGENTS.md and docs/MEMORY.md bind. In particular: a fix serves the person or it is not made;
  discover where things really are on every look; no waiting periods; ASCII only; words a person
  or an agent reads live in `crates/orchestrator/src/language/`.
- Each lane builds in its own `CARGO_TARGET_DIR` (`.target-lane-rc-<lane>`), stages only its own
  paths, and commits each slice with its LEDGER row.
- A gate on one system never compiles the other system's gated code: gate system-only calls and
  read such code twice; CI's other-system clippy is the first check.
- Nothing is deployed, released or published by the lanes. The coordinator deploys in place,
  checks live and pushes.
