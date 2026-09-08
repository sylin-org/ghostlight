# Ghostlight 1.0 model-facing language

## Contract rules

The 23 tools below are the complete 1.0 catalog. Input objects and nested objects set
`additionalProperties` to `false`. Every input schema is a top-level object without root-level
`oneOf`, `allOf`, or `anyOf`, because current Kiro and Bedrock reject those otherwise valid JSON
Schema forms. Conditional inputs advertise one portable teaching envelope; the typed decoder
enforces the exact branch before governance or browser dispatch. Omitted optional fields use the
defaults stated here.

Each declaration includes concise field descriptions, one shortest valid example, a truthful
output schema, and standard MCP annotations. The implementation owns bounds and defaults as named
constants shared by schema rendering and decoding.

`browser_flow` is the single composition tool for one to twenty ordinary tool calls. Step IDs
are optional for short batches and available for explicit result references. Each child uses the
invocation's immutable configured authority snapshot and the ordinary executor. `on_error` defaults
to `stop`. Captured result payloads have a bounded total budget; progress remains explicit.

Authority comes from configured managed/local policy and human controls. Tools do not accept
`restrict_hosts`, `restrict_capabilities`, or `browser_flow.dry_run`. The retired `browser_sequence`
tool is replaced by ordinary tool steps in `browser_flow`. Obsolete calls fail before browser work
with catalog-refresh guidance; nothing retries automatically. Existing history remains readable.
See [ADR-0162](../adr/0162-configured-authority-and-one-flow-tool.md).

An optional `tab` selects an opaque controlled tab. Omission selects the only controlled tab, or
the sole active controlled tab when ownership is unambiguous. Otherwise the call is rejected and
points to `browser_tabs`. Tab handles are durable correlation slots: navigating by a handle whose
tab has closed recreates that tab through the governed open path under the same handle and says
so in the summary. Closing a handle whose tab is already gone succeeds without touching the
browser. Target handles are different by design -- they are perception, not identity. A target
handle is tied to one tab and document generation, a committed navigation makes prior target
handles stale, and acting on one forces re-inspection of a page the model has actually seen.
Typed semantic selectors (name, optional role, optional exact match) are accepted wherever target
handles are, including `selector_present` waits, so drivers can prefer what a control is called
over stashing handles.

View handles are returned by screenshots. They bind rendered coordinates to one tab, document
generation, viewport origin, viewport size, device scale, and zoom. Coordinate input is rejected
when that binding is no longer current. Coordinates use the returned image dimensions, with the
top-left pixel at `(0, 0)`. A bounded region screenshot returns a new view with its own transform,
so another region can be selected from the magnified image.

Timeouts are bounded from 100 to 30000 milliseconds and default to 8000 milliseconds. Text is
UTF-8 and bounded. URLs must be absolute `http` or `https` URLs.

Ordinary bursts may wait within the original invocation deadline. Sustained waiting is visible
in the human workbench; it does not create a new tool call or a popup. An exceptional admission
capacity refusal returns `reason: capacity`, no effect, and safe guidance for a request that never
started. Human Pause/Stop preserves its fixed directive and terminal history without repeated
guardrail popups while queued work drains (ADR-0160).

## Result envelope

Every invocation returns one envelope:

| Field | Type | Meaning |
| --- | --- | --- |
| `invocation` | string | Opaque correlation handle. |
| `status` | enum | `succeeded`, `blocked`, `failed`, `cancelled`, `attention_required`, or `unknown`. |
| `effect` | enum | `none`, `applied`, `partial`, or `unknown`. |
| `readiness` | enum | `not_applicable`, `loading`, `interactive`, `complete`, or `unknown`. |
| `repeat_safe` | boolean | Whether repeating the same call is known safe. |
| `summary` | string | Bounded Ghostlight-authored explanation. |
| `facts` | object | Tool-specific canonical facts. |
| `next_steps` | string array | Zero to two Ghostlight-authored safe suggestions. |

The MCP edge renders the complete envelope twice for client compatibility: `structuredContent`
retains the machine-readable object, and the ordinary text block contains the authored summary and
safe next steps followed by compact JSON for the same opaque envelope. A client that ignores
`structuredContent` therefore still receives every canonical fact. Bounded rich content crosses
the bridge in a separate generic content vocabulary. `browser_screenshot` returns an image block.
`browser_record` save returns a GIF image block only when the caller asked for the replay itself;
a save that stays inside the browser returns none. Image bytes are not copied into structured
facts or the textual JSON projection.

