# Ghostlight 1.0 model-facing language

## Contract rules

The 23 tools below are the complete 1.0 catalog. Input objects and nested objects set
`additionalProperties` to `false`. Every input schema is a top-level object without root-level
`oneOf`, `allOf`, or `anyOf`, because current Kiro and Bedrock reject those otherwise valid JSON
Schema forms. Conditional inputs advertise one portable teaching envelope; the typed decoder
enforces the exact branch before governance or browser dispatch. Omitted optional fields use the
defaults stated here.

A field that several branches share is advertised as one schema that admits every value any
branch admits. Constants join one `enum`, length and numeric bounds widen, and each value of a
discriminator such as `action` or `condition` keeps its own description. `anyOf` appears only
inside a field whose alternatives are different JSON types. The only `oneOf`, a key name in
`browser_press_key`, has alternatives no value can match twice. A strict JSON Schema validator therefore accepts every
ordinary call. Tests validate every example and a list of ordinary calls against the advertised
schemas, decode them, and check that every next step naming a tool names only fields that tool
advertises (ADR-0196).

Each declaration includes field guidance, one shortest valid example, and standard MCP
annotations. `docs/1.0/catalog.json` is the exact catalog, pinned by a test. The catalog says only what a field's name, type, enum, bounds and
default do not. Guidance every tool shares, the optional `tab` and the `timeout_ms` bound, is in
the server instructions and in a short `tab` description; `timeout_ms` and `visual_settle` carry
none. The coordinate fact lives once in each `view` description: `x` and `y` are pixels in the
view's screenshot image, not CSS pixels. The governance phrases stay and are pinned by a test:
"for this request", "admitted before the effect", "may cause an external effect", "needs write
even for unsent drafts", and "screenshot image". The serialized `tools` array that `tools/list`
returns has a size budget (31,800 characters, with caps on the five largest tools) that only
ratchets down: a later change raises it by the measured size of a clause its new behavior needs.
The implementation owns bounds and defaults as named constants shared by schema rendering and
decoding. See [ADR-0198](../adr/0198-agent-experience.md).

`browser_flow` is the single composition tool for one to twenty ordinary tool calls. Step IDs
are optional for short batches and available for explicit result references. Each child uses the
invocation's immutable configured authority snapshot and the ordinary executor. `on_error` defaults
to `stop`. Captured result payloads have a bounded total budget; progress remains explicit.

Authority comes from configured managed/local policy and human controls. Tools do not accept
`restrict_hosts`, `restrict_capabilities`, or `browser_flow.dry_run`. The retired `browser_sequence`
tool is replaced by ordinary tool steps in `browser_flow`. The retired `browser_workspace` tool is
gone because a session works only in its own workspace. Obsolete calls fail before browser work
with catalog-refresh guidance; nothing retries automatically.
See [ADR-0162](../adr/0162-configured-authority-and-one-flow-tool.md) and
[ADR-0196](../adr/0196-simple-pipeline-and-contained-boundaries.md).

An optional `tab` selects an opaque controlled tab. Omission selects the only controlled tab, or
the sole active controlled tab when ownership is unambiguous. Otherwise the call is rejected
before any browser work, and the refusal says which case it hit:

| Case | Summary | `reason` | Next step |
| --- | --- | --- | --- |
| No tab yet | `This session has no open tab yet.` | `no_tab` | `browser_navigate` with `url` |
| Several tabs could be meant | `More than one tab is open in this session, so the call must name one.` | `ambiguous_tab` | `browser_tabs` list, then repeat with `tab` |
| Named tab is gone | `That tab is no longer open.` | `tab_unavailable` | `browser_tabs` list |

Each session works only in its own workspace. A tab, target, or view handle that belongs to
another session is refused before any browser work with `reason: ownership_mismatch`. When the
owning session has a client label, the summary names that label, for example `That tab handle
belongs to Codex.`, the `owner_workspace` fact carries it, and the next step is `browser_tabs` list
in the caller's own session. Otherwise, including a client that reported an empty name, the
summary is `That handle belongs to a different Ghostlight session.` and the next step is to collect
fresh handles in this session. A refusal never carries another session's workspace id. No tool
lists other sessions or moves a session into another workspace (ADR-0196 Decision 8).

Tab handles are durable correlation slots: navigating by a handle whose
tab has closed recreates that tab through the governed open path under the same handle and says
so in the summary. Closing a handle whose tab is already gone succeeds without touching the
browser. Target handles are different by design -- they are perception, not identity. A target
handle is tied to one tab and document generation, and a committed navigation makes prior target
handles stale. Within one document a handle follows its control through a re-render when the page
proves it is the same control: its row's own link, unique on the page, its declared row name, its
role, exact name and position in the row, and an unchanged view. The action then reports
`rebound: true` in its facts and says its usual sentence; the audit keeps the bool and never a
handle, row or link. Anything short of proof refuses as removed. Reading the same control again in
the same document returns the same handle, also after a re-render the page proves.
A stale target handle refuses with `reason: stale_target`, no effect and `repeat_safe: true`, and
a `stale` fact that says why in one of four words, whoever noticed. The service notices a closed
tab or a left document; the page notices a control that left it. A handle of a closed tab, of a
document the tab left, or of a control the page reported removed refuses at once, before anything
is sent to the browser. The sentence and the step depend on the word only, and never quote page
text:

| `stale` | Summary | Next step |
| --- | --- | --- |
| `removed` | `That target is no longer on the page.` | `browser_find` or `browser_inspect` |
| `navigated` | `That target belongs to an older version of the page.` | `browser_inspect` |
| `tab_closed` | `That target's tab is closed.` | `browser_tabs` |
| `unknown` | `Ghostlight does not know that target handle.` | `browser_find` or `browser_inspect` |

Candidates the live page still has are listed in `recovery_candidates` only when the tab's address
is readable under the session's authority. Each carries a fresh handle (`target`), its `role`, its
`nth` among controls that share its row, and, only when the session preserves target names, its
`name` and its row `context`. For a control the page found removed, the candidates are the page's
own lookup in the failing call: up to three controls with the same role, exact name and credential
class, in that document. For a left document they come from a separate unpainted query, and the
step stays `browser_inspect`. The refusal names one control as `replacement`, and its step is
`Repeat <tool> with target <handle>.`, only when all of these hold: the word is `removed`; nothing
was sent; the tool takes exactly one target (`browser_click`, `browser_hover`,
`browser_type_text`, `browser_press_key` and `browser_scroll` with a target, `browser_upload`,
`browser_wait` with a target; never `browser_fill_form` fields or `browser_drag`); the page found
exactly one control that matches role, exact name, credential class and row, and the row's own
link when the row had one, in an unchanged view; and the lookup was not cut short. The summary then
adds `One control on the page has the same role, name and row.` Otherwise, with candidates, the
step is `Pick a target from recovery_candidates by its context, or call browser_find for current
target handles.` The candidates are listed in the order of their `nth`, those with none last.
When `privacy.preserve_target_names` is false a candidate carries only `target`, `role` and `nth`,
so the step says `by its role and nth` in place of `by its context`. The audit and the history
keep the stale word only: never a replacement, a handle, a row or a candidate. A handle the service never issued, dropped to bound memory, or whose
control the page no longer holds in its registry is `unknown`. A control the page still holds under
a handle but which now sits in a different row, because its list recycled the node, is also
`removed`.
A same-document change is not a committed navigation. When an action moves the address within the
same origin and no document commits, as an anchor link or a `history.pushState` route does, the
generation and target handles stay valid. The tab's governed URL follows the page, landing
governance still checks the new address, the action's `landing` fact carries the new `url` with
the unchanged `document_generation`, and view handles expire because the rendered page may have
moved. An address in another origin always starts a new generation. An explicit
`browser_navigate` or `browser_history` call always starts one too.
Typed semantic selectors (name, optional role, optional exact match) are accepted wherever target
handles are, including `selector_present` waits, so drivers can prefer what a control is called
over stashing handles. A selector's `role` is one of `button`, `link`, `checkbox`, `radio`,
`textbox`, `searchbox`, `combobox`, `listbox`, `slider`, `spinbutton`, `tab`, `menuitem` or
`option`; each is a role an observed control can have.

