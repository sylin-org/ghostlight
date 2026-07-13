# North Star & Design Principles

**Date:** 2026-07-01 · **Status:** GOVERNING, set by the project owner. Overrides the emphasis
of [00-synthesis-and-decisions.md](00-synthesis-and-decisions.md) and governs how every report in
this folder is used.

> Prior art in this folder is a **concern surface**, not a feature catalog. We harvest the
> *problems* others hit and the *design questions* they were forced to answer, then answer them
> **our own way** against the north star below. We do **not** import other vendors' paradigms,
> feature sets, or alignments. They optimize for their own concerns, which are not ours.

---

## North Star

Browser MCP gives an AI agent governed access to the user's **own, authenticated, live browser
context**: their session, cookies, SSO, tabs. **The value *is* that it's the user's real
context.** Anything that moves away from that context (cloud browsers, fresh/clean profiles,
separate `--user-data-dir`s, stealth/anti-bot personas) is **off-mission by definition** and is
rejected no matter how common it is in prior art.

---

## Principle 1: The engine is unconstrained; governance is an overlay

The MCP↔CDP engine enables **full capability with no built-in limits**. Access control is a
**separable overlay** with its own lifecycle. The engine never bakes in policy.

> Microsoft-product model: in an enterprise space, behavior is set by policy; in a user space,
> the user chooses. Same binary, different overlay.

## Principle 2: "All open" is a first-class mode, not a degraded one

For personal use, **zero restrictions is a valid, fully-supported configuration** (the default,
even). It is *not* "enterprise minus governance." The unrestricted experience must be excellent on
its own terms.

Three postures, one engine:

| Posture | Who sets limits | Default stance |
|---|---|---|
| **All-open (personal default)** | nobody | a great unrestricted browser-automation MCP |
| **User-chosen** | the user | whatever limits *they* opt into |
| **Policy-enforced (enterprise)** | deployment channel (Intune/GPO) | default-deny |

## Principle 3: Delight is layered, mirroring the architecture

Responsibility and delight are not opposite ends of a slider. Thoughtless control makes them feel
opposed; careful product design makes responsibility part of why the tool is pleasant to use.
Visibility reduces uncertainty. A precise boundary prevents cleanup. A useful denial preserves
momentum. An inspectable local system earns confidence. These qualities belong at every layer, not
only in an enterprise overlay.

Delight is a **stack**, composed the same way the engine + overlay are:

- **L0: Base capability delight** *(engine; every persona, every mode)*. Automating the
  monotonous browser work in your **own authenticated context**: fast, token-lean,
  install-just-works, agent-friendly. The floor everyone stands on, all-open included.
- **L1: Control delight** *(overlay; user-chosen)*. Confidence the agent stays where you want.
  A *personal* user can want a slice of this too ("don't let it touch my banking tab"), so the
  overlay is a user-facing feature, not only an IT control.
- **L2: Governance delight** *(overlay; enterprise)*. The org can say **yes** to a powerful tool
  *because* it's default-deny, audited, identity-bound: delight for the enterprise end user
  (allowed to use it at all, and protected) and for the admin (safe rollout, clean audit).

**Governance-as-delight is real, but it is composite and additive (L0 + L2), never a substitute
for L0.** The enterprise user's total delight still rests on the base engine delight; weak L0
cannot be rescued by L2. So build L0 to be excellent for *everyone*, then layer L1/L2 for those who
want or need them.

Two consequences:
1. **L0 is load-bearing for every persona, including enterprise**: the engine is the foundation
   the enterprise delight rests on, not "the personal-user track."
2. **The overlay itself must be delightful, not merely correct**: governance UX (manifest
   authoring, a pleasant "keep it to these sites" personal mode, agent-adaptable denials) must
   not read as a tax, even to the person it constrains.

> **Corollary (keep the layers un-mixed):** token efficiency is an **L0/engine** property (lean
> element refs, screenshot discipline). The all-open user gets it. It is *not* sourced from
> tool-filtering, which is an **L2** side-benefit that only reaches restricted users. Don't let an
> overlay side-benefit masquerade as base delight, or vice-versa.

## Principle 4: The user's context is sacred

We attach to the real, logged-in browser. We **never relocate** the user's work to a clean/cloud
session to gain a technical property (e.g., an independent CDP oracle). Where a hardening
technique requires leaving the user's context, it is at most an **optional, opt-in deployment
profile**, never the default, never a requirement of the core value.

## Principle 5: Separation of concerns

Engine, policy overlay, identity resolution, audit: **independent lifecycles, no bleed.** A
change to one must not force a change to another.

## Principle 6: Prior art is a concern surface, not a paradigm to copy

Every idea harvested from the reports here must **earn its place against this north star.** We
take *questions* and *hazards*, not *paradigms*:

| We take (concern/hazard) | We reject (paradigm) |
|---|---|
| npx/Windows install pain → build a self-registering single binary | copying anyone's distribution model wholesale |
| service-worker death, requested-vs-committed-URL bug → correctness lessons | their architecture |
| prompt-injection reality, policy-shadowing footguns, PHI-in-URLs → design constraints | their RBAC shape / audit schema as-is |
| Stagehand's `extract` *idea* (schema-typed page data) → maybe an engine capability | Stagehand's cloud/CUA execution model |
| (none) | **cloud/fresh-session execution (violates Principle 4)** |

---

## How this re-weights the synthesis (00)

The eight forks split cleanly once engine and overlay are separated. This is the correct lens:

| Fork | Layer | Applies in all-open mode? |
|---|---|---|
| 1: semantic `extract` | **Engine capability** | ✅ yes: benefits every user |
| 2: snapshot-first / token efficiency | **Engine capability** | ✅ yes: *this* is the real token lever |
| 4: self-registering installer | **Delight (universal)** | ✅ yes |
| 7a: true committed-URL reporting (`frameNavigated`, per-frame) | **Engine correctness** | ✅ yes: the engine should always know the real URL |
| 3: risk annotations / HITL approval | **Overlay (user- or policy-chosen)** | optional: user may opt in even personally |
| 5: audit + standards (OCSF/CEF/hash-chain) | **Overlay (governance)** | off by default; stderr/none in personal |
| 7b: domain enforcement + extension-trust posture | **Overlay (governance)** | inactive when all-open |
| 8: policy resolution semantics | **Overlay (governance)** | inactive when all-open |
| 6: positioning | **Corrected below** | (n/a) |

**Positioning, corrected:** the base delight (L0) is *the user's own context + an unconstrained,
efficient engine*, and every persona stands on it. Governance is an **additive delight layer**
(L1 user-chosen / L2 enterprise), composed on top, not a substitute for L0. 00's "governance as
delight" framing is *re-situated*, not discarded: it is real for the enterprise persona, but as
L0 + L2, never as the source of the base delight. (See Principle 3.)

---

## Where this should graduate to

These principles belong in `SPEC.md §1` (they sharpen the existing "Engine / Policy / Identity
have independent lifecycles" statement) and should seed an ADR. This doc is the discovery-time
capture; the spec is the durable home.
