# ADR-0086: Ranked find presentation

Status: Accepted

Date: 2026-07-20

## Context

The old `find` treatment reused the full-page read scan. That hid an important distinction from
the person watching the browser. A direct browser-search user already knows that a search is in
progress because they invoked the browser UI. A Ghostlight user may only be observing an agent.
The presentation must therefore answer both "what is the agent doing?" and "where did it find the
result?"

The tool can return as many as 20 ranked semantic candidates, strongest first, with a `more` flag.
A single target cue would misrepresent that result. Cloning matched DOM into an overlay would be
fragile, could change typography or layout, and would create a second copy of page content.

ADR-0081 provides document-aware presentation delivery. ADR-0083 allows composed treatments when
their parts communicate different necessary facts, but its initial coverage ledger classified
`find` as only a page scan. This ADR amends that classification.

## Decision

### D1. Find is a signature plus ranked spatial outcome

`find` uses a compact, signal-aware magnifying-lens badge while work is in progress. The badge
shares the action-signature medallion shell and placement rules. It identifies search in ordinary
user language without exposing the query.

After the result arrives:

- every returned, paintable match receives a gentle spatial treatment;
- the strongest match has more definition than the remaining matches;
- a result count settles onto the badge, using `20+` when the tool reports more results;
- no result settles as a question mark and the optional caption says `No match`.

The result treatment is bounded and temporary. A later find replaces it.

### D2. Text keeps the page's own typography

When literal text for a semantic candidate can be located, the isolated-world content script
creates DOM `Range` objects. The renderer registers those ranges with the CSS Custom Highlight API.
The browser paints white text over Ghostlight dark blue with the usual sky glow while preserving
the page's font, size, wrapping, and position. The strongest and secondary ranges use separate
named highlights.

Some semantic matches exist only through an accessible name, placeholder, control value, icon, or
other non-text surface. Those candidates receive a pointer-transparent element halo instead. DOM
nodes are never cloned and page layout is never changed.

### D3. Offscreen results use directional horizon glows

The renderer retains the matched elements for the visual lifetime and recomputes their viewport
geometry on scroll and resize. A match above the viewport activates a wide top-edge glow. A match
below activates a wide bottom-edge glow. If the strongest match is in that direction, the glow has
slightly more definition.

These are ambient horizon glows, not progress bars. They communicate that results continue beyond
the visible page without implying duration or completion percentage.

### D4. Presentation data stays inside the tab

The Presentation Broker carries only a fixed phase plus aggregate `count` and `more` values. It
never carries the query, matched text, DOM references, ranges, page content, or geometry.

The content script that performs the search passes live elements and ranges directly to the
renderer through the existing same-isolated-world `GhostlightFx` seam. Page scripts cannot invoke
that seam. Presentation failure never changes or fails the tool result.

### D5. Existing visual safety rules apply

Find visuals are pointer-transparent, use `ghostlight-` identifiers, honor the effects preference
and reduced-motion preference, and clear before Ghostlight captures a screenshot. They expire on
navigation, extension replacement, cancellation, timeout, or result completion.

The trained tool schema and the model-facing `find` result are unchanged.

## Consequences

- A person can distinguish search from reading without seeing the query.
- The visible page explains both result multiplicity and ranking.
- Offscreen findings are discoverable without an automatic scroll or viewport disruption.
- Text highlighting depends on the CSS Custom Highlight API. Element halos provide a truthful
  fallback for non-text candidates and browsers without that API.
- The renderer temporarily retains at most 20 element references and at most 80 text ranges.
