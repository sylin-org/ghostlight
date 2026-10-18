# Latest result

## [0040] linux-1: L07 passes after five fixes; the model's answers about captures and recordings

linux-claude, on test-01, on the real installation with the system Chromium. Details:
[the ledger](../docs/tasks/linux-1/LEDGER.md), L07.

Passed: the popup in Ready, browsing, Paused, Session ended, Not connected, Recording and
Release debugger states; the options page's connection card and switches; the setup page from
waiting to connected; real agent work on every demo stage L07 names; `browser.startup` manual
(names Chromium, starts nothing) and on_demand (starts the ordinary profile).

Fixed, each deployed and checked:

- efe54caf `fix(browser)`: a screenshot of a session tab its window does not show waited 8 s (or
  30 s) for a frame Chromium never draws and told the model to raise the timeout. It is refused at
  once as `tab_not_showing`, with the focus step. Adds `BrowserError::TabNotShowing` and
  `Refusal::TabNotShowing`; captures first send `list_tabs` for that tab.
- 5c4adb4d `fix(diagnostics)`: `diagnostics show` no longer runs the invocation id into the tool.
- be54ad44 `fix(language)`: a stopped recording's status no longer says "Recording ... so far".
- 6734f99b `fix(language)`: a recording save the page did not take (target not a file input) is
  `recording_not_attached` and keeps the recording, instead of "could not be turned into a replay"
  with advice to discard it. The adapter recognizes the extension's fixed reason
  (`wire::PAGE_REFUSED_REPLAY`).
- 7769fd97 `fix(browser)`: the browser the service starts on demand is reaped and has its own
  process group (it was a `<defunct>` child of the service).

For the owner or the coordinator (not changed): on plain Chromium the extension reports no
browser name, so the ADR-0149 last-browser record is never written for it; an upload calls a file
input a "text field" because the page assets give it the textbox role.

Please pull before touching `translate/capture.rs`, `translate/records.rs`, `work/recording.rs`,
`browser/recovery.rs` or the recording and capture refusals in `language/outcome.rs`. My claims
on them end here.

Next: L08, setup on a fresh home.
