# Ghostlight 1.0 model-facing language

## Contract rules

The 22 tools below are the complete 1.0 catalog. Input objects and nested objects set
`additionalProperties` to `false`. Conditional inputs use discriminated schema branches, so the
advertised schema accepts exactly the shapes the decoder accepts. Omitted optional fields use the
defaults stated here.

Each declaration includes concise field descriptions, one shortest valid example, a truthful
output schema, and standard MCP annotations. The implementation owns bounds and defaults as named
constants shared by schema rendering and decoding.

Every tool may accept these flat request restrictions:

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `restrict_hosts` | string array | absent | Allow only these host patterns for this invocation. |
| `restrict_capabilities` | string array | absent | Allow only the named capabilities for this invocation. |

Restrictions can only reduce configured authority. Capabilities are `read`, `action`, `write`,
and `execute`.

An optional `tab` selects an opaque controlled tab. Omission selects the only controlled tab, or
the sole active controlled tab when ownership is unambiguous. Otherwise the call is rejected and
points to `browser_tabs`. Target handles are tied to a tab and document generation. A committed
navigation makes prior target handles stale.

View handles are returned by screenshots. They bind rendered coordinates to one tab, document
generation, viewport origin, viewport size, device scale, and zoom. Coordinate input is rejected
when that binding is no longer current. Coordinates use the returned image dimensions, with the
top-left pixel at `(0, 0)`.

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

The MCP edge renders this envelope as text plus `structuredContent`. Bounded rich content crosses
the bridge in a separate generic content vocabulary. `browser_screenshot` returns an image block.
`browser_record` save without a target returns a GIF image block. Image bytes are not copied into
structured facts.

The language context produces `summary`, `next_steps`, and the content-free observation projection
from one typed outcome. A sentence that names a host, count, or capture size carries the same value
in that projection. Page content never authors `summary` or `next_steps`. A result with `effect`
equal to `partial` or `unknown`, or with a committed effect unsafe to duplicate, has
`repeat_safe: false` and does not suggest replay.

## Catalog

### `browser_tabs`

List, focus, or close controlled tabs. Actions are:

- `list`: no `tab`; shortest call `{"action":"list"}`; capability `read`.
- `focus`: required `tab`; capability `action`.
- `close`: required exact `tab`; capability `action` and the tab-close policy constraint.

Close also respects the browser's local preserve-tabs interlock. Facts for list contain `tabs`,
each with `tab`, bounded `title`, governed `url`, `active`, and `readiness`. Focus facts include
`tab`, `active`, and `window_focused`. Close facts include `tab` and `closed`.

### `browser_navigate`

Navigate to a governed URL. Shortest call: `{"url":"https://example.com"}`.

Inputs: required `url`; optional `tab`; optional `new_tab`, default `false`; optional
`timeout_ms`; optional restrictions. `tab` and `new_tab:true` cannot be combined.

With `new_tab:true`, Ghostlight creates and navigates a new controlled tab. With `tab`, it
navigates that exact tab. With neither, it uses the unambiguous controlled tab, creates one when
none exists, and rejects ambiguous selection. Capability: `action`.

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
  320 to 7680 and `height` from 240 to 4320; optional `tab`; capability `action`.

Resize affects every tab in the window and may rerender the page. Either action invalidates a
current view handle when its bound geometry no longer matches. Facts include the selected tab,
action, requested dimensions or zoom, and observed geometry.

### `browser_read`

Read useful bounded prose from a page or target. Use `browser_inspect` or `browser_find` when an
action target is needed. Shortest call: `{}`.

Inputs: optional `tab`; optional `target`; optional `max_chars` from 500 to 20000, default 8000;
optional restrictions. Capability: `read`.

Facts: `tab`, governed `url`, bounded `title`, `text`, `truncated`, and
`document_generation`.

### `browser_inspect`

Inspect semantic controls or page structure and return fresh target handles. Shortest call: `{}`.

Inputs: optional `tab`; optional `scope` of `controls`, `structure`, or `all`, default `controls`;
optional `max_items` from 1 to 200, default 80; optional restrictions. Capability: `read`.

Facts: `tab`, `document_generation`, and `items`. Each item has a target handle, semantic role,
bounded accessible name, state, and credential-class flag. Selectors are not exposed.

### `browser_find`

Find current semantic targets by visible or accessible text. Use it when the desired label or text
is known. Shortest call: `{"text":"Submit"}`.

Inputs: required non-empty `text`; optional `tab`; optional `scope` of `any`, `control`, or `text`,
default `any`; optional `max_results` from 1 to 50, default 20; optional restrictions. Capability:
`read`.

Facts: `tab`, `document_generation`, and bounded ranked `matches` with target, role, name, and
state.

### `browser_screenshot`

Capture the viewport, full page, or one target and return a view handle for coordinate actions.
Shortest call: `{}`.

Inputs use one of three schema branches: optional `tab` only for viewport capture; optional `tab`
plus required `full_page:true`; or optional `tab` plus required `target`. Optional `timeout_ms` and
restrictions apply to every branch. Target and full-page capture cannot be combined. Capability:
`read`.

Facts: `tab`, `view`, `mime_type`, `width`, and `height`, plus one bounded MCP image content block.

### `browser_click`

Click a current semantic target or a point in a current screenshot. Shortest call:
`{"target":"target_..."}`.

