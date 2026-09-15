# ADR-0171: Load-sensitive settle sensor for browser DOM operations

Date: 2026-09-14. Status: Accepted by owner direction.

Builds on ADR-0005, ADR-0007, ADR-0037, ADR-0103, and ADR-0169.

## Context

In modern client-rendered single-page applications (SPAs built with React, Vue, Angular, Next.js,
and similar frameworks), browser navigation commits the initial HTML shell first. Chromium reports
`tab.status === "complete"` when the initial HTML network payload arrives, before client-side
JavaScript executes to mount components, fetch REST or GraphQL resources, or populate text and
interactive controls.

When an automated agent interacts with such a page, a read or inspection call (`browser_read`,
`browser_find`, `browser_inspect`) frequently dispatches within milliseconds of navigation
completion. In the existing implementation:

1. `content.js` executed `read_text`, `find`, and `inspect` immediately and synchronously against
   whatever nodes existed in the DOM at tick zero.
2. If client-side mounting was still in progress, `browser_read` extracted 0 words, `browser_find`
   found 0 matches, and `browser_inspect` recorded 0 controls.
3. While Ghostlight reported truthful summaries and contextual `next_steps` recovery guidance,
   recovering required the LLM agent to spend an extra turn calling `browser_wait` or retrying the
   read, burning 1,500+ tokens and multiple seconds of unnecessary round-trip latency.

Ghostlight already possessed private settlement loops in `content.js`: `prepareStableFill` guarded
form fields against hydration overwrites, and `observe` powered `browser_wait`. However, reading and
inspection lacked a shared, bounded sensor to handle the initial hydration window transparently.

## Decision

1. **Agnostic Settle Sensor (`GhostlightSensor`)**:
   Introduce a shared settlement sensor in `extension/lib/sensor.js` that coordinates DOM settlement
   across all load-sensitive reading and inspection operations.

2. **Hot-Path Zero-Latency Invariant**:
   The sensor evaluates `sample()` immediately on dispatch. If `isSatisfied(candidate)` is true
   (e.g., words exist, matching targets were found, controls are present), the sensor returns
   synchronously on tick zero with 0ms added latency and zero observer allocation.

3. **Adaptive Mutation Settlement**:
   Only when the initial sample is unsatisfied (0 words, 0 matches) does the sensor attach a bounded
   `MutationObserver` on `document.documentElement || document`.
   - When mutations occur, the sensor re-evaluates `sample()`.
   - Once `isSatisfied(candidate)` becomes true, a brief quiet debounce window (default 50ms)
     confirms that atomic multi-element updates have completed, then resolves immediately.
   - If the satisfaction condition is not reached within a bounded ceiling (`maxWaitMs`, default
     1,000ms--1,200ms), the observer disconnects and the sensor cleanly returns the latest sample.

4. **Animation and Ticker Safety**:
   The sensor terminates the moment satisfaction is established. Unlike a naive quiescence check
   that waits for all DOM mutations to cease, background animations, CSS spinners, or continuous
   ticker updates do not stall the sensor or trigger timeouts.

5. **Integrated Load-Sensitive Primitives**:
   Wire `GhostlightSensor.settle` into:
   - `read_text`: satisfied when extracted visible text is non-empty (`words > 0`).
   - `find`: satisfied when matching semantic targets are found (`targets.length > 0`).
   - `inspect`: satisfied when interactive controls are found (`targets.length > 0`).
   - `inspect_tree`: satisfied when document nodes exist (`nodes > 0`).
   - `query_semantic`: satisfied when matching semantic targets are found (`targets.length > 0`).

## Consequences

- SPA navigation races are resolved on turn 1: agents read populated content and find controls on
  their initial attempt without manual `browser_wait` intervention.
- Static and already-rendered pages incur zero overhead: the satisfaction predicate succeeds on
  the initial evaluation, bypassing asynchronous observer setup entirely.
- Genuinely empty pages or absent search queries cleanly time out within ~1,000ms and receive the
  established `next_steps` guidance without hanging.
- The extension eliminates duplicated ad-hoc polling logic across DOM extraction primitives.
- Wire protocols, MCP tool schemas, and orchestrator boundaries remain byte-compatible.
