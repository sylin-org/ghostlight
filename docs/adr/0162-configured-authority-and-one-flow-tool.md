# ADR-0162: Configured authority and one flow tool

- Status: Accepted; implemented, verified locally, and deployed
- Date: 2026-09-07
- Amends: ADR-0107, ADR-0133, ADR-0157
- Preserves: ADR-0103, ADR-0156, ADR-0158

## Context

An all-open Reddit session repeatedly supplied `restrict_hosts` on ordinary tool calls. That
model-authored list excluded a Google embed and caused script refusals without any configured
policy. The history surface attributed the refusal to the person's rules. Separate reproduction
showed flow dry-run reporting success for invalid child inputs and failing valid result references.

The owner authorized removing both request restriction fields and flow dry-run, consolidating
sequence into flow, and adjusting policy handling. Governance sets reliable boundaries for people
and organizations. An ordinary model call describes browser work; it does not author policy.

## Decision

1. Remove `restrict_hosts` and `restrict_capabilities` from every tool schema, typed operation,
   authority snapshot, and admission path. Configured managed and local policies still intersect.
   Capability classification, protected destinations, frame handling, and human controls remain.
   All-open work has no model-authored host or capability overlay.
2. Remove `browser_flow.dry_run`. A valid flow performs its requested work. Human policy previews
   and installation CLI dry-run serve different purposes and remain available.
3. Retire `browser_sequence` and its action DSL. `browser_flow` is the one batch executor for
   one to twenty ordinary tool calls. Step IDs are optional; omitted IDs become `step_1`, `step_2`,
   and so on. Explicit IDs stay available for result references. IDs must be unique, including
   generated IDs. Missing arguments default to `{}`. Nested composites remain forbidden.
4. A flow's optional `tab` supplies the default for tab-scoped children. Explicit child tabs win;
   creating a new tab and tab-independent operations keep their ordinary semantics. The flow
   deadline bounds the whole operation. `on_error` defaults to `stop`; explicit `continue`, actual
   effects, references, child receipts, and human-control boundaries retain their existing meaning.
5. Obsolete fields or the retired tool fail before browser work with catalog-refresh guidance.
   Their presence is never silently ignored. Structural validation rejects obsolete restrictions
   in flow children before an earlier child can run. There is no automatic retry with wider scope.
6. Historical sequence identities and request restriction evidence remain readable. Legacy
   request denials are identified as request denials, without blaming the person's policy or
   linking to current policy as their source. New decisions never record a caller restriction.
7. The orchestrator owns these changes. IPC contracts, connector binaries, and extension mechanisms
   need no change. A fresh MCP connection retrieves the new catalog; clients caching old schemas
   must refresh or reconnect. This is an intentional model-facing compatibility break.

## Scope and validation

`submit_target` remains supported. Removing it was not part of this authorization. Client signature
verification and other undecided epic choices still need their planned ideation session.

Tests use real isolated configured policy files for capability and host denial cases. They cover
every catalog variant directly and inside a flow, draft and credential boundaries, document access,
recording sources and destinations, obsolete-input rejection without effects, optional IDs and tab
defaults, and old-history attribution. Process and Chromium journeys retain their existing effects,
control, privacy, and lifecycle assertions. Installed all-open acceptance uses disposable pages and
does not change the person's policy to simulate a restriction.
