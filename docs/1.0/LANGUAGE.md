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

### browser_flow

`browser_flow` composes one to twenty uniquely named steps. Each step names a current advertised
non-composite tool and supplies its argument object; any argument value may be an explicit
`{"flow_ref":{"step","pointer"}}` reference into an earlier step's canonical result envelope.
References resolve before the ordinary child decoder runs again on the substituted arguments.
Children classify and authorize normally under the invocation's immutable authority snapshot;
steps may not carry their own restriction fields. `on_error` defaults to `stop`, `dry_run` decodes
and classifies without dispatching, and captured step envelopes stop being recorded past a bounded
total budget while execution continues to a truthful terminal aggregate.

Every tool may accept these flat request restrictions:

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `restrict_hosts` | string array | absent | Allow only these host patterns for this invocation. |
| `restrict_capabilities` | string array | absent | Allow only the named capabilities for this invocation. |

Restrictions can only reduce configured authority. Capabilities are `read`, `action`, `write`,
and `execute`.

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

Inputs: required `url`; optional `tab`; optional `new_tab`, default `false`; optional
`beforeunload` whose only value is `discard`, accepting just that navigation's own unsaved-change
prompt; optional `timeout_ms`; optional restrictions. `tab` and `new_tab:true` cannot be combined.
Without `beforeunload`, a blocking prompt stops the navigation and is reported, never accepted.

With `new_tab:true`, Ghostlight creates and navigates a new controlled tab. With `tab`, it
navigates that exact tab. With neither, it uses the unambiguous controlled tab, creates one when
none exists, and rejects ambiguous selection. Capability: `read`.

Facts: `tab`, governed `url`, bounded `title`, `created`, and `document_generation`.

### `browser_history`

Move through history or reload. Shortest call: `{"action":"back"}`.

Inputs: required `action` of `back`, `forward`, or `reload`; optional `tab`; optional
`timeout_ms`; optional `bypass_cache`, default `false`, valid only for reload; optional
restrictions. Capability: `action`.

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

Read useful bounded prose from a page or target. Use `browser_inspect` or `browser_find` when an
action target is needed. Shortest call: `{}`.

Inputs: optional `tab`; optional `target`; optional `mode` of `article` or `visible` (article text
first, falling back to visible text; ignored with `target`); optional `max_chars` from 500 to 50000,
default 8000; optional restrictions. Capability: `read`.

Facts: `tab`, governed `url`, bounded `title`, `text`, `truncated`, and
`document_generation`.

### `browser_inspect`

Inspect semantic controls or page structure and return fresh target handles. Shortest call: `{}`.

Inputs: optional `tab`; optional `scope` of `controls`, `structure`, `all`, or `document`, default
`controls`; with `document`, an optional bounded subtree `root` handle and `max_depth` from 1 to 12;
optional `max_items` from 1 to 200, default 80; optional restrictions. Capability: `read`.

Facts: `tab`, `document_generation`, and `items`. Each item has a target handle, semantic role,
bounded accessible name, state, and credential-class flag. Selectors are not exposed. A
`document` scope returns one bounded structure-only tree, records it under a snapshot handle that
is superseded per tab, and reports a structural diff against the current prior snapshot when one
exists. Editable values are never returned; hidden content is excluded.

### `browser_find`

Find current semantic targets by visible or accessible text. Use it when the desired label or text
is known. Shortest call: `{"text":"Submit"}`.

Inputs: required non-empty `text`; optional `tab`; optional `scope` of `any`, `control`, or `text`,
default `any`; optional `max_results` from 1 to 50, default 20; optional restrictions. Capability:
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
a positive rectangle wholly inside the current view. Optional `timeout_ms` and restrictions apply
to every branch. Target, full-page, and region capture cannot be combined. Capability: `read`.

Facts: `tab`, `view`, `mime_type`, `width`, and `height`, plus one bounded MCP image content block.

### `browser_click`

Click a current semantic target or a point in a current screenshot. Shortest call:
`{"target":"target_..."}`.

Inputs use exactly one location branch: required `target`, or required `view`, `x`, and `y`.
Optional `tab`; optional `button` of `primary`, `middle`, or `secondary`, default `primary`;
optional `click_count` from 1 to 3 for single, double, or triple, default 1; optional `timeout_ms`;
optional restrictions. Capability: `action`.

Facts: `tab`, optional `target`, optional `view`, `activated`, and any governed committed landing.

### `browser_scroll`

Scroll in a direction or reveal a semantic target. Shortest call: `{}`, which scrolls down by a
medium amount.

