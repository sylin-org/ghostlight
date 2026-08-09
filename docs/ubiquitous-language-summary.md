# Ghostlight ubiquitous language: quick reference

Status: Non-normative summary of the accepted ADR-0102 contract

Ghostlight gives models obvious browser tools that work with short calls, fill in safe defaults,
report exactly what happened, and offer useful recovery when work cannot continue.

For exact descriptions, schemas, bounds, and design decisions, use the
[full ubiquitous-language primer](ubiquitous-language.md). ADR-0102 is the production authority
for implementation.

The companion [governance quick reference](governance-language-summary.md) and
[full governance primer](governance-language.md) define policy, settings, runtime safety, audit,
and recovery. Every supported MCP revision renders the same Ghostlight contract.

## Most tasks

Start with the browser job itself. Do not call status, list tabs, take a screenshot, or wait as a
routine setup step.

- Go somewhere with `browser_navigate`.
- Find controls with `browser_inspect_page`.
- Read prose with `browser_read_page`.
- Act with `browser_click` or `browser_fill_form`, and explore with `browser_scroll_page`.
- Use `browser_take_screenshot` only when appearance or spatial layout matters.

Navigation and semantic actions already wait briefly for the page to become usable.

## Only when needed

- Use status and tab tools to diagnose availability, create a separate tab, recover a tab handle,
  deliberately select a tab, or close one.
- Use back, forward, or reload only when that exact navigation is intended.
- Use target reveal, hover, a targeted key press, Escape, or drag only when the page requires that
  interaction. Ordinary actions reveal their own exact target automatically.
- Use `browser_wait_for` for one named condition, not as a generic delay.
- Use `browser_run_sequence` only for 2 to 10 page calls on one tab when every step and argument is
  already known. Status and tab-management calls stay separate.
- Inspect or handle a dialog only when a JavaScript dialog is relevant.

## Choose a tool

The working core has 24 tools. The count is an outcome, not a target.

| Need | Tool | Shortest call |
| --- | --- | --- |
| Check whether browser work is available | `browser_get_status` | `{}` |
| Open a separate blank tab | `browser_open_tab` | `{}` |
| Open a separate tab at a URL | `browser_open_tab` | `{"url":"https://example.com"}` |
| List controlled tabs | `browser_list_tabs` | `{}` |
| Show a tab and select it as current | `browser_focus_tab` | `{"tab":"t_example"}` |
| Close a named tab | `browser_close_tab` | `{"tab":"t_example"}` |
| Open a URL in the current tab | `browser_navigate` | `{"url":"https://example.com"}` |
| Go back one page | `browser_go_back` | `{}` |
| Go forward one page | `browser_go_forward` | `{}` |
| Reload the current page | `browser_reload_page` | `{}` |
| Find controls or page structure | `browser_inspect_page` | `{}` |
| Read page text | `browser_read_page` | `{}` |
| Capture page appearance | `browser_take_screenshot` | `{}` |
| Click a target | `browser_click` | `{"target":"Save"}` |
| Hover over a target | `browser_hover` | `{"target":"Account menu"}` |
| Bring a known target into view | `browser_scroll_to_target` | `{"target":"Pricing"}` |
| Explore above or below the viewport | `browser_scroll_page` | `{"direction":"down"}` |
| Press a named non-text key on a target | `browser_press_key` | `{"key":"Enter","target":"Search"}` |
| Send Escape to the page | `browser_press_escape` | `{}` |
| Drag between two targets | `browser_drag` | `{"from":"Card A","to":"Done"}` |
| Fill one or more fields | `browser_fill_form` | `{"fields":[{"field":"Email","value":"person@example.com"}]}` |
| Wait for a named condition | `browser_wait_for` | `{"condition":"Saved"}` |
| Run fixed known page calls together | `browser_run_sequence` | `{"steps":[{"tool":"browser_navigate","arguments":{"url":"https://example.com"}},{"tool":"browser_read_page"}]}` |
| Inspect the current dialog | `browser_get_dialog` | `{}` |
| Resolve the current dialog | `browser_handle_dialog` | `{"action":"accept"}` |

Choice boundaries:

- Open a tab when a separate tab is part of the intent. Navigate to reuse the current tab.
- Inspect for controls and fresh targets. Read for prose. Take a screenshot for appearance.
- Scroll to a target when you know what you need. Scroll the page to explore.
- Ordinary operations settle automatically. Wait only for a specific named condition.
- Fill form fields with `browser_fill_form`. Use `browser_press_key` for non-text keys.
- Sequence only page calls whose arguments are known before the sequence starts. Put every child
  input inside `arguments`.
  Steps share one root tab, so child `arguments` never contain `tab`. Use separate calls when a
  later step depends on an earlier result or work must cross tabs.
