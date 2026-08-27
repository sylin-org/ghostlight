# D1 -- presentation stylesheet to its own module

The STATUS "Owed" item: "The extension stylesheet could move to its own module now that it is
static. Lowest value of the maintainability steps; needs about eight test assertions reworked."

## Current-tree facts (as of authoring, 2026-08-26)

- The in-page visual stylesheet lives as a template literal inside
  `extension/lib/presentation.js` `mount()` (from `style.textContent = \`` through the closing
  backtick-semicolon). Its only interpolations are `${TOKENS}` and `${REDUCED_FADE_SELECTOR}`,
  both constructed in presentation.js; `tests/shared.test.js` pins that exact shape.
- presentation.js is a classic content script listed in `extension/manifest.json`
  content_scripts as `lib/shared.js, lib/presentation.js, content.js` at `document_start`,
  `all_frames: true`. The service worker does not load it; `tests/content.test.js` stubs
  `GhostlightPresentation` and never reads its source.
- `ui.css` is the separate page-chrome stylesheet for popup/options/setup and is not part of
  this task.

## Behavior

A pure move with no visual or behavioral change:

1. New classic script `extension/lib/presentation-css.js` exposing
   `globalThis.GhostlightPresentationCss.build(tokens, reducedFadeSelector)`, returning the
   stylesheet verbatim (byte-identical CSS text, placeholders renamed to the lowercase
   parameters). Module doc comment states its role and the static-CSS rule.
2. `manifest.json` loads `lib/presentation-css.js` before `lib/presentation.js`.
3. presentation.js calls
   `globalThis.GhostlightPresentationCss.build(TOKENS, REDUCED_FADE_SELECTOR)`; the template
   leaves the file. TOKENS and REDUCED_FADE_SELECTOR construction stay in presentation.js.
4. `tests/shared.test.js`: CSS-derived assertions and the template slice move to the new
   module's source; renderer assertions stay. New guards: the renderer must call
   `GhostlightPresentationCss.build(TOKENS, REDUCED_FADE_SELECTOR)` and must no longer contain
   `style.textContent = \``; the interpolation allowlist for the module template is exactly
   `${reducedFadeSelector}` and `${tokens}`.

## Verification

- `npm test --prefix extension` (all tests, including the reworked assertions).
- `node --check` on the changed and new JavaScript.
- Manual: reload the unpacked extension in the dev browser and confirm the scope glow and one
  transient effect render as before (the owner's daily authority does this implicitly on next
  use; not a gate).

## Out of scope

- No CSS text changes of any kind. Any color, curve, or keyframe edit is a separate change.
- No restructuring of ui.css or of the effect registry.
