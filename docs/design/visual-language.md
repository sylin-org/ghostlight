# The Ghostlight visual language

Ghostlight drives a person's real browser, and the person is allowed to watch. Every visible
effect exists for one reason: to answer, at a glance, "what is the agent doing to my page right
now?" The effects are a product surface with a designed vocabulary, not decoration. This document
is the normative reference for that vocabulary. The 2.0 renderer is two page assets under
`crates/orchestrator/src/browser/adapter/chromium/protocol4/assets/`: `presentation.js` builds the
markup and owns every timer, and `presentation-css.js` is its stylesheet.

The principle behind all of it: **show, never surprise.** The agent's presence should feel like a
courteous guest who narrates what they touch -- visible enough to trust, quiet enough to ignore.

## Foundations

- **One accent.** Everything the agent does renders in the brand's luminous sky blue
  (`#38bdf8` / rgb `56,189,248`). One color means one meaning: "the agent did this." The denial
  ribbon is the one exception. It is red on the near-black ground (`#0c0f14`, ink `#eaf6ff`), so a
  guardrail reads as a different authority, not as another effect.
- **Soft, omnidirectional light.** Glow comes from layered `rgba()` box-shadows and radial
  gradients, radiating evenly -- never a directional drop shadow. Shapes are rounded: circles,
  pills, rounded rectangles.
- **Spring motion.** Enters settle with `cubic-bezier(.22,1,.36,1)`; exits fade with `ease-out`.
  A transient effect is on screen for 260ms to 1600ms. Nothing snaps. The controlled-tab border
  alone breathes continuously, because its scope stays true. A medallion lasts exactly as long as
  its action.
- **Monospace chrome.** Text the renderer draws (the caption, the navigate pill, the denial
  ribbon) uses the `ui-monospace` stack -- instrument-panel text, distinct from the page's type.

## The 2.0 vocabulary

What `presentation.js` and `presentation-css.js` draw in a page today. Each row names the
activity the service sends and the signal it arrives on: `start` while the action runs, `target`
when the service names the element, `completion` when it is done. The service sends a `target`
only for `click`, `hover`, `drag` and `type`, and a completion names no element, so the table
says what reaches the page, not every branch the renderer has.

| Effect | Fires on | What it says | Shape |
| --- | --- | --- | --- |
| Controlled-tab border | a tab Ghostlight manages, while the runtime is reachable | "an agent can act inside this tab" | inset sky glow on the viewport edge, breathing over four seconds; off while the tab is recorded |
| Phantom cursor | `click` and `hover` targets | "the agent's hand is here" | sky arrow with a white outline, gliding to the target's centre over 150ms |
| Target glow | `click` target | "this element was the target" | rounded halo hugging the element, 720ms |
| Click ripple | `click` target | "clicked here, this many times, with this button" | one expanding ring per click, at most three, 150ms apart, 620ms each; dashed for a secondary button |
| Drag trail | `drag` target | "dragged along this path" | twelve fading sky dots from the last pointer position to the target |
| Field splash | nothing today: the renderer draws it on a `fill` or `upload` target, and the service sends neither, so `browser_fill_form` and `browser_upload` show only the caption, when it is on | "the agent just set this field" | ring and interior wash hugging the field, settling then releasing outward, 700ms |
| Field shimmer | `type` target | "typing into this field" | soft outline pulse on the field, 900ms |
| Scroll cue | `scroll` start | "scrolled" | three cascading chevrons at the viewport centre, 1100ms |
| Read scan | `read` start | "the agent is reading, not touching" | a sky band sweeping down the viewport, 1450ms |
| Navigate pill | `navigate` completion | "arrived at this destination" | top-centre pill: a small sky right arrow (shaft and head), drawn as SVG, then the host and path (58 characters at most), 1600ms |
| Keycap | each `key` start | "a key was pressed" | one unlabelled glowing keycap in a bottom-centre lozenge, 1250ms; it never names the key |
| Screenshot frame | `screenshot` completion | "this page was captured" | a brief flash, then a frame that shrinks into the bottom-right corner, 1500ms, with the camera medallion |
| Zoom frame | `zoom` completion (set zoom or window resize) | "the agent changed this tab's zoom or resized its window" | a frame converging onto the middle of the viewport (its central 60 percent), 1150ms |
| Signature medallion | `script`, `type`, `wait` and `find` start; `screenshot` completion | "the agent is doing this kind of work" | one 58px rounded tile fixed top-right, on the near-black ground with a sky stroke: a turning workwheel with three particles, a pulsing keyboard, three waiting lights, a searching lens, or a camera; a white glint when its invocation completes |
| Caption | any activity, opt-in | names the activity ("Clicking", "Reading page") | bottom-centre pill, 1500ms; off by default |
| Denial ribbon | one enforced denial | "a guardrail held; here is why" | centred ribbon on the near-black ground with a red shield badge, a title and one line of detail; replaced by the next denial, gone after five seconds |