The language context produces `summary`, `next_steps`, and the content-minimized observation
projection from one typed outcome or refusal. A sentence that names a host, count, or capture size
carries the same value in that projection. Ghostlight owns the sentence. The browser may return the
role and accessible name of the physical element in the same action receipt, without a describe
round trip. The role is narrowed to a closed Ghostlight noun; an unknown role becomes `control`.

The name is normalized, bounded to 80 visible characters, and included by default, so an action can
say `Clicked the "Save" button on example.com.` Governance may remove all target names with
`privacy.preserve_target_names: false`, leaving `Clicked a button on example.com.` Editable values are
never name sources. A result with `effect` equal to `partial` or `unknown`, or with a committed effect
unsafe to duplicate, has `repeat_safe: false` and does not suggest replay.

The same language owner supplies a separate typed audit projection. Readable explanations,
measurements, and governed target names survive; arbitrary browser details and result facts do
not enter retained history. A permitted client result may still contain those details. Expanding
existing history does not authorize more capture. Additional retention profiles and richer
diagnostic capture remain deferred (ADR-0103 H1 amendment).

When no browser is connected and startup is left to the person -- because `browser.startup` is
`manual`, or because more than one installed browser could serve and Ghostlight does not choose
where to direct attention -- the refusal addresses the MCP model: ask the user to open one of the
eligible installed browser windows it names, with the Ghostlight extension installed, then repeat
the call. Facts carry the closed `browser_startup_manual` reason and a `browsers` array; one
choice also retains the singular `browser` fact. The summary contains the whole recovery
instruction, so `next_steps` is empty. Stale Ghostlight-owned registrations among several
installed browsers are repaired silently first, so the named browsers can actually connect; when
no installed browser has a usable registration, the refusal names the browsers found and one
choice-free remedy instead. No browser refusal ever says that Ghostlight declined to choose.
When the registration names a connector belonging to a different Ghostlight installation,
recovery changes nothing and answers `Another Ghostlight installation owns the browser
registration.` with the `native_host_owned_elsewhere` fact and one next step: the user runs
`ghostlight install` from the installation that should own the browsers. Silent repair applies
only to stale details within the running installation's own directory (ADR-0149 amendment).

## Catalog

### `browser_tabs`

List, focus, or close controlled tabs. Actions are:

- `list`: no `tab`; shortest call `{"action":"list"}`; capability `read`. The list is read live
  from the connected browser on every call and names only this workspace's bound tabs, so it
  requires a connected browser and refuses without one.
- `focus`: required `tab`; no RAWX capability.
- `close`: required exact `tab`; capability `action` and the tab-close policy constraint.

Close also respects the browser's local preserve-tabs interlock. Facts for list contain `tabs`,
each with `tab`, bounded `title`, governed `url`, `active`, and `readiness`. Focus facts include
`tab`, `active`, and `window_focused`. Close facts include `tab` and `closed`.

### `browser_navigate`

Navigate to a governed URL. Shortest call: `{"url":"https://example.com"}`.

Inputs: required `url`; optional `tab`; optional `new_tab`, default `false`; optional `reuse` of
`domain` or `never`, default `domain`; optional `beforeunload` whose only value is `discard`,
accepting just that navigation's own unsaved-change prompt; optional `timeout_ms`. `tab` and `new_tab:true` cannot be combined, and `reuse` cannot be combined with
`new_tab`. Without `beforeunload`, a blocking prompt stops the navigation and is reported, never
accepted.

With `new_tab:true`, Ghostlight creates and navigates a new controlled tab. With `tab`, it
navigates that exact tab. With neither, it uses the unambiguous controlled tab, and when none
exists it opens one -- adopting an existing unbound same-host tab (exact URL preferred) unless
`reuse:"never"` asks for a strictly fresh tab (ADR-0137). A reused open says so: the summary
reads "Reused the example.com tab." rather than "Opened example.com." Capability: `read`.

Facts: `tab`, governed `url`, bounded `title`, `created`, `reused`, and `document_generation`.

### `browser_history`

Move through history or reload. Shortest call: `{"action":"back"}`.

Inputs: required `action` of `back`, `forward`, or `reload`; optional `tab`; optional
`timeout_ms`; optional `bypass_cache`, default `false`, valid only for reload. Capability: `action`.

