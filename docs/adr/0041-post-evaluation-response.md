# 0041. Post-evaluation response: standards posture, capability onboarding, and the origin-flow bet

- Status: Accepted (2026-07-07)

## Relationship to other decisions

- RESPONDS TO docs/research/14-post-evaluation-2026-07.md (the 2026-07-07 landscape
  post-evaluation) and its P1-P10 proposal list, all accepted by the project owner.
- RATIFIES ADR-0039 (saved scripts): Proposed -> Accepted (Decision 4 here).
- SPAWNS ADR-0042 (origin-flow provenance) and ADR-0043 (WebMCP stance).
- BUILDS ON ADR-0034 Decision 7 (additive tool growth), the research-12 harvest discipline,
  and the open-spec charter (open-spec/README.md).
- Does not amend any prior ADR other than the 0039 status change.

## Context

The post-evaluation (research 14) re-tested the positioning thesis from research 13 against a
moved landscape and found: the four-way intersection (real-session automation + client-agnostic
MCP + fused governance + open/local/single-binary) still uncontested; the independent
architectural twins stale; Anthropic shipping a first-party Claude Code + Chrome integration
whose permission design converges on Ghostlight's own vocabulary; Microsoft publishing an
open-source Agent Governance Toolkit that makes "agent governance" a named category; and a
University of Washington study establishing cross-origin data movement by injected agents as
the publicly named attack class.

The owner reviewed the findings on 2026-07-07 and issued three rulings (quoted in Provenance):
the vocabulary validation should be converted into capability onboarding; generic governance
players are to be met with alternatives and standards, not competition; and origin-flow
governance is the focus. All ten proposals were accepted.

## Decision

### Decision 1: standards posture -- alternatives, not competition

Public materials (README, COMPARISON.md, guides, open-spec) never frame first-party
integrations or generic governance toolkits as adversaries. The stance is: Ghostlight offers
the open, vendor-neutral alternative, and publishes its vocabulary as open specifications so
others can adopt it. Concretely:

- Comparison documents are decision guides ("when to use which"), not scorecards. The
  first-party Claude Code + Chrome path is described accurately and recommended for the cases
  it serves well.
- External vocabularies are met with bridges: a RAWX mapping to the OWASP agentic top 10 lives
  in open-spec/ (accepted proposal P3), and future mappings (e.g. to generic policy-engine
  vocabularies) follow the same pattern.
- The open-spec charter is the vehicle: the durable asset is the vocabulary, and its value
  grows with adoption by others, including players larger than us.

### Decision 2: capability onboarding from the validated vocabulary

The first-party integration independently arrived at read-vs-write call gating, escalating
argument flags, and a read-only-gated batch tool. This validates RAWX and the composition
design; it also surfaces official capabilities Ghostlight does not have. Onboarding follows
the harvest discipline (research 12; ADR-0008): re-baseline first, then one ADR per adopted
capability, additive growth only (ADR-0034 Decision 7). Dispositions:

- `browser_batch`: no action. `script` (ADR-0035) already covers it with strictly more
  governance (per-step verdicts, dry-run, budget, audit correlation).
- Session recording (GIF): a harvest CANDIDATE. Strong delight fit with the existing visual-FX
  identity. Requires its own ADR (engine + extension capability, capability classification for
  the recording action, disk-write escalation semantics). Not scheduled yet.
- Scheduling: folds into the saved-scripts direction (ADR-0039); a schedule is a trigger for a
  named, hash-bound artifact, not a new primitive. Considered at saved-scripts design time,
  not before.
- Re-baseline itself: accepted proposal P7; an operator-assisted study task (the official
  extension must be inspected live), producing a research-12 style delta note before any of
  the above ADRs are written.

### Decision 3: origin-flow provenance is the focus bet

The owner's ruling on the UW findings: focus here. ADR-0042 carries the design. Sequencing
consequence: the provenance audit key lands before the saved-scripts implementation, because
ADR-0039's retrospective creation path (Decision 6 there) reconstructs workflows from the
audit stream, and it should reconstruct them from the provenance-enriched stream rather than
migrating later.

### Decision 4: ratifications

- ADR-0039 (saved scripts as governed artifacts): Proposed -> Accepted, as the direction-level
  decision it is. Its own text still governs implementation timing ("until real scripts exist
  to learn from"); the implementation gets a dedicated design pass and execution batch
  (working name scripts-1) that resolves its open-questions list, after the landscape-1 batch
  lands.
- ADR-0040 (pipeline idempotency gate): stays Proposed; it is ratified or discarded by its own
  implementation batch, not by this ADR.

### Decision 5: MCP spec currency as a standing gate

The MCP 2026-07-28 revision (stateless core, Tasks, extensions framework, authorization
hardening) ships three weeks after this ADR. The currency audit (accepted proposal P4) is
recorded in docs/design/mcp-spec-currency-2026-07.md. Standing rule going forward: any new
transport or adapter work checks the then-current MCP revision first, and the currency note is
refreshed when a new revision is published. The one code-level finding (the server pins
`protocolVersion: "2024-11-05"` while emitting later-revision features, with no version
negotiation) is closed by the landscape-1 batch. Mapping `script` and future saved-script runs
onto the spec's Tasks primitive is recorded as direction, evaluated when the final revision is
published.

### Decision 6: proposal dispositions

| # | Proposal (research 14) | Vehicle | Owner |
|---|---|---|---|
| P1 | Publish v0.2.0 release; finish CWS submission | FOUNDER-TODO.md | Operator |
| P2 | Comparison positioning vs first-party path | COMPARISON.md + README (landed with this ADR) | Done |
| P3 | RAWX mapping to OWASP agentic top 10 + UW findings | open-spec/rawx-owasp-agentic-mapping.md (landed with this ADR) | Done |
| P4 | MCP 2026-07-28 currency audit | docs/design/mcp-spec-currency-2026-07.md (landed) + landscape-1 L2 | Done / Batch |
| P5 | Ratify and land ADR-0039 | Ratified here (Decision 4); scripts-1 design pass + batch to follow | ADR / Next batch |
| P6 | Origin-flow governance | ADR-0042 + landscape-1 L1 (phase 1, audit-only) | ADR / Batch |
| P7 | Re-baseline against the official extension | Operator-assisted study task (research-12 pattern) | Operator + agent |
| P8 | Enterprise proof pack | landscape-1 L3 | Batch |
| P9 | WebMCP stance ADR | ADR-0043 (landed with this ADR) | Done |
| P10 | Standing verification debts | FOUNDER-TODO.md (LIVE-VERIFY, Linux, e2e-smoke, license skim) | Operator |

## Consequences

- Positioning language across the repo has one recorded posture; drift toward adversarial
  framing is now an ADR violation, not a style preference.
- New official-surface capabilities cannot be copied in ad hoc; each needs its ADR, which keeps
  the additive-growth discipline of ADR-0034 intact under harvest pressure.
- The origin-flow bet gets sequencing priority over saved-scripts implementation; if the bet is
  wrong, the cost is bounded (phase 1 is one additive audit key).

## Provenance

Owner rulings, 2026-07-07, on the three deltas in research 14 (verbatim): (1) "good to know
our vocab is validated. we can onboard the new capabilities."; (2) "we're not competing, we're
offering alternatives/standards."; (3) "excellent, let's focus on this." [origin-flow]. And on
the proposals: "All accepted." These are decided questions; do not re-litigate them.
