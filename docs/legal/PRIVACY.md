# Ghostlight in Browser: Privacy Policy

Last updated: 2026-08-22

Canonical public URL: https://sylin.org/ghostlight/privacy/

This policy covers the `Ghostlight in Browser` Manifest V3 extension published by Sylin. It
explains what the extension can access, why it needs that access, and where data does and does not
go.

## What the extension is

Ghostlight has a native application installed on the same machine and a thin browser extension.
The native orchestrator owns browser jobs, policy, runtime controls, completion truth, and
payload-free audit. The extension owns physical Chromium operations, page-local DOM access, local
observation, and content-free visual feedback.

The extension receives typed instructions through Chrome native messaging. It does not make policy
decisions, expose model-facing tools, maintain an allow-list, send telemetry, or operate without
the separately installed native application.

## Data used for requested browser work

For controlled Ghostlight tabs, the extension may handle:

- **Page content and structure.** Text, DOM structure, accessibility-relevant controls, open shadow
  DOM, element state, and bounds support requested reading, discovery, and interaction.
- **Browser state.** Current tab URL, title, loading state, window, opener, and tab-group facts keep
  controlled work visible and correctly associated with its local Ghostlight session.
- **Screenshots.** Ghostlight captures the current viewport, page, or target only when requested.
- **Recordings.** During an explicitly requested recording, compressed frames remain in bounded
  volatile extension memory. The extension uses a bundled encoder in a local offscreen document to
  create the animated GIF; individual frames never cross native messaging. Recording stops after
  at most two minutes. Finished or interrupted bytes remain for at most five additional minutes
  unless explicitly discarded; extension worker loss erases the volatile state. Saving to a page
  target or through Chrome's download mechanism remains inside the browser. Only a save requested
  by the MCP client returns the finished GIF through the local native-messaging chain. A browser
  download creates a file only after that destination is explicitly requested.
- **Console and network diagnostics.** When explicitly enabled for a controlled tab, Ghostlight may
  retain bounded, volatile console entries and sanitized network metadata. It does not retain
  headers, bodies, cookies, authorization values, post data, query strings, or fragments, and it
  does not use diagnostics to build browsing history.
- **Synthetic input and navigation.** Pointer, keyboard, scroll, drag, zoom, navigation, dialog,
  and tab-management actions perform the browser work requested by the connected MCP client.
- **Ordinary form values.** Values supplied by the connected MCP client may be placed into
  non-credential fields.
- **User-requested page JavaScript.** When the MCP client explicitly requests `browser_execute`,
  bounded script text is evaluated through the Chrome Debugger API in the attached page. Its
  bounded serializable result may return through the local chain. The text is not installed,
  retained, or executed in the extension's own origin.
- **Host-supplied files.** The native application may supply bounded file bytes for an explicitly
  named page file input.

The extension does not browse the filesystem. File paths are validated and bytes are read by the
native application only for the requested upload. Ghostlight refuses credential-class fields and
does not type passwords, one-time codes, or payment secrets.

## Where data goes

Browser results travel through Chrome native messaging to the local Ghostlight application and
then to the MCP client the user chose. Browser-local recording saves to a page target or Chrome
download do not use that path; a client-requested recording save sends only the finished GIF, never
its individual frames. This local process channel does not send data to Sylin. The MCP client's own
handling of tool results is governed by that client's configuration and privacy terms.

Ghostlight's local audit and desktop history are payload-free. They contain opaque ids, tool,
capability, decision, reason, terminal status, effect class, and bounded operational measurements.
They may contain a normalized governed host and, when the configured privacy setting permits it, a
bounded accessible name for the control Ghostlight acted on. They never contain URL paths, query
strings, fragments, page text, selectors, form values, file paths, scripts, screenshots, recording
frames or GIFs, dialog text, console text, or network payloads.

## What Sylin does not do

- No developer-operated runtime service receives browser data.
- No analytics, telemetry, advertising, profiling, creditworthiness, or lending use.
- No sale or developer transfer of browser data.
- No remotely hosted extension logic or runtime update code.
- No Chrome sync storage; adapter identity and user settings remain local to the browser profile.
- No browsing-history database or background collection unrelated to a requested operation.

The use of information received from Google APIs will adhere to the Chrome Web Store User Data
Policy, including the Limited Use requirements.

## User control

The user controls whether the native application and extension are installed, which MCP harness is
registered, whether a session is active or paused, whether controlled tabs may be closed by the
model, and whether Ghostlight remains running. Visual browser receipts, the extension popup, and
the tray workbench expose those controls without reading page content.

Removing the extension or stopping the native application ends its browser access. Closing only
the workbench window destroys that local surface and does not imply the orchestrator stopped.

## Policy changes

If the extension's data access, use, or destination changes, this policy will be updated before the
changed version is submitted.

## Contact

Questions or concerns can be raised at https://github.com/sylin-org/ghostlight or by email at
hello@sylin.org.
