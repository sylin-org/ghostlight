# Ghostlight 1.0 model-facing language

## Contract rules

Tool names and schemas below are the complete 1.0 catalog. Input objects set
`additionalProperties` to `false`. Omitted optional fields use the defaults stated here.

Every tool may accept these flat request restrictions:

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `restrict_hosts` | string array | absent | Allow only these host patterns for this invocation. |
| `restrict_capabilities` | string array | absent | Allow only the named capabilities for this invocation. |

Restrictions can only reduce configured authority. Capabilities are `read`, `action`, `write`,
and `execute`.

An optional `tab` selects an opaque controlled tab. Omission selects the only controlled tab,
or the active controlled tab when ownership is unambiguous. Otherwise the call is rejected.
Target handles are tied to a tab and document generation. A committed navigation makes prior
target handles stale.

View handles are returned by screenshots. They bind rendered coordinates to one tab, document
generation, viewport origin, viewport size, device scale, and zoom. Coordinate input is rejected
when that binding is no longer current. Coordinates use the returned image dimensions, with the
top-left pixel at `(0, 0)`.

Timeouts are bounded from 100 to 30000 milliseconds. A tool without an explicit timeout uses
8000 milliseconds. Text is UTF-8 and bounded. URLs must be absolute `http` or `https` URLs.

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
the bridge in a separate generic content vocabulary. In 1.0 the only rich content is the image
block returned by `browser_take_screenshot`; image bytes are not copied into structured facts.

Page content never authors `summary` or `next_steps`. A result with `effect` equal to `partial`
or `unknown`, or with a committed effect that is unsafe to duplicate, has `repeat_safe: false`
and does not suggest replay.

## Catalog

### `browser_list_tabs`

List controlled tabs. Shortest call: `{}`.

Inputs: optional restrictions only. Capability: `read`.

Facts: `tabs`, each with `tab`, bounded `title`, governed `url`, `active`, and `readiness`.
URLs are returned only after landing governance succeeds.

### `browser_activate_tab`

Bring one exact controlled tab and its containing window into view. Shortest call:
`{"tab":"tab_..."}`.

Inputs: required `tab`; optional restrictions. Capability: `action`.

Facts: `tab`, `active`, and `window_focused`.

### `browser_open_page`

Open a URL, govern every committed landing, wait briefly for useful readiness, and return the
controlled tab. Shortest call: `{"url":"https://example.com"}`.

Inputs: required `url`; optional `timeout_ms`; optional restrictions. Capability: `action`.
Ghostlight opens and groups the URL as one physical browser effect. A denied landing in the new
tab is compensated by closing it when that is known safe.

Facts: `tab`, governed `url`, bounded `title`, `created`, and `document_generation`.

### `browser_navigate_page`

Navigate a controlled tab and govern the landing. Shortest call:
`{"url":"https://example.com"}` when tab selection is unambiguous.

Inputs: required `url`; optional `tab`; optional `timeout_ms`; optional restrictions.
Capability: `action`.

Facts: `tab`, governed `url`, bounded `title`, and `document_generation`.

### `browser_navigate_history`

Move a controlled tab backward or forward through browser history. Shortest call:
`{"direction":"back"}` when tab selection is unambiguous.

Inputs: required `direction` of `back` or `forward`; optional `tab`; optional `timeout_ms`;
optional restrictions. Capability: `action`.

Facts: `tab`, `direction`, governed `url`, bounded `title`, and `document_generation`.

### `browser_reload_page`

Reload a controlled tab and govern the resulting landing. Shortest call: `{}` when tab selection
is unambiguous.

Inputs: optional `tab`; optional `bypass_cache`, default `false`; optional `timeout_ms`; optional
restrictions. Capability: `action`.

Facts: `tab`, governed `url`, bounded `title`, and `document_generation`.

### `browser_close_tab`

Close one exact controlled tab. Shortest call: `{"tab":"tab_..."}`.

Inputs: required `tab`; optional restrictions. Capability: `action`. The exact handle is always
required because close is committed and unsafe to target ambiguously. Dispatch occurs only when
service policy permits model-driven tab closure and the browser's local preserve-tabs setting is
off. Either gate may block with no effect.

Facts: `tab` and `closed`. Successful close is not repeat-safe.

### `browser_read_page`

