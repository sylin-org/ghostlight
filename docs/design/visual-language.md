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
  (a phrase). Nothing snaps. The controlled-tab border alone breathes continuously because its
  scope remains true. Active signatures last exactly as long as their operation and retain a
  bounded stale fallback; confirmation and spatial effects stay brief.
- **Monospace chrome.** When an effect carries text (captions, keystroke lozenges, guardrails),
  it uses the `ui-monospace` stack -- instrument-panel text, distinct from the page's own type.

## The vocabulary

One coherent treatment per agent action. A treatment may compose spatial and non-spatial parts only
when they answer different necessary questions, such as a screenshot frame saying what was captured
and a camera medallion saying that capture completed. Rendering lives in
`agent-visual-indicator.js`; fixed signature vocabulary and placement scoring live in their pure
domain modules.

| Effect | Fires on | What it says | Shape |
| --- | --- | --- | --- |
| Phantom cursor | pointer moves | "the agent's hand is HERE" | sky arrow glyph, glides with a 150ms transition |
| Controlled-tab border | every Ghostlight-managed tab | "an agent can act inside this tab" | persistent inset sky border with a slow, low-amplitude breathing pulse |
| Click ripple | left/middle click | "clicked, this many times" | expanding ring per click, staggered for double/triple; right-click ring is dashed |
| Drag trail | click-drag path | "dragged along this path" | comet trail of fading radial dots |
| Type shimmer | typing into the focused element | "typing into THIS field" | soft outline pulse on the focused element |
| Field splash | a form write lands (`form_input`, `form_fill`, `file_upload`, `upload_image`) | "the agent just SET this field" | ring + interior wash hugging the field's own rectangle (borrows its border-radius), settles then releases outward |
| Keystroke lozenge | named `key` chords | "these named keys were pressed" | bottom-center pill showing the chord; ordinary typed values never appear |
| Target glow | ref/coordinate click | "THIS element was the target" | brief radial halo at the point (Playwright-highlight lineage) |
| Scroll cue | scroll | "scrolled this direction" | cascading chevrons |
| Read scan | `read_page` / `get_page_text` | "the agent is reading, not touching" | a luminous scan line sweeping down the page |
| Navigate pill | `navigate` | "leaving for this destination" | top-center pill naming the host/path |
| Screenshot frame | screenshot taken | "this page was captured" | frame flashes and files itself into the corner |
| Zoom frame | `zoom` | "the agent is inspecting this region" | rectangle converging onto the region |
| Action signature medallion | non-spatial activity without presentational content | "the agent is doing THIS kind of work" | one signal-aware corner shell: JavaScript workwheel, glowing typing keyboard, three waiting lights, or post-capture camera |
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
3. **Ephemeral by default.** Spatial action effects are fire-and-fade confirmations (`addEphemeral`:
   removed on `animationend` with a timeout fallback). An active signature begins before its action,
   finishes through that action's `finally` path, and has a stale fallback. A confirmation signature
   appears only after its action. An isolated denial lasts three seconds. Narration is bounded
   state: one caption per tab until its timer expires or a new narration replaces it. The attention
   overlay persists because its service-owned pause persists. The controlled-tab border persists
   because the tab remains agent-reachable.
4. **Reduced motion respected.** Animated effects use a plain-fade `-rm` variant or disable their
   internal motion under `prefers-reduced-motion`.
5. **Optional decoration; mandatory scope and governance.** The extension options' master switch
   (`ghostlight_effects`) silences decorative action effects and narration. The controlled-tab
   border is exempt because agent reachability must remain visible. Denial and attention
   presentation is exempt because a guardrail explanation is substantive.
6. **Denials replace; they do not stack.** A new isolated denial replaces the active sticker. An
   open attention overlay supersedes stickers and remains until the service reports a disposition.
7. **Wire text is text.** Any string that can carry page- or policy-influenced content (captions,
   narration, denial title/description, attention labels) is inserted via `textContent`, never `innerHTML` -- this runs as a
   content script on `<all_urls>`.
