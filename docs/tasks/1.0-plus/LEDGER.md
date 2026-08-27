# 1.0-plus LEDGER

Progress authority for the batch. One task = one commit. RESUME HERE, then read upward.

## RESUME HERE

Next task: **D2 GIF palette quality**. Task list and rules are in
[BOOTSTRAP.md](BOOTSTRAP.md).

## Task table

| Task | What | State |
| --- | --- | --- |
| D1 | Presentation stylesheet to its own module | complete |
| D2 | GIF palette quality: shared palette, optional dithering | next |
| D3 | Browser-window attention routing (ADR-0084 full scope) | pending, ADR first |
| E1 | Install the store adapter on a lane machine; id + 1.0.0 match | pending |
| E2 | npm channel lane: public 0.8 upgrades to 1.0 in place | pending |
| E3 | G8 KDE Wayland accessibility half on the existing host | pending |
| E4 | G7 candidate browser + three public harnesses | pending |
| E5 | G4 Ubuntu GNOME Wayland / G5 clean Windows when environments exist | pending, environment-gated |
| X1 | Scoop bucket submission | parked, owner action (metadata prepared in `.target-pkg-metadata`) |
| X2 | WinGet manifest submission | parked, owner action |
| X3 | SignPath acceptance follow-ups | parked, application pending |

## Log

### D1 -- presentation stylesheet to its own module

- Commit: `f8bff79a` (2026-08-26).
- Intent: [D1-presentation-stylesheet-module.md](D1-presentation-stylesheet-module.md).
- Result: complete as specified, with one addition beyond the task file. The moved template was
  proven byte-identical before commit by extracting the old template from git
  (`HEAD:extension/lib/presentation.js`), renaming the new module's placeholders back
  (`${tokens}` -> `${TOKENS}`, `${reducedFadeSelector}` -> `${REDUCED_FADE_SELECTOR}`), and
  comparing byte for byte: EQUIVALENT. All 153 extension tests pass; `node --check` passes on
  the new module, presentation.js, and the reworked test. The manifest loads
  `lib/presentation-css.js` before `lib/presentation.js` in the content-script list; the
  packager copies `lib/` wholesale, so the module rides into any future package with no
  packaging change.
- Extension source changed after the published 1.0.0 store revision, per the BOOTSTRAP rule:
  normal forward flow, no store action taken or authorized.

## Deviations

(numbered as they arise; a deviation is a place the tree or the result differed from the task file)
