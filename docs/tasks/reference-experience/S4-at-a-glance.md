# S4: At a glance

## Objective

Make At a glance the native workbench front door: one calm answer to whether Ghostlight is ready,
connected, working, paused, recovering, or in need of attention, with prominent contextual human
controls.

## Prompt outline

1. Establish one orchestrator-owned projection for the At a glance state. The view renders it and
   does not infer product state from process fragments or presentation-local booleans.
2. Lead with the aggregate answer. Show only the browser, harness, authority, current operation,
   recent recovery, or next action needed to explain that answer.
3. Show `PAUSE` or `RESUME` and `STOP` prominently when they apply. Preserve unambiguous per-session
   behavior when more than one session is active; keep a distinct global safety control if the
   accepted contract requires one.
4. Keep logs, raw audit, paths, protocol revisions, and diagnostic detail behind deliberate depth.
   At a glance links to them when a fact needs explanation.
5. Make healthy idle state useful without filling it with empty cards. Make recovery visible
   without turning routine success into notifications or ceremony.
6. Remove or demote presentation made redundant by the new front door. Do not leave two competing
   status summaries or control locations.
7. Prove keyboard-only use, accessible names, large text, high contrast, reduced motion, and
   content-independent state rendering.

## Completion evidence

- Every explicit workbench open lands on At a glance.
- A user can identify overall readiness and the active controller in a few seconds.
- The surface exposes no invented task intent and no page content.
- Controls act through the S3 owner and update from sequenced domain state.
- Empty, active, paused, recovering, degraded, and plural-session states have focused tests.
- Superseded front-door chrome and stale guards are removed or named in the ledger.

## Stop conditions

- JavaScript must reconstruct domain state from unrelated fields.
- Technical health becomes the primary hierarchy in a healthy state.
- The page needs its own pause or stop semantics.
- Accessibility depends on color, animation, pointer use, tray presence, or notifications.