8. **Mechanism only.** The layer renders what it is told; it makes no policy decisions
   (ADR-0005, ADR-0053). Governance decides in the binary; the extension only presents state.

## Seams (how effects are triggered)

- **Presentation Broker** (`extension/lib/presentation-broker.js`): the normal tool-level path.
  The binary/service worker knows which tool ran and publishes the matching `AGENT_*` intent,
  usually with viewport coordinates. The broker binds it to the current Chrome document,
  activates the packaged renderer on demand, and requires an exact channel/revision/document
  acknowledgement. Controlled scope, narration, notifications, and attention replay while their
  state remains true. Transient action effects and signature lifecycle events never cross a
  document change.
- **The `GhostlightFx` same-world export** (bottom of `agent-visual-indicator.js`): for sibling
  content scripts that know the target ELEMENT (e.g. `content.js`'s form writers calling
  `fieldSplash`). Both scripts share the extension's isolated world, so this is a direct,
  page-unreachable call -- deliberately NOT a DOM `CustomEvent`, which any page could forge. Use
  this seam when the trigger's natural home is in-page and a rect would otherwise have to ride a
  wire message.

The controlled-tab border uses a deadline-free broker state tied exactly to ADR-0066's
`managedTabs` set. It survives navigation, detachment from the visible tab group, and a Manifest
V3 worker restart. It is hidden only during capture and restored immediately afterward. A gentle
four-second breathing cycle signals presence without resembling progress or urgency.

Narration uses the broker's timed-state channel and has a longer lifecycle than an action effect.
Its browser-session-only record survives navigation and a Manifest V3 worker restart only until
the original deadline, then replays the remaining duration into the current acknowledged
document. It is commentary, never governance: sky accent, inset compact caption, optional under
the effects switch, and always below the guardrail layer. Auto placement chooses top or bottom
once from recent touched-control, pointer, and scroll signals, then stays put. Narration, stickers,
and overlays use bounded viewport-responsive sizing. The attention overlay is central, visually
stronger, and never truncates its security text or controls.

Action signatures use fixed, content-free start, finish, and confirm messages from
`extension/lib/action-signature.js`. The renderer owns one medallion and its bounded timers; broker
events provide ordered exact-document delivery but no replay. `extension/lib/presentation-placement.js`
scores narration edges and four signature corners from recent pointer, focus, touched-control,
scroll, and occupied-presentation signals. Each presentation chooses once and stays put.

## Adding a new effect

1. Name what it must SAY in one sentence ("the agent just X"). If it says nothing a watcher needs,
   it does not join the vocabulary.
2. Build it from the foundations: sky accent (unless it speaks with governance's authority),
   omnidirectional glow, rounded geometry, spring enter / ease-out exit, and a `-rm` variant.
3. Wire it through the right seam (Presentation Broker for tool-level triggers, `GhostlightFx` for
   in-page ones), gate it on `hiddenForTool`/`document.hidden` and, for decoration,
   `effectsEnabled`; give its element the `ghostlight-` prefix, and route it through
   `addEphemeral` unless it is genuinely state.
4. Decide its replacement, timeout, and cleanup owner explicitly.
5. Add its row to the vocabulary table above and its tool/action coverage to
   [tool-visual-signatures.md](tool-visual-signatures.md). The tables and code move together.

## Provenance

The vocabulary draws on Screen Studio (cursor glide + click rings), KeyCastr/Keyviz (keystroke
lozenges, scroll cues), and Playwright's `.highlight()` (confirm the target); the concept of an
always-visible agent affordance follows the official Claude-in-Chrome extension (interface
harvested, never code -- ADR-0050 D1). The denial severity taxonomy is SAPS PRES-HIGH-01. The
field splash and this document were added when form filling joined the demo's visible repertoire
(2026-07); the owner's directive: show the user what we're touching, and make it part of the
visual language of the service. The controlled-tab border was clarified during the ADR-0081 live
gate: the border discloses agent-reachable scope, while the effects inside it explain activity.