Read useful bounded text from a page or target. Shortest call: `{}` when tab selection is
unambiguous.

Inputs: optional `tab`; optional `target`; optional `max_chars` from 500 to 20000, default 8000;
optional restrictions. Capability: `read`.

Facts: `tab`, governed `url`, bounded `title`, `text`, `truncated`, and
`document_generation`.

### `browser_inspect_page`

Inspect semantic controls or structure. Shortest call: `{}`.

Inputs: optional `tab`; optional `kind` of `controls`, `structure`, or `all`, default `controls`;
optional `max_items` from 1 to 200, default 80; optional restrictions. Capability: `read`.

Facts: `tab`, `document_generation`, and `items`. Each item has a target handle, semantic role,
bounded accessible name, state, and credential-class flag. It does not expose selectors.

### `browser_find`

Find semantic targets by visible or accessible text. Shortest call: `{"text":"Submit"}`.

Inputs: required non-empty `text`; optional `tab`; optional `kind` of `any`, `control`, or
`text`, default `any`; optional `max_results` from 1 to 50, default 20; optional restrictions.
Capability: `read`.

Facts: `tab`, `document_generation`, and bounded `matches` with target, role, name, and state.

### `browser_take_screenshot`

Capture the viewport, full page, or one target. Shortest call: `{}`.

Inputs: optional `tab`; optional `target`; optional `full_page`, default `false`; optional
`timeout_ms`; optional restrictions. `target` and `full_page: true` are mutually exclusive.
Capability: `read`.

Facts: `tab`, `view`, `mime_type`, `width`, and `height`, plus one bounded MCP image content block.
Screenshots are returned only by this tool.

### `browser_click`

Activate one current semantic target or a point from a current screenshot. Shortest call:
`{"target":"target_..."}`.

Inputs: exactly one location: required `target`, or required `view`, `x`, and `y`; optional `tab`;
optional `button` of `primary`, `middle`, or `secondary`, default `primary`; optional
`click_count` of 1 or 2, default 1; optional `timeout_ms`; optional restrictions. Capability:
`action`.

Facts: `tab`, optional `target`, optional `view`, `activated`, and any governed committed landing.
A landing invalidates old target and view handles.

### `browser_scroll_page`

Scroll a page in a direction or reveal one semantic target. Shortest call: `{}`, which scrolls
down by a medium amount when tab selection is unambiguous.

Inputs: optional `tab`; optional `target`; optional `direction` of `up`, `down`, `left`, or
`right`; optional `amount` of `small`, `medium`, `large`, or `page`; optional `timeout_ms`;
optional restrictions. When `target` is present, `direction` and `amount` must be omitted.
Without a target, omitted direction is `down` and omitted amount is `medium`. Capability: `read`.

Facts: `tab`, optional `target`, `scrolled`, and the observed horizontal and vertical offsets.

### `browser_set_zoom`

Set the visible zoom of a controlled tab. Shortest call: `{"percent":100}`.

Inputs: required integer `percent` from 25 to 500; optional `tab`; optional restrictions.
Capability: `read`.

Facts: `tab`, `percent`, and `zoomed`.

### `browser_hover`

Hover one current semantic target or a point from a current screenshot. Shortest call:
`{"target":"target_..."}`.

Inputs: exactly one location using the same `target` or `view` plus `x` and `y` contract as
`browser_click`; optional `tab`; optional `timeout_ms`; optional restrictions. Capability: `read`.

Facts: `tab`, optional `target`, optional `view`, and `hovered`.

### `browser_fill_form`

Fill a group of ordinary controls as one user job. Shortest call:
`{"fields":[{"target":"target_...","value":"Ada"}]}`.

Inputs: required `fields` array of 1 to 30 typo-closed objects with required `target` and
`value`; optional `tab`; optional `submit_target`; optional `timeout_ms`; optional restrictions.
Capability: `write` without submit and `execute` with `submit_target`.

Ghostlight describes every target before sending values to the adapter. If any target is
credential-class, no values are sent and the result requests visible user handoff.

Facts: `tab`, `filled_count`, `submitted`, and any governed committed landing.

### `browser_type_text`

Type ordinary text through browser input events. Shortest call:
`{"target":"target_...","text":"Ada"}`.

