# ADR-0110: Action receipts name the physical subject

Date: 2026-08-12
Status: Accepted
Supersedes: ADR-0103 Decision 4 and its amendment where they prohibit target labels in audit
Builds on: ADR-0005, ADR-0060, ADR-0088, ADR-0103, ADR-0107

## Context

An action log such as "Clicked a button" is technically safe but often useless. A page with ten
buttons needs the visible identity of the button that was actually used. The orchestrator already
knows the target handle requested by the caller, but that is not reliable evidence of the physical
element at the effect boundary. Names can change after inspection, and coordinate actions may have
no semantic handle at all.

Fetching a description before or after every action would add a browser round trip, create a race
between description and effect, and spread one physical observation across two commands. Chrome is
already resolving the element when it performs the action. That is the truthful and cheapest place
to observe its role and accessible name.

Target names are page content. They are valuable by default, but some deployments need action
history without them. The choice belongs to governance, not the extension.

## Decision

### 1. The action receipt carries the observed subject

The extension returns an optional physical action subject in the same receipt as the effect. The
subject contains only `role` and `name`. It never contains a selector, target handle, DOM state,
form value, or another page payload.

Semantic click, target scroll, hover, type, key, drag, and upload receipts carry the subject they
actually used. Coordinate click, hover, scroll, and drag perform a best-effort hit test and include
a subject when one is observable. Absence is valid and falls back to coordinates or a generic noun.
There is no describe round trip.

### 2. Names describe elements, never entered values

The extension derives the name from accessible naming sources and bounded visible label fallbacks.
Editable values, including textareas and contenteditable regions, are never a name source. A fixed
button caption such as an input-submit value remains a label. The subject is captured at the effect
boundary, before an action such as typing can change the element.

### 3. The orchestrator owns safe language

The browser reports raw physical facts. The orchestrator narrows the role into Ghostlight's closed
role vocabulary, normalizes whitespace and control characters, replaces sentence-breaking quotes,
and bounds the retained label to 80 visible characters. Unknown or hostile roles become `control`.
The page may supply the bounded quoted name, but it cannot choose the sentence or the noun.

The default sentence is specific when possible, for example:

`Clicked the "Save" button on sylin.org.`

Without a usable name it remains truthful:

`Clicked a button on sylin.org.`

### 4. Governance may remove names

Policy has one monotonic optional field: `preserve_target_names`. Omission defaults to `true`.
`false` in any configured authority layer removes names from model results, workbench history, and
audit summaries. `true` cannot reopen another layer's refusal.

The extension stays policy-free and always returns the physical subject. The immutable authority
snapshot decides whether the orchestrator retains its name when constructing the terminal outcome.
The closed role remains available as the safe fallback.

### 5. Audit is content-minimized, not content-free

The terminal audit may contain one governed, normalized, bounded target label inside Ghostlight's
summary. It still excludes selectors, handles, form values, file paths, scripts, screenshots,
recording bytes, full URLs, diagnostic payloads, and arbitrary page text. The structured observation
shape remains unchanged.

This supersedes ADR-0103's claim that host is the only page-derived audit text. The stronger honest
claim is that action history is content-minimized and target names can be disabled by governance.

## Consequences

- Logs distinguish actions on pages with many similar controls.
- Receipt-time observation is both cheaper and more truthful than cached inspect metadata.
- The browser bridge gains an optional physical fact but no model language or policy.
- Page content can identify the target but cannot author Ghostlight prose.
- Deployments that treat accessible names as sensitive can remove them with one monotonic policy
  field.

## Rejected alternatives

### Use the name cached with the target handle

Rejected because it describes what inspection saw, not necessarily what the browser acted on.

### Describe the element in a second browser call

Rejected because it adds latency and a race while splitting one effect receipt across commands.

### Keep all logs generic

Rejected because "Clicked a button" is not useful evidence on a page containing many buttons.

### Preserve no names by default

Rejected because useful local action history is the default product experience. Governance already
provides the correct seam for stricter deployments.
