# ADR-0111: Name page JavaScript execution honestly

Date: 2026-08-12
Status: Accepted
Amends: ADR-0107 tool catalog

## Context

ADR-0107 named the page-context JavaScript tool `browser_evaluate`. The mechanism uses CDP
`Runtime.evaluate`, but the model-facing name describes much more than that mechanism. The script
may read page state, mutate the document, cause network work, or navigate. "Evaluate" sounds
observational and understates the tool's consequential surface.

`browser_run` would be terse but too broad. It also competes with `browser_sequence`, which runs a
short list of semantic browser actions. The required `script` field already says what is executed,
so `browser_run_script` adds length without useful disambiguation.

## Decision

Rename the unreleased model-facing tool to `browser_execute`:

```json
{"script":"document.title"}
```

Its title is `Execute JavaScript`. Its description begins `Execute explicit bounded JavaScript in
the page main world.` The result sentence is `Executed JavaScript on example.com.`

This is a clean break. `browser_evaluate` is not advertised or accepted as an alias. The internal
`RunScript` operation, `EvaluateScript` browser primitive, CDP mechanism, bounds, execute-capability
classification, landing governance, and audit exclusions remain unchanged. Those internal names
describe their own layers accurately and are not model-facing vocabulary.

## Consequences

- The tool name signals its real authority class before a model reads the description.
- `browser_sequence` remains the obvious surface for running known semantic actions.
- Cached clients must refresh the tool catalog after upgrading this unreleased build.
- No bridge or extension protocol change is required.

## Rejected alternatives

### Keep `browser_evaluate`

Rejected because it sounds read-only while the capability is explicitly `execute`.

### Use `browser_run`

Rejected because it does not say what runs and is too close to `browser_sequence`.

### Use `browser_run_script`

Rejected because the required `script` parameter already supplies that distinction.

