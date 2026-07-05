# Ghostlight in Browser: Permission Justifications

Last updated: 2026-07-03

This document gives one paragraph per permission requested by the Ghostlight in Browser
extension, written to be pasted directly into the corresponding justification text box in the
Chrome Web Store developer dashboard. Ghostlight in Browser is the browser-side half of a governed
browser-automation system: a local native application (installed separately, not distributed
through the Chrome Web Store) drives this extension over Chrome's native messaging protocol so
that a connected AI agent (through a local MCP client such as Claude Code) can read and act on
a browser tab on the user's own authenticated session. Each permission below backs a specific,
named piece of that mechanism.

## tabs

The `tabs` permission is required so the extension can identify and track the specific tabs it
is automating: `chrome.tabs.query` (to list the tabs currently in the dedicated automation tab
group, including querying by `groupId`), `chrome.tabs.get` (to look up a tab's URL and title so
tools like `tabs_context_mcp` can report accurate tab context back to the connected AI agent),
`chrome.tabs.create` and `chrome.tabs.update` (to open new tabs and navigate existing ones on
instruction from the native application), `chrome.tabs.reload` (to support tab reload), and the
`chrome.tabs.onRemoved` / `chrome.tabs.onUpdated` event listeners (to keep the extension's record
of which tabs belong to the current automation session accurate as tabs are closed or navigated,
including after a Manifest V3 service worker restart). Without this permission the extension
cannot reliably identify which tabs belong to its own automation session or report their URLs
and titles back to the connected agent.

## debugger

The `debugger` permission is required because Ghostlight in Browser attaches the Chrome DevTools
Protocol (CDP, version 1.3) to the single tab it is automating, via
`chrome.debugger.attach`. This is the only mechanism that gives the extension a single,
unified session for the three capabilities the tool depends on together: (1) dispatching
low-level synthetic input (mouse clicks, drags, key presses, scrolling) with the same
coordinate and timing fidelity a real user produces, (2) capturing on-demand screenshots of the
exact rendered tab, and (3) capturing console and network events as they happen. No combination
of public, non-debugger extension APIs provides all three in one coherent session against one
target tab: `chrome.tabs.captureVisibleTab` cannot dispatch input or read console/network
activity, and there is no public API for CDP-fidelity input dispatch or console/network event
streaming. Content-script-simulated input (dispatching synthetic DOM events) is a fundamentally
different and less reliable capability: it does not reproduce trusted, OS-level input the way
CDP's `Input.dispatchMouseEvent` / `Input.dispatchKeyEvent` do, so pages that distinguish
trusted from synthetic events behave differently under it, which defeats the purpose of a
general-purpose automation tool. The extension attaches the debugger only to the specific
tab(s) it is actively automating, only for the duration of that automation session, and shows
Chrome's own "being debugged" indicator on that tab for the whole time, so the user always has
a visible signal that the capability is in use.

## scripting

The `scripting` permission is required to inject two content scripts into the automated tab on
demand: one that performs in-page DOM reads (building the accessibility tree, element lookup,
and shadow-DOM-aware form field interaction, all of which are far more reliable run inside the
page than driven remotely over the debugger protocol), and one that draws a purely cosmetic
on-page indicator (a cursor/glow effect) so the user can visually see where automated input is
occurring on the page in real time. Neither injected script makes any access-control decision;
they are mechanism only.

## nativeMessaging

The `nativeMessaging` permission is required because this extension is a thin executor for a
separately installed local native application, and native messaging is the only channel Chrome
provides for an extension to exchange messages with a native process on the user's machine.
Every instruction the extension carries out (navigation, input dispatch, screenshots, page
reads, tab management) originates from that native application over this channel. Without this
permission the extension has no way to receive instructions and cannot function at all.

## tabGroups

The `tabGroups` permission is required so the extension can create, label, and locate a single
dedicated tab group for automated tabs (visually labeled so it is clearly distinguishable from
the user's own browsing tabs), and so it can find that same group again and recover its state
after a Manifest V3 service worker restart (which can happen mid-session due to browser-managed
service worker lifecycle, independent of anything the user does). This keeps automated tabs
visually separated from the user's regular tabs and keeps tab-group state consistent across
service worker restarts.

## windows

The `windows` permission is required to open a new, dedicated browser window the first time
automation starts, so the automated tab group described above has a clear window of its own
rather than being mixed into whichever window the user already has open.

## storage

The `storage` permission is required to use `chrome.storage.session`, which persists small
amounts of ephemeral session state (which tab IDs and tab group ID belong to the current
automation session, and a panic "session killed" flag used by the extension's kill switch)
across Manifest V3 service worker restarts. This is `chrome.storage.session`, not
`chrome.storage.sync`: nothing stored this way is synced to any account or device, and it does
not persist beyond the browser session.

## alarms

The `alarms` permission is required to create a periodic keepalive alarm (roughly every 0.4
minutes) that prevents the Manifest V3 service worker from being terminated mid-automation-
session by the browser's normal idle-eviction behavior. Without this, the service worker could
be killed mid-session, silently dropping the native messaging connection and leaving a debugger
attached to a tab with no controlling process.

## host_permissions: <all_urls>

The broad `<all_urls>` host permission is required because Ghostlight in Browser is a
general-purpose browser automation tool: it has to be able to operate on whatever site the
connected AI agent or the user navigates the automated tab to, and that set of sites is not
known in advance. The extension itself deliberately has no per-domain allowlist or blocklist
logic; that would duplicate policy the extension is not designed to hold. Domain-level access
control (including refusing to operate on specifically protected domains) is enforced at
runtime by the separate local native application's policy configuration, not by this
extension. `<all_urls>` reflects that the extension is mechanism, not policy, rather than an
unused or unnecessary broad grant.
