# 0020. Organization policy experience: policy as code with explain, simulate, shadow

- Status: Accepted
- Date: 2026-07

## Context

ADR-0019 gives organizations a lockable, layered configuration delivered as a
machine-scope file over their existing deployment channel. That is functional,
but the administrator experience in all the harvested prior art is authoring
blind: ADMX XML or registry values with no preview, deploy-and-pray rollout,
verification by asking a user to run a diagnostic, and documentation that
drifts from the software. The north star demands delight for every persona,
and the administrator is a persona.

Three assets make a far better experience nearly free for us: the typed,
self-describing key registry (ADR-0019), a policy engine that is a pure
decision function at a single dispatch choke point (ADR-0013), and an audit
flight recorder that ships before enforcement does (ADR-0018). Together they
allow something no harvested prior art offers: governance an organization can
test against recorded reality before switching it on.

## Decision

The organization surface is policy as code, composed from tools organizations
already trust (their editor, git, their MDM, their SIEM). Six commitments:

1. **Generated schema.** Each release publishes a JSON Schema generated from
   the key registry (hand-rolled generation, no new dependencies). Editing a
   manifest in any modern editor gives autocomplete, types, ranges, and key
   descriptions inline. Reference documentation for keys is generated from
   the same registry, so the schema, the editor experience, and the docs
   cannot drift apart.
2. **`policy explain`.** A deterministic plain-language rendering of any
   manifest: which identities may read and write where, what is locked, what
   users may still change, what happens on denial. The same renderer backs
   the user-facing import preview for shared manifests, so the sentence an
   admin reviews is the sentence a user sees.
3. **`policy simulate`.** Evaluates a candidate manifest against recorded
   audit JSONL and reports every action that would have been denied, with the
   responsible grant. Because audit ships before enforcement (ADR-0018), an
   organization baselines real agent traffic in observe mode, then tests
   candidate policy against actual usage instead of guessing what will break.
4. **Shadow enforcement.** A `mode: observe | enforce` switch at manifest and
   per-grant level. Observe evaluates every decision and writes it to audit
   without blocking anything. Staged rollout needs no new infrastructure:
   ship the manifest in observe mode to a pilot group, read the would-deny
   events in the SIEM, flip to enforce. Status surfaces must badge shadow
   mode plainly; observing must never present as protection.
5. **Manifest identity everywhere.** Manifests carry a name, version, and
   content hash, stamped into every audit record and shown by `doctor` and
   `config list`. Every logged decision is attributable to the exact policy
   version that made it.
6. **Structured denials.** Each denial carries a stable denial id and the
   denying grant, in both the tool result text and the audit record. A
   developer hitting a denial can hand the id to their admin; the admin can
   trace it in the SIEM and tune the manifest, closing the feedback loop.

Explicit non-goals, reaffirming SPEC sec 10: no web console, no remote policy
service, no SaaS control plane. Delivery stays file-over-MDM; the delight
comes from making the file trustworthy, previewable, and testable, not from
building a portal.

## Consequences

- Positive: the enterprise admission ticket becomes a delight story. This is,
  to our knowledge, the only agent-governance design an organization can test
  against its own recorded traffic before enforcing.
- Positive: explain and simulate call the same pure decision function as live
  enforcement, so previews cannot lie about behavior by construction.
- Positive: manifest templates (the shareable gallery) ride the same schema
  as `policy init --template <name>` starting points.
- Negative: simulate only covers recorded behavior; genuinely new workflows
  can still hit denials in production. Shadow mode is the mitigation.
- Negative: `policy explain` is a trust surface. A rendering bug that
  misstates policy is a serious defect; the renderer needs golden tests that
  pin sentences to decisions.
- Follow-up: implement inside stage 2 (ADR-0018): the audit record shape must
  carry manifest identity from its first version so simulate has stable input.

## Amendment (2026-07-05, ADR-0030)

ADR-0030 (Ghostlight Hub) Decision 9 introduces "the Console": a loopback-pinned static site,
embedded in the binary and served from the local web API's own HTTP stack, showing live
sessions/groups and a provenance-aware, read-mostly config view (per key: value, which of the five
ADR-0019 layers set it, whether an org-mandatory lock renders it read-only). This is a
documentation cross-reference clarifying scope, not a reversal of this ADR's non-goal, which
remains in force UNCHANGED for what it actually rejected.

This ADR's "no web console, no remote policy service, no SaaS control plane" line was written
about the ORGANIZATION policy experience specifically (this ADR's title): a hosted or
network-reachable surface for AUTHORING or DEPLOYING organization policy in place of
file-over-MDM. That rejection stands. The Console is categorically different on every axis that
mattered to the original decision:

- **Local, not remote.** Loopback-pinned by default (`127.0.0.1`); it becomes reachable off-box
  only if the machine owner deliberately flips a user-layer policy key from inside it (Decision 5),
  and an org-mandatory lock on that key renders the control read-only and shuts remote down
  immediately -- the org retains the last word, exactly as this ADR's layered-lock model requires.
- **A view, not an authoring surface.** The config panel renders the ALREADY-RESOLVED effective
  value/layer/lock per key -- the `chrome://policy` analog ADR-0019 named outright (see that ADR's
  own amendment below) -- never a manifest editor. Organization policy is still authored and
  deployed exclusively as a file over the org's existing channel (GPO/Intune/Jamf); the Console
  cannot write, edit, or push a manifest, mandatory layer, or org-recommended default. `policy
  explain`, `policy simulate`, shadow enforcement, manifest identity, and structured denials are
  all unaffected and remain the CLI/file-based surfaces this ADR specifies.
- **Single-machine, not a control plane.** The Console has no concept of fleets, deployment, or
  cross-machine state; it shows one local Hub's own sessions, config, and web-API tokens. There is
  still no remote policy service and no SaaS control plane.

Follow-up: `docs/tasks/console/` (a task batch in the same BOOTSTRAP/LEDGER/PINS shape as
`docs/tasks/hub/`) implements the Console against ADR-0030 Decision 9's description.
