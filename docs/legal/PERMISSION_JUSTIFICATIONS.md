# Ghostlight in Browser: Permission Justifications

Last updated: 2026-08-22

These blocks correspond exactly to `extension/manifest.json`. Recheck store field names and length
limits at submission time.

## alarms

```text
The alarms permission schedules local reconnection and lifecycle checks for the Manifest V3 service worker. It does not schedule browsing, contact a server, or collect activity. Ghostlight uses it only to keep the local native-messaging relationship durable while browser work is active.
```

## debugger

```text
The debugger permission attaches Chrome DevTools Protocol only to Ghostlight-controlled tabs. It captures requested screenshots, dispatches pointer and keyboard input, scrolls, drags, changes zoom, handles page navigation state, and evaluates an explicitly requested bounded page script. Chrome displays its normal debugging indicator. Ghostlight detaches when the controlled relationship ends.
```

## downloads

```text
The downloads permission is used only when the user explicitly asks to save a completed Ghostlight recording through the browser's normal download mechanism. The extension creates the finished GIF locally, gives Chrome a temporary object URL, observes only that download until it settles, and then revokes the URL. It does not browse existing downloads, choose an arbitrary filesystem path, or fetch a recording from a remote server.
```

## nativeMessaging

```text
The nativeMessaging permission connects the extension to the separately installed local Ghostlight browser connector. This on-device channel carries typed browser instructions and results. Without it the extension cannot receive work. The channel does not send browser data to Sylin.
```

## offscreen

```text
The offscreen permission creates a local extension document only while encoding an explicitly requested browser recording as an animated GIF. Manifest V3 service workers may be suspended during encoding, so the document provides the browser media environment needed to complete the operation reliably. It loads only code bundled in the reviewed extension, makes no network request, has no visible UI, and closes after encoding.
```

## storage

```text
The storage permission retains the opaque local adapter installation identity and user-controlled presentation settings, including effects, captions, diagnostics, and preserve-tabs. Restart-local connection and notice state is also kept locally. Ghostlight does not use Chrome sync storage and does not store URLs, titles, page content, selectors, values, scripts, file paths, screenshots, or policy.
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
HTTP and HTTPS host permissions are required because a user may ask Ghostlight to operate a site not known at publication time. Packaged content scripts read and interact only in Ghostlight-controlled tabs and render local action feedback. Host and capability authority is enforced by the native orchestrator; the extension contains no policy logic.
```

## Page-context JavaScript

```text
All extension logic ships in the reviewed package. When the MCP client explicitly requests browser_execute, bounded script text arrives from the local Ghostlight application and is evaluated through the documented Debugger API only in the attached web page. It is not retained, installed, or executed in the extension origin.
```
