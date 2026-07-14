# Ghostlight visual-language refinement, 2026-07

Status: Accepted and implemented on `dev`. ADR-0079 and the marked ADR-0072/0073 amendments are
authoritative; [The Ghostlight visual language](visual-language.md) now carries the normative
production vocabulary.

## Goal

Make every Ghostlight surface feel related while preserving the best part of the current product:
fluid, legible feedback that lets a person understand what an agent is doing in their real browser.
Visual weight should match operational meaning.

The current normative source is [The Ghostlight visual language](visual-language.md). This design
record preserves the reasoning and rejected alternatives behind the implemented refinement.

## One metaphor, four roles

Ghostlight's common metaphor is light revealing activity and meeting a boundary.

| Role | Visual owner | Meaning |
| --- | --- | --- |
| Invite | Mascot | This is approachable and belongs to Ghostlight. |
| Activity | Luminous sky blue | The agent is reading, pointing, changing, or confirming. |
| Authority | Shield plus severity text | A boundary held or the agent is paused. |
| Context | Quiet neutral surface | Explanations and controls belong to the product, not the page. |

The mascot can welcome, guide installation, and celebrate connection. It should not issue a policy
decision. Governance needs the calm precision of a shield, clear language, and an exact state.

## Shared surface grammar

The repository, product site, post-install pages, extension popup, and in-page chrome should share:

- the Ghostlight name and mark in a consistent header treatment;
- teal brand identity for product ownership and sky blue for live agent activity;
- the same rounded geometry, soft omnidirectional light, neutral ink, and concise labels;
- explicit transition copy when Chrome or GitHub becomes the next surface; and
- the vocabulary `Local service`, `Browser extension`, `MCP client`, and `Ready`.

Third-party browser and GitHub pages cannot look like Ghostlight. The preceding Ghostlight surface
should say where the user is going and what to do there so the visual break feels intentional.

## Proposed in-browser vocabulary

### Action and read effects: retain

Keep the current action effects, especially the read scan, active glow, target feedback, and field
effects. They are brief, tied to concrete events, absent from model captures, and already express
the product metaphor well. Refinement should focus on token consistency and accessibility, not a
wholesale redesign.

### Narration: a caption, not a countdown

Replace the wide narration ribbon and progress line with a compact caption surface at the selected
edge. It retains the `Agent` label and the one-message replacement semantics from ADR-0072.

The proposed temporary signal is three small dots that vanish once in sequence after entrance.
They do not loop, count down, or map to remaining milliseconds. The narration itself remains for
its requested bounded lifetime. This avoids teaching the user that a precise action or deadline is
tied to the animation.

The caption remains pointer-transparent, avoids recent user activity under `auto` placement, hides
from model captures, respects the visual-effects setting, and has a no-travel reduced-motion form.
Long text wraps within a bounded width. It never borrows a shield, lock, severity color, or modal
geometry.

Accepting this change requires an ADR-0072 amendment because the current ADR deliberately specifies
a responsive edge ribbon and progress styling.

### Ordinary denial: a brief sticker

An isolated enforced denial produces a compact sticker near the viewport center:

```text
              +------------------------------+
              | [shield] Write blocked       |
              | Domain is outside your grant |
              +------------------------------+
```

It is pointer-transparent, appears for about three seconds, and is replaced by the next denial.
It does not blur the page or imply that future calls are paused. The shield can receive a short
one-shot glow; severity is always expressed through icon and text, not color alone.

The user can inspect durable detail in a trusted Ghostlight surface, but expiry or dismissal never
changes policy. If no such detail surface exists at implementation time, omit the affordance rather
than creating a dead control.

### Repeated denials: a real pause overlay

Only the service-side `AttentionRequired` state proposed in
[research 16](../research/16-denial-burst-circuit-breaker.md) may produce a blocking overlay.

```text
     [page dimmed and softly defocused]

          +-------------------------------------------+
          | [shield] Agent paused                     |
          | Repeated denied actions need your review. |
          |                                           |
          | [Keep paused]  [Resume agent]             |
          | [Quiet repeats for this site and resume]  |
          | [End session]                             |
          +-------------------------------------------+
```

