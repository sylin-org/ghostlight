# R2: Precision input

## Goal

Restore the missing low-level input behaviors through current target and `view_` semantics.

## Required work

- Extend `browser_click` with unique modifiers and click count one through three.
- Extend `browser_press_key` with a typo-closed ordered stroke sequence of at most twenty and a
  repeat count of one through one hundred. Preserve the shortest one-key call.
- Add the focused branch to `browser_type_text`. Describe the focused editable control before
  dispatch and apply credential handoff.
- Add the zero-through-10,000-millisecond duration branch to `browser_wait`, with cancellation and
  deadline checks.
- Add current-view point wheel scrolling with a direction and one through ten ticks.
- Add revision-2 physical mechanisms only where a revision-1 adapter cannot perform the exact
  request. Reuse the existing point transform and modifier vocabulary.

## Evidence

- Decoder/schema parity at every bound and mutually exclusive branch.
- Workspace tests for stale, foreign, mismatched, and out-of-bounds views.
- Executor tests for capability class, credential handoff, cancellation, landing truth, and no
  dispatch on invalid input.
- Extension tests for CDP event order, modifier masks, triple click, repeated keys, focus
  description, and wheel coordinates.
- Process journey covering at least one modified click, key sequence, duration wait, and point
  scroll.

## STOP conditions

- A coordinate path bypasses `view_` ownership or geometry checks.
- Focused typing cannot classify credentials before text dispatch.
- Repeat execution cannot observe cancellation between repetitions.

## Commit

`feat(browser): restore precision input`