View handles are returned by screenshots. They bind rendered coordinates to one tab, document
generation, viewport origin, viewport size, device scale, and zoom. Coordinate input is rejected
when that binding is no longer current. Coordinates use the returned image dimensions, with the
top-left pixel at `(0, 0)`. A bounded region screenshot returns a new view with its own transform,
so another region can be selected from the magnified image.

Every tool accepts `timeout_ms`, bounded from 100 to 30000 milliseconds. It defaults to 8000
milliseconds, except that `browser_fill_form` and `browser_record` default to 30000. Text is
UTF-8 and bounded. URLs must be absolute `http` or `https` URLs.

Handles are short. Ghostlight issues `tab_`, `target_`, `view_`, `snapshot_` and `image_` handles
as the prefix and eight random base32 characters (`tab_k3m9x2ab`), 12 to 17 characters in all
where the same handles were 36 to 41. A handle is still checked against the session that issued
it, and it is unique within that session.

Ordinary bursts may wait within the original invocation deadline. Sustained waiting is visible
in the human workbench; it does not create a new tool call or a popup. An exceptional admission
capacity refusal returns `reason: capacity`, no effect, and safe guidance for a request that never
started. Human Pause/Stop preserves its fixed directive and terminal history without repeated
guardrail popups while queued work drains (ADR-0160).

Ordinary page tools share optional `visual_settle`, default true: read, inspect, find,
screenshot, click, scroll, hover, fill, type, key input, drag, upload and script execution.
Settlement is best effort, with one maximum 1000 ms preparation budget per invocation.
A quiet page completes preparation sooner; a busy page remains usable after that budget.
False skips implicit waiting. Lookup and execution share the budget; each flow child has
its own preference within the parent deadline. Actual targets and permissions still apply.
Composite waits keep a met primary condition when optional settlement expires and report
`visual_settled` separately. An explicit `visual_settle` wait retains its condition.
See [ADR-0195](../adr/0195-bounded-settlement-and-subject-scoped-page-work.md).

## Server instructions

Every protocol edge sends the same service-owned instructions at initialization. A client that
loads tool schemas lazily picks its first calls from this text alone, so it teaches the working
loop in six short sentences and names only catalog tools:

> Ghostlight controls the user's visible Chromium browser. Open or reuse a tab with
> browser_navigate, or list this session's tabs with browser_tabs; omit tab when one tab is
> unambiguous. Read a page with browser_read, and get handles for its controls from browser_find or
> browser_inspect, which also show what each form control accepts. Act with those handles, for example with browser_click or browser_fill_form;
> timeout_ms bounds the whole call. Handles expire when the page navigates; a handle follows its
> control through a re-render the page can prove, and a stale handle's refusal says why. Every
> result states its effect and whether repeating it is safe.

## Result envelope

Every invocation returns one envelope:

| Field | Type | Meaning |
| --- | --- | --- |
| `history_storage` | enum | `saved` or `unconfirmed`; storage confirmation is independent of browser effects. |
| `invocation` | string | Opaque correlation handle. |
| `status` | enum | `succeeded`, `blocked`, `failed`, `cancelled`, or `unknown`. |
| `effect` | enum | `none`, `applied`, `partial`, or `unknown`. |
| `readiness` | enum | `not_applicable`, `loading`, `interactive`, `complete`, or `unknown`. |
| `repeat_safe` | boolean | Whether repeating the same call is known safe. |
| `summary` | string | Bounded Ghostlight-authored explanation. |
| `facts` | object | Tool-specific canonical facts. |
| `next_steps` | string array | Zero to two Ghostlight-authored safe suggestions. |

The canonical envelope stays inside the service for audit, history, flow references and the
workbench. The MCP edge renders the lean model view of it (ADR-0198 Decisions 21 and 22), once, in
`structuredContent`. The view is the same envelope with every field at its default left out, and
an absent key means its default:

- `history_storage` of `saved`, `readiness` of `not_applicable`, and an empty `next_steps`, on the
  envelope and on each flow step result; `history_storage` of `saved` on a flow step row.
- `facts.coverage` when nothing limited the read (see Document coverage).
- On each entry of `facts.items` and `facts.matches`, an empty `state` and a false
  `credential_class`.
- On each `facts.tree` node, an empty `label` and an empty `children`.

`status`, `effect`, `repeat_safe`, `summary`, `invocation` and `facts.tab` always stay. Explicit
facts stay at any value, including a `value` of `""` or `"[hidden]"` and a `history_storage` of
`unconfirmed`. Page data is never rewritten: a script's `facts.value` reaches the client exactly
as the script returned it. The edge adds no field and changes no value.

The ordinary text block carries the authored summary and safe next steps. A session negotiated at
an MCP revision before 2025-06-18 cannot read `structuredContent`, so its text block also carries
the same lean JSON after them (ADR-0198 Decision 24). Bounded rich content crosses the relay in
a separate generic content vocabulary. `browser_screenshot` returns an image block.
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

Refusals say what is true, and their next steps suggest only what the caller can do:

- A cancellation that wins before any browser command answers `Cancelled before anything reached
  the browser.` with status `cancelled`, no effect, and no next step. It never says the browser
  disconnected.
- A browser that cannot take a call is waited for first when there is evidence it is coming
  back. A session that already works in a browser always waits for that browser. A session with
  no browser yet waits only while a connection is registered but initializing or silent, a
  browser connection ended less than the wait ago, or the service published its endpoint in the
  last 1.5 seconds. It stops waiting as soon as none of these holds, so a browser closed long ago
  goes straight to browser startup with the whole deadline to launch it. The wait lasts up to
  about seven seconds and ends early enough to leave the call time to run.
  A silent browser gets one liveness probe at once. Under `on_demand`, when Ghostlight knows
  which installed browser a session's own browser is, the session waits only while a browser is
  evidently returning, and a browser still gone after that is started again, so the start keeps
  the call's budget. The call then goes through when the same browser connects within the
  deadline, even after another profile of it connected first; another profile alone at the
  deadline is `browser_wrong_profile`, and a start whose adapter never connects is
  `browser_handshake_timeout`, with the words every browser start has. A retry within 30 seconds
  of a start that has not connected waits for that start instead of starting the browser again,
  so repeating the call after a handshake timeout is safe. No new sentence is
  needed (ADR-0196 Decision 12 amendment, 2026-10-17). Otherwise a session that already works in
  a browser is never sent to browser startup. Then the refusal says which fact stopped the call,
  with `failed`, no effect, and `repeat_safe: true`:

  | Fact | `reason` | Summary | Next step |
  | --- | --- | --- | --- |
  | Connected but not answering | `browser_not_answering` | `The browser this session works in stopped answering. Nothing was sent.` | `Check that the browser window is open and responsive, then repeat the call.` |
  | Not connected, no other browser is | `browser_disconnected` | `The browser this session works in is not connected. Nothing was sent.` | `Ask the user to reopen that browser, then repeat the call.` |
  | Not connected, a different browser is | `browser_wrong_profile` | `This session works in a browser that is not connected; a different browser is.` | `Ask the user to reopen the browser profile this session started in, then repeat the call.` |
  | Connection busy with other work | `browser_busy` | `The browser connection was busy, so nothing was sent.` | `Repeat the call.` |
  | Bridge contract failure | `browser_contract_failed` | `Ghostlight and the browser extension do not agree on this request.` | `Ask the user to update Ghostlight in Browser (to update now, open chrome://extensions, turn on Developer mode and click Update), then repeat the call.` |

  None of these says "disconnected" for a browser that is still attached, and none tells the
  model to reconnect an adapter it cannot reach (ADR-0196 Decision 12). The contract summary
  does not say whether anything was sent. A receipt in the wrong shape arrives after the
  browser already received the request.