Inputs use exactly one location branch: required `target`, or required `view`, `x`, and `y`.
Optional `tab`; optional `button` of `primary`, `middle`, or `secondary`, default `primary`;
optional `click_count` of 1 or 2, default 1; optional `timeout_ms`; optional restrictions.
Capability: `action`.

Facts: `tab`, optional `target`, optional `view`, `activated`, and any governed committed landing.

### `browser_scroll`

Scroll in a direction or reveal a semantic target. Shortest call: `{}`, which scrolls down by a
medium amount.

Inputs use one branch: optional `tab` plus required `target`; or optional `tab`, optional
`direction` of `up`, `down`, `left`, or `right` defaulting to `down`, and optional `amount` of
`small`, `medium`, `large`, or `page` defaulting to `medium`. Optional `timeout_ms` and restrictions
apply to both. Capability: `read`.

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

Inputs: required `fields` array of 1 to 30 typo-closed objects with required `target` and `value`;
optional `tab`; optional `submit_target`; optional `timeout_ms`; optional restrictions. Capability:
`write` without submit and `execute` with `submit_target`.

Credential-class targets stop before any value dispatch and request visible user handoff. Facts:
`tab`, `filled_count`, `submitted`, and any governed committed landing.

### `browser_type_text`

Type ordinary text through browser input events. Shortest call:
`{"target":"target_...","text":"Ada"}`.

Inputs: required `target`; required bounded `text`; optional `tab`; optional `clear_first`, default
`false`; optional `timeout_ms`; optional restrictions. Empty text is valid only with
`clear_first:true`. Capability: `write`.

Credential-class targets stop before text dispatch. Facts: `tab`, `target`, `typed`,
`character_count`, and any governed committed landing.

### `browser_press_key`

Send one explicit keyboard action. Shortest call: `{"key":"Enter"}`.

Inputs: required `key` as one character or one of `Enter`, `Tab`, `Escape`, `Backspace`, `Delete`,
`ArrowUp`, `ArrowDown`, `ArrowLeft`, `ArrowRight`, `Home`, `End`, `PageUp`, `PageDown`, or `Space`;
optional `tab`; optional `target`; optional unique `modifiers` from `Alt`, `Control`, `Meta`, and
`Shift`; optional restrictions. Capability: `action`.

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
`target_absent` require `target`. Every branch accepts optional `tab`, `timeout_ms`, and
restrictions. Capability: `read`.

Facts: `tab`, `condition`, `satisfied`, `elapsed_ms`, and governed readiness.

### `browser_dialog`

Inspect or resolve the current JavaScript dialog.

- `{"action":"status"}` reports whether a dialog is blocking; capability `read`.
- `{"action":"accept"}` accepts it; capability `action`.
- `{"action":"dismiss"}` dismisses it; capability `action`.
- `{"action":"respond","text":"Ada"}` supplies non-secret prompt text; capability `write`.

All branches accept optional `tab` and restrictions. `text` is required only for `respond` and is
invalid for every other action. Facts: `tab`, `dialog_type`, `present`, `accepted`, and `handled`
as applicable. Dialog text is never audited.

### `browser_upload`

Upload explicitly named bounded local files to one ordinary file input. Shortest call:
`{"target":"target_...","paths":["C:\\path\\document.pdf"]}`.

Inputs: required `target`; required `paths` array of one to five unique absolute local paths;
optional `tab`; optional `timeout_ms`; optional restrictions. Capability: `write`.

Ghostlight rejects directories, missing files, files larger than 5,000,000 bytes, and a combined
payload larger than 5,000,000 bytes before browser dispatch. File paths, names, and contents never
enter audit or presentation. Facts: `tab`, `target`, `uploaded_count`, and `uploaded_bytes`.

### `browser_evaluate`

Evaluate explicit bounded JavaScript in the page. It may read, mutate, or navigate, so use a
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
`scroll`, `hover`, and `wait`; other catalog operations are not silently accepted. Capability is
the highest capability required by any step.

Direct and sequence steps use the same operation executor and browser port. Facts: `tab`,
`completed_steps`, `total_steps`, and bounded per-step statuses. Execution stops at the first
non-success. Partial sequences are never repeat-safe.

### `browser_record`

Create a short memory-only GIF of browser work. Usual flow:
`{"action":"start"}`, ordinary browser calls, then `{"action":"save"}`.

Actions are:

- `start`: optional `tab`; ask the extension to start an owned recording; capability `read`.
- `status`: optional `recording`; report state and deadlines; no new capability.
- `stop`: optional `recording`; capture a final frame, stop, and freeze; no new capability.
- `save`: optional `recording`; optional semantic `target`; auto-stop if active. Without target,
  return the GIF to the client and require `read`. With target, attach it to the page and require
  `write`.
- `discard`: optional `recording`; erase captured bytes; no new capability.

`recording` may be omitted only when exactly one owned recording can be resolved. `target` is valid
only for save. The extension owns recording identity, frames, bounds, deadlines, stop, retention,
and erase. Frames and encoded bytes are volatile, owner-scoped, and never written to Ghostlight
storage, extension storage, logs, audit, or restart state. Save is immutable and
can be prepared repeatedly until retention expires; target delivery is a separate Write effect
and is not thereby repeat-safe. Discard is destructive.

Facts include `recording`, state, deadlines, frame and byte counts, stop reason, encoded size, and
delivery disposition as applicable. Client save returns one bounded `image/gif` content block.
Target delivery distinguishes dispatched-unverified from outcome-unknown and never claims page or
remote acceptance.

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
