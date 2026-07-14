# ADR-0072: Agent narration as a user-visible workflow channel

Date: 2026-07-13
Status: Accepted
Builds on: ADR-0012 (visible, faithful browser interaction), ADR-0034 (additive capability
registry), ADR-0035 (`script` composition), ADR-0050 (`browser_batch`), and the normative visual
language in `docs/design/visual-language.md`. Preserves ADR-0005 (the extension is mechanism only)
and ADR-0007's freeze over the 13 trained schemas.

Amendment 2026-07-13 (implementation correction): D3 originally classified narration as RAWX
Action because it changes the local visual surface. The owner corrected that classification before
implementation: narration does nothing to page content and therefore requires no RAWX capability.
It is domainless (`requires: []`, `ResourceShape::DomainLess`). This amendment preserves ordinary
schema validation, tab ownership, take-the-wheel, sacred-tab defense in depth, dispatch, audit, and
script/batch correlation. It removes grant-resource domain resolution, grants, and session-overlay
authorization from the tool because none of those page-content authorities are exercised. The
always-on sacred-tab check may still resolve the tab host and seed it as audit context; that host is
not a governing resource and does not change the RAWX-none classification.

Amendment 2026-07-13 (responsive edge ribbons): live review found the first narration card too
small to read as a video caption. The `position` vocabulary in D1 is now `auto`, `top`, and
`bottom`, with `auto` as the default; the unshipped `center` option is removed. Auto placement
chooses one edge when the narration appears, preferring the edge away from a recent touched or
focused control, pointer position, and scroll direction. It does not move an active narration as
those signals change. D4's compact card becomes a wide responsive edge ribbon with viewport-bounded
type, spacing, and height. Short messages receive a taller chapter treatment through product-owned
styling, not a model parameter. The governance notification remains a separate, full-width central
ribbon with visual priority; its height, badge, type, padding, and wrapping now respond within firm
minimum and maximum bounds. Governance text wraps rather than truncates. These changes preserve
the two layers' separate authority, pointer and capture behavior, deterministic replacement, and
the ban on arbitrary styling parameters.

## Context

Ghostlight already explains low-level actions visually. A watcher can see the phantom cursor,
click ripples, field splashes, read scans, navigation pills, and optional action captions. It also
has a persistent notification ribbon through which governance explains a denial. Those surfaces
answer two different questions:

- action effects answer "what just happened?";
- the governance ribbon answers "which guardrail acted, and why?".

Neither lets an agent explain the intent of a longer workflow in plain language. That gap is most
visible in `script` and `browser_batch`: a sequence can be correct and visibly active while the
person watching still has to infer why it moved from one phase to the next. The same gap makes a
product demo need an external subtitle track even though the product already knows the story.

The missing primitive is deliberate agent-to-human narration: a short, temporary sentence such as
"Checking the result before making changes" or "Filling the release form". It is useful beyond a
demo. It makes long-running automation legible, gives composed workflows natural chapter breaks,
and lets an agent communicate intent without writing into the page it is operating.

This channel must not borrow the governance ribbon. That ribbon speaks with policy authority and
is intentionally exempt from ordinary visual-effect controls. If an agent could render the same
treatment, a user could no longer tell a guardrail from commentary.

## Decision

### D1. `narrate` is a first-class additive browser tool

Add one advertised tool named `narrate` through the ADR-0034 browser capability registry. It is
additive: the names, descriptions, schemas, parameter order, and enum order of the 13 trained tools
do not change.

The model-facing description is:

```text
Show a short, temporary narration card in the controlled browser tab so the person watching
understands the current workflow phase. Use it for meaningful phase changes, not routine clicks or
keystrokes. A new narration replaces the current one.
```

Its input schema is:

```json
{
  "type": "object",
  "properties": {
    "tabId": {
      "type": "number",
      "description": "Tab ID in which to show the narration. Must be a tab owned by this session."
    },
    "text": {
      "type": "string",
      "minLength": 1,
      "maxLength": 240,
      "description": "One short, user-visible sentence describing the current workflow phase."
    },
    "position": {
      "type": "string",
      "enum": ["top", "center", "bottom"],
      "default": "bottom",
      "description": "Where to place the narration card. Defaults to bottom."
    },
    "duration_ms": {
      "type": "integer",
      "minimum": 1000,
      "maximum": 30000,
      "default": 5000,
      "description": "How long to show the narration, in milliseconds. Defaults to 5000."
    }
  },
  "required": ["tabId", "text"],
  "additionalProperties": false
}
```