- A deadline before dispatch suggests a longer `timeout_ms` only when the call's own value is
  below the 30000 ms maximum and no edge deadline overrides it. Every tool takes the field, so
  the suggestion always describes a call the decoder accepts. Calls already at the maximum (such
  as `browser_fill_form` with its default) are told to repeat the call and narrow a very large
  page first. A deadline after dispatch has an unknown effect, so it says to check the page with
  `browser_read` or `browser_screenshot` before repeating.
  The `timeout_ms` of a decoded operation is the one record of its call time, and the deadline
  itself is derived from it.
- A policy denial (host, protected host, capability, tab close, screenshot, client kind, or an
  unspecified policy reason) points to `policy_explain`. Pause, Stop, a required-history outage,
  and an unreadable policy keep no next step.
- A refusal of Ghostlight's own control page names the origin as the caller wrote it (scheme,
  host and port, never a path) and says other ports on this computer are not affected. It keeps
  no next step, because no policy lifts it. History records the reason and the host, never the
  origin. `policy_explain` lists the control page among the ceilings no policy can lift.
- The preserve-tabs veto says the tab stays open because the user chose to keep Ghostlight's
  tabs, and to continue without closing it.
- A workspace refusal found after the browser already acted, such as a tab the browser opened
  that another session in the same browser controls, keeps its reason but reports
  `effect: applied` and `repeat_safe: false`, because repeating it would act again.

The same language owner supplies a separate typed audit projection. Readable explanations,
measurements, and governed target names survive; arbitrary browser details and result facts do
not enter retained history. A permitted client result may still contain those details. Expanding
existing history does not authorize more capture. Additional retention profiles and richer
diagnostic capture remain deferred (ADR-0103 H1 amendment).

When no browser is connected and startup is left to the person -- because `browser.startup` is
`manual`, or because more than one installed browser could serve and none of them is a usable
browser that last connected to Ghostlight -- the refusal addresses the MCP model: ask the user to open one of the
eligible installed browser windows it names, with the Ghostlight extension installed, then repeat
the call. Facts carry the closed `browser_startup_manual` reason and a `browsers` array; one
choice also retains the singular `browser` fact. The summary contains the whole recovery
instruction, so `next_steps` is empty, with one exception: while setup is incomplete and no
browser profile holds the extension, one step asks the user to add Ghostlight in Browser from the
store listing, or to open the Ghostlight window for the setup steps (ADR-0200). The same step
replaces the extension-absent recovery step, and the step after a launched browser whose adapter
never arrived, under the same condition. Stale Ghostlight-owned
registrations among several installed browsers are repaired silently first, so the named browsers can actually connect; when
no installed browser has a usable registration, the refusal names the browsers found and one
choice-free remedy instead. No browser refusal ever says that Ghostlight declined to choose. Under `on_demand`, the
browser that last connected is started instead of asking, so no new sentence is needed for it
(ADR-0149 amendment, 2026-10-17).
When the registration names the edge of a different Ghostlight installation,
recovery changes nothing and answers `Another Ghostlight installation owns the browser
registration.` with the `native_host_owned_elsewhere` fact and one next step: the user runs
`ghostlight install` from the installation that should own the browsers. Silent repair applies
only to stale details within the running installation's own directory (ADR-0149 amendment).

## Catalog

### `browser_tabs`

List, focus, or close controlled tabs. Actions are:

- `list`: no `tab`; shortest call `{"action":"list"}`; capability `read`. The list is read live
  from the connected browser on every call and names only this session's own tabs, so it
  requires a connected browser and refuses without one. The summary reads `Listed 2 tabs in this
  session.`; an empty list points to `browser_navigate` with `url`.
- `focus`: required `tab`; no RAWX capability.
- `close`: required exact `tab`; capability `action` and the tab-close policy constraint.

Close also respects the browser's local preserve-tabs interlock. Facts for list contain `tabs`,
each with `tab`, bounded `title`, governed `url`, `active`, and `readiness`. Focus facts include
`tab`, `active`, and `window_focused`. Close facts include `tab` and `closed`.

### `browser_navigate`

Navigate to a governed URL. Shortest call: `{"url":"https://example.com"}`.

Rules: `tab` and `new_tab:true` cannot be combined. `new_tab:true` always creates a tab, so
`reuse:"domain"` beside it is refused and `reuse:"never"` beside it changes nothing.
`beforeunload:"discard"` accepts just that navigation's own unsaved-change prompt; without it, a
blocking prompt stops the navigation and is reported, never accepted.

With `new_tab:true`, Ghostlight creates and navigates a new controlled tab. With `tab`, it navigates
that exact tab. With neither, it uses the unambiguous controlled tab, and when none exists it opens
one. With `reuse:"domain"`, the open first reuses a tab Ghostlight opened on the same host and port
and a released session of the same client left open, in the same browser, with no page loaded in it
since the release. Same client means the same client label, so the tab already sits in the caller's
`Ghostlight - <client label>` group; a tab another client released is never reused, and the open
creates a fresh tab. A released tab stays open only when the person's preserve-tabs setting kept it,
so with that setting off nothing is reused. An exact URL wins, then the lowest tab id. The service
makes that choice, checks that the browser still shows the tab at its release address, and navigates
that exact tab. A tab the person opened is never reused, and the browser is never asked to pick one
(ADR-0196 Decision 9). `reuse:"never"` always creates a fresh tab. A reused open says so: the
summary reads "Reused the example.com tab." rather than "Opened example.com." Capability: `read`.

Facts: `tab`, governed `url`, bounded `title`, `created`, `reused`, and `document_generation`.

### `browser_history`

Move through history or reload. Shortest call: `{"action":"back"}`.

Rules: `bypass_cache` is valid only for `reload`. Capability: `action`.

Facts: `tab`, `action`, governed `url`, bounded `title`, and `document_generation`.

### `browser_window`

Set tab zoom or resize the containing browser window.

- Zoom: `{"action":"zoom","percent":100}`. Requires `percent`; capability `read`.
- Resize: `{"action":"resize","width":1280,"height":800}`. Requires `width` and `height`; no RAWX
  capability.

Resize affects every tab in the window and may rerender the page. Either action invalidates a
current view handle when its bound geometry no longer matches. Facts include the selected tab,
action, requested dimensions or zoom, and observed geometry.

### `browser_read`

Read useful bounded visible text from the full composed page or one target. A full-page read
includes visible top-document text, open shadow trees, and http(s) embedded frames in stable frame
order. Visible form values are included, checkboxes and radios read as checked or unchecked, and
placeholders are never values. Every credential-class field, as the write path classifies it, is
masked: its value reads as the short marker `[hidden]`. Closed shadow roots and hidden content remain absent. Use
`browser_inspect` or `browser_find` when an action target is needed. A read of one native select
returns its chosen text and then a last line, `Options: ` followed by every option in order and
separated by semicolons, bounded only by `max_chars`; a disabled option reads `(disabled)`. This is
the way to see an option list that an item cut at 25. A credential-class select reads `[hidden]`
and lists nothing. Shortest call: `{}`.

Rules: `mode` defaults to `visible`; explicit `article` prefers a useful
article in the top document and falls back to the same full-page visible read, and is ignored with
`target`. Capability: `read`.

Facts: `tab`, governed `url`, bounded `title`, `text`, `truncated`, and
`document_generation`.

### `browser_inspect`

