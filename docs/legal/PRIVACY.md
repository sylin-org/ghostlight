# Ghostlight in Browser: Privacy Policy

Last updated: 2026-08-10 for the planned 1.0 adapter

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

- page text, structure, accessibility-relevant controls, open shadow DOM, element state and bounds;
- current tab URL, title, loading state, window, opener, and tab-group facts;
- screenshots explicitly requested for the current viewport, page, or target;
- pointer, keyboard, scroll, drag, zoom, navigation, and dialog actions;
- ordinary form values supplied by the connected MCP client;
- explicit page-script source and its bounded serializable result; and
- file bytes supplied by the native application for an explicitly named page file input.

The extension does not browse the filesystem. File paths are validated and bytes are read by the
native application only for the requested upload. Ghostlight refuses credential-class fields and
does not type passwords, one-time codes, or payment secrets.

## Where data goes

Browser results travel through Chrome native messaging to the local Ghostlight application and
then to the MCP client the user chose. This local process channel does not send data to Sylin.
The MCP client's own handling of tool results is governed by that client's configuration and
privacy terms.

Ghostlight's local audit and desktop history are payload-free. They contain opaque ids, tool,
capability, decision, reason, terminal status, and effect class, never URLs, page text, selectors,
form values, file paths, scripts, screenshots, or dialog text.

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
