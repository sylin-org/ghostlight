# ADR-0083: Unified action signature medallions

Status: Accepted

Date: 2026-07-15

## Context

Ghostlight already has a deliberate page-presentation vocabulary. Spatial work is easy to explain
at its target: clicks ripple, fields splash, reads scan, and zooms frame a region. Other operations
have no honest page coordinate. JavaScript execution, typing, waiting, and screenshot confirmation
need clear user-visible disclosure, but one-off icons would produce inconsistent placement,
lifecycle, capture behavior, and privacy rules.

The audience includes people who use MCP clients for ordinary documentation-heavy work. A code
glyph or raw tool name is not a sufficient explanation for them. The presentation must communicate
the visible user concept, not the implementation mechanism. It also must not expose a password or
other sensitive value merely to prove that typing occurred.

ADR-0081 provides the truthful document-aware delivery boundary. This ADR defines the reusable
visual domain that rides that boundary.

## Decision

### D1. Every tool action belongs to one presentation class

The tool-signature coverage ledger classifies each action as one of:

- content surface: the user needs text or a decision, such as narration, navigation, or governance;
- spatial effect: the action has a meaningful page target, such as a click or field write;
- action signature: non-spatial activity is worth disclosing but needs no arbitrary text;
- native or self-evident: the browser already makes the result clear;
- quiet: the operation does not touch the page and extra signage would add noise.

One action may have a composed treatment when the parts say different necessary things. Screenshot
confirmation keeps its spatial capture frame and gains the shared camera signature. A form fill
keeps one field splash per touched control and a click treatment if it submits. Composite tools do
not add an umbrella signature on top of their visible substeps.

### D2. One in-page signature medallion owns non-spatial activity

The renderer owns one compact action-signature medallion per tab. Every signature shares the same
near-black translucent shell, sky border, omnidirectional glow, size, entrance, completion, and
exit. The inner treatment carries the action meaning.

The initial vocabulary is:

| Kind | Meaning | Inner treatment |
| --- | --- | --- |
| JavaScript | Ghostlight is performing a custom operation in this page | rotating workwheel with three light particles |
| Typing | Ghostlight is using the keyboard | keyboard glyph with a gentle blue illumination rhythm |
| Wait | Ghostlight is waiting for time or a page condition | three fading lights |
| Screenshot | Ghostlight just captured the page | camera glyph with a shutter glint |

Product copy may call this an action badge. Code calls it a signature medallion so it cannot be
confused with Chrome's toolbar badge.

### D3. Placement is signal-aware and stable

A pure presentation-placement domain scores four corner anchors. It considers the current viewport,
recent pointer position, the focused or recently touched control, scroll direction, and active
narration edge. Narration uses the same domain and considers an active signature edge.

The resolver chooses once when a signature starts. The medallion does not chase the pointer. A new
signature may choose again. Governance remains visually dominant; an attention overlay suppresses
ordinary decoration. Narration ranks above a signature, signatures rank above ordinary spatial
decoration, and the controlled-tab border remains independent scope disclosure.

### D4. Active and confirmation lifecycles are distinct

An active signature begins before the underlying operation and finishes in a `finally` path:

- JavaScript covers the actual `Runtime.evaluate` lifetime;
- typing covers the actual keyboard-dispatch loop;
- wait covers the actual sleep or condition-wait lifetime.

A confirmation signature appears only after the operation happened. Screenshot remains hidden
during capture and shows its camera only after the capture barrier is released.

The extension publishes bounded document-local start, finish, and confirm events through the
Presentation Broker. They are not durable broker state: activity must not replay into a new
document or after a worker restart. The renderer owns a bounded stale-activity fallback so an
interrupted worker cannot leave a medallion forever. A fast operation transitions immediately into
its completion flourish rather than pretending it is still active. The tab's page FIFO means one
ordinary page command owns this lane at a time; replacement still clears old timers defensively.

### D5. Signature payloads are fixed and content-free

Signature messages carry only a fixed kind and phase. They never carry JavaScript source, typed
text, page content, URLs, form values, filenames, results, or errors.

`computer.type` no longer renders the literal typed value. The field shimmer continues to show
where typing occurs, while the keyboard medallion says what is happening. Named `computer.key`
chords retain their existing lozenge because the chord itself is the user-relevant action; a
separate printable-key privacy pass remains an inventory item.

### D6. Existing presentation invariants apply

The medallion is optional decoration under the effects switch, pointer-transparent, prefixed with
`ghostlight-`, excluded from page reads, hidden during model capture, and reduced-motion aware. It
uses only compositor-friendly opacity and transform motion while page JavaScript may occupy the
main thread. Start delivery is acknowledged before the underlying operation begins, with a short
bounded wait so presentation failure never prevents browser work.

Recording stays in Chrome's truthful toolbar badge and popup state. It is persistent capture state,
not a transient in-page action signature.

### D7. Coverage is explicit and reviewed before growth

`docs/design/tool-visual-signatures.md` is the coverage ledger for the complete tool surface. New
tools and action variants must declare their presentation class there. A new visual treatment joins
the normative vocabulary in `docs/design/visual-language.md` in the same change as its code.

The first implementation migrates JavaScript, typing, screenshot, `computer.wait`, and `wait_for`.
Image-drop, scroll-target, and backstage diagnostic cues remain proposals until their meaning and
noise cost are reviewed.

## Consequences

- Regular users receive recognizable action feedback without needing tool or programming terms.
- A shared shell and placement resolver prevent visual drift across isolated effects.
- Typing feedback stops disclosing the value being entered.
- Wait feedback becomes truthful for both fixed waits and condition waits.
- Starting a visible active action adds one short acknowledged presentation hop. Failure or absence
  of the renderer degrades quietly and never blocks the tool beyond that bounded attempt.
- The renderer gains one tracked transient layer and a small fixed vocabulary, but no policy.

## Rejected alternatives

- A literal `{}` JavaScript badge. Rejected because it communicates poorly to non-developers.
- A badge for every tool. Rejected because spatial, textual, native, and quiet actions already have
  better explanations or need none.
- One fixed corner. Rejected because it can cover the control the person is watching.
- Pointer-following placement. Rejected because moving chrome competes with the work and feels
  evasive.
- Durable broker state for active signatures. Rejected because replay would falsely claim that an
  interrupted operation is still running.
- Raw typed text. Rejected because a visual effect must not defeat page-level masking.

## Related decisions

- ADR-0012: UI and input fidelity.
- ADR-0053: the extension remains a thin mechanism layer.
- ADR-0072: agent narration and its placement precedent.
- ADR-0079: governance presentation and attention hierarchy.
- ADR-0080: resource-scoped scheduling and the per-tab page FIFO.
- ADR-0081: document-aware Presentation Broker.
