# The RAWX Capability Model

A vocabulary for governing what an AI agent is allowed to do, classified by what a
governor can prove about an operation rather than by the effect it might have.

- Version: 0.1 (draft)
- Date: 2026-07
- Status: published for discussion and adoption
- Reference implementation: Ghostlight (https://github.com/sylin-org/ghostlight)

## Abstract

An autonomous agent acting in a live system issues operations whose downstream effect is
usually unknowable at the moment of the call. A click might do nothing, or it might wire
money. Access-control models that try to grade an operation by its predicted effect
therefore fail closed or fail open, because the prediction is the thing you do not have.

RAWX classifies each operation instead by its EPISTEMIC STATUS: what the governor can
actually prove about it. There are four capabilities -- read, action, write, execute --
and a grant is an allowance of some of them on some resources. The model is deliberately
small, mechanism-independent, and domain-neutral: the same four words govern a browser
today and a filesystem or desktop tomorrow, over CDP today and any successor mechanism
later. The vocabulary is meant to outlive every mechanism that carries it.

The name is a nod to Unix. File permissions gave us `r w x`. An agent acting in the world
needs one more bit -- Action -- because it can cause effects it never declared. RAWX is
`rwx` for agents.

## Conventions

The key words MUST, MUST NOT, SHOULD, and MAY are used as in RFC 2119. Wire and file
tokens are lowercase ASCII: `read`, `action`, `write`, `execute`.

## Motivation

Three properties make agent governance different from classic RBAC:

1. Effect is unknowable. The governor sees the operation, not its consequence. A model
   keyed on consequence cannot be evaluated at decision time.
2. The surface is large and shifting. Tool lists change; a policy written against tool
   names rots. A policy written against capabilities does not.
3. One operation can cause another. A single UI action can trigger a mutation the agent
   never asked for. The model must represent "can cause" distinctly from "declares."

RAWX answers all three by grading operations on provable status, binding grants to
capabilities rather than tool names, and giving "can cause" its own primitive (Action).

## The four capabilities (normative)

An implementation MUST assign every governed operation a required capability set drawn
from exactly these four primitives.

- `read` -- The operation is provably retrieval or observation only. It returns state and
  changes none. Examples: taking a screenshot, reading a page's accessibility tree,
  listing open tabs.

- `action` -- The operation dispatches input whose effect is determined by the target
  system and is not knowable to the governor. Examples: a mouse click, a keypress, a
  drag. An `action` MAY cause a mutation (a click can submit a form); that is precisely
  why it is its own capability and not a subtype of `read`.

- `write` -- The operation is a declared mutation: the agent states the change and the
  change is what is performed. Example: setting a form field to a given value.

- `execute` -- The operation runs unbounded, arbitrary code in the target context.
  Example: evaluating a script in a page. `execute` is independent and is never implied by
  any other capability.

Definitions are about knowability, not severity. `write` is often less dangerous than
`action` even though it is a declared mutation, because a declared mutation is bounded
and an action's effect is not.

## Capabilities are independent primitives, not tiers (normative)

The four capabilities MUST be treated as an unordered set of independent primitives. No
capability implies another. In particular:

- `action` does NOT imply `write`, and `write` does NOT imply `action`. They describe
  different epistemic situations (unknowable effect versus declared change), not two
  points on one scale.
- `execute` is NOT a superset of the others and MUST NOT be granted implicitly by
  granting any combination of `read`, `action`, or `write`.

A grant that allows `read` and `write` allows exactly those two. It does not allow
`action` or `execute`.

## Grants, resources, and polarity (normative)

A grant binds a set of capabilities to a set of resources. A resource is whatever the
domain governs: a host for a browser, a path for a filesystem, an endpoint for an API.
The reference implementation governs hosts; this section uses "host" as the running
example, but the rules are stated over "resource".

A grant's resource scope is expressed with polarity:

- An `allow` list of resource patterns, and an optional `deny` list of carve-outs.
- Evaluating a grant against a concrete resource yields one of three outcomes:
  - Allowed -- an `allow` pattern matches and no `deny` pattern overrides it.
  - Denied -- a `deny` pattern matches and wins.
  - Unmatched -- neither list resolves the resource (including the case of an empty
    `allow` list).
- The grant-level default is DENY. An Unmatched grant MUST NOT resolve a call; it simply
  does not apply. Coverage is opt-in per grant.

Polarity lets "everywhere except here" be written directly: `allow` the broad pattern,
`deny` the exception.

## The containment rule (normative)

Enforcement compares the operation's required capability set against individual applicable
grants for the resource. The call is permitted only if the complete required set is a subset of
one resolving grant's allowed set. Capabilities from separate resource grants MUST NOT be pooled
to manufacture an admission:

- An empty required set is a subset of every set, including the empty set. Operations
  that require nothing are always permitted with respect to capabilities.
- Duplicates do not affect the result.
- Because no capability implies another, a required `execute` is satisfied only by an
  allowed `execute`, and never by any combination of the other three.

## Operation directory (normative)

Each governed operation MUST map to a fixed required capability set, published by the
implementation as a directory. The mapping is intrinsic to the operation, not to the
caller. A conforming implementation SHOULD expose this directory to the agent so the
agent can reason about what a call will require before making it. (In the reference
implementation this is the `explain` tool.)

The directory is where a domain expresses judgement once, in the open, rather than
scattering it across policy files. For example, in the browser reference implementation:
navigation is `read`; a screenshot is `read`; a click is `action`; setting a form field
is `write`; evaluating script is `execute`.

## Wire format

Capabilities are lowercase strings. A grant is an object with a stable id, a resource
scope with polarity, and an allowed capability list. A minimal example, using the
reference implementation's host resources:

    {
      "id": "crm-read-write",
      "hosts": { "allow": ["*.crm.example.com"], "deny": ["admin.crm.example.com"] },
      "allowed": ["read", "action", "write"]
    }

This grant covers every host under `crm.example.com` except the admin console, and
permits observation, UI input, and declared field writes there, but not script
execution. A call requiring `execute` on any host this grant covers is denied by the
containment rule.

## Mechanism independence

RAWX classifies operations, not the transport that carries them. The reference
implementation drives a browser over the Chrome DevTools Protocol, but nothing in this
model depends on CDP, on screenshots, or on any particular automation surface. If the
underlying mechanism changes -- a new browser automation API, a declarative site-exposed
tool surface, a different domain entirely -- the classification of an operation as
`read`, `action`, `write`, or `execute` is unchanged. The governance vocabulary is the
durable asset; the mechanism is the transient one.

## What RAWX is not

- It is not a permission hierarchy or a set of tiers. There is no ordering and no
  implication.
- It is not effect prediction. It deliberately refuses to grade operations by guessed
  consequence, because the consequence is unknowable at decision time.
- It is not tied to one product, protocol, or domain. Any system that governs agent
  operations against resources can adopt it.

## Relationship to implementations

Ghostlight is the reference implementation and governs a browser: hosts are the
resources, and the 22 catalog tools plus their sub-actions map onto the
four capabilities through a published directory. Internally the model is documented as
"intent-calibrated" or "epistemic" capability classification; RAWX is the public name
for the vocabulary. Other implementations are welcome and encouraged; conformance means
honoring the normative sections above (the four definitions, independence, polarity with
default-deny, and the containment rule).

## References

- Ghostlight architecture decision record ADR-0022 (intent-calibrated capabilities): the
  originating design rationale.
- Unix file permission bits (`rwx`): the mnemonic ancestor this model extends with
  Action.

## License

This specification is published under Apache-2.0 OR MIT, the same terms as the Ghostlight
engine, so anyone may implement it freely. Corrections and proposals: open a discussion
in the reference repository, or email hello@sylin.org.
