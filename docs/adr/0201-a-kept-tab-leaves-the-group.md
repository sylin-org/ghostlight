# ADR-0201: A kept tab leaves Ghostlight's group

Date: 2026-10-18. Status: Accepted by owner direction (2026-10-18: "tab groups should be reused";
"Unfreeze the extension. I want a proper fix.").

Amends ADR-0066 Decision 4 (its premise that a browser restart without restore orphans nothing)
and Decision 6, the ADR-0137 correction of 2026-10-13 (a reused tab already sits in the caller's
group), and how the service sends the reuse ADR-0196 Decision 9 chooses. Lands with the ADR-0200
amendment of 2026-10-18, which unfroze the extension for it.

## Context

The owner found saved tab groups piling up in Chromium's bookmarks bar on the Linux test machine:
two "Ghostlight - linux-1 agent" and two "Ghostlight - codex-mcp-client". The owner's rule is one
group per client, reused (ADR-0066, ADR-0137): "instead of creating a new group with the same name,
we should locate if a group with the same name already existed in Chrome, and open the tab on it."

Chromium saves every tab group. Measured on Chromium 154.0.8037.97 in a throwaway profile, by
replaying the profile's saved-group store after each case:

| How a group ended | Saved group left |
| --- | --- |
| Its only tab closed with `chrome.tabs.remove` | No |
| Its tab ungrouped with `chrome.tabs.ungroup`, then closed | No |
| All its tabs closed in one `chrome.tabs.remove` | No |
| Its window closed | Yes |
| Still open when the browser exited | Yes |

A saved group that is not open is out of an extension's reach. `chrome.tabGroups` offers only
`get`, `query`, `update` and `move`, all on open groups, and `chrome.sessions.getRecentlyClosed`
reports a closed group's tabs without their group. `chrome.sessions.restore` of one of those tabs
does reopen the same saved group, but with every tab it saved plus a duplicate of the restored
one, and it needs the `sessions` permission.

Ghostlight kept a group open between sessions. When a session ends, the service closes its tabs as
released, and the person's preserve-tabs choice (on by default) keeps them open, in the group. The
next window close or browser exit saved that group, the next session found no open group with its
title and made one, and the bookmarks bar gained one more group of the same name each time.
ADR-0066 Decision 4 assumed that a restart without restore orphans nothing. With saved groups, it
orphans one group per client.

## Decision

1. **A kept tab leaves Ghostlight's group.** When preserve-tabs keeps a released tab open, the
   extension ungroups it if it sits in one of the groups the adapter keeps. A tab the person moved
   into a group of their own stays there. When the last kept tab leaves, Chromium deletes the group
   instead of saving it, so between sessions no Ghostlight group is open for a window close or a
   browser exit to save. The tabs of live sessions keep the group.
2. **Reuse puts the tab back.** The service reuses a kept tab with `AdoptTab`, which is the
   adapter's `open_tab` with `adopt_tab_id`. The wire and the extension already carried it. The
   adopt puts the tab in the client's group and loads the page, moving the tab to the group's window
   when it is elsewhere, as merging same-title groups already does. A tab the person put in a group
   of their own stays in it. The reuse choice is unchanged (ADR-0196 Decision 9): the service picks
   the exact tab, and a client reuses only tabs its own sessions kept.
3. **What stays out of reach.** A session still live when its window closes or the browser exits
   leaves its group saved, and the next tab that session or a later one opens starts a group with
   the same title. Ghostlight does not reopen a saved group by restoring a tab, because that brings
   back every page the group saved. It does not read or change Chromium's saved-group store, and it
   does not change the person's browser settings. When Chromium continues where the person left
   off, the group comes back open and the title match reuses it (ADR-0066 Decision 4).

## Consequences

- Ending a session no longer leaves a saved group behind. Measured on Chromium 154 with the real
  extension and the installed service: four sessions that ended, two of them reusing the kept tab,
  left no saved group. One session still live when the browser closed left one.
- When a session ends, the person sees its tabs leave the "Ghostlight - <client>" group and stay
  open. The group's header marks the tabs an agent is working in.
- Saved groups that already exist stay until the person removes them. Ghostlight never touches
  them.
- `BrowserCommand::AdoptTab` is new inside the service. Adapter protocol 4 is unchanged.
- One packaged extension file, `lib/tabs.js`, changed after the freeze, by the owner's decision
  recorded in ADR-0200's amendment of 2026-10-18.
