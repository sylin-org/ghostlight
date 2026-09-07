# ADR-0156: Grouped composition history and permission explanations

- Status: Accepted
- Date: 2026-09-06
- Amends: ADR-0102, ADR-0103, ADR-0121, ADR-0133

## Decision

The owner accepted H4: one collapsed history entry per composition, incremental safe child
receipts, detail on demand, and permission explanations for both allowance and refusal. Opening
an entry brings the relevant problem into view; an open entry updates without extra alerts.

Retain receipts at child completion under the parent's lease and authority snapshot. Parent
invocation and one-based position identify each child. Terminal operations and preparation
failures are distinct record kinds; never-run rows require the final composition's evidence.
Missing receipts remain unconfirmed. Parent and child accounting must not double-count actions
or denial thresholds, and a child must not settle the parent's lifecycle.

Capture positive grants from the same policy evaluation that admits work, including evaluated
layers, the complete capability set, request restriction presence and whether its checks were reached, and observe-mode admission. Do not
reconstruct permission from current policy later. Existing denial attribution stays readable.
Only bounded grant identities, normalized hosts, closed causes, and H1-permitted outcome metadata
are retained. Caller-authored step labels and operation payloads are excluded.

The workbench groups records in the existing bounded local history and reconstructs the same
truth after restart. Policy previews use actual child operations rather than the empty wrapper.
All-open remains first-class. No new process, external protocol, policy setting, or generic event
framework is introduced. Automatic resumption, H5 timing/scope, and H7 durability policy are outside
this decision. See [H4](../tasks/security-hardening/h4-grouped-history.md) for implementation evidence.

## Implementation

Implemented and verified locally in the H4 commit. The common receipt seam is `work/receipt.rs`;
evaluation evidence is `governance/evidence.rs`; history language is `language/history.rs`;
whole-group projection is `workbench/history.rs`. The existing workbench change vocabulary gains
one composition update that leaves the parent lifecycle active. Permission evidence does not
change policy evaluation order or admission semantics. The H4 task records exact validation and
remaining platform/deployment limits. No new connector contract or extension policy logic.