Inputs use one of three branches: required `target` to reveal; or optional `tab`, optional
`direction` of `up`, `down`, `left`, or `right` defaulting to `down`, and optional `amount` of
`small`, `medium`, `large`, or `page` defaulting to `medium`; or coordinate wheel input with
required `view`, `x`, `y`, and `ticks` from 1 to 10 plus a two-way `direction` of `up` or `down`.
Optional `timeout_ms` and restrictions apply to every branch. Capability: `read`.

Facts: `tab`, optional `target`, `scrolled`, and observed horizontal and vertical offsets.

### `browser_hover`

Hover a current semantic target or a point in a current screenshot. Shortest call:
`{"target":"target_..."}`.

Inputs use exactly one location branch: required `target`, or required `view`, `x`, and `y`;
optional `tab`; optional `timeout_ms`; optional restrictions. Capability: `read`.

Facts: `tab`, optional `target`, optional `view`, and `hovered`.

### `browser_fill_form`

Fill one or more ordinary controls. It does not submit unless `submit_target` is present. Use
`browser_type_text` when per-character input events matter. Shortest call:
`{"fields":[{"target":"target_...","value":"Ada"}]}`.

Inputs: required `fields` array of 1 to 30 typo-closed objects, each with required `value` and
exactly one location (`target` or typed semantic `selector`); a value is a bounded string, a boolean
for checkboxes and radios, or a finite number for numeric inputs; optional `tab`; optional
`submit_target`; optional `timeout_ms`; optional restrictions.
Capabilities: `read + write` without submit and `read + write + action` with `submit_target`.

Credential-class targets stop before any value dispatch and request visible user handoff. Facts:
`tab`, `filled_count`, `submitted`, and any governed committed landing.

### `browser_type_text`

Type ordinary text through browser input events. Shortest call:
`{"target":"target_...","text":"Ada"}`.

Inputs use one location: `target` with bounded `text`; or `selector` with `text`; or
`focused:true` to type into the currently focused editable control. Optional `clear_first`, default
`false`; optional `tab`; optional `timeout_ms`; optional restrictions. Empty text is valid only as an
explicit clear together with `clear_first:true`. Capability: `action`.

Credential-class targets stop before text dispatch. Facts: `tab`, `target`, `typed`,
`character_count`, and any governed committed landing.

### `browser_press_key`

Send one explicit keyboard action. Shortest call: `{"key":"Enter"}`.

Inputs: exactly one of required `key` as one character or one named key from the closed list, or
required `strokes`, an ordered sequence of 1 to 20 of the same items, with optional `repeat` from 1
to 100 defaulting to 1; optional `tab`; optional `target`; optional unique `modifiers` from `Alt`,
`Control`, `Meta`, and `Shift`; optional restrictions. Capability: `action`.

Facts: `tab`, `key`, `pressed`, and any governed committed landing.

### `browser_drag`

Drag one semantic target to another, or drag between two points in a current screenshot. Shortest
call: `{"source_target":"target_...","destination_target":"target_..."}`.

Inputs use exactly one schema branch: required `source_target` and `destination_target`; or
required `view`, `start_x`, `start_y`, `end_x`, and `end_y`. Optional `tab`, `timeout_ms`, and
restrictions apply to both. Capability: `action`.

Facts: `tab`, `dragged`, and any governed committed landing.

### `browser_wait`

Wait for one explicit observable condition. Shortest call: `{"condition":"load_ready"}`.

Inputs use one condition-specific branch: `load_ready` accepts neither value nor target;
`url_contains`, `text_present`, and `text_absent` require `value`; `target_present` and
`target_absent` require `target`; `selector_present` requires a typed `selector` and polls the
live page until a control matching it exists; `duration` requires a whole millisecond `value`
from 0 to 10000 and waits executor-side. Every branch accepts optional `tab`, `timeout_ms`, and
restrictions. Capability: `read`.

Facts: `tab`, `condition`, `satisfied`, `elapsed_ms`, and governed readiness.

### `browser_dialog`

Inspect or resolve the current JavaScript dialog.

- `{"action":"status"}` reports whether a dialog is blocking; capability `read`.
- `{"action":"accept"}` accepts it; capability `action`.
- `{"action":"dismiss"}` dismisses it; capability `action`.
- `{"action":"respond","text":"Ada"}` supplies non-secret prompt text; capability `action`.

All branches accept optional `tab` and restrictions. `text` is required only for `respond` and is
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
`timeout_ms`, and restrictions. Capability: `write`.