This surface is keyboard navigable, focus trapped while expanded, and announced as an alert dialog.
`Keep paused` minimizes it to a persistent shield indicator without clearing the service latch.
The minimized indicator remains available outside page capture and returns to the control surface
on activation.

The blur is atmospheric and semantic: it indicates that agent dispatch is paused, not that the
user has lost the browser. It must not make page text unreadable to the human at extreme zoom or
leave an invisible focus trap after minimization.

### Screenshot: add a camera beat after capture

Add a small camera glyph to the existing shutter/frame effect. It must still begin only after the
screenshot is captured and remain hidden from subsequent model captures. No persistent badge or
sound is needed for a single still image.

### Recording: persistent, tab-scoped, and honest

A recording is ongoing state, so it deserves a persistent signal. The preferred first design is:

- a tab-scoped extension action badge containing `REC`, using Chrome's supported per-tab badge;
- a tooltip or popup state saying `Recording this tab` and `Frames are held in local memory`; and
- an optional in-page red dot with a soft sky-blue halo, visually subordinate to page controls.

Chrome's MV3 action API supports per-tab badge state, which keeps the signal in browser chrome and
outside the captured page. The W3C Screen Capture specification treats a live indicator and an
accurate description of what is shared as important user-agent responsibilities. Ghostlight is not
using `getDisplayMedia`, but the expectation is still sound prior art.

Sources: [Chrome action API](https://developer.chrome.com/docs/extensions/reference/api/action),
[W3C Screen Capture](https://www.w3.org/TR/screen-capture/)

Do not implement a literal live picture-in-picture mirror. It can recursively record itself,
duplicate sensitive pixels, and obscure the actual target. A stylized viewfinder card may be
evaluated later, but it must say what surface is recorded without reproducing page pixels. An ADR-
0073 amendment must decide whether any in-page recording indicator is captured in the exported GIF.

## Motion and attention budget

Use motion to mark a transition, not to prove that time is passing.

- Action confirmation: one short physical response, then disappear.
- Read state: one gentle scan tied to the read operation.
- Narration: one entrance and optional one-shot dot sequence; no loop and no progress meter.
- Ordinary denial: one arrival and shield glow; no pulsing alarm.
- Paused session: no looping page-wide animation. A minimized shield may breathe slowly, with a
  static reduced-motion equivalent.
- Recording: the `REC` badge is persistent; any dot pulse must be slow, subtle, and replaceable by
  a static indicator under reduced motion.

More simultaneous animation is not more legibility. When governance appears, decorative effects
yield. When recording and narration coexist, neither may cover the active control or denial state.

## Accessibility and trust invariants

1. Every semantic surface has adequate contrast, a text label, and an icon; color is supplemental.
2. Decorative dots, glows, and camera marks are hidden from assistive technology.
3. Narration uses a restrained live-region strategy so replacement does not repeatedly interrupt.
4. The paused overlay has an accessible title and description, initial focus, keyboard operation,
   escape behavior defined by the selected safe default, and focus restoration.
5. Hover treatments have equivalent focus-visible treatments.
6. `prefers-reduced-motion` removes travel, scaling, scanning, and pulse without removing state.
7. Page- or policy-influenced strings enter the DOM through `textContent`, never HTML.
8. Decorative page-layer effects remain pointer-transparent. Only real user controls accept input.
9. The model does not see or target its own visual feedback. Any exception for an exported user
   recording is explicit, documented, and tested.
10. A visual claim about pause, capture, or authority is backed by real service state before it is
    rendered.

## Closure sequence

1. DONE: compare narration, isolated denial, and pause treatments.
2. DONE: mark the ADR-0072 compact-caption amendment.
3. DONE: accept ADR-0079 with service-owned per-session thresholds and controls.
4. DONE: mark the ADR-0073 recording-indicator amendment.
5. DONE: implement the surfaces with pure-store, renderer-contract, and state-machine tests.
6. OWED: rehearse each surface in a real browser and run another non-author comprehension review.
