# ADR-0103: Language-owned outcome voice and measurement projection

- Status: Accepted
- Date: 2026-08-11
- Builds on: ADR-0101, ADR-0102, and `docs/design/action-observations.md`

## Context

The language context owned what Ghostlight accepted, but completed-action sentences and recovery
copy were inline literals throughout the application executor. Content-free measurements were
gathered independently at the browser dispatch seam. A summary and its machine-readable count
could therefore compute the same value twice, and the workbench had to guess whether a row should
show the host or the sentence.

The browser seam still needs an exhaustive account of facts common to browser crossings. Counts
and sizes are different: their meaning comes from the product sentence that names words, fields,
matches, files, steps, elapsed milliseconds, or pixels.

## Decision

### 1. One language module owns terminal voice

`crates/orchestrator/src/language/outcome.rs` owns:

- `Outcome`, which renders a completed action's `summary`, `next_steps`, and `observed` projection;
- `Refusal`, which renders unchanged refusal summaries and safe next steps;
- `WorkspaceReason`, which maps workspace failures to stable facts and recovery; and
- `Observed`, moved from governance without changing its five fields or JSON shape.

Every successful executor completion requires an `Outcome`. Inline completed-action sentences are
not accepted by `succeeded`.

### 2. The browser seam and language outcome own disjoint facts

`Executor::dispatch` remains exhaustive over `BrowserOutcome` and records host/readiness only.
`Outcome::observed` records counts and capture dimensions, plus a host when its own sentence names
that host. Count conversion saturates at `u32::MAX`.

The one completion path merges the outcome projection over the seam observation. This preserves
seam-owned host/readiness when the outcome has no corresponding value and makes the outcome's
sentence and named measurement one value.

### 3. Presentation renders the orchestrator's decision

Workbench rows render the Ghostlight-authored summary directly and may append a readiness note.
They do not infer whether an observation is measured or replace an outcome sentence with a host.
The hero may still carry host as separate metadata.

### 4. Audit remains payload-free and wire-compatible

The audit record and result envelope do not change. `Observed` remains a typo-closed object with
`host`, `readiness`, `count`, `width`, and `height`. The host remains the only page-derived audit
value and never includes path, query, or fragment. Facts, status, effect, readiness, repeat safety,
capability, and governance decisions are unchanged.

## Consequences

- Success and refusal wording can be reviewed in one module.
- Whatever a sentence names is projected from the same typed value.
- A new browser outcome must still decide host/readiness at the exhaustive seam.
- A new successful action must choose an `Outcome` before it compiles.
- The workbench no longer carries product-language selection logic.
- Governance continues to own audit records and authority while depending on the language-owned
  observation shape.

## Rejected alternatives

### Keep summaries inline and add more regression assertions

Rejected because tests would compare two independently maintained accounts rather than remove the
source of drift.

### Gather every count at the browser seam

Rejected because the seam does not know the noun that gives a general count product meaning, and
some outcomes such as sequence completion are application facts rather than one browser receipt.

### Let the workbench choose between host and summary

Rejected because presentation should render a product decision, not reconstruct one from nullable
measurements.

## Amendment: human outcome language and safe nouns (2026-08-12)

The same language owner now says what a person would say. Sentences lead with the action, name the
governed host when useful, and omit mechanism words such as target, browser job, condition, and
physical effect. Closed caller inputs such as direction, named key, coordinates, counts, and wait
condition may appear. Caller text and page labels may not.

A page can author any string as an accessibility role. `TargetRole` is therefore a closed
language vocabulary. The workspace narrows the raw role once, when it registers a generation-bound
target handle, and stores only the closed value. Unknown or invented roles become `control`. Action
sentences can say `button`, `checkbox`, or `text field` without allowing a page to write audit
language.

`Refusal::AuthorityBlocked` carries a closed denial reason and an optional sanitized host. Its
summary and observation projection read the same typed refusal. An explicit refused navigation may
therefore record `example.com`, but never the full URL, path, query, fragment, target description,
or request value. This amends Decision 4: the host field is the only page-or-request-derived audit
text and may describe either a governed attempted host or a governed landing.

The `Observed` JSON shape does not change. Refusals use the same single completion merge as
successful outcomes; the executor does not construct an independent audit account.

### Amendment acceptance evidence

1. Exact oracle tests cover every success and refusal sentence branch.
2. A hostile role such as `Save my document` becomes `control` before target state is stored and
   cannot enter the action summary or audit record.
3. A refused URL containing a path, query, fragment, and secret records only its normalized host.
4. Every sentence that names a host or measurement projects that same typed value.

## Acceptance evidence

1. Exact oracle tests cover every `Outcome` and `Refusal` sentence.
2. Tests prove named hosts, counts, and capture dimensions agree with `Outcome::observed`.
3. `Observed` round-trips with its unchanged JSON shape.
4. Workspace errors map exhaustively to `WorkspaceReason`.
5. The browser-seam test proves counts are absent while host/readiness remain.
6. Executor tests prove completion merges seam and outcome observations without page-payload leak.
7. A source guard proves the workbench renders the sentence and has no `measured()` inference.
