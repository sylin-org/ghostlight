# ADR-0136: The model-facing policy explain tool

Date: 2026-08-24. Status: Accepted (implemented in this revision).

## Context

ADR-0121 Decision 3 promised "an always-available policy explain operation [that] renders the
same directory and current policy passport". In the tree it existed only as `ghostlight policy
explain <file>`, a CLI command over a file path, and the catalog had no policy tool.
[ADR-0122](0122-readable-policy-destination-and-authored-user-layer.md) Decision 9 recorded that gap
and deferred the model-facing rendering to "a later ADR". This is that ADR. It was pulled forward
by the owner on 2026-08-24 as part of the pre-freeze debt window
([tasks/pre-freeze-debt](../tasks/pre-freeze-debt/LEDGER.md)).

The raw material exists: `GovernanceFacade::effective_authority()` compiles one orchestrator-owned
projection -- situation sentence, one line per capability with polarity and decider, ordered rules,
authored settings, permanent ceilings, browser-startup posture, passport -- and the CLI explain
already renders grants, settings, and the RAWX capability directory from typed manifest data.

## Decision

1. **One new tool, `policy_explain`.** It takes no input beyond the shared request restrictions,
   crosses no browser seam, holds no workspace lease, and writes nothing. Its RAWX requirement set
   is EMPTY (the ADR-0121 table already reserved this row), so it is advertised under every valid
   authority including narrowed ones, and admitted wherever runtime control permits work at all.
   All-open grows from the exact 23-tool catalog to the exact 24-tool catalog; every count pin and
   active-truth document moves in the same commit.

2. **The model sees what the person's CLI sees, in the projection's own words.** The result payload
   is the effective-authority projection: situation sentence, capability lines (polarity, detail,
   decider), each layer's rules and settings, ceilings, browser startup, organization identity, and
   passport provenance. Two fields are deliberately withheld from the model result that the
   workbench destination does render: layer document texts and filesystem paths. They are
   machine-local reading aids for a person; an agent that needs them can be pointed at the file by
   a person. Withholding keeps model results as free of machine topology as the audit is.

3. **The outcome stays measured and content-minimized.** The terminal summary states what happened
   and names its measurement ("Explained current authority across N capability areas."); the full
   projection rides the structured result facts, exactly like tab listings. Audit records keep
   carrying only tool, requirements, decision, summary, and duration.

4. **Presentation is quiet by existing vocabulary.** The operation maps to the Quiet activity, so
   no new presentation beat is invented for a read that changes nothing on screen.

### Rejected alternatives

- Serving the raw parsed manifest instead of the compiled projection would hand the model authoring
  dialects to reverse-engineer, duplicating what `explain` already renders in plain words.
- Gating the tool behind a grant would contradict ADR-0121's empty requirement class and leave the
  model unable to learn why its other calls refuse -- the exact failure explain exists to prevent.
- Reusing `browser_read` against a synthetic page was rejected as a category error: authority is
  not page content and must not cross the browser seam.

## Consequences

- ADR-0121 Decision 3 and ADR-0122 Decision 9 are closed as implemented; neither text is rewritten.
- `docs/1.0/LANGUAGE.md` gains the tool's catalog section; ARCHITECTURE and ACCEPTANCE move their
  counts with this commit. Older ADRs' "22-tool" phrasing stays as written history.
- The workbench Policy destination remains the human surface; this tool adds no second renderer --
  both consume the same `effective_authority()` compilation.