Inputs: required `target`; required bounded `text`; optional `tab`; optional `clear_first`,
default `false`; optional `timeout_ms`; optional restrictions. Capability: `write`.

Ghostlight rechecks the target immediately before sending text. Credential-class targets stop
before text dispatch and request visible user handoff.

Facts: `tab`, `target`, `typed`, `character_count`, and any governed committed landing.

### `browser_press_key`

Send one explicit keyboard action. Shortest call: `{"key":"Enter"}`.

Inputs: required `key` as one character or one of `Enter`, `Tab`, `Escape`, `Backspace`,
`Delete`, `ArrowUp`, `ArrowDown`, `ArrowLeft`, `ArrowRight`, `Home`, `End`, `PageUp`, `PageDown`,
or `Space`; optional `tab`; optional
`target`; optional `modifiers` containing unique values from `Alt`, `Control`, `Meta`, `Shift`;
optional restrictions. Capability: `action`.

Facts: `tab`, `key`, `pressed`, and any governed committed landing.

### `browser_drag`

Drag one semantic target to another, or drag between two points in a current screenshot.
Shortest call: `{"source_target":"target_...","destination_target":"target_..."}`.

Inputs: exactly one mode: required `source_target` plus `destination_target`, or required `view`,
`start_x`, `start_y`, `end_x`, and `end_y`; optional `tab`; optional `timeout_ms`; optional
restrictions. Capability: `action`.

Facts: `tab`, `dragged`, and any governed committed landing.

### `browser_upload_files`

Upload explicitly named local files to one ordinary file input. Shortest call:
`{"target":"target_...","paths":["C:\\path\\document.pdf"]}`.

Inputs: required `target`; required `paths` array of one to five absolute local paths; optional
`tab`; optional `timeout_ms`; optional restrictions. Capability: `write`. Ghostlight rejects
directories, missing files, files larger than 5,000,000 bytes, and a combined payload larger than
5,000,000 bytes before browser dispatch. File paths, names, and bytes never enter audit or
presentation.

Facts: `tab`, `target`, `uploaded_count`, and `uploaded_bytes`.

### `browser_run_script`

Evaluate an explicit bounded script in the controlled page and return its serializable result.
Shortest call: `{"script":"document.title"}`.

Inputs: required non-empty `script` up to 20000 characters; optional `tab`; optional
`max_result_chars` from 100 to 20000, default 8000; optional `timeout_ms`; optional restrictions.
Capability: `execute`.

Facts: `tab`, `value`, `truncated`, and any governed committed landing. Script source and result
never enter audit or presentation.

### `browser_wait`

Wait for one explicit observable condition. Shortest call:
`{"condition":"load_ready"}`.

Inputs: required `condition` of `load_ready`, `url_contains`, `text_present`, `text_absent`,
`target_present`, or `target_absent`; optional `tab`; optional `value`; optional `target`;
optional `timeout_ms`; optional restrictions. `value` is required only for URL and text
conditions. `target` is required only for target conditions. Capability: `read`.

Facts: `tab`, `condition`, `satisfied`, `elapsed_ms`, and governed readiness.

### `browser_run_sequence`

Run two to eight fully specified steps against one controlled tab. Shortest useful call:
`{"steps":[{"action":"click","target":"target_..."},{"action":"wait","condition":"load_ready"}]}`.

Inputs: required `steps`; optional `tab`; optional `timeout_ms`; optional restrictions. A step is
a typo-closed flat object with `action` plus only the fields used by the corresponding direct
operation. Allowed actions are `click`, `fill`, `type_text`, `press_key`, `scroll`, `hover`, and
`wait`. Sequence is the one intentional structured-input exception because ordered arguments are
the user's intent. Capability is the highest capability required by any step.

Direct and sequence steps use the same operation executor and browser port. Facts: `tab`,
`completed_steps`, `total_steps`, and bounded per-step statuses. Execution stops at the first
non-success. Partial sequences are never repeat-safe.

### `browser_handle_dialog`

Resolve the current visible JavaScript dialog. Shortest call: `{"accept":true}`.

Inputs: required `accept` boolean; optional `tab`; optional `text` for a prompt response;
optional restrictions. Supplying `text` when accepting a prompt requires `write`; other dialog
handling requires `action`.

Facts: `tab`, `dialog_type`, `accepted`, and `handled`. Dialog text is never audited.