Inspect semantic controls or page structure. The `controls`, `structure`, and `all` scopes return
fresh target handles; the `document` scope returns the page outline as a tree. Shortest call: `{}`.

Rules: `root` and `max_depth` belong to the `document` scope. Capability: `read`.

Facts: `tab`, `document_generation`, and `items`. Each item has a target handle, an accessibility
role, bounded accessible name, state, and credential-class flag, and a control adds the keys in
the table below when they apply. An element with no role of its
own reads as `generic`, never as a tag name. Native controls have their HTML-AAM roles: a range
input is a `slider`, a number input a `spinbutton`, a search input a `searchbox`, a select that
takes several choices or shows more than one row a `listbox`, and any other select a `combobox`.
An input of type hidden is not a control and is never listed. A field with no label, `aria-label` or title is named
by its placeholder, as browsers do, so `browser_find` and semantic selectors reach it by that
text. Selectors are not exposed. It answers once the page has been quiet for 300 ms, within the
settle budget.

Item keys beyond the five above appear only when they apply, so an absent key means the page
offered nothing of that kind:

| Key | Meaning |
|---|---|
| `value` | What the field holds, at most 80 characters, with `value_chars` (the full length) when longer. Empty when the field is empty or a select shows its empty placeholder option. A multiple select gives an array of at most 25 chosen texts. Covers text fields, textareas, native selects, rich editors, range and number. |
| `options` | A native select only, in document order, at most 25 entries of 60 characters, with `options_total` when cut. An entry is the bare text, `{"text","value"}` when the text is not unique, `{"text","disabled":true}` when the option or its optgroup is disabled, or `{"group","options"}` for an optgroup. The placeholder is the first option when its value is empty; it is left out and shown as `hint`. Any other option with an empty value is a real choice. |
| `type` | The input type, only for `date`, `time`, `datetime-local`, `month`, `week`, `number`, `range`, `color`, `file`, `email`, `url` and `tel`. |
| `limits` | The constraints the page declared, among `min`, `max`, `step`, `minlength`, `maxlength`, `pattern`, `accept`, and `multiple:true`. |
| `hint` | A placeholder that says something other than the name, else the text `aria-describedby` points to, else the select's placeholder option. At most 120 characters. |
| `group` | For a radio or checkbox, the fieldset legend or the nearest labelled group. At most 80 characters. |
| `context` | The label of the control's row or group, only when its role and name repeat among the results. A row is the nearest list item, table row, article, tree item, named group or fieldset; its label is the name the page declares, else a legend, else its first heading, else its first link's text. With no row, the group or the nearest named region or panel. One line of at most 80 characters, never a field's value. Never on a credential-class control. |
| `nth` | The control's position, counted from 1 in document order, among the results that share its role, name and context. Present only when that set has two or more. Counted over the whole page's results, across frames, after any cut. For display only; use the handle to act. |
| `error` | Present when the control is invalid after the person touched it, or the page says `aria-invalid`. The page's own error text when it names one; otherwise a phrase such as `is required`, `is not a valid email address` or `is too short (at least 8 characters)`. The browser's validation message is never used except for a custom error. |

Any item, control or not, also says where it sits, with these keys only when they apply (ADR-0198
decision 19):

| Key | Meaning |
|---|---|
| `area` | The page area that holds the item: `header`, `nav`, `aside`, `footer`, `dialog` or `search`. A native `header` or `footer` is a page area only when no `article`, `aside`, `main`, `nav`, `section` or dialog contains it; inside a dialog it reports `dialog`. Absence means page content. |
| `form` | The number of the form that owns the item, counted from 1 over the forms of the page in the order they appear, frames included, so a number keeps naming the same form from one call to the next. The `form` attribute counts, and so does a control inside a component that sits in a form. Absence means no form owns the item. |

Items come out in the order the page shows them, including content inside components. When the
page has more than `max_items`, the cut goes by tier and never by position: hidden items go first,
then items in a header, nav or footer, then asides, and page content last. Content here means
everything else, including dialogs, forms, the main region and search. Within a tier the earlier
items stay, and what is kept stays in page order, so a long navigation bar does not push the form
out. The fact `left_out` counts what was left out, by `hidden`, `chrome` (header, nav and footer),
`aside` and `content`, and shows only the keys that have a count. The result then adds a next step
that gives the number and the two ways to reach an item: raise `max_items` (up to 200) or use
`browser_find` for one by name. The summary does not change.

A control that cannot be acted on is not listed at all, and is counted as `behind_dialog`: one
under an `inert` ancestor, or outside an open modal dialog. It does not use up `max_items`, and it
does not make the result "reach its size limit". When a modal dialog is open the facts also say
`modal_open:true`. Structure items such as headings and landmarks stay listed. `browser_find` cuts
by the same tiers when it has more matches than `max_results`.

State words gain `required`, `readonly` and `invalid`. A control inside a disabled fieldset reads
`disabled` and is refused as not actionable, as one with the `disabled` attribute is.

A control is credential-class by its type, its autocomplete token, or its name or id, which is
split at camelCase before matching, so `apiKey`, `cardNumber`, `userPin`, `ssn` and `iban` count.
A credential-class control carries `value` as `[hidden]` when set and empty when empty, and none of
the other keys. Option text, hints and errors are page content and follow read authorization on the
document, like `browser_read`.

A `document` scope returns no target handles. Its facts are `tab`, `document_generation`,
`tree`, `nodes`, `truncated`, `snapshot`, and sometimes `diff`. `tree` is one bounded
structure-only composed tree. Each node has a `kind` of `control`, `heading`, or `container`, a
`label` of at most 120 characters, and a `children` array. A control or heading is labelled by its
accessible name. A container is labelled only by a name the page declared for it, and by no text
from inside it, because its descendants carry that text themselves. An unlabeled container whose
children were all left out is itself left out and does not count against the node ceiling; a
container cut by depth or budget stays, and so does the root. Without a root, the tree includes open
shadow roots, assigned slots, and http(s) embedded frames in stable order under one 400-node
ceiling. A root limits it to that target's composed subtree. Editable values, hidden
content, closed roots, and frame identity are never returned. A summary reads, for example:
`Returned the document outline: 283 nodes.`

The tree is also recorded under a `snapshot` handle, superseded per tab. When the previous
snapshot of that tab, in the same document, used the same `root` and `max_depth`, the result adds
`diff` and the summary counts the differences, or says the outline matches the previous one. Trees of another root or depth are never compared,
because they differ without the page changing. `diff` compares nodes by position and counts
`added`, `removed`, and `changed` nodes. Its `paths` list up to 50 positions in the returned
`tree`: `/` is the root and `/1/0` is the first child of the root's second child. A path names a
new node, a node whose kind or label changed, or a node that lost children.

### `browser_find`

Find current semantic targets by visible or accessible text across the composed page, including
open shadow roots and http(s) embedded frames. Use it when the desired label or text is known.
Text search also matches `div`, `[draggable]`, `[aria-label]`, and `[tabindex]` elements, and
prefers the deepest match over a container that only holds it. Names never include field values.
Option text finds its dropdown. A credential-class dropdown is never found by its options.
Shortest call: `{"text":"Submit"}`.

Capability: `read`.

Facts: `tab`, `document_generation`, and bounded `matches` in document order with target, role, name, and
state, and the same optional control keys as `browser_inspect` items. When a role and name repeat among the results, each carries `context` (the label of its row or group) and, when that repeats too, `nth` (its position among those, in document order). A unique control carries neither. It answers once the page has been quiet for 300 ms, within the settle budget.

### `browser_screenshot`

Capture the viewport, full page, one target, or a magnified region from a current view. Every
capture returns a view handle for later coordinate actions or another region capture. Shortest
call: `{}`.

