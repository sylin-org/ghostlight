# The Ghostlight visual language

Ghostlight drives a person's real browser, and the person is allowed to watch. Every visible
effect exists for one reason: to answer, at a glance, "what is the agent doing to my page right
now?" The effects are a product surface with a designed vocabulary, not decoration. This document
is the normative reference for that vocabulary; the implementation lives in
`extension/agent-visual-indicator.js`.

The principle behind all of it: **show, never surprise.** The agent's presence should feel like a
courteous guest who narrates what they touch -- visible enough to trust, quiet enough to ignore.

## Foundations

- **One accent.** Everything the agent DOES renders in the brand's luminous sky blue
  (`#38bdf8` / rgb `56,189,248`). One color means one meaning: "the agent did this." Governance
  interventions (the denial sticker and attention overlay) are the deliberate exception -- they carry a severity
  color (red/amber/sky/slate) on a neutral near-black chrome (`#0c0f14`, ink `#eaf6ff`), so a
  guardrail reads as a different AUTHORITY, not just another effect.
- **Soft, omnidirectional light.** Glow comes from layered `rgba()` box-shadows and radial
  gradients, radiating evenly -- never a directional drop shadow (nothing in this language has a
  "sun"). Shapes are rounded: circles, pills, rounded rectangles.
- **Spring motion.** Enters settle with `cubic-bezier(.22,1,.36,1)` (a gentle overshoot-free
  spring); exits fade with `ease-out`. Durations sit between roughly 500ms (a beat) and 1600ms
  (a phrase). Nothing snaps, nothing loops forever except the deliberate "alive" pulses (active
  glow, wait pulse).
- **Monospace chrome.** When an effect carries text (captions, keystroke lozenges, guardrails),
  it uses the `ui-monospace` stack -- instrument-panel text, distinct from the page's own type.

## The vocabulary

One visible treatment per agent action. Each row is implemented as its own function and keyframe
set in `agent-visual-indicator.js`.

| Effect | Fires on | What it says | Shape |
| --- | --- | --- | --- |
| Phantom cursor | pointer moves | "the agent's hand is HERE" | sky arrow glyph, glides with a 150ms transition |
| Active glow | any tool running | "the agent is present on this page" | soft pulsing inset vignette around the viewport |
| Click ripple | left/middle click | "clicked, this many times" | expanding ring per click, staggered for double/triple; right-click ring is dashed |
| Drag trail | click-drag path | "dragged along this path" | comet trail of fading radial dots |
| Type shimmer | typing into the focused element | "typing into THIS field" | soft outline pulse on the focused element |
| Field splash | a form write lands (`form_input`, `form_fill`, `file_upload`, `upload_image`) | "the agent just SET this field" | ring + interior wash hugging the field's own rectangle (borrows its border-radius), settles then releases outward |
| Keystroke lozenge | `type` / `key` | "these keys were pressed" | bottom-center pill showing the text or chord |
| Target glow | ref/coordinate click | "THIS element was the target" | brief radial halo at the point (Playwright-highlight lineage) |
| Scroll cue | scroll | "scrolled this direction" | cascading chevrons |
| Read scan | `read_page` / `get_page_text` | "the agent is reading, not touching" | a luminous scan line sweeping down the page |
| Navigate pill | `navigate` | "leaving for this destination" | top-center pill naming the host/path |
| Screenshot frame | screenshot taken | "a capture just happened" | frame flashes, files itself into the corner, and shows a brief camera glyph |
| Zoom frame | `zoom` | "the agent is inspecting this region" | rectangle converging onto the region |
| Wait pulse | `wait` | "deliberately pausing" | breathing dot, center-screen |
| Caption track | any action (opt-in) | subtitle naming the action | bottom-center pill; off by default, gorgeous for demos |
| Narration caption | `narrate` | "the agent wants the watcher to understand this workflow phase" | compact timed sky-accent caption with an Agent label and one three-dot activity cue; auto, top, or bottom |
| Denial sticker | one enforced denial via `Browser::notify()` | "a guardrail held; here is why" | compact centered sticker, replaced or removed after three seconds |
| Attention overlay | ADR-0079 denial burst | "this MCP session is paused until a person decides" | page-softening modal with service-provided controls and popup fallback |
| Recording badge | active screencast lifecycle | "Ghostlight is recording" | truthful red REC extension badge and popup state, never a simulated live preview |