An activity with no row, such as `dialog`, shows only the caption, when the caption is on. The
toolbar's REC badge says a recording is running; it belongs to the extension, not to the page.

Two switches in the extension options shape this. **Effects** (`ghostlight_effects`, on by
default) silences every row except the controlled-tab border and the denial ribbon: reachable
scope and a guardrail's reason are never decoration. **Captions** (`ghostlight_captions`, off by
default) turns the caption on, whatever Effects says.

## How a click describes itself

A click is the one action whose confirmation has to match what actually landed, so it is the one
indication that carries a shape.

`ClickShape { clicks, button }` rides on `PresentationSignal`, populated from
`DomainEvent::TargetIndicated` at the click emit site in
`crates/orchestrator/src/work/pointer.rs`. The other target indications -- hover, drag and type -- pass
`None`, because they have no click to describe. The renderer draws one ring per click on the 150ms
stagger unit and dashes the ring when the button is secondary.

The field is optional on the wire with a serde default, so an adapter that predates it renders one
primary ring, exactly as every click did before. The shape carries how the click landed and never
what the page holds.

## The desktop workbench shares this language

The workbench window is a second surface for the same vocabulary, with two deliberate differences.

- **Tempo is shared.** A treatment that means the same thing in both places keeps the same beat.
  The workbench names them as `--beat-*` tokens in `crates/orchestrator/ui/styles.css`, taken from
  the renderer's frozen `visualIdentity`: the workwheel turns over 2400ms in both, waiting lights
  breathe over 1050ms with a 150ms stagger in both, the keyboard pulses over 1150ms, the read scan
  runs 1450ms, and scope breathes over four seconds.
- **Accent differs on purpose.** The workbench carries Ghostlight's published teal, while the
  in-page renderer keeps the sky signal users are trained on. See the 2026-08-11 amendment to
  ADR-0102. In the window, teal means live: it marks work happening and the one primary action in
  a view, and everything at rest is ink.
- **The medallion shapes are the same four**: workwheel, keyboard, waiting lights, camera. In the
  page they mark live activity; in the workbench they animate while an action runs and freeze when
  it settles, so the frozen glyph in the queue is the same shape that was moving a moment earlier.

## Invariants

Every effect, present and future, obeys these.

1. **Invisible to the agent.** The renderer lives in a closed shadow root on one host,
   `ghostlight-presentation-root`. The page cannot reach into it, and the whole host is hidden
   while a capture runs, so the model's own screenshots stay clean. The agent never sees, or acts
   on, its own reflection.
2. **Always passive on the page.** The host and everything in it use `pointer-events:none`, also
   set inline so it holds without the stylesheet. Nothing contains a control, takes focus,
   intercepts input, or dims the page. Interactive control belongs in the Ghostlight window.
3. **Ephemeral by default.** A transient effect is removed on `animationend`, with a fallback
   timer one grace period after its beat. A medallion begins with its action and leaves when that
   invocation completes, with a 35-second stale fallback. The denial ribbon lasts five seconds.
   When the runtime is paused, ended or disconnected, every transient effect clears.
4. **Reduced motion respected.** Under `prefers-reduced-motion`, every registered effect becomes
   a short plain fade, the border stops breathing, and the denial ribbon appears without motion.
5. **One at a time.** One medallion, one caption and one denial ribbon at most. A new one replaces
   the old; nothing stacks.
6. **Wire text is text.** The denial title and detail are bounded and set with `textContent`.
   Markup is built with DOM calls, never an HTML string, so pages that enforce Trusted Types still
   render it.
7. **Mechanism only.** The renderer draws what the service tells it and makes no policy decision
   (ADR-0005, ADR-0053). Governance decides in the service.

## Adding a new effect

1. Name what it must say in one sentence ("the agent just X"). If it says nothing a watcher needs,
   it does not join the vocabulary.
2. Build it from the foundations: sky accent, omnidirectional glow, rounded geometry, spring enter
   and ease-out exit.
3. Add its row to `TRANSIENT_EFFECTS` in `presentation.js`. The row's `beat` is its lifetime, and
   the row enrolls it for reduced motion; nothing else about it is kept by hand.
4. Draw it from the matching branch of `render`, and add its rule to `presentation-css.js`.
5. Add its row to the 2.0 vocabulary table above, with every activity and signal the service
   sends for it in the "Fires on" column. A renderer branch the service never reaches is not
   coverage; the row says so. That column is the coverage record; the table and the code move
   together.

## Provenance