Rules: the decoder accepts one of four branches: viewport capture with no other capture field;
`full_page:true`; `target`; or a region with `view`, `x`, `y`, `width`, and `height`. Region
coordinates are image pixels and must form a positive rectangle wholly inside the current view.
Target, full-page, and region capture cannot be combined. Capability: `read`.

Facts: `tab`, `view`, `mime_type`, `width`, and `height`, plus one bounded MCP image content block.

### `browser_click`

Click a current semantic target or a point in a current screenshot. Shortest call:
`{"target":"target_..."}`.

Rules: exactly one location branch: `target`, typed semantic `selector`, or `view` with `x` and
`y`. `click_count` 1, 2 and 3 are a single, double and triple click. Capabilities: `action`, plus
`read` when `selector` or `expect` is supplied.

Facts: `tab`, optional `target`, optional `view`, `activated`, `rebound: true` when the target handle
followed its control through a re-render, and any governed committed landing.

### `browser_scroll`

Scroll in a direction or reveal a semantic target. Shortest call: `{}`, which scrolls down by a
medium amount.

Rules: one of three branches: `target` to reveal; or a page scroll by `direction` and `amount`; or
coordinate wheel input with `view`, `x`, `y`, and `ticks`, whose `direction` is `up` or `down`.
Capability: `read`.

Facts: `tab`, optional `target`, `scrolled`, and observed horizontal and vertical offsets.

### `browser_hover`

Hover a current semantic target or a point in a current screenshot. Shortest call:
`{"target":"target_..."}`.

Rules: exactly one location branch: `target`, or `view` with `x` and `y`. Capability: `read`.

Facts: `tab`, optional `target`, optional `view`, and `hovered`.

### `browser_fill_form`

Fill one or more form controls. It does not submit unless `submit_target` is present. Use
`browser_type_text` when per-character input events matter. Shortest call:
`{"fields":[{"target":"target_...","value":"Ada"}]}`.

Rules: each field has exactly one location, `target` or typed semantic `selector`.
Capabilities: `read + write` without submit and `read + write + action` with `submit_target`.

Ordinary textual inputs, textareas, and rich-text controls use the browser's editing transaction
so controlled forms and editors retain the replacement across later renders. Selects, checkboxes,
and radios keep their semantic control setters. Empty values clear only the named control. Filling
never activates a submit control unless the caller supplied `submit_target`.
Form preparation shares the call's settle budget; only `visual_settle` false skips it.

Active controlled tabs emulate focus between calls to avoid focus-triggered refresh interrupting
automation (ADR-0168). Pause, stop, disconnection, and release restore normal browser lifecycle.
An application can still reset its own form, especially after control ends; a fill result does
not claim that the draft was saved or will survive future application refreshes.

Semantic selectors and target handles can address the same ordinary controls, including editors
outside an HTML `form`. The selector does not impose an unadvertised form-ancestry requirement.
An explicit `submit_target` is still checked against the first resolved field's containing form.

How a value picks a choice (ADR-0198 Decisions 8 and 9). One page routine decides it for the check
before the fill, the fill, and the check that the page kept the value, so they cannot disagree. For
a select, an option answers to its value, its text and its label. A value that is exactly one of
those picks that option. If two or more options answer to it exactly, the fill is refused with
their values. Only when no option answers exactly does the page compare again after Unicode
composition, trimming, white space collapse and case folding on both sides. One match picks it, and
the result says so: `Filled 1 field on example.com: Role = Administrator (matched "administrator").`
The facts carry `"match":"normalized"`. No match, or several, is refused with the options. There is
no prefix, substring or closest match, and Ghostlight never picks an option the value does not
name. An option that is disabled, or inside a disabled option group, is refused as disabled. An
empty string picks the empty placeholder option, and the result reads `no value`; no other text
picks a placeholder. A multiple select takes a list, each entry resolved by these rules, and ends
up holding exactly that set; an empty list clears it. A single string on a multiple select picks
only that option. A list given to any control that takes one value is refused. A checkbox takes a
boolean or one of `true`, `false`, `yes`, `no`, `on`, `off`, `1`, `0`, `checked`, `unchecked`
without regard to case, sets that state and never toggles it. Any other word is refused and
nothing changes. A radio takes a boolean for itself. A string is matched, by the select rules,
against the labels of the radios in its group (same name, form owner and root) and picks the radio
it names, which may be a sibling of the target. A string that names no label but is one of the
boolean words applies to the target radio. Any other string is refused with the group and its
labels.

A fill that cannot take its values refuses before any edit and says what would succeed
(ADR-0198 Decisions 8 and 10). The page checks every field first, so one refused call names every
field that cannot take its value, never only the first. The result has status `failed`, effect
`none`, and is safe to repeat. The summary starts `Nothing was changed.`; when more than one field
is refused it starts `Nothing was changed: N of M fields cannot take their values.` Then one clause
follows for each field, for example `The "Role" dropdown has no option "Engineer". It offers:
Developer, Administrator, Researcher, Other.` The next step names the handle and the route, at most
two steps in all: `Repeat browser_fill_form with target_... set to one of the offered options.`

The clauses by problem word:

| Problem | Clause | Next step |
| --- | --- | --- |
| `option_missing`, 25 options or fewer | `The "<n>" dropdown has no option "<v>". It offers: A, B, C.` When the names are too wide for one clause: `It offers: A, B and 12 more, all in the facts.` | `Repeat browser_fill_form with target_... set to one of the offered options.` |
| `option_missing`, a longer list, or a list the page sent without its options | `... has no option "<v>" among its 195 options.` and `Options containing "<v>": X, Y.` when any | `Read target_... with browser_read to list all its options.` |
| `option_missing`, after an earlier choice field in the call | the clause above, then `Its options can depend on earlier fields.` | `Fill the earlier fields first, then repeat for target_....` |
| `option_missing` on a radio button | `The "<group>" choice has no option "<v>". It offers: Standard, Express.` | `Repeat with one of the offered labels for target_....` |
| `option_ambiguous` | `"<v>" matches 2 options of the "<n>" dropdown: Other (value other-staff), Other (value other-guest).` For a list, `<v>` is the entry that matched twice. | `Repeat with the option's value for target_....` On a radio: `Find the radio you mean with browser_find, then fill that radio's own handle with true.` |
| `option_disabled` | `The option "<v>" of the "<n>" dropdown is disabled.` | `Choose an enabled option, or inspect the form for the field that enables it.` |
| `multiple_values` | `The "<n>" dropdown takes one value, not a list.` | `Repeat browser_fill_form with target_... set to a single value.` |
| `not_boolean` | `The "<n>" checkbox takes true or false, not "<v>".` | `Repeat with true or false for target_....` |
| `format` | `The "<n>" date field takes YYYY-MM-DD, not "<v>".` and `Allowed: 2026-10-01 to 2026-12-31.` when the control declares limits. A range always has them: `Allowed: 0 to 100, in steps of 10.` | `Repeat browser_fill_form with target_... set to a corrected value.` With two or more fields to correct (`format`, `not_boolean`, `multiple_values`), one shared step: `Repeat browser_fill_form with corrected values for target_14 and target_15.` |
| `wrong_tool`, a file input | `The "<n>" field is a file input.` | `Attach the file with browser_upload on target_....` |
| `wrong_tool`, a button, or a submit, reset or image input, whatever role it carries | `The "<n>" control is a button, not a field.` | `Click it with browser_click on target_....` |
| `wrong_tool`, a page widget | `The "<n>" dropdown is a page widget, not a form field.` | `Open it with browser_click on target_..., then browser_click the option by name.` |
| `submit_not_owned` | `The submit control does not belong to the form that holds these fields.` | `Repeat without submit_target, then click the submit control with browser_click.` |