## Invariants

Every effect, present and future, obeys these. They are what make the language trustworthy rather
than merely pretty.

1. **Invisible to the agent.** Every effect hides during a capture (`HIDE_FOR_TOOL_USE` /
   `hiddenForTool`) so the model's own screenshots stay clean, and every effect element's id is
   `ghostlight-`-prefixed so `read_page`/`find` skip it. The agent must never see -- or act on --
   its own reflection.
2. **Untouchable by default.** Transient effects and stickers use `pointer-events:none`. The
   attention overlay's service-provided controls are real, keyboard-focusable `<button>` elements.
3. **Ephemeral by default.** Effects are fire-and-fade confirmations (`addEphemeral`: removed on
   `animationend` with a timeout fallback). An isolated denial lasts three seconds. Narration is
   bounded state: one caption per tab until its timer expires or a new narration replaces it. The
   attention overlay persists because its service-owned pause persists.
4. **Reduced motion respected.** Every animated effect has a `-rm` keyframe variant (plain fade,
   no travel/scale) selected via `prefers-reduced-motion`.
5. **Optional, except governance.** The extension options' master switch (`ghostlight_effects`)
   silences every decorative effect, including narration. Denial and attention presentation is
   deliberately exempt because a guardrail explanation is substantive.
6. **Denials replace; they do not stack.** A new isolated denial replaces the active sticker. An
   open attention overlay supersedes stickers and remains until the service reports a disposition.
7. **Wire text is text.** Any string that can carry page- or policy-influenced content (captions,
   narration, denial title/description, attention labels) is inserted via `textContent`, never `innerHTML` -- this runs as a
   content script on `<all_urls>`.
8. **Mechanism only.** The layer renders what it is told; it makes no policy decisions
   (ADR-0005, ADR-0053). Governance decides in the binary; the extension only presents state.

## Seams (how effects are triggered)

- **Service-worker messages** (`chrome.tabs.sendMessage` -> the indicator's `onMessage` switch):
  the normal path. The binary/service worker knows which tool ran and sends the matching
  `AGENT_*` message, usually with viewport coordinates.
- **The `GhostlightFx` same-world export** (bottom of `agent-visual-indicator.js`): for sibling
  content scripts that know the target ELEMENT (e.g. `content.js`'s form writers calling
  `fieldSplash`). Both scripts share the extension's isolated world, so this is a direct,
  page-unreachable call -- deliberately NOT a DOM `CustomEvent`, which any page could forge. Use
  this seam when the trigger's natural home is in-page and a rect would otherwise have to ride a
  wire message.

Narration uses the service-worker seam but has a longer lifecycle than an action effect. Its
memory-only worker record survives navigation only until the original deadline, then replays the
remaining duration into the new document. It is commentary, never governance: sky accent, inset
compact caption, optional under the effects switch, and always below the guardrail layer. Auto
placement chooses top or bottom once from recent touched-control, pointer, and scroll signals, then
stays put. Narration, stickers, and overlays use bounded viewport-responsive sizing. The attention
overlay is central, visually stronger, and never truncates its security text or controls.

## Adding a new effect

1. Name what it must SAY in one sentence ("the agent just X"). If it says nothing a watcher needs,
   it does not join the vocabulary.
2. Build it from the foundations: sky accent (unless it speaks with governance's authority),
   omnidirectional glow, rounded geometry, spring enter / ease-out exit, and a `-rm` variant.
3. Wire it through the right seam (message for tool-level triggers, `GhostlightFx` for in-page
   ones), gate it on `hiddenForTool`/`document.hidden`/`effectsEnabled`, give its element the
   `ghostlight-` prefix, and route it through `addEphemeral` unless it is genuinely state.
4. Decide its replacement, timeout, and cleanup owner explicitly.
5. Add its row to the vocabulary table above. The table and the code move together.

## Provenance

The vocabulary draws on Screen Studio (cursor glide + click rings), KeyCastr/Keyviz (keystroke
lozenges, scroll cues), and Playwright's `.highlight()` (confirm the target); the concept of an
always-visible agent affordance follows the official Claude-in-Chrome extension (interface
harvested, never code -- ADR-0050 D1). The denial severity taxonomy is SAPS PRES-HIGH-01. The
field splash and this document were added when form filling joined the demo's visible repertoire
(2026-07); the owner's directive: show the user what we're touching, and make it part of the
visual language of the service.