Facts: `tab`, `action`, governed `url`, bounded `title`, and `document_generation`.

### `browser_window`

Set tab zoom or resize the containing browser window.

- Zoom: `{"action":"zoom","percent":100}`. `percent` is an integer from 25 to 500;
  optional `tab`; capability `read`.
- Resize: `{"action":"resize","width":1280,"height":800}`. Required integer `width` from
  320 to 7680 and `height` from 240 to 4320; optional `tab`; no RAWX capability.

Resize affects every tab in the window and may rerender the page. Either action invalidates a
current view handle when its bound geometry no longer matches. Facts include the selected tab,
action, requested dimensions or zoom, and observed geometry.

### `browser_read`

Read useful bounded visible text from the full composed page or one target. A full-page read
includes visible top-document text, open shadow trees, and http(s) embedded frames in stable frame
order. Closed shadow roots, hidden content, and editable values remain absent. Use
`browser_inspect` or `browser_find` when an action target is needed. Shortest call: `{}`.

Inputs: optional `tab`; optional `target`; optional `mode` of `visible` or `article` (`visible` is
the default; explicit `article` prefers a useful article in the top document and falls back to the
same full-page visible read; ignored with `target`); optional `max_chars` from 500 to 50000, default
8000. Capability: `read`.

Facts: `tab`, governed `url`, bounded `title`, `text`, `truncated`, and
`document_generation`.

### `browser_inspect`

Inspect semantic controls or page structure and return fresh target handles. Shortest call: `{}`.

Inputs: optional `tab`; optional `scope` of `controls`, `structure`, `all`, or `document`, default
`controls`; with `document`, an optional bounded subtree `root` handle and `max_depth` from 1 to 12;
optional `max_items` from 1 to 200, default 80. Capability: `read`.

Facts: `tab`, `document_generation`, and `items`. Each item has a target handle, semantic role,
bounded accessible name, state, and credential-class flag. Selectors are not exposed. A
`document` scope returns one bounded structure-only composed tree, records it under a snapshot
handle that is superseded per tab, and reports a structural diff against the current prior
snapshot when one exists. Without a root, the tree includes open shadow roots, assigned slots, and
http(s) embedded frames in stable order under one 400-node ceiling. A root limits it to that
target's composed subtree. Editable values, hidden content, closed roots, and frame identity are
never returned.

### `browser_find`

Find current semantic targets by visible or accessible text across the composed page, including
open shadow roots and http(s) embedded frames. Use it when the desired label or text is known.
Shortest call: `{"text":"Submit"}`.

Inputs: required non-empty `text`; optional `tab`; optional `scope` of `any`, `control`, or `text`,
default `any`; optional `max_results` from 1 to 50, default 20. Capability:
`read`.

Facts: `tab`, `document_generation`, and bounded ranked `matches` with target, role, name, and
state.

### `browser_screenshot`

Capture the viewport, full page, one target, or a magnified region from a current view. Every
capture returns a view handle for later coordinate actions or another region capture. Shortest
call: `{}`.

Inputs use one of four schema branches: optional `tab` only for viewport capture; optional `tab`
plus required `full_page:true`; optional `tab` plus required `target`; or optional `tab` plus
required `view`, `x`, `y`, `width`, and `height`. Region coordinates are image pixels and must form
a positive rectangle wholly inside the current view. Optional `timeout_ms` apply
to every branch. Target, full-page, and region capture cannot be combined. Capability: `read`.

Facts: `tab`, `view`, `mime_type`, `width`, and `height`, plus one bounded MCP image content block.

### `browser_click`

Click a current semantic target or a point in a current screenshot. Shortest call:
`{"target":"target_..."}`.

Inputs use exactly one location branch: required `target`, typed semantic `selector`, or required
`view`, `x`, and `y`.
Optional `tab`; optional `button` of `primary`, `middle`, or `secondary`, default `primary`;
optional `click_count` from 1 to 3 for single, double, or triple, default 1; optional `timeout_ms`;
optional `expect` postcondition. Capabilities: `action`, plus `read` when
`selector` or `expect` is supplied.

Facts: `tab`, optional `target`, optional `view`, `activated`, and any governed committed landing.

### `browser_scroll`

Scroll in a direction or reveal a semantic target. Shortest call: `{}`, which scrolls down by a
medium amount.