Typed inputs take the ISO shapes (ADR-0198 Decision 10). The page checks `date`, `time`,
`datetime-local`, `month`, `week`, `number`, `range` and `color` before any edit by assigning the
value to a detached input of the same type with the same `min`, `max` and `step`, and reading it
back. The value must come back as given and inside the declared range. A range input also moves
a value to its nearest step and into its bounds, whatever the page declares (0 to 100 in steps of
1 when it declares none), so a range value it would move is refused and the refusal names the
bounds and the step in force. A range with the step `any` is clamped but not snapped, so its
refusal names the bounds only. A number written in another form, such as `20.0` for `20`, is the
same number and is accepted. The shapes are `YYYY-MM-DD`,
`HH:MM`, `YYYY-MM-DDTHH:MM`, `YYYY-MM`, `YYYY-Www`, `a number` and `#rrggbb`. There is no lenient
date parsing: `10/14/2026` is refused, not guessed at. A color compares without regard to case. An
empty value clears a field, except a range, which has none. A number input keeps a value that does
not land on its step and is not refused; the field's invalid state shows it. Wrong tools are named instead of failing in the page: a file input (use
`browser_upload`), a `<button>` element or a submit, reset or image input (use `browser_click`),
and an element that is not a form control but has the role `combobox` or `listbox` (use
`browser_click`). The page names the route from the element itself, so a button with the role
`switch`, `tab` or `menuitem`, or an image input, is a button and never a widget. One call
with a bad date, a checkbox word that is not a boolean and a file input ends with every one of them
named, nothing edited, and two steps: `Repeat browser_fill_form with corrected values for target_14
and target_15.` and `Attach the file with browser_upload on target_16.`

The control noun comes from its role and input type (`dropdown`, `date field`, `checkbox`, `text
field`). A given value is quoted up to 40 characters, on one line. Page text in a clause (option
text, group names) is bounded, quotes become single quotes, and anything shaped like a handle
becomes `[handle]`. The summary stays within 500 characters: clauses that do not fit are counted
(`N more are in the facts.`) and the facts carry them all. Names, option text and the given value
reach a sentence only when `preserve_target_names` is on. With it off, a clause uses the unnamed
noun (`A dropdown has no matching option.`) and the options appear in the facts only. A
credential-class control never lists options or echoes a value: `The "<n>" text field is
credential-class.`

Facts of a refused fill: `{"reason":"field_value_rejected","problems":[{"field":0,"target":"target_...",
"problem":"option_missing","options":["Developer","Administrator","Researcher","Other"]}]}`. Each
problem carries the index into `fields`, the field's handle, the problem word and what would
succeed (`options`, `options_total`, `containing`, `expected`, `limits`, `group`). The facts never
hold the given value. The audit keeps one count and no names, options or values: `Refused a form
fill before any edit: N fields could not take their values.`

A fill says what the page now holds (ADR-0198 Decision 11). After the edits the page checks that
every field kept its value, and reads back the fields worth reading: choice and typed controls
(select, radio, checkbox, the date family, number, range, color), and a text field whose retained
value differs from the given one only in formatting. Free text that equals what was given is not
read back, and a credential-class control never is. The comparison removes white space, `(`, `)`,
`-` and `/` and folds case on both sides; `.` and `,` stay significant, so `10.50` held as `1050`
is a real difference. A tolerated difference is reported, not refused.

With `preserve_target_names` on, the summary names each reading: `Filled 3 fields on example.com:
Role = Administrator, Shipping method = Express. Phone now reads "(555) 123-4567".` A choice or
typed control reads as `<name> = <value>` and a reformatted text field as its own sentence. Values
are bounded to 40 characters on one line, and readings the summary has no room for are counted
(`N readings more in the facts`). With it off the summary is `Filled 3 fields on example.com.` The
facts carry every reading either way, because the fill was authorized to read the document:
`"fields":[{"target":"target_12","now":"Administrator"},{"target":"target_9","now":"(555) 123-4567"}]`.
A checkbox or radio button reads `checked` or `unchecked`. A radio picked through a sibling's label
is not the radio the handle names, so the handle keeps its own reading and the fact adds the pick:
`{"target":"target_12","now":"unchecked","picked":"Pro","group":"Plan"}`. The sentence speaks of
the group, `Plan = Pro`, or `Pro is checked` when the group has no name, and only when names are
preserved. The page focuses the picked radio and sends it the input and change events. A choice the page matched only after normalizing reads `Role = Administrator
(matched "administrator")` with the value the caller gave, and its fact adds `"match":"normalized"`;
an exact match carries no mark. The audit keeps `Filled 3 fields on example.com.` and no name or
value.

When the page changes a field back after the fill set it, the fill is partly applied and says so.
The result has status `failed`, effect `partial`, and is not safe to repeat. The summary is `Set 2
of 3 fields; the "Role" dropdown went back to "Choose one..." after the page reacted.`; fields past
the first three are counted, and a control that holds nothing reads `no value`. With names off the
control is unnamed and its value is left out (`A dropdown went back`). The next step is `Read
target_... with browser_read to see its current value.`, at most two in all. The facts are
`{"reason":"fields_reverted","set":2,"reverted":[{"field":0,"target":"target_...","problem":"reverted",
"now":"Choose one..."}]}`; a credential-class field never carries `now`. A submit control is not
clicked after a revert. The audit keeps one count: `A form fill was partly applied: N fields held
their values.` A page that never settles within the fill's budget, with no field it can name, still
leaves the effect unknown.

Credential-class targets return guidance before any grouped value dispatch unless
`user_authorized_credentials` is `true`. The guidance names the control and its handle:
`Nothing was typed. The "<n>" text field (target_...) is credential-class.` Its next step is `If
the user explicitly authorized it, repeat with user_authorized_credentials true; otherwise ask the
user to type it.` Set it only when the user explicitly authorized credential
entry; an existing instruction in the conversation is sufficient, including on the first call.
This per-request caller acknowledgement is not independently verified proof of user intent and
does not override configured policy or human controls. Missing acknowledgement creates no session
hold or resume requirement. Values never enter audit, diagnostics, or presentation. Facts:
`tab`, `filled_count`, `submitted`, and any governed committed landing.

### `browser_type_text`

Type text through browser input events. Shortest call:
`{"target":"target_...","text":"Ada"}`.

Rules: one location: `target`, `selector`, or `focused:true` to type into the currently focused
editable control. Empty `text` is valid only as an explicit clear together with `clear_first:true`.
Capabilities: `action`, plus `read` when `selector` or `expect` is supplied. Targeted and focused
typing without a postcondition require `action`.

Credential-class targets return guidance before text dispatch unless
`user_authorized_credentials` is `true`, with the same per-request acknowledgement semantics as
`browser_fill_form`. This applies to both targeted and focused typing and creates no session hold.
Facts: `tab`, `target`, `typed`,
`character_count`, and any governed committed landing.

### `browser_press_key`

Send one explicit keyboard action. Shortest call: `{"key":"Enter"}`.

Rules: exactly one of `key` or `strokes`, whose items are one character or one named key from the
closed list. Modifiers are held for every stroke, so a shortcut is a key with modifiers:
`{"key":"a","modifiers":["Control"]}`, or `{"strokes":["a","c"],"modifiers":["Control"]}` to select
all and copy. A modifier is never a stroke. Capabilities: `action`, plus `read` when `expect` is
supplied.

Facts: `tab`, `key`, `pressed`, and any governed committed landing.

The optional `expect` on click, fill, type, and key calls is checked after the action for at most
2 seconds. Rules: `load_ready` takes no `value`; `url_contains`, `text_present` and `text_absent`
require a non-empty `value`. Lookup and postcondition Read requirements are admitted with the
complete request before an effect. Text conditions match page text, not field values.

