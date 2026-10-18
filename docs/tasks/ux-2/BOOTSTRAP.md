# ux-2: ground rules

On 2026-10-18, after rc-1 and request-1 were deployed, the UX designer and the UX specialist
walked the owner's live window again. They checked each item they had agreed that morning and
looked for new problems. Of 49 items, 25 are confirmed. The rest, plus what the walk found, is
this batch: 37 items, ranked by what a person notices. The owner asked for exactly this ("ux-2
will be just what the re-walk finds, plus the 13 later items"). [LEDGER.md](LEDGER.md) is the
record and lists every item with its slice.

The owner's rule binds every slice: **teal means live**. Teal marks work happening and the one
primary action in a view. Everything at rest is ink.

## Decided by the coordinator (2026-10-18, under the owner's delegation)

- **The Ready lamp rests in ink**, as the owner's rule says. Teal and the breath appear only while
  work runs. The owner sees it at validation and can reverse it.
- **The extension takes its share of the fixes.** These are the setup command that clips at mid
  widths, the popup's teal eyebrow, the idle Ready lantern, the ended state's primary step and
  the breathing tempo. The 2.0 package is not staged with any store yet, so this costs no
  resubmission. Staging and submitting stay the owner's call (AGENTS.md, ADR-0200 Decision 16).
- **The light-taskbar tray ghost is redrawn** as a monochrome glyph. The owner approves it at
  validation.
- **The taskbar app icon (n-d-09) waits** for the owner's look on a light-mode account. It is
  only changed if the problem shows up there.
- **The width-stacked layout stays as it is.** The owner saw it and said "it's fine that way then".

## Lanes

- **window**: every change to `crates/orchestrator/ui`, `tests/workbench-*`,
  `language/workbench.rs`, `language/readiness.rs`, `governance/effective.rs`,
  `workbench/mod.rs`, `intake/desktop.rs` and `tests/golden/desktop`. The slices run in order,
  u0 to u7, because most of them share the window's page files.
- **native**: `crates/desktop` (the light-taskbar tray glyph).
- **extension**: `extension/` and its package record.
- **outcome**: `language/outcome.rs` and the result goldens.
- **docs**: the brand reference and the 1.x feedback dictionary.

## Rules

- AGENTS.md and docs/MEMORY.md bind. A fix serves the person or it is not made. Words a person or
  an agent reads live in `crates/orchestrator/src/language/`. ASCII only.
- The reviewers cite `file:line`. Treat those as hints: find the code by name.
- Each lane builds in its own `CARGO_TARGET_DIR` (`.target-lane-ux-<lane>`). It stages only its
  own paths and commits each slice with its LEDGER row.
- A window layout change runs the history journey headless before it lands
  (`GHOSTLIGHT_TEST_HEADLESS=1 node tests/workbench-history-browser.mjs`). It starts its own
  isolated Chromium and no Ghostlight. No other journey, installer, dev-loop or Ghostlight binary
  runs in a lane: the owner's machine holds one live Ghostlight.
- Nothing is deployed, released or published by the lanes. The coordinator deploys in place,
  checks live and pushes.
