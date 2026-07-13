# ADR-0069: Agent journey evaluation artifacts

- Status: Proposed
- Date: 2026-07-12
- Builds on: ADR-0035 (script), ADR-0038 (structured results), ADR-0039 (saved scripts),
  ADR-0042 (origin-flow provenance), and ADR-0052 (GIF capture)

## Context

Ghostlight can show what an agent did, correlate composed calls, preserve structured results,
record in-band data provenance, and export a visual GIF. Those are useful records, but none is an
evaluation artifact.

An evaluation answers a different question: given a user journey and an agent configuration, did
the agent choose appropriate tools, respect the intended boundaries, recover from expected
failures, and reach an acceptable outcome? Developers comparing models or MCP clients need a
portable record of that journey. Today they must reconstruct it from terminal output, screenshots,
and an audit stream designed for governance rather than evaluation.

This direction is especially relevant as browser applications expose semantic tools through
WebMCP. A semantic tool does not remove probabilistic model behavior. It creates another surface
whose discovery, selection, arguments, outcome, and user interaction must be evaluated.

The artifact must not become a covert browser-history archive. The user's authenticated session is
sensitive. Evaluation capture therefore needs explicit boundaries, local ownership, minimization,
and honest replay semantics from birth.

## Proposed decision

### 1. Define a journey artifact, not a session dump

An evaluation artifact represents one bounded user journey. Its logical contents are:

- artifact schema version and Ghostlight version;
- user-supplied journey name, intent, and optional acceptance criteria;
- client and model identifiers when the client supplies them;
- ordered tool calls, normalized arguments, structured outcomes, timing, and terminal status;
- governance verdicts and correlation fields already emitted by the engine;
- in-band provenance already attested by ADR-0042;
- explicit checkpoints and reviewer notes; and
- optional visual evidence selected by the user.

The storage format is a directory or archive with a small manifest and append-only event data. The
exact format is deferred to the implementation ADR. It must be versioned, inspectable without
Ghostlight, and suitable for diffing after normalization.

### 2. Keep four artifacts distinct

- The audit stream is the authoritative per-call governance record.
- A saved script is reusable competence: a reviewed workflow that may run again.
- A GIF is a human-readable visual account.
- An evaluation artifact is evidence used to assess agent behavior against a journey.

An evaluation may reference or derive from the other three, but none silently changes meaning.
In particular, exporting an evaluation does not automatically create an approved saved script.

### 3. Capture is explicit and minimized

Evaluation recording is off until a user or test harness starts a bounded capture. By default it
records the structured control-plane facts Ghostlight already owns. It does not archive full page
HTML, cookies, browser storage, arbitrary response bodies, or continuous screenshots.

Optional page text, screenshots, console output, network details, and GIFs require explicit capture
settings. Existing secret-redaction and response-budget rules apply before material enters the
artifact. The export command previews included data and supports deletion without a vendor service.

Everything remains local unless the user deliberately exports or uploads it. No telemetry or
vendor upload path is introduced.

### 4. Evaluation is not deterministic replay

A captured journey does not prove that a mutable website can be restored to its prior state. It
does not promise deterministic playback and never silently repeats writes.

The first implementation should support inspection, comparison, and assertion evaluation over the
captured record. Any later live replay is a separate decision. It must pass through ordinary
governance, identify side effects before execution, and require an explicit destination session.

### 5. Make comparison useful across clients and models

Normalized reports should make it possible to compare:

- task completion and checkpoint results;
- number and sequence of calls;
- tool-selection and argument errors;
- recovery attempts;
- context-heavy outputs and elapsed time;
- governance denials or requested capabilities; and
- cross-step and cross-origin data-flow shape where Ghostlight can attest it.

Scores are optional consumers, not authoritative truth. The core artifact preserves evidence and
declared criteria rather than manufacturing one universal quality number.

### 6. Start through the management and test surfaces

The first design pass should prefer CLI, Console, and lightbox entry points such as start, stop,
annotate, export, inspect, and compare. It should not add an LLM-facing tool until real workflows
show that the model itself needs to control capture. Any tool-surface growth remains additive under
ADR-0034 and must not alter the trained schemas.

### 7. Keep the recorder in the open engine

Capture and inspection are developer capabilities and belong in the permissive engine. Applying
organization policy to evaluation retention, mandatory evidence, or centrally managed destinations
may belong to the governance layer later. License state never gates recording or access to an
artifact.

## Implementation gates

Before acceptance, the implementation design must provide:

1. Three concrete evaluation journeys, including one read-only and one consequence-bearing case.
2. A field-level data inventory with default retention and redaction behavior.
3. An artifact schema and compatibility policy.
4. A threat review covering secret capture, accidental export, and replayed side effects.
5. A lightbox path that produces useful artifacts without a live personal browser.
6. Evidence from at least two MCP clients or model configurations that the comparison is useful.

## Consequences

- Developers gain a reason to use the flight recorder even without organizational governance.
- Ghostlight can become useful for model and client selection, regression testing, and bug reports.
- Saved scripts gain better evidence about how competence was originally learned.
- WebMCP experiments can be evaluated through the same journey vocabulary as ordinary browser
  actuation.
- The privacy surface grows. Minimization and explicit capture are product requirements, not
  documentation caveats.
- Reproducibility remains bounded by the live web. The artifact records what happened; it does not
  claim to package the world in which it happened.