- Get a dialog to observe it. Handle a dialog only when the resolution is known.

## What you can omit

Ghostlight applies these safe defaults:

- A sessionful client supplies its workspace automatically.
- An omitted `tab` means the workspace's selected current tab. `browser_focus_tab` changes that
  selection as well as showing the tab to the user.
- `browser_navigate` creates the first workspace and tab when none exists.
- Navigation and semantic actions settle automatically. Navigation has a fixed 10-second
  readiness budget.
- Page inspection returns interactive controls by default.
- Page reading returns at most 20,000 characters by default.
- Screenshots capture the current viewport by default.
- Clicks use the left button once by default.
- Form filling does not submit unless `submit_target` names the exact control to activate.
- Sequences stop at the first non-success and never roll back completed effects.

Ghostlight does not guess a workspace or tab when no exact current selection exists. It also does
not guess a URL, an ambiguous or stale target, a risky action, a credential, or whether an uncertain
effect should be repeated.

## Common calls

Navigate, then read the landed page.

`browser_navigate`:

```json
{"url":"https://example.com"}
```

`browser_read_page`:

```json
{}
```

Find a control, then click the fresh ref returned by inspection:

`browser_inspect_page`:

```json
{"query":"Save changes"}
```

`browser_click`:

```json
{"target":"r_save"}
```

Fill fields without submitting with `browser_fill_form`:

```json
{
  "fields": [
    {"field":"Name","value":"Ada"},
    {"field":"Email","value":"ada@example.com"}
  ]
}
```

To submit with `browser_fill_form`, name the exact control instead of sending a boolean:

```json
{
  "fields": [{"field":"Search","value":"Ghostlight"}],
  "submit_target": "Search"
}
```

Wait for one explicit condition with `browser_wait_for`:

```json
{"condition":"Saved"}
```

Wait for something to disappear:

```json
{"condition":"Loading","state":"gone"}
```

There is no empty generic wait call. If no condition is known, inspect the page instead.

## Results and recovery

Every result answers four questions:

- `status`: did the requested job complete?
- `effect`: did the browser change, and can Ghostlight prove it?
- `repeat`: is repeating the same call `safe`, something to `check_state_first`, or something to
  `do_not_repeat`?
- `summary`: what happened, in one short service-authored sentence?

Readiness is separate. A navigation can commit successfully and still time out while the page
settles. Never repeat a committed or uncertain effect just because later observation failed.

When useful, a result may include up to two `suggested_next_steps`. They are advisory. They never
run automatically, grant authority, bypass governance, or include page-authored instructions or
secret values.

| Situation | Safe next move | Never do |
| --- | --- | --- |
| Target is stale or ambiguous | Inspect again, optionally with a narrower query | Guess or click the old target |
| Tab is unavailable | List tabs, open a new tab, or ask the user | Claim the tab was definitely closed |
| A committed navigation timed out while settling | Inspect, read, or wait for one specific condition | Repeat the navigation blindly |
| Effect is unknown | Observe current state or ask the user | Replay the action |
| A field requires a credential or one-time code | Hand control to the user | Find another way to enter the secret |
| A sequence stopped after committed steps | Continue only from steps proven not to have run | Run the whole sequence again |

Example recovery:

```json
{
  "status": "blocked",
  "summary": "That tab is not available in this workspace.",
  "effect": "none",
  "repeat": "check_state_first",
  "problem": {
    "code": "tab_unavailable",
    "message": "The tab may have been closed or released."
  },
  "suggested_next_steps": [
    {
      "kind": "call",
      "tool": "browser_list_tabs",
      "arguments": {},
      "reason": "Recover another tab already controlled by this workspace."
    },
    {
      "kind": "ask_user",
      "question": "Would you like me to continue in a new tab?",
      "reason": "The previous tab is no longer available."
    }
  ]
}
```

## Specialist capabilities

Opt-in packs hold specialist work such as governance preflight, narration, coordinate input, file
transfer, diagnostics, unsafe JavaScript, recording, and future multi-browser selection. Ordinary
models should not see a pack until its complete capability is real and enabled.

Governance, audit, ownership, user hold, attention, activity cues, and credential handoff are
service behavior, not setup tools the model must call.

## Authority

This quick reference reflects the accepted clean-slate contract, including
`browser_scroll_to_target`, `submit_target`, explicit condition waits, focus-as-current selection,
and the `repeat` result field. The full primer owns the exact working definitions.

Ghostlight exposes no Legacy, Claude, Codex, or other vendor dialect. Client identity never changes
the catalog, canonical result, governance, or execution path.
