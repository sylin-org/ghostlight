# ADR-0139: Shadow-complete interaction

Date: 2026-08-25. Status: Accepted (implemented in this revision).
Builds on ADR-0078 (D2's "top document and same-origin shadow trees" scope), ADR-0133
(semantic selectors below narrow tools), and ADR-0138 (frame-transparent semantic layer).

## Context

Open shadow trees have been first-class since the semantic layer landed: the content script
pierce-collects every reachable `shadowRoot`, so inspection, find, tree reads, and every
locator-bearing action already work on shadow controls, in any frame. Two seams stayed
light-DOM-only, and both surface exactly where a model is mid-task:

1. Focused-control discovery read `document.activeElement`. Inside a component that element
   is the shadow HOST, so `type_text` with `focused:true` and focused-clear described the
   wrapper and refused the inner field as "not text-editable".
2. Coordinate paths resolved subjects with `closest()`, which stops at the shadow boundary.
   A point-click on a component's inner node reported the raw child element instead of the
   named control that would receive the action.

Closed shadow roots are a deliberate author boundary. The ecosystem's automation norm is
open-only, and piercing them would require patching page-visible prototypes or a second
observation mechanism beside the content script -- exactly the divergence this project
records as a lesson whenever it grows one.

## Decision

1. **Focus is resolved deeply.** Focused-control primitives walk the
   `shadowRoot.activeElement` chain to the real focused element before describing, clearing,
   or typing.
2. **Subjects cross boundaries.** Coordinate-path subject resolution walks ancestor chains
   through `getRootNode().host`, so the subject of a point action is the nearest actionable
   element in any enclosing tree, light or shadow. The same walk serves the `[inert]` check.
3. **Closed stays closed.** Ghostlight neither pierces nor patches closed roots. Observation
   simply does not list their contents, and refusals name the boundary. Revisit only with a
   demonstrated user need and a single-mechanism design.

## Consequences

- Web-component UIs (design systems, payment elements, embedded widgets) behave like plain
  markup to a model: inspect once, act by meaning, focus and type without surprises.
- No new wire fields, tools, or Rust changes; the work is content-script mechanism plus
  tests, matching ADR-0138's opacity rule.
- A closed-root component remains a visible, honest gap rather than a silent wrong answer.

## Acceptance

- Unit tests pin deep focus resolution through nested roots and boundary-crossing subject
  resolution, including the light-DOM passthrough.
- The public stage `/ghostlight/demo/shadow/` hosts a web-component form completed end to
  end by the ordinary tool path, plus a closed-root widget whose contents are visibly
  absent from inspection.
- All existing gates stay green; live proof runs on the daily-Chrome authority after an
  explicit unpacked-extension reload.
