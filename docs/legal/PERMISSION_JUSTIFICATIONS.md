# Ghostlight in Browser: Permission Justifications

Last updated: 2026-10-16

These blocks correspond exactly to `extension/manifest.json`. Recheck store field names and length
limits at submission time.

## alarms

```text
The alarms permission schedules local reconnection and lifecycle checks for the Manifest V3 service worker. It does not schedule browsing, contact a server, or collect activity. Ghostlight uses it only to keep the local native-messaging relationship durable while browser work is active.
```

## debugger

```text
The debugger permission attaches the Chrome DevTools Protocol only to Ghostlight-controlled tabs. It captures requested screenshots, dispatches pointer and keyboard input, scrolls, drags, changes zoom, follows navigation state, evaluates a script the person's agent asks for, and installs Ghostlight's page code in an isolated world of those tabs (see Page-context JavaScript). Credential input requires the local application to acknowledge the person's explicit instruction for that request. Chrome displays its normal debugging indicator. Ghostlight detaches when the controlled relationship ends.
```

## downloads

```text
The downloads permission is used for two saves, both into the browser's normal download mechanism. When the person's agent asks to save a finished Ghostlight recording to a file, the extension builds the GIF locally, gives Chrome a temporary object URL, watches only that download until it settles, and revokes the URL. When the person presses Save developer diagnostics on the options page, the extension saves a small JSON file of bounded connection details. It does not browse existing downloads, choose an arbitrary filesystem path, or fetch anything from a remote server.
```

## nativeMessaging

```text
The nativeMessaging permission connects the extension to the Ghostlight application installed on this computer; its ghostlight program is the native messaging host. This on-device channel carries typed browser instructions and results, and the application's page code, which the extension checks against a hash before installing. Without it the extension cannot receive work. The channel does not send browser data to Sylin.
```

## offscreen

```text
The offscreen permission creates a local extension document only while encoding a requested browser recording as an animated GIF. Manifest V3 service workers may be suspended during encoding, so the document provides the browser media environment needed to complete the operation reliably. It loads only scripts packaged in the extension (a vendored GIF encoder and the extension's own modules), makes no network request, has no visible UI, and closes after encoding.
```

## storage

```text
The storage permission keeps, in this browser profile, an opaque installation identity, the presentation settings (effects, captions, diagnostics, preserve-tabs), a flag that the setup page has played its welcome, and, only while diagnostics are on, a bounded log of connection events. Session storage, which the browser clears when it closes, holds the ids of controlled tabs, queued unseen denial notices, and Ghostlight tab-group names. Ghostlight does not use Chrome sync storage and does not store URLs, titles, page content, selectors, values, scripts, file paths, screenshots, or policy.
```

## tabGroups

```text
The tabGroups permission creates and reuses the visible blue group named for each Ghostlight client. It keeps controlled tabs together, finds the same-name group across normal windows, and respects where the user moved it. This avoids creating duplicate groups or inserting new work into an unrelated active window.
```

## tabs

```text
The tabs permission lets Ghostlight observe and manage only its controlled tabs: list their current facts, activate them, create a tab in the correct group, navigate, reload, adopt an unambiguous child, and close when both service authority and the user's local preserve-tabs setting permit it. It does not build browsing history.
```

## webNavigation

```text
The webNavigation permission observes document commits and loading lifecycle in controlled tabs. Ghostlight uses those facts to invalidate stale target and screenshot handles, reattach content-free visual feedback, and report the committed landing truthfully after navigation. It does not monitor unrelated tabs for analytics or history.
```

## windows

```text
The windows permission locates the user-placed window containing an existing same-name Ghostlight group, focuses an exact controlled tab when requested, and creates a dedicated normal window when no group exists. This prevents browser work from disrupting the user's unrelated active window.
```

## Host permissions: `http://*/*` and `https://*/*`

```text
HTTP and HTTPS host permissions are required because a person may ask an agent to work on a site not known at publication time. The extension has no content scripts and does not use the scripting API; it reads and acts in a page through the Debugger API, in tabs Ghostlight controls. Which hosts an agent may touch is decided by the local Ghostlight application under the person's policy; the extension contains no policy logic.
```

## Page-context JavaScript

```text
Ghostlight's page code is part of the open-source Ghostlight application installed on this computer. The application supplies it over native messaging, never from a network server; the extension checks each script against the hash the application sends and installs it only through the documented Debugger API, in an isolated world of tabs Ghostlight controls. It changes only when the person updates the application.

When the person's AI agent asks for browser_execute, the script text is written by that agent, which may run on a remote model service. It reaches the extension only through the local application, under the person's local policy, and is evaluated through the Debugger API only in the attached page. No code runs in the extension origin.
```
