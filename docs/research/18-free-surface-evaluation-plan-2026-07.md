# Free-surface candidate evaluation plan, 2026-07

Status: Baseline harness implemented and wired into blocking Linux CI; the first CI result,
visible-browser repetition, and repeated model runs are pending. No product surface is accepted by
this document.

## Purpose

Research 17 identified two small capabilities that could improve model, user, and governance
delight without turning Ghostlight into a testing runtime:

1. ref-linked annotated screenshots; and
2. optional memorable labels for session-owned tabs.

Both ideas are plausible. Plausibility is not enough to change an MCP surface. This plan defines
the evidence each candidate must produce before an ADR or implementation can be accepted.

## Common rules

- Compare against the current released Ghostlight baseline with the same client, model, prompt,
  page fixture, viewport, and policy posture. Record the exact version and commit with every run;
  the current baseline is v0.6.0.
- Record tool-call count, model-visible text characters, screenshot count, recovery turns,
  elapsed time, terminal result, and any wrong-target action.
- Run each journey at least three times per configuration. Report raw observations as well as the
  median. Do not claim statistical significance from this small product experiment.
- Use deterministic local fixtures for the first pass. Repeat the winning candidate in the visible
  live browser before acceptance.
- Keep page content out of governance audit and ordinary debug logs. Evaluation evidence remains
  local and contains only the bounded fixture data needed for the comparison.
- Raw screenshots and numeric tab ids remain available. A candidate must be additive and optional.
- A candidate fails if it weakens tab ownership, document freshness, capture truthfulness,
  governance classification, or the trained schema fidelity guarantees.

## Candidate A: ref-linked annotated screenshots

### Hypothesis

When layout matters, a model often needs both a screenshot and a semantic observation. One result
that visually marks the same bounded refs returned in structured content should remove a
roundtrip and reduce coordinate recovery without increasing authority.

### Journeys

1. Dense toolbar: choose one icon-only action among at least eight nearby controls.
2. Repeated form: distinguish repeated labels in two visible panels and focus the requested field.
3. Mixed viewport: select one visible control while another semantic match is below the fold.

Each fixture must expose an ambiguity that a screenshot alone or an unbounded tree handles poorly.
The baseline uses current `computer screenshot`, `read_page`, `find`, and action calls as the model
chooses. The candidate run may request one annotated capture.

### Minimum payload contract to prototype

- The underlying screenshot dimensions, JPEG budget, and raw screenshot default do not change.
- Only visible interactive elements in the bounded observation set receive markers.
- Every marker uses the exact document-local ref accepted by existing ref-based actions.
- Structured content returns a bounded legend with ref, role, accessible name, and marker geometry.
- The result identifies the document generation used for both image and legend. Navigation or
  document replacement makes the refs stale rather than silently retargeting them.
- Ghostlight's own border, narration, denial, recording, and camera chrome remain excluded from the
  captured page image under the existing capture barrier.
- Annotation is observation, not authority. Existing read classification and provenance rules
  apply; no page content enters policy or audit.

### Acceptance gate

Proceed to an ADR amendment only if the prototype:

- saves at least one model-to-tool roundtrip in at least two of the three journeys;
- produces no wrong-target action or image-to-ref mismatch across all measured runs;
- keeps its structured legend within 4,000 characters and 40 visible interactive elements;
- leaves raw screenshot output byte-shape and default behavior unchanged; and
- survives navigation, extension-worker restart, reduced-motion settings, and capture cleanup in
  deterministic tests.

Token count may improve because a second observation disappears, but image tokens are not claimed
to shrink. The primary promise is fewer turns with one coherent visual-semantic payload.

## Candidate B: optional owned-tab labels

### Hypothesis

A short session-chosen label such as `invoice` may make multi-tab plans easier to express and
recover than repeating large composite tab ids. Existing ids already provide correctness, so the
candidate must show a comprehension or recovery benefit rather than merely looking friendlier.

### Journeys

1. Compare: read two similar product pages, return to the cheaper page, and cite the chosen tab.
2. Transfer: read a value in one tab and enter it into a named field in a second tab.
3. Recovery: navigate and reorder three owned tabs, then resume work in the originally named tab.

The baseline uses numeric ids. The candidate run may assign labels when tabs are created and use
those labels only where the additive prototype explicitly permits them.

### Minimum payload contract to prototype

- Labels are optional, ASCII, session-scoped presentation metadata supplied by the caller.
- Labels are never inferred from page titles, URLs, or content and are never authority.
- Authorization always resolves the label to one exact session-owned internal tab id before URL
  resolution or dispatch.
- Labels are unique within a session, bounded in length, and rejected on ambiguity. Renaming and
  replacement semantics must be explicit.
- Numeric tab ids remain canonical in audit, wire transport, and low-level browser execution.
- Teardown erases labels. Browser or service recovery never guesses a label-to-tab binding.

### Acceptance gate

Proceed to an ADR only if measured runs show at least one of:

- a recovery turn is removed in at least two of the three journeys;
- model-visible tab-reference characters fall by at least 20 percent without extra lookup calls;
  or
- the candidate prevents a demonstrated wrong-tab recovery that occurs with the baseline.

The prototype also must produce zero cross-session resolution, stale-label retargeting, or
unlabeled-call regression. If none of the benefit gates pass, keep numeric ids and do not ship
labels.

## Execution order

1. Add deterministic fixture definitions and capture the released baseline. The fixture and opt-in
   full-stack runner are implemented; see `docs/testing/free-surface-baseline.md`. Blocking Linux
   CI now executes the mechanical baseline. A visible local-browser repetition remains pending.
2. Prototype annotated screenshots behind a non-default test seam, not the public schema.
3. Measure Candidate A and decide whether an ADR amendment is warranted.
4. Measure the tab-label baseline before writing Candidate B production code.
5. Prototype Candidate B only if the baseline demonstrates recurring reference or recovery cost.
6. Repeat any passing candidate in a visible local browser and at least two MCP client or model
   configurations before calling the benefit general.

Candidate A is first because it can collapse two complementary observations into one coherent
payload. Candidate B remains behind evidence because stable numeric ids already solve the
correctness problem.

## First mechanical baseline

On 2026-07-14, Codex drove the installed v0.5.8 Windows stack against the deterministic local
fixture. This was one configuration and one run per visual journey, not the repeated model study.
The local HTTP server was stopped after capture.

| Journey | Observation calls | Screenshot text chars | Read text chars | Image base64 chars | Total ms |
|---|---:|---:|---:|---:|---:|
| Dense toolbar | 2 | 174 | 600 | 18,724 | 280 |
| Repeated form | 2 | 174 | 628 | 22,272 | 244 |
| Mixed viewport | 2 | 174 | 366 | 21,476 | 282 |

Every read contained the expected exact target vocabulary. The current mechanical shape is two
complementary observations per journey. A coherent annotated result therefore has room to remove
one roundtrip, but this run does not prove that a model will need both observations every time or
choose the right target more often.

The three product tabs each used an 11-character composite id, or 33 characters before any id was
repeated in a plan. `tabs_context_mcp` returned in 19 ms. Its 664 text characters also described a
pre-existing fourth tab, so that payload size is not a clean three-tab comparison. The id result
supports keeping labels second: there is a measurable repetition cost, but no recovery failure or
20 percent end-to-end benefit has been demonstrated.

## Explicit non-goals

This work does not add headless or isolated browsers, copied profiles, visual regression tooling,
OCR, arbitrary DOM labeling, content logging, cross-origin frame authority, or model hosting.
Testing-specialist breadth continues to compose with agent-browser and Playwright outside the
live-user-context core.