Inputs use one of three branches: required `target` to reveal; or optional `tab`, optional
`direction` of `up`, `down`, `left`, or `right` defaulting to `down`, and optional `amount` of
`small`, `medium`, `large`, or `page` defaulting to `medium`; or coordinate wheel input with
required `view`, `x`, `y`, and `ticks` from 1 to 10 plus a two-way `direction` of `up` or `down`.
Optional `timeout_ms` apply to every branch. Capability: `read`.

Facts: `tab`, optional `target`, `scrolled`, and observed horizontal and vertical offsets.

### `browser_hover`

Hover a current semantic target or a point in a current screenshot. Shortest call:
`{"target":"target_..."}`.

Inputs use exactly one location branch: required `target`, or required `view`, `x`, and `y`;
optional `tab`; optional `timeout_ms`. Capability: `read`.

Facts: `tab`, optional `target`, optional `view`, and `hovered`.

### `browser_fill_form`

Fill one or more ordinary controls. It does not submit unless `submit_target` is present. Use
`browser_type_text` when per-character input events matter. Shortest call:
`{"fields":[{"target":"target_...","value":"Ada"}]}`.

Inputs: required `fields` array of 1 to 30 typo-closed objects, each with required `value` and
exactly one location (`target` or typed semantic `selector`); a value is a bounded string, a boolean
for checkboxes and radios, or a finite number for numeric inputs; optional `tab`; optional
`submit_target`; optional `timeout_ms`; optional `expect` postcondition.
Capabilities: `read + write` without submit and `read + write + action` with `submit_target`.

Rich-text controls use the browser's editing transaction so controlled editors can retain the
replacement. Empty values clear only the named editor. Filling never activates a submit control
unless the caller supplied `submit_target`.

Semantic selectors and target handles can address the same ordinary controls, including editors
outside an HTML `form`. The selector does not impose an unadvertised form-ancestry requirement.
An explicit `submit_target` is still checked against the first resolved field's containing form.

Credential-class targets stop before any value dispatch and request visible user handoff. Facts:
`tab`, `filled_count`, `submitted`, and any governed committed landing.

### `browser_type_text`

Type ordinary text through browser input events. Shortest call:
`{"target":"target_...","text":"Ada"}`.

Inputs use one location: `target` with bounded `text`; or `selector` with `text`; or
`focused:true` to type into the currently focused editable control. Optional `clear_first`, default
`false`; optional `tab`; optional `timeout_ms`. Empty text is valid only as an
explicit clear together with `clear_first:true`. Optional `expect` adds a postcondition.
Capabilities: `action`, plus `read` when `selector` or `expect` is supplied. Targeted and focused
typing without a postcondition require `action`.

Credential-class targets stop before text dispatch. Facts: `tab`, `target`, `typed`,
`character_count`, and any governed committed landing.

### `browser_press_key`

Send one explicit keyboard action. Shortest call: `{"key":"Enter"}`.

Inputs: exactly one of required `key` as one character or one named key from the closed list, or
required `strokes`, an ordered sequence of 1 to 20 of the same items, with optional `repeat` from 1
to 100 defaulting to 1; optional `tab`; optional `target`; optional unique `modifiers` from `Alt`,
`Control`, `Meta`, and `Shift`; optional `expect` postcondition.
Capabilities: `action`, plus `read` when `expect` is supplied.

Facts: `tab`, `key`, `pressed`, and any governed committed landing.

The optional `expect` on click, fill, type, and key calls uses one condition: `load_ready` with no
value, or `url_contains`, `text_present`, or `text_absent` with a required non-empty `value` of at
most 2,000 characters. Lookup and postcondition Read requirements are admitted with the complete
request before an effect. A later observation failure preserves the effect already applied.

### `browser_drag`

Drag one semantic target to another, or drag between two points in a current screenshot. Shortest
call: `{"source_target":"target_...","destination_target":"target_..."}`.

Inputs use exactly one schema branch: required `source_target` and `destination_target`; or
required `view`, `start_x`, `start_y`, `end_x`, and `end_y`. Optional `tab`, `timeout_ms` apply to both. Capability: `action`.

Facts: `tab`, `dragged`, and any governed committed landing.

### `browser_wait`

Wait for one explicit observable condition. Shortest call: `{"condition":"load_ready"}`.

