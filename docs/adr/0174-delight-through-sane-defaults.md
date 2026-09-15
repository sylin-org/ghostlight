# ADR-0174: Delight Through Sane Defaults (Autonomous Settle and Action Stabilization)

Date: 2026-09-14. Status: Accepted by owner direction.

Builds on ADR-0005, ADR-0007, ADR-0037, ADR-0103, ADR-0171, and ADR-0173.

## Context

Under ADR-0173, Ghostlight introduced `GhostlightSensor.settleVisual` and enabled composite visual
settling on `browser_wait` through an explicit `visual_settle: true` flag.

In real-world LLM and autonomous agent interactions, requiring explicit parameterization for
fundamental stability creates unnecessary cognitive load and recurring failure modes:
1. When an agent calls `browser_screenshot`, it expects a stable, fully rendered visual capture.
   Capturing immediately while CSS animations, layout shifts, or web-font renders are in-flight
   yields distorted views and causes subsequent coordinate-based interactions (`browser_click` with
   view coordinates) to miss their targets.
2. When an agent calls `browser_wait` with conditions like `load_ready`, `target_present`,
   `selector_present`, or `duration`, the underlying page condition may be structurally satisfied
   while elements are still actively transitioning into their final resting places.
3. Requiring models to explicitly pass `visual_settle: true` on every screenshot and wait operation
   imposes an unnatural tax. The principle of "delight through sane defaults" dictates that
   Ghostlight should deliver correct, stable outcomes out-of-the-box without requiring defensive
   configuration, while still providing an unconstrained escape hatch (`visual_settle: false`) for
   callers who explicitly need raw immediacy.

## Decision

1. **Default Visual Settlement on Screenshots (`browser_screenshot`)**:
   - `browser_screenshot` defaults `visual_settle` to `true`.
   - Before capturing pixels from the viewport, full page, target element, or region, Ghostlight
     executes a bounded visual quiescence check using `GhostlightSensor.settleVisual`.
   - If the page or target is already stationary with no active finite animations, the check
     completes on tick 0 with 0ms added delay.
   - If animations or layout adjustments are in progress, Ghostlight waits up to a bounded budget
     (min of remaining timeout or 1000ms) for visual quiescence before capturing the frame.
   - If settling times out or encounters continuous motion (e.g. perpetual carousel shifts),
     the capture still proceeds gracefully rather than failing hard.
   - Callers can opt out explicitly by passing `visual_settle: false`, bypassing the settlement
     sensor entirely for zero-overhead raw capture.

2. **Default Visual Settlement on Composite Wait Conditions (`browser_wait`)**:
   - For composite wait conditions (`load_ready`, `target_present`, `selector_present`, `duration`),
     `visual_settle` defaults to `true` unless explicitly set to `false`.
   - When the primary condition is satisfied (e.g., selector is found or document reaches interactive/
     complete), Ghostlight automatically stabilizes visual state within the remaining invocation
     budget before returning `Status::Succeeded`.
   - Standalone conditions (`visual_settle` and `layout_stable`) remain dedicated single-phase
     settle operations.
   - Passing `visual_settle: false` immediately concludes the wait upon primary condition match.

3. **Outcome and Audit Observability**:
   - Both `browser_screenshot` and `browser_wait` record `visual_settle` in their terminal outcome
     facts, ensuring complete audit transparency regarding whether visual stabilization was applied
     or explicitly bypassed.