When the check does not confirm the expectation, the action still happened. The result keeps
`effect: applied`, `repeat_safe: false`, and the action's own facts, and adds a `reason`. The
summary is the action's sentence followed by why the check fell short:

| Check | Status | `reason` | Summary after the action's sentence |
| --- | --- | --- | --- |
| Ran, condition false | `failed` | `expectation_unmet` | `The expected condition did not hold afterward.` |
| Caller cancelled | `cancelled` | `cancelled` | `The expected condition was not checked because the call was cancelled.` |
| Out of time | `failed` | `deadline` | `The expected condition was not checked because the call ran out of time.` |
| Browser disconnected | `failed` | `browser_disconnected` | `The expected condition was not checked because the browser disconnected.` |
| No browser reachable | `failed` | `browser_unavailable` | `The expected condition was not checked because Ghostlight could not reach the browser.` |
| Page access unverified | `failed` | `document_unavailable` | `The expected condition was not checked because access to the page could not be verified.` |
| Browser could not check | `failed` | `browser_refused` | `The expected condition was not checked because the browser could not run the check.` |
| Policy refused the check | `blocked` | the policy reason | `The expected condition was not checked.` and the policy sentence |

The next step is `Check the page with browser_read or browser_inspect before repeating; the action
already happened.` A policy refusal points to `policy_explain` instead. A
history-storage refusal and an unreadable policy give no step. Pause and Stop put their fixed
directive first, then the action's sentence and `The expected condition was not checked.`, with no
next step. History records `action_unconfirmed` with the same cause, and a flow treats the step by
that cause: a Pause, Stop, cancellation, or deadline during the check stops the flow as it would
for any step.

### `browser_drag`

Drag one semantic target to another, or drag between two points in a current screenshot. Shortest
call: `{"source_target":"target_...","destination_target":"target_..."}`.

Rules: exactly one branch: `source_target` and `destination_target`; or `view`, `start_x`,
`start_y`, `end_x`, and `end_y`. Capability: `action`.

Facts: `tab`, `dragged`, and any governed committed landing.

### `browser_wait`

Wait for one explicit observable condition. Shortest call: `{"condition":"load_ready"}`.

Rules: each condition has its own branch. `load_ready` accepts neither `value` nor `target`;
`url_contains`, `text_present`, and `text_absent` require `value`; `target_present` and
`target_absent` require `target`; `selector_present` requires a typed `selector` and polls the
live page until a control matching it exists; `duration` requires a decimal string `value`
of whole milliseconds (for example, `"1200"`) and waits executor-side; `visual_settle` is met when
the page, or an optional `target`, stops moving and repainting; `layout_stable` was the same
condition and is refused with a pointer to `visual_settle`. The optional `visual_settle` field adds
a short layout settle after the condition holds. Its default depends on the condition, so the
schema declares none. Capability: `read`.

Text conditions match composed visible text across the top document, open shadow roots, assigned
slots, and http(s) embedded frames. Hidden content and closed roots remain absent. Text conditions
match page text, not field values. The expected `value` is normalized: a whitespace run holding a
newline matches one page line break and any other whitespace run matches one space.

Facts: `tab`, `condition`, `satisfied`, `elapsed_ms`, and governed readiness.

### `browser_dialog`

Inspect or resolve the current JavaScript dialog.

- `{"action":"status"}` reports whether a dialog is blocking; capability `read`.
- `{"action":"accept"}` accepts it; capability `action`.
- `{"action":"dismiss"}` dismisses it; capability `action`.
- `{"action":"respond","text":"Ada"}` supplies non-secret prompt text; capability `action`.

`text` is required only for `respond` and is invalid for every other action. Facts: `tab`,
`dialog_type`, `present`, `accepted`, and `handled` as applicable. Status also reports the dialog's message, one line of at most 500 characters, in its
summary. Dialog text is never audited. When status finds a dialog, its next step is `Call
browser_dialog with action accept or dismiss (or respond with text for a prompt) before
continuing.`

A dialog is answered by the browser from its own record and authorized by the one document that
raised it, not by the tab's frame inventory, because a dialog blocks the page and the inventory
cannot be read while one is open. A dialog raised by a document the authority refuses is refused
as document access and its message is not returned. A dialog whose raising document the browser
cannot name fails closed as `Document access could not be verified.` `accept`, `dismiss` and
`respond` ask the browser once; when no dialog is showing the result is `No JavaScript dialog is
currently visible.`

Any other page work sent while a dialog is open is refused at once instead of waiting out its
deadline: `A JavaScript dialog is open on this page, so nothing was sent.` The status is `failed`,
nothing changed, repeating is safe, the `reason` fact is `dialog_open`, and the next step is the
dialog step above. If the page work had already dispatched an effect, the result is an unknown
effect as for any other failure after dispatch.

A dialog the page opens while an action runs is reported by the browser as it opens. When the
action completes with a dialog open on its tab, the result keeps the action's own sentence, adds
`A confirm dialog is now open.` (or alert, prompt, leave-page, or a plain JavaScript dialog), carries a
`dialog_opened` fact naming the kind, and takes the dialog next step above. This holds for clicks,
key presses, typing, form fills, drags, uploads, scripts, scrolls, hovers and navigation.

### `browser_upload`

Attach explicitly supplied files or one captured image to one ordinary file input, or drop one
captured image at a point in a current view. Shortest call:
`{"target":"target_...","paths":["C:\\path\\document.pdf"]}`.

Rules: exactly one source: `paths`, or `files`, or one `source_image` handle from an earlier
capture, optionally with `view`, `x`, and `y` to drop it at a point instead of attaching. The
destination is `target` or typed semantic `selector`. Capabilities: `write`, plus `read` when
`selector` is supplied. Target-handle attachment and current-view image drops require `write`.

As with form fill, a semantic selector can address an ordinary file input outside an HTML `form`.

Ghostlight rejects directories, missing files, any file larger than 5,000,000 bytes, and a combined
payload larger than 5,000,000 bytes before browser dispatch, and refuses an upload above the
capture-reuse ceiling. Inline bytes decode only after authorization and file-target preflight.
Credential-like metadata on a file control does not turn an explicit upload into credential entry. File
paths, names, and contents never enter audit or presentation. Facts: `tab`, `target`,
`uploaded_count`, and `uploaded_bytes`.

### `browser_execute`

Execute explicit bounded JavaScript in the page. It may read, mutate, or navigate, so use a
semantic tool when one fits. Shortest call: `{"script":"document.title"}`.

Capability: `execute`.

The service sends a script that its lexical scan proves to be one expression as `await (<script>)`,
so a promise it evaluates to resolves. The scan declines anything it cannot read with certainty:
statements and declarations, a top-level `;` or `return`, a `/` that is not a comment, unpaired
brackets, and unterminated strings, templates or comments. If the adapter refuses that form before
evaluation, the original script is sent instead, once, and only after that refusal as an invalid request
that came before any evaluation. Any other script is sent as written. The adapter
parses the source before dispatch and selects the async form for a top-level return. Scripts that
are not wrapped retain REPL scope, including top-level await and repeated declarations. A local
syntax rejection sends no page evaluation. Browser evaluation happens at most once: a runtime
exception or lost reply remains uncertain regardless of its text or exception class, and only a
refusal before any evaluation allows the original to be sent.

A promise that the script evaluates to, in an awaited form, is waited for. A promise that rejects
reports an unknown effect with the rejection as its description, as a thrown exception does,
because the script may already have acted. A promise that never settles holds the call until
`timeout_ms` ends it.

A `{}` value adds one next step that names what else reads as `{}`: a DOM node, a Map or a class
instance, and, for a script sent as written, an unawaited promise.

Facts: `tab`, `value`, `truncated`, and any governed committed landing. Script source and result
never enter audit or presentation.

### `browser_flow`