The vocabulary draws on Screen Studio (cursor glide + click rings), KeyCastr/Keyviz (keystroke
lozenges, scroll cues), and Playwright's `.highlight()` (confirm the target); the concept of an
always-visible agent affordance follows the official Claude-in-Chrome extension (interface
harvested, never code -- ADR-0050 D1). The pre-1.0 denial severities came from SAPS PRES-HIGH-01. The
field splash and this document were added when form filling joined the demo's visible repertoire
(2026-07); the owner's directive: show the user what we're touching, and make it part of the
visual language of the service. The controlled-tab border was clarified during the ADR-0081 live
gate: the border discloses agent-reachable scope, while the effects inside it explain activity.

## History: the pre-1.0 in-page vocabulary (until 2026-08-10)

The sections below describe the 0.x extension, before the 1.0 rebuild of 2026-08-10. Its
`agent-visual-indicator.js` drew the effects and `extension/lib/presentation-broker.js` delivered
them; the rebuild deleted both. The tool names below (`computer.key`, `read_page`, `form_input`
and the rest) are the 0.x tools. Find highlights and horizon glows, narration, image drops and four
denial severities belong to this vocabulary, and no 1.x or 2.0 release drew them.

Every 1.x release, v1.0.0 to v1.3.15, drew from the renderer the rebuild wrote,
`extension/lib/presentation.js`, and its tools were already the `browser_*` tools. That renderer
drew the set in the 2.0 table above, with two differences: it also drew an explicit attention
notice in the denial ribbon, which 2.0 dropped, and its navigate pill's arrow was typed text
rather than an SVG arrow. On 2026-10-14 the renderer moved into the orchestrator's Chromium
adapter, and adapter protocol 4 placed it under `protocol4/assets/`. The sections below are kept
as they were written.

### Foundations

- **One accent.** Everything the agent DOES renders in the brand's luminous sky blue
  (`#38bdf8` / rgb `56,189,248`). One color means one meaning: "the agent did this." Governance
  interventions (denial stickers and explicit attention notices) are the deliberate exception -- they carry a severity
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

### The vocabulary

One coherent treatment per agent action. A treatment may compose spatial and non-spatial parts only
when they answer different necessary questions, such as a screenshot frame saying what was captured
and a camera medallion saying that capture completed. Rendering lives in
`agent-visual-indicator.js`; fixed signature vocabulary and placement scoring live in their pure
domain modules.

| Effect | Fires on | What it says | Shape |
| --- | --- | --- | --- |
| Phantom cursor | pointer moves | "the agent's hand is HERE" | sky arrow glyph, glides with a 150ms transition |
| Controlled-tab border | every Ghostlight-managed tab | "an agent can act inside this tab" | persistent inset sky border with a slow, low-amplitude breathing pulse |
| Click ripple | any click | "clicked here, this many times, with this button" | one expanding sky ring per click, staggered by 150ms; a secondary-button ring is dashed |
| Drag trail | click-drag path | "dragged along this path" | comet trail of fading radial dots |
| Type shimmer | typing into the focused element | "typing into THIS field" | soft outline pulse on the focused element |
| Field splash | a form write lands (`form_input`, `form_fill`, `file_upload`, `upload_image`) | "the agent just SET this field" | ring + interior wash hugging the field's own rectangle (borrows its border-radius), settles then releases outward |
| Keystroke lozenge | `computer.key` | "these keys were pressed, without exposing protected input" | bottom-center chord groups; ordinary printable keys stay literal, protected or unobservable keys become an unlabeled glowing keycap |
| Target glow | ref/coordinate click | "THIS element was the target" | brief radial halo at the point (Playwright-highlight lineage) |
| Scroll cue | scroll | "scrolled this direction" | cascading chevrons |
| Scroll destination | ref-based `scroll_to` | "arrived at THIS element" | chevrons settle into an exact target halo; semantic composition does not repeat the halo |
| Image drop | coordinate `upload_image` | "dispatched an image HERE" | fixed photo tile settles into the destination halo without image metadata or an acceptance claim |
| Read scan | `read_page` / `get_page_text` | "the agent is reading, not touching" | a luminous scan line sweeping down the page |
| Find | `find` | "the agent is searching; these are its ranked results" | signal-aware lens badge, native text highlights, shape fallback, and top/bottom horizon glows |
| Navigate pill | `navigate` | "leaving for this destination" | top-center pill naming the host/path |
| Screenshot frame | screenshot taken | "this page was captured" | frame flashes and files itself into the corner |
| Zoom frame | `zoom` | "the agent is inspecting this region" | rectangle converging onto the region |
| Action signature medallion | non-spatial activity without presentational content | "the agent is doing THIS kind of work" | one signal-aware corner shell: JavaScript workwheel, glowing typing keyboard, three waiting lights, or post-capture camera |
| Caption track | any action (opt-in) | subtitle naming the action | bottom-center pill; off by default, gorgeous for demos |
| Narration caption | `narrate` | "the agent wants the watcher to understand this workflow phase" | compact timed sky-accent caption with an Agent label and one three-dot activity cue; auto, top, or bottom |
| Denial sticker | one enforced denial | "a guardrail held; here is why" | passive centered sticker, replaced or removed after five seconds |
| Explicit attention notice | legacy explicit attention signal | "see the workbench for details" | passive five-second notice; no modal, page dimming, focusable controls, or denial-burst trigger (ADR-0185) |
| Recording badge | active screencast lifecycle | "Ghostlight is recording" | truthful red REC extension badge and popup state, never a simulated live preview |