Ghostlight rejects directories, missing files, any file larger than 5,000,000 bytes, and a combined
payload larger than 5,000,000 bytes before browser dispatch, and refuses an upload above the
capture-reuse ceiling. Inline bytes decode only after authorization and credential preflight. File
paths, names, and contents never enter audit or presentation. Facts: `tab`, `target`,
`uploaded_count`, and `uploaded_bytes`.

### `browser_execute`

Execute explicit bounded JavaScript in the page. It may read, mutate, or navigate, so use a
semantic tool when one fits. Shortest call: `{"script":"document.title"}`.

Inputs: required non-empty `script` up to 20000 characters; optional `tab`; optional
`max_result_chars` from 100 to 20000, default 8000; optional `timeout_ms`; optional restrictions.
Capability: `execute`.

Facts: `tab`, `value`, `truncated`, and any governed committed landing. Script source and result
never enter audit or presentation.

### `browser_sequence`

Run two to eight fully specified steps on one controlled tab. Shortest useful call:
`{"steps":[{"action":"click","target":"target_..."},{"action":"wait","condition":"load_ready"}]}`.

Inputs: required `steps`; optional `tab`; optional `timeout_ms`; optional restrictions. A step is a
typo-closed discriminated object. Allowed actions are `click`, `fill`, `type_text`, `press_key`,
`scroll`, `hover`, and `wait`; other catalog operations are not silently accepted. The sequence
wrapper requires no RAWX capability. Every step is classified, admitted, and audited independently
through the same executor path as a direct call.

Direct and sequence steps use the same operation executor and browser port. Facts: `tab`,
`completed_steps`, `total_steps`, and bounded per-step statuses. Execution stops at the first
non-success. Partial sequences are never repeat-safe.

### `browser_flow`

Compose one to twenty steps on one controlled tab. Shortest useful call:
`{"steps":[{"id":"open","tool":"browser_navigate","arguments":{"url":"https://example.com"}}]}`.

Inputs: required `steps` array of 1 to 20 uniquely named objects, each with a required bounded
`id`, a required `tool` naming one current advertised non-composite Ghostlight tool, and an
optional `arguments` object; optional `on_error` of `stop` or `continue`, default `stop`; optional
`dry_run`, default `false`; optional `tab`, `timeout_ms`, and restrictions. The wrapper requires no
RAWX capability; every child step classifies, admits, and audits independently under the same
immutable invocation snapshot. Steps carry no restriction fields of their own.

Any argument value may be an explicit reference object,
`{"flow_ref":{"step":"earlier_id","pointer":"/facts/..."}}`, resolved from that step's canonical
result envelope by JSON Pointer before the ordinary child decoder runs on the substituted
arguments. A reference that does not resolve fails its step without effect.

`dry_run:true` decodes and classifies every step without dispatching anything. Captured per-step
envelopes stop being recorded past a bounded byte budget while execution continues to a truthful
terminal aggregate; the aggregate reports applied, partial, or unknown effects. A flow with failed
or unknown work is never repeat-safe. Facts: `completed`, `total`, `stopped`, and bounded per-step
rows with each step's envelope where the budget allowed.

### `browser_record`

Create a short memory-only GIF of browser work. Usual flow:
`{"action":"start"}`, ordinary browser calls, then `{"action":"save"}`.

Actions are:

- `start`: optional `tab`; ask the extension to start an owned recording; capability `read`.
- `status`: optional `recording`; report state and deadlines; no new capability.
- `stop`: optional `recording`; capture a final frame, stop, and freeze; no new capability.
- `save`: optional `recording`; auto-stop if active. One replay goes to one place: with `target`,
  the browser attaches it to that file input and Ghostlight requires `write`; with
  `"download": true`, the browser saves it as a file and Ghostlight requires `read`; with neither,
  the GIF is returned to the client and Ghostlight requires `read`. `target` and `download`
  together are refused.
- `discard`: optional `recording`; erase captured bytes; no new capability.

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
`match`; optional opaque `after` cursor; optional `limit` from 1 to 200, default 50; optional
restrictions. Capability: `read`.

Problems are console warnings, errors, exceptions, failed requests, and HTTP error responses.
All detail also includes ordinary console events and successful requests. The first call enables
bounded volatile observation and may contain no earlier evidence; reproduce or reload when needed.
Reads are non-destructive. There is no clear input.

Console text is length-bounded. Network facts exclude headers, bodies, cookies, authorization,
post data, query strings, and fragments. Results contain ordered bounded entries, an opaque next
cursor, truncation and eviction facts, and counts of host-filtered entries. Diagnostic evidence is
untrusted model-visible content. It is never policy input, audit payload, persistent storage, or
page presentation.
