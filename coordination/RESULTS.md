# Latest result

## [0048] linux-claude: a kept tab leaves Ghostlight's group, so saved groups stop piling up

linux-claude, on test-01, at the owner's request ("Unfreeze the extension. I want a proper fix.").
Record: [ADR-0201](../docs/adr/0201-a-kept-tab-leaves-the-group.md) and the ADR-0200 amendment of
2026-10-18.

- 0018941b `fix(tabs)`: when preserve-tabs keeps a released tab open, the extension ungroups it from
  Ghostlight's group, so a group with no live session's tab is deleted by Chromium instead of saved
  at the next window close or browser exit. Reuse sends `BrowserCommand::AdoptTab` (the adapter's
  `open_tab` with `adopt_tab_id`) instead of `Navigate`, which puts the kept tab back in its client's
  group. A tab the person put in a group of their own stays there both ways.
- Measured on Chromium 154.0.8037.97: a group emptied by closing or ungrouping its tabs is deleted;
  a group still open at a window close or browser exit is saved; no extension API can find, reopen
  or delete a saved group, and `chrome.sessions.restore` reopens one only with every page it saved.
- Live check (throwaway profile, real extension, installed service): ten groups from sessions that
  ended left no saved group; a session live at browser close left one, the limit ADR-0201 records.
- Packaged extension: only `lib/tabs.js` changed; the freeze golden is regenerated from the new zip.
  The version stays 2.0.0 because the tree records no 2.0 store submission yet.

For the owner: the existing duplicate saved groups in your browser are untouched. Your Chromium
runs the new extension code after a reload of Ghostlight in `chrome://extensions`.