Inputs use one condition-specific branch: `load_ready` accepts neither value nor target;
`url_contains`, `text_present`, and `text_absent` require `value`; `target_present` and
`target_absent` require `target`; `selector_present` requires a typed `selector` and polls the
live page until a control matching it exists; `duration` requires a decimal string `value`
of whole milliseconds from 0 to 10000 (for example, `"1200"`) and waits executor-side.
Every branch accepts optional `tab`, `timeout_ms`. Capability: `read`.

Text conditions match composed visible text across the top document, open shadow roots, assigned
slots, and http(s) embedded frames. Hidden content, editable values, and closed roots remain absent.

Facts: `tab`, `condition`, `satisfied`, `elapsed_ms`, and governed readiness.

### `browser_dialog`

Inspect or resolve the current JavaScript dialog.

- `{"action":"status"}` reports whether a dialog is blocking; capability `read`.
- `{"action":"accept"}` accepts it; capability `action`.
- `{"action":"dismiss"}` dismisses it; capability `action`.
- `{"action":"respond","text":"Ada"}` supplies non-secret prompt text; capability `action`.

All branches accept optional `tab`. `text` is required only for `respond` and is
invalid for every other action. Facts: `tab`, `dialog_type`, `present`, `accepted`, and `handled`
as applicable. Dialog text is never audited.

### `browser_upload`

Attach explicitly supplied files or one captured image to one ordinary file input, or drop one
captured image at a point in a current view. Shortest call:
`{"target":"target_...","paths":["C:\\path\\document.pdf"]}`.

Inputs: exactly one source of required `paths`, an array of 1 to 5 unique absolute local paths;
or `files`, 1 to 5 inline objects with `name` and base64 `data_base64`; or one `source_image`
handle from an earlier capture, optionally with `view`, `x`, and `y` to drop it at a point instead
of attaching. The destination is optional `target` or typed semantic `selector`. Optional `tab`,
`timeout_ms`. Capabilities: `write`, plus `read` when `selector` is supplied.
Target-handle attachment and current-view image drops require `write`.

As with form fill, a semantic selector can address an ordinary file input outside an HTML `form`.

Ghostlight rejects directories, missing files, any file larger than 5,000,000 bytes, and a combined
payload larger than 5,000,000 bytes before browser dispatch, and refuses an upload above the
capture-reuse ceiling. Inline bytes decode only after authorization and credential preflight. File
paths, names, and contents never enter audit or presentation. Facts: `tab`, `target`,
`uploaded_count`, and `uploaded_bytes`.

### `browser_execute`

Execute explicit bounded JavaScript in the page. It may read, mutate, or navigate, so use a
semantic tool when one fits. Shortest call: `{"script":"document.title"}`.

Inputs: required non-empty `script` up to 20000 characters; optional `tab`; optional
`max_result_chars` from 100 to 20000, default 8000; optional `timeout_ms`.
Capability: `execute`.

The adapter parses the source before dispatch and selects the async form for a top-level return.
Ordinary scripts retain REPL scope, including top-level await and repeated declarations. A local
syntax rejection sends no page evaluation. Browser evaluation is sent once; a runtime exception
or lost reply remains uncertain regardless of its text or exception class.

Facts: `tab`, `value`, `truncated`, and any governed committed landing. Script source and result
never enter audit or presentation.

### `browser_flow`

Compose one to twenty ordinary tool calls. A short batch:
`{"steps":[{"tool":"browser_click","arguments":{"target":"target_..."}},{"tool":"browser_wait","arguments":{"condition":"load_ready"}}]}`.

Inputs: required `steps` array of 1 to 20 objects. Each step has a required `tool` naming a current
advertised non-composite tool, optional bounded `id`, and optional `arguments` defaulting to `{}`.
Omitted IDs become `step_1`, `step_2`, and so on; all IDs must be unique. Optional `on_error` is
`stop` or `continue`, default `stop`. Optional `tab` supplies a default for tab-scoped children;
explicit child tabs win, new-tab navigation and tab-independent work keep their ordinary meaning.
Optional `timeout_ms` bounds the entire flow. There is no simulation mode.

The wrapper requires no RAWX capability. Every child classifies and admits independently under
the same immutable configured authority snapshot. Each attempted child records a safe receipt
before parent completion. History groups these under one expandable parent, with distinct input
preparation and missing receipt states. Payloads and caller labels stay out of audit (ADR-0156).