Compose one to twenty ordinary tool calls. A short batch:
`{"steps":[{"tool":"browser_click","arguments":{"target":"target_..."}},{"tool":"browser_wait","arguments":{"condition":"load_ready"}}]}`.

Rules: all step IDs must be unique. A step's `arguments` default to `{}`. `tab` supplies a default
for tab-scoped children; explicit child tabs win, new-tab navigation and tab-independent work keep
their ordinary meaning. `timeout_ms` bounds the entire flow. There is no simulation mode.

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
non-success when any child fails. Human pause/stop, a local interlock, cancellation, and deadlines end
execution even under Continue.

Captured per-step envelopes stop being recorded past a bounded byte budget while execution continues to a truthful
terminal aggregate. Facts: `completed`, `total`, `stopped`, `progress`, and bounded per-step rows
with each step's envelope where the budget allowed. `completed` counts successes. References still
resolve against the full volatile envelope when its client payload is omitted.

Flow uses this progress contract; historical sequence receipts retain it:

- `progress.counts`: `total`, `succeeded`, `failed`, `blocked`, `cancelled`, `unknown`,
  `not_started`, and `not_run`. The categories are disjoint and sum to `total`.
  `not_started` means runtime reference resolution or decoding prevented entering a child;
  `not_run` means execution never reached it. Neither fabricates a failed child invocation.
- `progress.effects`: counts of entered children with `none`, `applied`, `partial`, and `unknown`
  effects. A later unknown never removes earlier known applied or partial effects.
- `progress.stopped` says execution ended at a stopping boundary. `progress.issue`, when present,
  names a one-based step and closed cause relevant to recovery. Human directives take precedence
  over interlock/invocation limits, then uncertainty, then ordinary failures.
- Every step row keeps its one-based position, status, effect, and repeat safety. Entered rows
  carry their closed cause when relevant; unresolved inputs also retain their client error.
  Omitted payloads cannot remove this metadata. Not-run rows contain no invented child result.
- Aggregate status remains in the ordinary vocabulary. Unknown effects/status take precedence,
  then cancellation, blocks, and failure. Success requires every step to succeed.
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

- `start`: optional `tab`; ask the extension to start the session's recording; capability `read`.
  A session holds one recording. Starting again on the tab it is still capturing answers with that
  recording. A stopped recording that was never saved refuses the start with
  `This session already has a recording.` and the next step to save or discard it. A recording
  that was saved is replaced: the start discards it and begins the new one.
- `status`: report state and deadlines; no new capability.
- `stop`: capture a final frame, stop, and freeze; no new capability.
- `save`: auto-stop if active. One replay goes to one place: with `target`,
  the browser attaches it to that file input and Ghostlight requires `read + write`; with
  `"download": true`, the browser saves it as a file and Ghostlight requires `read`; with neither,
  the GIF is returned to the client and Ghostlight requires `read`. `target` and `download`
  together are refused. Credential-like metadata on a file input does not block an explicit save.
- `discard`: erase captured bytes; no new capability.

Save checks its complete capability requirement before stopping active capture. Read still
authorizes every captured source; Write separately authorizes the attachment destination.

No call names a recording, and no result reports one: the session has one or none, and an action
with none answers `This session has no recording.` `target` and `download` are valid only for
save. The extension owns recording identity, frames, bounds,
deadlines, stop, retention, erase, and the encode. Frames never leave the browser, and neither
frames nor encoded bytes are written to Ghostlight storage, extension storage, logs, audit, or
restart state. Save can be repeated until retention expires or the next start replaces the
recording; a target or download delivery is a real effect each time and is not thereby
repeat-safe. A save after retention expires is a refusal, not an empty result. Discard is destructive.

A recording also stops on its own when the page, or any frame inside it, loads new content or
goes away: a navigation or reload in any frame, or a frame added or removed. Status then reads
`Recording stopped after 12 seconds because the page, or a frame inside it, loaded new content or
went away. The captured portion is available.` with `stop_reason` `document_boundary`. It never
claims the tab itself moved, because often it did not.

A saved replay's sentence says how long it plays and where it went, because that is what someone
who asked for a recording wants to know. How many frames survived, how many were captured, and how
many bytes they became are real and stay in the facts, alongside state, deadlines,
stop reason, exact `duration_ms`, dimensions, and the delivery disposition. A client save returns one bounded
`image/gif` content block; the other two return none, and neither claims remote acceptance.

### `browser_diagnose`

Read bounded console and network evidence for a controlled tab. Tracking is opt-in. Shortest call:
`{}`, which selects both sources and returns problems only.

Capability: `read`.

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

Capability: empty requirement set (always available).
Read-only, never dispatches a browser, holds no workspace lease, and writes nothing.
It remains available during Pause or Stop and a required audit outage.
It does not change controls. Cancellation, deadlines, bounded admission, and
the ordinary completion/audit path still apply.

The result carries the orchestrator's compiled projection -- the same compilation the workbench
Policy destination renders -- with layer document texts and filesystem paths withheld from model
results. The summary names its measurements in plain words, for example `Explained what agents
may do: 4 capability areas, no policy layers.`

## Explicit human control and request guidance (ADR-0185)

Enforced denials retain their actual policy explanation regardless of how often they occur; they
never open a session hold or require review/resume. Credential input without the user's explicit
authorization returns guidance for that request and leaves later permitted work available.
Flow error handling follows the requested stop/continue choice for these ordinary refusals.
Pause and Stop remain human controls, not a denial or credential threshold.

Pause and Stop keep their fixed directives at the final dispatch check. A late refusal preserves
already acknowledged effects, including when it prevents a separate postcondition observation.
No-effect claims apply only to work that has not dispatched; lost effectful replies remain unknown.

## Document coverage (ADR-0158)

Scoped results carry `facts.coverage` in the canonical envelope: targeted versus whole-page
scope, inspected/excluded/page excluded/unavailable document counts and `limited_by_size`. Counts retain
observed maxima across physical preparation and execution, not repeated-read sums. Policy
exclusions, unavailable content, and size ceilings have separate qualifications. Negative findings
are limited to inspected content; absence across unseen content is never established.

The model view carries `facts.coverage` only when something limited the read: an excluded, page
excluded or unavailable document, or `limited_by_size`. A read without it saw every
document in scope. A composition carries a limited child's coverage even when the child's result
was cut from the parent (ADR-0198 Decision 22). The durable audit is unchanged.

Human notice preferences do not remove coverage from model results. Excluded document hosts,
identities, URLs, labels, values, and locators are not copied into the model/audit coverage.
The workbench keeps bounded excluded host names only in volatile human details.
Screenshots use the separate binary `browser.screenshots.enabled` permission (ADR-0195): an allowed
capture returns the complete rendered image, including embedded content. It has no frame inventory,
partial coverage, or masking. Tab Read checks still apply. Frame grants continue to govern text,
targets, and coordinate actions independently. Unverifiable document access and recording stops at the document boundary
have language-owned outcomes. No script scanner or retry is treated as document containment.

## History storage (ADR-0159)

Every terminal envelope also carries `history_storage`: `saved` or `unconfirmed`. The model view
omits `saved`, so an absent key means saved. This describes
receipt storage, independently of status, effect, readiness, and repeat safety. An unconfirmed
receipt adds "History could not be saved." to the action's actual outcome. Composition facts and
safe parent audit metadata retain `unconfirmed_history_steps`; a saved parent cannot confirm its
children's storage. No audit error offers automatic replay.

`policy_explain` includes the effective `audit` choice and content-free `audit_health`. It remains
available during an audit outage. Filesystem paths and raw operating-system errors are excluded.
Require audit uses `audit_unavailable` with authored policy attribution and refuses new browser
work during a known failure. The same typed cause stops a composition under Continue, preserves
prior effects, and directs the client to prepare only unfinished work after recovery.
