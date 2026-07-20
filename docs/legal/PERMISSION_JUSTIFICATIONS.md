# Ghostlight in Browser: Permission Justifications

Last updated: 2026-07-16

Each fenced block below is paste-ready for the matching Chrome Web Store Privacy practices field.
Every block is intentionally below the dashboard's 1,000-character limit. Keep the copy focused on
what uses the permission, why it is needed, and how its scope is bounded.

## tabs

```text
The tabs permission lets Ghostlight identify and manage tabs in its labeled automation groups. It uses chrome.tabs to list and inspect those tabs, report their URLs and titles, create or navigate tabs, reload them, and track navigation or closure. Without it, Ghostlight cannot maintain accurate automation-tab state or report tab context to the connected local host.
```

## debugger

```text
The debugger permission attaches Chrome DevTools Protocol 1.3 only to actively automated tabs. Ghostlight uses it to dispatch mouse, keyboard, scroll, and drag input; capture screenshots and user-requested recording frames; observe console and network events; and run explicitly requested javascript_tool code in the attached page. Chrome shows its standard debugging indicator while attached, and Ghostlight detaches when automation ends.
```

## Remote code use / page-context JavaScript

```text
All extension logic ships in the submitted package. When javascript_tool is explicitly requested, JavaScript text arrives from the separately installed local Ghostlight application and runs through the documented Debugger API Runtime.evaluate method only in the attached web page. It is not retained or executed in the extension origin. The Debugger API is a Manifest V3 permitted API for remote-source execution when used for its documented purpose.
```

Policy reference: [Chrome Web Store Manifest V3 requirements](https://developer.chrome.com/docs/webstore/program-policies/mv3-requirements).

## scripting

```text
The scripting permission lets Ghostlight inject or restore packaged content scripts in the active automation tab. Those scripts read page structure, find elements, interact with forms and shadow DOM, and render visible cursor and action feedback. They contain no access-control logic and are used only to support the current browser-automation session.
```

## nativeMessaging

```text
The nativeMessaging permission connects the extension to the separately installed local Ghostlight application. Chrome native messaging is the on-device channel that carries browser instructions and results between them. Without this permission, the extension cannot receive an instruction or function at all.
```

## tabGroups

```text
The tabGroups permission creates and labels Ghostlight automation groups, keeps automated tabs visibly organized, and locates those groups again after a Manifest V3 service-worker restart. Groups are scoped to the client and browser window. It is used only for tabs Ghostlight manages.
```

## windows

```text
The windows permission lets Ghostlight find the most recently focused eligible normal browser window and create tabs in that user-placed window. It creates a new normal window only when no eligible one exists. It also validates a session's selected window so work does not silently move elsewhere.
```

## storage

```text
The storage permission keeps the local browser identity and user display settings in chrome.storage.local, and ephemeral automation tab IDs, group IDs, recent eligible-window focus order, and panic-stop state in chrome.storage.session. Ghostlight does not use chrome.storage.sync, so this state is not synced to the user's Google account or other devices.
```

## alarms

```text
The alarms permission runs a periodic keepalive during browser automation so Chrome does not evict the Manifest V3 service worker mid-session, drop the native-messaging connection, or leave an attached tab without its controller. It performs no scheduled browsing or network activity.
```

## host_permissions: <all_urls>

```text
The <all_urls> host permission is required because Ghostlight is a general-purpose browser automation tool and the sites a user asks it to operate are not known in advance. It is used to run packaged content scripts in the active automation tab. Domain and capability restrictions are enforced by the separate local Ghostlight application's policy; the extension contains no allowlist or policy logic.
```
