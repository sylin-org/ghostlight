# ADR-0187: Readable Workbench Outcomes and Reconnection Controls

Date: 2026-10-01. Status: Accepted for the authorized unpublished local UX trial.

Amends ADR-0103's retained outcome projection and ADR-0126's front-door control readiness.
Preserves ADR-0156's grouped receipts, ADR-0159's independent storage truth, ADR-0185's
explicit human controls, and ADR-0186's exact Workbench Show tab behavior.

## Context

A real uncertain script receipt told the agent not to repeat and to observe first, but the
Workbench dropped those facts. A later refusal left the earlier uncertainty in ordinary
history. Browser loss disabled Pause even though the reachable authority could apply a hold
immediately and publish it after reconnect. Human Pause refusals looked like technical failures.

## Decision

1. The language-owned audit projection retains at most two existing authored safe suggestions.
   Completion records optional final `repeat_safe` alongside them. No client result facts,
   page script, browser exception, caller text, field value, or full URL is copied. Final unknown,
   partial, and applied-but-unsuccessful effects replace stale direct-operation retry advice with
   existing observation guidance. Composition guidance stays authored by its own typed progress.
2. `language/history.rs` owns one human outcome projection for direct, parent, and child receipts.
   Unknown and partial effects take visual precedence over human control. No-effect Pause/Stop
   uses calm human wording while the original summary, status, reason, effect, repeat safety,
   and exact workspace/invocation remain available in collapsed technical details.
3. The existing hero leads with that sentence. Historical rows retain their inline detail control
   and show unsafe-effect guidance even while collapsed. Child receipts expose the same recovery
   and technical depth. Missing legacy repeat safety stays unknown; existing effect evidence can
   still require observation. An incomplete parent never inherits a child's repeat safety.
4. A reachable authority invites global runtime control independently of browser health. A lost
   Workbench connection disables the control. Disconnection still owns the shared readiness word,
   while separate authored control detail describes the current hold and reconnect behavior.
   Browser delivery confirmation requires both a connected compatible browser and publication
   success; publication to an empty collection cannot imply browser delivery.
5. Existing Show tab gains local guidance: look with Show tab; Pause before takeover. Showing an
   exact owned tab changes no permission or runtime state. Resume and Start session permit only
   new requests. No prior action or whole composition is replayed.

## Boundaries

The five destinations, guardian About card, pixel art, palette, meaningful motion, policy,
physical dispatch, Stop semantics, and connector/adapter contracts remain intact. This adds
no hold, inferred task intent, acknowledgment queue, notification engine, or destination.
Exact-record targeting from global history search is deferred to a separate bounded change.

## Review amendment: confirmations state only authority effects (2026-10-01)

Independent review found that Decision 4's separate browser sample and publication success
cannot establish browser delivery. The last adapter can disconnect between those steps, and
broadcasting to the empty writer collection still succeeds. Publication has no receipt.

Human confirmations now state only the applied authority state, its global scope, and its
continuity across browser reconnection. They do not use `browser_notified`. The existing
best-effort metadata field remains compatible, with its limits explicit: it is a sampled browser
plus successful publication, never evidence of receipt or application. No acknowledgement
mechanism is added. A deterministic port fixture disconnects the final adapter inside publication
after the topology sample and proves identical truthful wording with or without that disconnect.

## Evidence

The [source record](../testing/in-service-ux-2026-10-01.md) records gates and review.
The [selected acceptance](../testing/in-service-ux-acceptance-2026-10-01.md) records the granted
native lane, actual screenshots, cleanup, and keyboard-activation limit.

## Native observation amendment: independent restriction evidence (2026-10-01)

A configured background-attention refusal can follow a permitted capability decision. The final
human refusal tone therefore also uses its closed retained refusal facts, including historical
request restrictions. The separate capability decision is not rewritten. Unknown/partial effect
presentation still takes precedence over refusal and calm control styling.