Any argument value may be an explicit reference object,
`{"flow_ref":{"step":"earlier_id","pointer":"/facts/..."}}`, resolved from that step's canonical
result envelope by JSON Pointer before the ordinary child decoder runs on the substituted
arguments. A reference that does not resolve fails its step without effect.

With `on_error: stop`, a child execution, argument-decoding, or reference-resolution failure ends
the flow before the next step. Effects from earlier steps remain applied; stopping does not roll
them back. Explicit `continue` permits later independent steps to run and still reports a
non-success when any child fails. Human pause/stop, attention, cancellation, and deadlines end
execution even under Continue.

Captured per-step envelopes stop being recorded past a bounded byte budget while execution continues to a truthful
terminal aggregate. Facts: `completed`, `total`, `stopped`, `progress`, and bounded per-step rows
with each step's envelope where the budget allowed. `completed` counts successes. References still
resolve against the full volatile envelope when its client payload is omitted.

Flow uses this progress contract; historical sequence receipts retain it:

- `progress.counts`: `total`, `succeeded`, `failed`, `blocked`, `cancelled`, `attention_required`,
  `unknown`, `not_started`, and `not_run`. The categories are disjoint and sum to `total`.
  `not_started` means runtime reference resolution or decoding prevented entering a child;
  `not_run` means execution never reached it. Neither fabricates a failed child invocation.
- `progress.effects`: counts of entered children with `none`, `applied`, `partial`, and `unknown`
  effects. A later unknown never removes earlier known applied or partial effects.
- `progress.stopped` says execution ended at a stopping boundary. `progress.issue`, when present,
  names a one-based step and closed cause relevant to recovery. Human directives take precedence
  over attention/invocation limits, then uncertainty, then ordinary failures.
- Every step row keeps its one-based position, status, effect, and repeat safety. Entered rows
  carry their closed cause when relevant; unresolved inputs also retain their client error.
  Omitted payloads cannot remove this metadata. Not-run rows contain no invented child result.
- Aggregate status remains in the ordinary vocabulary. Unknown effects/status take precedence,
  then attention, cancellation, blocks, and failure. Success requires every step to succeed.
  Aggregate effect is unknown if any effect is unknown; otherwise a partial child or applied
  effect with incomplete work is partial. Fully successful applied work remains applied.
- Repetition is safe only when every child succeeded, every child permits repetition, and there
  were no effects. Recovery respects confirmed changes and never suggests replaying the whole
  composition. Pinned human pause/stop directives remain intact.

Examples: `Completed all 5 steps.`, `Completed 3 of 5 steps. 2 failed.`, and
`Completed 2 of 5 steps. Step 3 could not start.` H1 audit may retain the same payload-free
progress as `composition`, without child ids, results, target handles, or error text.

### `browser_record`

Create a short memory-only GIF of browser work. Usual flow:
`{"action":"start"}`, ordinary browser calls, then `{"action":"save"}`.

Actions are:

- `start`: optional `tab`; ask the extension to start an owned recording; capability `read`.
- `status`: optional `recording`; report state and deadlines; no new capability.
- `stop`: optional `recording`; capture a final frame, stop, and freeze; no new capability.
- `save`: optional `recording`; auto-stop if active. One replay goes to one place: with `target`,
  the browser attaches it to that file input and Ghostlight requires `read + write`; with
  `"download": true`, the browser saves it as a file and Ghostlight requires `read`; with neither,
  the GIF is returned to the client and Ghostlight requires `read`. `target` and `download`
  together are refused.
- `discard`: optional `recording`; erase captured bytes; no new capability.

Save checks its complete capability requirement before stopping active capture. Read still
authorizes every captured source; Write separately authorizes the attachment destination.

`recording` may be omitted only when exactly one owned recording can be resolved. `target` and
`download` are valid only for save. The extension owns recording identity, frames, bounds,
deadlines, stop, retention, erase, and the encode. Frames never leave the browser, and neither
frames nor encoded bytes are written to Ghostlight storage, extension storage, logs, audit, or
restart state. Save can be repeated until retention expires; a target or download delivery is a
real effect each time and is not thereby repeat-safe. A save after retention expires is a refusal,
not an empty result. Discard is destructive.