There is no arbitrary style, color, icon, HTML, Markdown, voice, or persistence parameter. The
visual language stays a product decision rather than becoming model-generated UI.

### D2. Replacement and timer behavior are deterministic

At most one narration is active per tab.

- A successful call replaces the active narration immediately, even if it uses another position.
- Duration starts when the extension renders the card, not when the MCP request enters the queue.
- Each rendered message gets an internal generation id. An expired timer removes the card only if
  its generation is still current, so an old timer can never dismiss a replacement.
- A narration disappears when its timer expires, the controlling session ends, the user invokes
  the panic control, or the tab closes.
- Navigation does not silently lose a current narration. The policy-free service worker retains
  only `{generation, text, position, deadline}` for the tab and replays it into the new document
  with the remaining duration. Expired state is discarded rather than replayed.
- No narration history is retained. State is memory-only and is never synced, uploaded, or written
  into extension storage.

The tool returns an ordinary successful result confirming whether the card was shown and its
effective duration. A user-disabled visual layer is reported truthfully as not shown; it is not
reported as a successful display.

### D3. Narration requires no RAWX capability and follows the normal chokepoint

`narrate` is classified as RAWX none. It changes Ghostlight's local, pointer-transparent visual
surface but does not read, act on, write to, or execute in page content, and sends no data to the
page's server.

- Resource shape is domainless and the registry requirement is empty. It does not resolve a page
  domain or consume grants and session overlays.
- Schema validation, take-the-wheel, tab ownership, the argument-driven sacred-tab defense,
  ordinary timeout/error handling, and audit still run through the shared pipeline.
- A session cannot narrate into another session's tab.
- The extension receives a mechanism message only after the binary authorizes the call. It makes
  no policy decision and contains no grant or allowlist logic (ADR-0005).
- The normal audit record names the `narrate` tool with capability `none`, no grant attribution,
  its decision, and orchestration correlation. It may include the contextual tab host resolved by
  the sacred-tab pre-check. In keeping with the existing audit-minimization rule, the narration
  text is not copied into the audit record.

`narrate` must not reuse the current unlisted `notify` path. `notify` is the direct mechanism used
by governance to render an authoritative ribbon and is intentionally absent from `tools/list`.
`narrate` is an ordinary advertised, audited tool with a visibly different treatment.

### D4. The visual treatment is commentary, never authority

Narration joins `docs/design/visual-language.md` as its own vocabulary row and is implemented in
the existing policy-free visual layer.

- Default placement is bottom-center. `top` sits below any governance ribbon; `bottom` sits above
  action-caption and keystroke lanes. `center` uses a safe viewport inset. Governance always has
  visual priority and narration must move or yield rather than cover it.
- The card uses Ghostlight's sky-blue agent accent, neutral translucent chrome, rounded geometry,
  a small explicit "Agent" label, and a thin remaining-time indicator. It does not use severity
  colors, the lock/shield medallion, or the notification ribbon's full-width shape.
- Replacement cross-fades without stacking cards. Entry and exit honor `prefers-reduced-motion`.
- The entire card is `pointer-events:none`; there is no close button or interactive control.
- Text is assigned through `textContent`, never `innerHTML`, and is constrained by the schema.
- The element is `ghostlight-` prefixed, excluded from `read_page` and `find`, and hidden during
  model screenshot/zoom capture. A person and an out-of-band recorder such as OBS still see it.
- Narration respects the user's visual-effects control. Governance notifications remain exempt;
  agent commentary does not.

The renderer may share layout primitives with action captions, but narration is not an expanded
action caption. Action captions are mechanically derived labels such as "Click" or "Read page";
narration is semantic text deliberately supplied by the agent.

### D5. Composition needs no special execution path

`narrate` is a legal step inside `script` and `browser_batch`. It enters the same registry,
authorization, dispatch, result, and audit path as a standalone call. The batch id and step number
therefore correlate narration with the work it explains without adding a second composition
mechanism.

Agent guidance says to narrate phase boundaries, user-relevant waits, and meaningful transitions.
It explicitly discourages narration for each click, key, ref lookup, or other low-level action.
Composed workflows remain useful when narration is disabled; narration is presentation, never a
control dependency or a source of data for later steps.

### D6. The public demo adopts narration after the tool ships

`ghostlight demo` will narrate the purpose of each section before driving it. The intended story is
short and factual:

1. Ghostlight works in the browser session the person already uses.
2. The agent can point, click, and type visibly.
3. Structured tools can complete a form.
4. The agent can inspect console and network signals.
5. It can read page content without moving the session elsewhere.
6. Policy still decides where the agent may go.

The final narration appears while the tab is still on the granted demo domain. The subsequent
off-domain attempt produces the real governance ribbon, not a simulated narration. Keeping the two
treatments visible and semantically distinct is part of the demo's acceptance test.

### D7. Text-to-speech is deliberately deferred

V1 is visual only. It does not call browser `speechSynthesis`, play audio, enumerate voices, or add
a `voice`/`speak` parameter.

Text-to-speech adds user-consent, sensitive-text, voice-availability, language, interruption,
accessibility, page-audio coexistence, and recording-audio questions. It can also speak private
workflow context aloud in a shared physical environment. Those costs are not required to validate
the narration primitive.

TTS may return in a separate ADR after all of these exist: an explicit user-facing opt-in, a clear
local-only implementation, deterministic queue/cancel semantics, accessibility review, and live
tests across supported browsers and operating systems. If exposed to the model, prefer a separate
`speak` tool over broadening `narrate` with ambiguous audio behavior.

## Acceptance criteria

Implementation is complete only when:

1. `narrate` is advertised through the registry and the fidelity snapshot grows additively; the 13
   trained schemas remain byte-stable.
2. Schema validation covers required fields, enum values, text length, and duration bounds with
   corrective errors and a valid example.
3. Tests pin RAWX none and domainless classification, and prove tab ownership, sacred-tab defense,
   take-the-wheel, audit, and script/batch behavior without grant or session-overlay consumption.
4. Extension tests prove replacement, stale-timer immunity, timeout removal, navigation replay
   with remaining duration, tab/session/panic cleanup, and the user-disabled response.
5. Visual tests or pinned DOM/CSS assertions prove pointer transparency, `textContent` insertion,
   reduced-motion behavior, capture hiding, read/find exclusion, and non-collision with governance.
6. The visual-language document and interactive dictionary gain the narration treatment.
7. `ghostlight demo` uses narration at section boundaries and a live rehearsal confirms that the
   final policy denial remains visually authoritative and distinct.
8. The Chrome Web Store promotional recording is made only after that rehearsal, so the shipped
   behavior and the public video agree.

## Consequences

- Long scripts gain a quiet, explicit way to tell the watcher what phase they are in.
- The public demo can explain itself from inside the product instead of depending on edited-in
  captions or a sales voice-over.
- The advertised tool surface grows by one, with corresponding registry, fidelity, test, and
  documentation work.
- The extension gains small per-tab transient state and navigation replay logic, but no policy,
  persistence, network behavior, or telemetry.
- An agent can produce annoying commentary if prompted poorly. The short schema, visual-effects
  control, replacement semantics, and guidance against low-level narration bound that cost.
- Governance retains a unique visual and protocol path. Users can still tell "the agent says" from
  "the guardrail says" at a glance.

## Rejected alternatives

- **Reuse `notify`.** Rejected because it lets agent commentary impersonate policy authority.
- **Extend action captions with arbitrary text.** Rejected because mechanical action labels and
  semantic workflow intent have different authors, lifecycles, and trust meaning.
- **Write captions into the page.** Rejected because it mutates page content, can be observed by
  page scripts, and may affect layout or form state.
- **Arbitrary HTML/Markdown/style parameters.** Rejected because they create injection and visual
  spoofing surfaces while making Ghostlight's vocabulary inconsistent.
- **Persistent narration log.** Rejected because audit already records tool occurrence and a text
  history would retain potentially sensitive workflow context without a demonstrated need.
- **TTS in v1.** Deferred by D7.

## Provenance

Owner proposal and acceptance, 2026-07-13, while rehearsing the Chrome Web Store promotional demo:
add a `narrate` tool that displays a positioned, timed ribbon; a message remains until its timer
expires or another narration replaces it; pair it with scripts/batches to explain "now doing
this" phase changes; use it as the demo's own caption track; consider browser TTS as an alternative.

## Amendment: compact activity treatment (2026-07-14)

ADR-0079 replaces the original wide narration-card treatment. Narration is a compact caption with
one transient three-dot activity cue and no progress bar. This changes presentation only. The tool
contract, trust distinction, timing, replacement, navigation replay, audit, and capture-hiding
decisions above remain authoritative.
