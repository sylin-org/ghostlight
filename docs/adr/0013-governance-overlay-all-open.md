# 0013. Governance as a separable overlay; no-manifest = all-open

- Status: Accepted
- Date: 2026-07

## Context

The same binary must serve two very different postures without code changes: an
unrestricted personal browser-automation tool and a default-deny enterprise
deployment. A single tool that hardcodes either stance fails the other: baking
policy into the tool layer would cripple the personal case, while shipping a
separate "unrestricted build" would make the open mode a stripped, second-class
artifact.

The engine's only job is MCP-to-CDP translation over the full browser-automation
capability surface (navigation, input, reads, extraction, scripting), with no
built-in limits and no opinions about policy (SPEC sec 1.1). Governance
(identity-bound grants, per-domain read/write tool classification, and audit)
is a distinct concern with its own lifecycle (SPEC sec 4-5; NORTH-STAR Principles
1, 2, 5).

## Decision

The engine is fully capable and never bakes in policy. Governance is a separable
overlay that attaches at fixed seams, co-hosted in the binary but never in the
extension. The single dispatch chokepoint (`src/dispatch.rs`) is the seam: every
tool call flows through `policy_check` and `audit`. In v1.0 both are no-ops:
`policy_check` always returns `PolicyDecision::Allow` and `audit` does nothing,
so tool code carries zero policy logic (`src/lib.rs` documents this layering).

With no manifest the engine is all-open, a first-class supported mode, not
"enterprise minus governance." The manifest/policy engine (SPEC sec 4-5 grants,
domain matching, five enforcement points) is scaffolded but parked: `src/policy/`
holds only the typed config-key registry with a safe-by-default "Minimal" preset,
awaiting the v1.5 overlay that will resolve values from a manifest and thread
them through dispatch without touching tool code.

## Consequences

- Positive: one binary, three postures (all-open, user-chosen, policy-enforced),
  zero code changes between them. Separation of concerns holds: the overlay can
  land later at the seams without disturbing the engine.
- Positive: all-open is excellent on its own terms (SPEC sec 1.2); token
  efficiency and correctness live in the engine, not sourced from tool-filtering.
- Negative: the invariant depends on discipline. Tool code must never make
  access decisions, and the dispatch seam must remain the sole chokepoint.
- Negative: the parked engine is unfinished; `PolicyDecision` has no `Deny`
  variant yet, so denial formatting and enforcement arrive only with v1.5.
- Follow-up: v1.5 replaces the no-op hooks with manifest-driven enforcement;
  SPEC sec 5.3 STEP 0 short-circuits to Allow when no manifest is present, so
  all-open behavior is preserved by construction.
