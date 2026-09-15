# ADR-0173: Visual quiescence and layout-stability settlement sensor

Date: 2026-09-14. Status: Accepted by owner direction.

Builds on ADR-0005, ADR-0007, ADR-0037, ADR-0103, ADR-0169, and ADR-0171.

## Context

While ADR-0171 introduced `GhostlightSensor.settle` for DOM-mutation settlement during hydration,
modern web interfaces frequently utilize CSS transitions, CSS keyframe animations, dynamic layout
shifts, and transform animations (e.g. accordion expansions, dialog slides, opacity fades, canvas
redraws). These visual transitions do not necessarily mutate the DOM tree structure.

When automated agents or visual regression testing workflows take screenshots (`browser_screenshot`)
or verify visual readiness (`browser_wait`):

1. The page may report ready state (`load_ready`) and DOM presence (`target_present`), but elements
   are mid-animation or moving across the viewport.
2. Screenshots taken mid-transition produce inconsistent visual captures or coordinate alignment
   errors when clicking by coordinates.
3. Automated agents were forced to insert arbitrary `browser_wait duration` calls (e.g., 500ms--1000ms),
   wasting wall-clock time and adding fragility.

## Decision

1. **Visual Settlement Sensor (`GhostlightSensor.settleVisual`)**:
   Extend `GhostlightSensor` with a visual settlement sensor that observes layout stability and
   running CSS animations across animation frames.

2. **Animation Introspection**:
   Inspect `document.getAnimations()` for active animations and transitions in `running` or `pending`
   play states.
   - Finite animations (transitions, entry/exit keyframes) must reach completion before visual
     settlement is declared.
   - Infinite animations (`iterations === Infinity`, such as continuous loading spinners or pulsing
     indicators) are explicitly excluded so that perpetual visual effects do not deadlock the
     sensor.

3. **Geometry and Layout Stability**:
   Track the geometry (bounding client rectangle of the target element, or viewport and document
   scroll metrics for full-page observation).
   - The sensor samples geometry across consecutive animation frames (`requestAnimationFrame`).
   - Visual settle requires that geometry remains stable and no finite animations are running
     continuously throughout a configurable quiet window (default 50ms--100ms).

4. **First-Class Wait Condition and Optional Composite Flag**:
   - `browser_wait` natively supports `condition: "visual_settle"` (and its synonym `"layout_stable"`).
     When a target handle is provided, visual settle tracks that element's geometry and local
     animations; otherwise it tracks document-wide visual stability.
   - `browser_wait` also supports an optional `visual_settle: true` boolean property on other wait
     conditions (e.g., `load_ready`, `target_present`), ensuring that visual quiescence is verified
     before the wait is satisfied.
