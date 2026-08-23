# R3: Semantic action loops

## Goal

Restore label-driven action and form completion without exposing selectors or adding a wide action
tool.

## Required work

- Add one typed semantic selector owned by the orchestrator: accessible name, optional closed role,
  exactness, and optional form scope.
- Add a policy-free adapter query that observes matching current targets from visible DOM and open
  shadow roots using labels, placeholders, and ordinary accessible-name sources.
- Resolve zero, one, and many matches explicitly. Never choose among multiple matches.
- Permit selectors as alternatives to target handles on the narrow tools named by ADR-0133.
- Extend form fields to string, boolean, and finite-number values and add explicit contained-form
  submit while preserving the current handle branches.
- Add one optional typed postcondition shared with `browser_wait`. Report an applied effect
  truthfully when its expectation later fails.

## Evidence

- Matching tests for label, placeholder, name, role, form scope, shadow root, zero, one, and many.
- No-effect proof for ambiguity and credential handoff proof before any value or action dispatch.
- Direct-handle and semantic-selector paths produce the same typed outcome and landing checks.
- Typed checkbox, radio, select, number, text, multi-field, and contained-submit journeys.
- Applied-but-expectation-failed result is not repeat-safe and does not claim no effect.

## STOP conditions

- Matching requires policy or model-facing semantics in the extension.
- A page-authored role can reach audit or outcome text without narrowing.
- Form submit can escape the resolved containing form or bypass action authority.

## Commit

`feat(browser): restore semantic action loops`