### Invariants

Every effect, present and future, obeys these. They are what make the language trustworthy rather
than merely pretty.

1. **Invisible to the agent.** Every effect hides during a capture (`HIDE_FOR_TOOL_USE` /
   `hiddenForTool`) so the model's own screenshots stay clean, and every effect element's id is
   `ghostlight-`-prefixed so `read_page`/`find` skip it. The agent must never see -- or act on --
   its own reflection.
2. **Always passive on the page.** Every effect and notice uses `pointer-events:none`. No page
   treatment contains buttons or focusable controls, intercepts keyboard input, or disables the
   page, even when CSS fails. Interactive runtime control belongs in the Ghostlight window, the desktop shell's webview (ADR-0185).
3. **Ephemeral by default.** Spatial action effects are fire-and-fade confirmations (`addEphemeral`:
   removed on `animationend` with a timeout fallback). An active signature begins before its action,
   finishes through that action's `finally` path, and has a stale fallback. A confirmation signature
   appears only after its action. Denial and explicit attention notices last five seconds.
   Narration is bounded state: one caption per tab until its timer expires or a new narration
   replaces it. No attention overlay persists. The controlled-tab border persists
   because the tab remains agent-reachable.
4. **Reduced motion respected.** Animated effects use a plain-fade `-rm` variant or disable their
   internal motion under `prefers-reduced-motion`.
5. **Optional decoration; mandatory scope and governance.** The extension options' master switch
   (`ghostlight_effects`) silences decorative action effects and narration. The controlled-tab
   border is exempt because agent reachability must remain visible. Denial and attention
   presentation is exempt because a guardrail explanation is substantive.
6. **Notices replace; they do not stack.** A denial or legacy explicit attention signal replaces
   the active notice and expires after five seconds. Repeated denials never create a session hold.
7. **Wire text is text.** Any string that can carry page- or policy-influenced content (captions,
   narration, denial title/description, attention labels) is inserted via `textContent`, never `innerHTML` -- this runs as a
   content script on `<all_urls>`.
8. **Mechanism only.** The layer renders what it is told; it makes no policy decisions
   (ADR-0005, ADR-0053). Governance decides in the binary; the extension only presents state.

### Seams (how effects are triggered)

- **Presentation Broker** (`extension/lib/presentation-broker.js`): the normal tool-level path.
  The binary/service worker knows which tool ran and publishes the matching `AGENT_*` intent,
  usually with viewport coordinates. The broker binds it to the current Chrome document,
  activates the packaged renderer on demand, and requires an exact channel/revision/document
  acknowledgement. Controlled scope, narration, and notifications replay while their
  state remains true. Transient action effects and signature lifecycle events never cross a
  document change.
- **The `GhostlightFx` same-world export** (bottom of `agent-visual-indicator.js`): for sibling
  content scripts that know the target ELEMENT (e.g. `content.js`'s form writers calling
  `fieldSplash`, ref scrolling calling `scrollTarget`, or coordinate image placement calling
  `imageDrop`). Both scripts share the extension's isolated world, so this is a direct,
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
and explicit attention notices use bounded viewport-responsive sizing. No message becomes a modal
or prevents the person from interacting with the page.

Action signatures use fixed, content-free start, finish, and confirm messages from
`extension/lib/action-signature.js`. The renderer owns one medallion and its bounded timers; broker
events provide ordered exact-document delivery but no replay. `extension/lib/presentation-placement.js`
scores narration edges and four signature corners from recent pointer, focus, touched-control,
scroll, and occupied-presentation signals. Each presentation chooses once and stays put.

Key lozenges use the privacy-safe presentation structure from `extension/lib/keys.js`. The worker
derives it after trusted target observation and before publishing `AGENT_KEYSTROKE`; raw fallback
text never enters the page renderer. Named keys and command shortcuts retain their labels.
Printable keys remain literal on ordinary targets, while protected or unobservable targets use an
unlabeled glowing keycap.

Destination cues use the same-world seam so exact element geometry stays inside the current page.
Ref scrolling settles chevrons into the element and deduplicates a just-published semantic halo.
Coordinate image placement shows a fixed photo tile at the destination but never receives the
image payload or page-acceptance result. Console and network buffer reads remain intentionally
quiet because they do not manipulate the rendered document.

### Adding a new effect

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
