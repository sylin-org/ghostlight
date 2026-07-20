# ADR-0089: Destination-aware spatial cues

Status: Accepted

Date: 2026-07-20

## Context

ADR-0083 classified every tool action and deferred three presentation questions: direct
`computer.scroll_to`, coordinate image placement, and console/network inspection. The first two
actions have meaningful destinations but relied only on page movement or page response. A person
watching Ghostlight could see that something happened without always seeing where it landed.

Console and network inspection are different. They read extension-owned diagnostic buffers and do
not manipulate the rendered document. Giving them an in-page badge would suggest page activity
that did not occur and would add noise for ordinary users.

`act_on.scroll_to` also matters. It already publishes a semantic target cue before delegating to
`computer.scroll_to`. A destination treatment must compose with that cue without painting the
same target halo twice.

## Decision

### D1. Ref-based scroll-to gets a destination treatment

After `scrollIntoView` lands, three sky chevrons settle into the exact target element and fade. A
brief target halo confirms the destination. The content script invokes the renderer through the
same-isolated-world `GhostlightFx` seam because it already owns the resolved element.

The legacy coordinate compatibility path keeps visible page movement as its native treatment. It
does not invent an element target that the invocation did not supply. The trained `computer`
schema remains unchanged: `scroll_to` continues to teach refs as its model-facing input.

### D2. Semantic composition is deduplicated at the renderer

The renderer remembers the element most recently announced by `AGENT_SEMANTIC_TARGET` for a short,
bounded interval. When the subsequent scroll lands on the same element, the destination chevrons
still communicate arrival but a second halo is suppressed. A different or stale target receives
the complete treatment.

This is presentation state only. It does not enter the tool request, browser protocol, policy, or
audit path.

### D3. Coordinate image placement gets a fixed photo-drop treatment

After the content script dispatches `dragenter`, `dragover`, and `drop`, a fixed photo tile settles
into a halo around the destination. The treatment receives only the target element and viewport
point. It never receives or renders image bytes, filename, MIME type, page content, or the page's
acceptance result.

The cue means "Ghostlight dispatched an image here." It does not claim that the page accepted or
persisted the image. The model-facing result remains responsible for distinguishing page-signaled
handling from dispatch without a signal. Ref-based image upload keeps the existing field splash.

Same-origin iframe targets are translated into top-viewport geometry. A cross-origin iframe stays
represented by its reachable outer frame element.

### D4. Console and network inspection stay intentionally quiet

`read_console_messages` and `read_network_requests` receive no page effect or action medallion.
Their model-visible result already explains the operation, the persistent controlled-tab border
discloses reachability, and the rendered page is not being manipulated. A future diagnostics
surface may own a backstage activity language, but one-off buffer reads do not justify one.

### D5. Existing visual invariants apply

Both destination cues are optional decoration, pointer-transparent, `ghostlight-` prefixed,
excluded from page reads, hidden during capture, bounded and ephemeral, and reduced-motion aware.
They use the existing sky accent, soft omnidirectional glow, rounded geometry, and spring timing.

## Consequences

- A watcher can distinguish ordinary directional scrolling from arriving at an exact element.
- Image placement clearly identifies its destination without leaking image metadata or overstating
  the page's response.
- Semantic tools keep their richer target explanation without a visually noisy duplicate halo.
- Diagnostic reads remain honest and quiet.
- No MCP schema, Rust protocol type, policy rule, audit field, or action-signature medallion kind is
  added.
