# Visual feedback vocabulary

A user watching an agent drive their real browser should be able to see what it is doing, and a
recording of that should read clearly in a demo. Ghostlight gives every agent action a distinct,
consistent on-page treatment: a small visual "dictionary" the phantom cursor speaks.

## Principles

1. **Every action self-narrates.** One recognizable treatment per action type, not a generic blink.
2. **Confirm the target, not just the gesture.** For an agent, showing WHAT it acted on (the element
   glows) matters as much as where, and builds trust. This is Playwright `.highlight()`'s insight.
3. **One calm brand.** Sky blue (`#38bdf8`), the ghost identity, additive and never blocking.
4. **Hidden from the agent's own captures.** Every effect respects the same hide-during-capture path
   the cursor and ripple already use, so nothing pollutes the model's screenshot. The screenshot
   effect is the one that fires only AFTER a capture completes.
5. **Accessible.** Effects honor `prefers-reduced-motion` where they move, and are `aria-hidden` and
   `ghostlight-`-prefixed so page reads (`read_page` / `find`) skip them.

## The dictionary

| Action | Treatment |
| --- | --- |
| navigate | a destination pill (host + path) at top-center, after the new page loads |
| left / double / triple click | expanding ring(s), staggered, plus the element under the point glows |
| right-click | a dashed ring (a secondary action) |
| drag | a comet trail along the path |
| hover | the cursor glides in with a soft glow, no ring |
| type | the focused field shimmers, plus a keystroke lozenge of the text |
| key / shortcut | a keystroke lozenge, e.g. `Ctrl` + `A` |
| scroll | directional chevrons cascading the way the page moves |
| screenshot | a sky shutter flash, then the frame "files itself" into the bottom-right corner |
| zoom | a magnifier frame closes on the captured region |
| read_page / find / get_page_text | a scan-line sweeps down the page ("the agent is reading") |
| wait | a soft breathing dot |
| narrate | a timed, responsive sky-accent Agent ribbon that explains a meaningful workflow phase |
| ambient | the "agent active" glow while a tool runs, plus an optional action caption (below) |

The scan-line is ours: no recording tool has it, because none of them is an agent reading a page.

## Where it lives

- **Render layer:** `extension/agent-visual-indicator.js` (a policy-free content script). Each
  treatment is a small function that appends an ephemeral, pointer-transparent element to the FX
  layer and removes it when its animation ends. Narration is the bounded-state exception: the
  service worker owns its timer and navigation replay. Timings are constants at the top of those
  files.
- **Triggers:** `extension/service-worker.js` sends one message per action
  (`AGENT_TARGET_GLOW`, `AGENT_KEYSTROKE`, `AGENT_SCROLL_CUE`, `AGENT_READ_SCAN`,
  `AGENT_NAVIGATE_PILL`, `AGENT_SCREENSHOT_FX`, `AGENT_ZOOM_FRAME`, `AGENT_WAIT_PULSE`,
  `AGENT_NARRATION`) at the point
  the action runs. The worker holds mechanism only; the effects carry no policy.
- **Interactive reference:** the vocabulary was designed and approved in a standalone preview that
  plays each treatment on a mock browser, preserved and extended at
  [visual-feedback-dictionary.html](visual-feedback-dictionary.html). Open it in a browser and click
  any entry to replay a single effect, or "Run the tour" for the full sequence. It is a good source
  for a marketing GIF, separate from the store screenshots, which should show the real extension.

## Action caption (off by default)

An optional subtitle track (`SET_CAPTIONS`) names the current action bottom-center: gorgeous for a
recorded demo, too chatty for everyday driving, so it is off unless turned on. It is hidden during
captures like every other effect, so it never lands in the model's image.

## Agent narration

`narrate` is the semantic track for a person watching a longer workflow. It is not a mechanical
action caption and cannot imitate the governance ribbon. One wide edge ribbon appears per tab, a
new call replaces it, and the original deadline survives navigation. `auto` chooses top or bottom
once, away from recent focus, pointer, and scroll activity; explicit edges stay deterministic. The
ribbon does not chase the user after it appears. It requires no RAWX capability because it does not
touch page content. It still follows tab ownership, audit, take-the-wheel, capture hiding, and the
visual-effects preference.

## Sources

The grammar is borrowed from the recording and demo craft, given Ghostlight's accent:

- Screen Studio -- spring-eased cursor glide and expanding click rings.
- KeyCastr and Keyviz -- keystroke lozenges and directional scroll cues.
- Playwright `.highlight()` -- glow the target element to confirm what is being acted on.
