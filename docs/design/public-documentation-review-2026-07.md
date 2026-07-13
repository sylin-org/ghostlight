# Public documentation review, 2026-07

Status: Applied editorial review. This is a content map, not an architecture decision.

## Goal

The public documentation should help the right reader recognize their problem, try Ghostlight, and
form an accurate mental model without a sales performance. Engineering depth and enterprise
evidence remain available, but they should answer curiosity rather than become the entrance fee.

The product philosophy is responsible delight for everyone in the loop:

- the developer should get a useful local tool with little ceremony;
- the person whose browser is moving should understand and control the experience;
- the agent should receive compact tools, corrective errors, and clear boundaries; and
- the security or ISO professional should be able to inspect the mechanism and evidence.

Responsibility is not positioned as a compromise against capability. It is part of product quality.

## Documents reviewed

- `README.md`
- `ROADMAP.md`
- `PRICING.md`
- `MAINTENANCE.md`
- `SECURITY.md`
- `CONTRIBUTING.md`
- `docs/COMPARISON.md`
- `docs/guides/installation.md`
- `docs/guides/solo-developer.md`
- `docs/guides/compliance-team.md`
- `docs/trust/README.md`, `security-overview.md`, `data-flows.md`, and relevant FAQ answers
- `open-spec/README.md`

## Findings and dispositions

### The opening proved the system before naming the felt problem

The old README established technical credibility quickly, but a reader had to infer whether the
tool fit their own work. The revised opening now names the real-session problem, gives concrete fit
and anti-fit cases, and describes the visible experience before presenting the full mechanism.

### Responsible delight was present but implicit

Visible actions, dedicated tab groups, useful denials, local ownership, and continuity were spread
through the document as features. The revision names the common principle directly while keeping
the concrete behavior beside it. `NORTH-STAR.md` now records that responsibility and delight are not
opposing ends of a slider.

### Installation described internal stages rather than one journey

The old fast path asked the user to add an MCP server, connect the browser side, and install the
extension as separate concepts. The installer already owns supported client registration. The new
path begins with one command, leads to the required visible extension step, and ends with one client
restart. ADR-0070 records the bidirectional handoff.

### Several current-state claims had drifted

- The README, comparison, installation guide, security overview, and data-flow page described an
  older single-binary or direct-binary topology. They now name the persistent service and thin
  relays.
- The solo-developer guide said audit was off by default. Current safe defaults enable the local
  flight recorder even in all-open mode; the guide now says so and explains that recording is not
  enforcement.
- The roadmap still listed offline licensing and `managed://` distribution as future work after
  both had shipped. It now reflects the actual near-term work.
- "No Node" blurred install-time and runtime truth. The npm path needs Node for `npx`; the running
  Ghostlight service does not run on Node. Public wording now keeps those facts separate.

### The audience layers were not equally visible

The governance and Trust Center documentation serves security readers well. Agent ergonomics and
the human-watching experience were technically present but less legible in the main entrance. The
README now makes all four participants visible without turning them into separate product editions.

## Public content contract

Future revisions should preserve this order of understanding:

1. The concrete problem: an agent needs the browser where the user is already signed in.
2. The felt experience: local, visible, model-friendly, controllable, and honest.
3. The shortest successful path: install, add extension, restart client, ask one useful question.
4. Fit, anti-fit, and current-state candor.
5. Capabilities and governance mechanics for readers who want depth.
6. Architecture, licensing, continuity, and procurement evidence at their natural destinations.

The README should not attempt to reproduce the Trust Center, SPEC, or competitive research. It
should make the reader want the next relevant detail and link there directly.

## Remaining high-value work

The canonical service-first page required by ADR-0070 is live at
`https://sylin.org/ghostlight/service/post-install/`.

1. Finish the Chrome Web Store listing so the visible browser step becomes one click.
2. Capture the README hero GIF using Ghostlight itself. The empty hero slot remains the largest
   gap between the promised experience and the public page.
3. Live-verify macOS and Linux, then replace the current-state caveat with evidence.
4. Ask first-time users two questions: what made you try it, and where did installation hesitate?
5. Review public architecture statements as part of every topology-changing ADR and release.

## Voice guardrails

- Show the mechanism when making a strong claim.
- Name absences and unfinished work without apology theater.
- Prefer a useful example over a superlative.
- Recommend another tool when it better fits the reader.
- Do not make governance sound like punishment or personal use sound incomplete.
- Do not call attention to care as a marketing claim when the interaction can demonstrate it.
