# ADR-0060: Composable policy tiers and the per-session tighten-only overlay

- Status: Accepted
- Date: 2026-07-12
- Supersedes/relates: ADR-0022 (capability grants), ADR-0025 (config layering, manifest hot-reload), ADR-0055 (managed:// central policy)

## Context

Governance evaluates one service-wide policy: `run_tool_call` reads `store.current()` and the
service's single `Governance` facade. A session's identity (guid) drives tab-ownership and audit,
never policy. Two needs pushed against that:

1. A client should be able to run under a *narrower* policy than the service's for its own session
   -- e.g. `ghostlight demo` wants the guardrail finale (a blocked navigation) to fire regardless
   of the service's operational mode, with no operator pre-configuration.
2. More broadly, organizations will want to *compose* policy from reusable layers, the way real
   security systems do.

Rather than invent an algebra, we ran a prior-art pass on how mature security software layers
managed rules. The findings were decisive:

- **AWS IAM** is our situation almost exactly: policy *types* in a strict hierarchy (SCP -> permission
  boundary -> identity -> **session policy**), effective permission = **intersection** of all types,
  **explicit deny overrides everything**, and SCPs/boundaries *grant nothing* -- they only cap. Its
  **session policies "can only reduce permissions."** Our per-session overlay is exactly that.
- **XACML** names its combining algorithms explicitly (deny-overrides, permit-overrides,
  first-applicable, only-one-applicable) -- the lesson is to name the algorithm, never leave it
  implicit.
- **Firewalls / AWS NACLs / Windows GPO link order** use *ordered, first-match, allow+deny* rules --
  but always *within a single administrative tier*. **AWS Security Groups / K8s RBAC** are
  *deny-by-default, allow-only, union* -- order-independent. Across trust boundaries it is always
  intersection + deny-overrides; ordered override lives only inside one authority.
- **Windows GPO** (Enforced beats Block Inheritance) and Chrome enterprise policy
  (mandatory vs recommended) are the layered-managed-precedence-with-a-ceiling model -- which this
  codebase already has for config keys (`config/layers.rs`: `OrgMandatory > User > OrgRecommended >
  Preset > Builtin`, `org_mandatory` = lock).

## Decision

### The composition model (normative)

A **policy tier** is a policy in the existing schema-3 manifest format (grants + config). Tiers, by
authority, highest to lowest:

```
managed-mandatory  ->  org  ->  user  ->  session
   (exists via config layering + managed:// + manifest)      (new, this ADR)
```

Composition rules -- three of which the engine already runs:

- **Grants compose by AND / intersection.** A call is allowed only if *every* active tier grants it.
  This is forced by the existing grant design: grants are deny-by-default, allow-only
  (Security-Group / RBAC style), so intersection is the only sound cross-tier semantics and
  **tighten-only falls out for free** -- adding a tier can only ever remove capability. (= AWS
  `SCP INTERSECT boundary INTERSECT identity INTERSECT session`.)
- **Sacred (deny ceilings) compose by union.** Any tier's `content.security.sacred_domains` entry
  denies. The config-list layering already unions lists; the session tier adds one more.
- **Mode composes by strictest.** The existing mode precedence (`apply_mode`) with the session tier
  as strictest-wins.
- **Authority is a ceiling.** A lower tier can only cap a higher one, never widen it -- already true
  for config via the `org_mandatory` lock, and true for grants by the intersection algebra above.

The named cross-tier algorithm is therefore **intersection + deny-overrides** (the AWS/XACML shape),
stated explicitly rather than left implicit.

### What this ADR implements now: the `SessionClient` tier

The bottom tier -- a client-declared, tighten-only overlay -- lands in two steps: the decision
core (`governance::overlay::SessionOverlay`, unit-tested) first, then the session-lifecycle wiring
that consults it per call. The design:

- A client declares an overlay (a schema-3 manifest) at session `initialize`. It is parsed by the
  SAME `parse_manifest` path and validated identically to a service manifest.
- The overlay decides through the SAME audit-free `Governance::decide` the service policy uses, just
  against the overlay's own grants. Only `.decide()` is ever called on it, so its audit sink (a
  `NullSink`) is never touched: the one audit record per call stays the service's.
- Per call: after the service decision, the overlay decision is intersected in (deny-overrides). A
  service `Deny` stands; a service `Allow` + overlay `Deny` becomes `Deny`; both `Allow` -> `Allow`.
  The overlay's sacred domains union into the pipeline's always-on sacred check.

**Escalation-safety is by construction, not by validation.** Because composition is pure
intersection, an overlay that "grants everything" intersected with the service policy *is* the
service policy -- no change. A client can never widen its own reach, so the overlay needs no
escalation-validation, only well-formedness parsing. The service policy is always the ceiling,
automatically.

### What this ADR designs but does NOT implement: intra-tier ordered composition

Ordered, first-match, allow+deny override *within* one administrative tier (the firewall / NACL /
GPO-link-order model) is a **sanctioned future axis**, not built here. It is unnecessary today:
allow-only grants compose by intersection, full stop. It becomes relevant only when an org wants
*deny-rules ordered within its own tier*, at which point we adopt NACL-style **numbered
first-match** with named semantics -- the one part that genuinely needs a real org scenario to pin
down, so it stays designed-not-coded. Critically: ordered override may only ever live *inside* a
single authority tier; across the client/org boundary it is always tighten-only.

## Consequences

- One policy format everywhere: the demo's overlay, an org's mandatory tier, and a user's manifest
  are all the same schema-3 file. The engine already in the codebase (`decide` + config layering)
  *is* the composition engine; the session tier is a small combine step over it.
- `ghostlight demo` declares an `enforce` overlay granting `sylin.org` at `initialize`; its finale
  (a navigation to `example.com`) is refused in any service mode with zero operator setup.
  `examples/demo-policy.json` is that overlay payload, not a service manifest.
- Any client can self-restrict, not just the demo -- a general, proven capability (AWS session
  policies) rather than a demo-specific hack.
- The model is complete on paper; org-composed ordered layers slot in as an additive tier/axis
  without reworking the algebra.