A saved replay's sentence says how long it plays and where it went, because that is what someone
who asked for a recording wants to know. How many frames survived, how many were captured, and how
many bytes they became are real and stay in the facts, alongside `recording`, state, deadlines,
stop reason, exact `duration_ms`, dimensions, and the delivery disposition. A client save returns one bounded
`image/gif` content block; the other two return none, and neither claims remote acceptance.

### `browser_diagnose`

Read bounded console and network evidence for a controlled tab. Tracking is opt-in. Shortest call:
`{}`, which selects both sources and returns problems only.

Inputs: optional `tab`; optional `source` of `both`, `console`, or `network`, default `both`;
optional `detail` of `problems` or `all`, default `problems`; optional case-insensitive literal
`match`; optional opaque `after` cursor; optional `limit` from 1 to 200, default 50. Capability: `read`.

Problems are console warnings, errors, exceptions, failed requests, and HTTP error responses.
All detail also includes ordinary console events and successful requests. The first call enables
bounded volatile observation and may contain no earlier evidence; reproduce or reload when needed.
Reads are non-destructive. There is no clear input.

Console text is length-bounded. Network facts exclude headers, bodies, cookies, authorization,
post data, query strings, and fragments. Results contain ordered bounded entries, an opaque next
cursor, truncation and eviction facts, and counts of host-filtered entries. Diagnostic evidence is
untrusted model-visible content. It is never policy input, audit payload, persistent storage, or
page presentation.

### `policy_explain`

Read the authority in force as one compiled answer: situation sentence, one line per capability
stating its polarity and deciding layer, the rules behind those lines, authored settings,
permanent ceilings, browser startup posture, organization identity, and passport provenance.
Available under every authority including all-open; use it to learn why another call was refused
or what is allowed before acting.

Inputs: none. Capability: empty requirement set (always available).
Read-only, never dispatches a browser, holds no workspace lease, and writes nothing.
It remains available during session attention, global Pause or Stop, and a required audit outage.
It does not clear attention or change controls. Cancellation, deadlines, bounded admission, and
the ordinary completion/audit path still apply.

The result carries the orchestrator's compiled projection -- the same compilation the workbench
Policy destination renders -- with layer document texts and filesystem paths withheld from model
results. The summary names its measurement: capability areas explained over layers in force.

## Session attention and human control (ADR-0157)

Configured observe-mode policy reports would-deny decisions without refusing ordinary work. A denial that reaches its session's
attention threshold retains the actual policy explanation and returns `attention_required`.
Further browser work in that session requires explicit human review/resume in the workbench; global Resume
leaves it intact. Model tools cannot resume attention. The triggering composition stops even under
Continue; recovery permits new requests and never replays prior steps.

Pause and Stop keep their fixed directives at the final dispatch check. A late refusal preserves
already acknowledged effects, including when it prevents a separate postcondition observation.
No-effect claims apply only to work that has not dispatched; lost effectful replies remain unknown.

## Document coverage (ADR-0158)

Scoped results carry `facts.coverage`: targeted versus whole-page scope, inspected/excluded/page
excluded/unavailable document counts, `limited_by_size`, and `masked_regions`. Counts retain
observed maxima across physical preparation and execution, not repeated-read sums. Policy
exclusions, unavailable content, and size ceilings have separate qualifications. Negative findings
are limited to inspected content; absence across unseen content is never established.

Human notice preferences do not remove coverage from model results. Excluded document hosts,
identities, URLs, labels, values, and locators are not copied into the model/audit coverage.
The workbench keeps bounded excluded host names only in volatile human details. A mask says
`Excluded by policy`. Unverifiable document access and recording stops at the document boundary
have language-owned outcomes. No script scanner or retry is treated as document containment.

## History storage (ADR-0159)

Every terminal envelope also carries `history_storage`: `saved` or `unconfirmed`. This describes
receipt storage, independently of status, effect, readiness, and repeat safety. An unconfirmed
receipt adds "History could not be saved." to the action's actual outcome. Composition facts and
safe parent audit metadata retain `unconfirmed_history_steps`; a saved parent cannot confirm its
children's storage. No audit error offers automatic replay.

`policy_explain` includes the effective `audit` choice and content-free `audit_health`. It remains
available during an audit outage. Filesystem paths and raw operating-system errors are excluded.
Require audit uses `audit_unavailable` with authored policy attribution and refuses new browser
work during a known failure. The same typed cause stops a composition under Continue, preserves
prior effects, and directs the client to prepare only unfinished work after recovery.
