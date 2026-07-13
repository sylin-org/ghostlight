# Ghostlight in Browser: Privacy Policy

Last updated: 2026-07-13

Canonical public URL: https://sylin.org/ghostlight/privacy/

This policy covers the "Ghostlight in Browser" Chrome extension (Manifest V3), published by
Sylin. It explains what the extension can access, why it needs that access, and where the data
goes and does not go.

## What Ghostlight in Browser is

Ghostlight has two local components:

1. A native Rust application installed separately on the user's machine. It provides the MCP
   server, policy engine, and audit log.
2. This Chrome extension. It is a thin browser executor that receives instructions from the
   native application over Chrome native messaging.

The extension reads and acts only when the local application requests a named browser operation.
It does not make access-control decisions, maintain a domain policy, or send telemetry. Without
the separately installed and registered native application, the extension cannot receive
instructions or automate the browser.

## Data and capabilities used

For tabs in the visible Ghostlight automation session, the extension can use:

- **Page content and structure.** Text, DOM structure, accessibility information, shadow DOM, form
  fields, and element locations support page reading, element discovery, and interaction.
- **Screenshots and screen-capture frames.** On-demand screenshots support visual browser tools.
  During an explicitly requested recording, the extension relays frames to the local application,
  which assembles the animated GIF.
- **Console and network information.** Console messages and network request metadata support
  debugging tools. Ghostlight does not use this capability to build a browsing history.
- **Browser state.** Tab URLs, titles, identifiers, window state, and tab-group state let
  Ghostlight create and maintain its visible automation workspace.
- **Synthetic input and navigation.** Clicks, keystrokes, scrolling, drags, navigation, and tab
  management are the actions the automation tools perform.
- **User-requested page JavaScript.** When the connected MCP client explicitly invokes
  `javascript_tool`, the JavaScript text supplied through the local native application is
  evaluated in the attached web page through the Chrome DevTools Protocol. It runs in that page's
  context, not in the extension's own origin, and is not installed or retained as extension code.
- **Host-supplied files and images.** When requested, the local application may supply file or
  image bytes for placement into a page's file input or drop target. The extension does not browse
  or read the user's filesystem.

The extension does not collect these data in the background for Sylin. Each access supports a
specific operation requested through the local Ghostlight installation.

## Where data goes

Data returned by the extension travels over Chrome native messaging to the Ghostlight application
on the same machine. Native messaging is a direct local process-to-process channel; the data does
not cross the network to reach Ghostlight.

The user chooses the MCP client connected to the local application. That client's own handling of
tool results is governed by the client's configuration and privacy terms. Sylin does not operate a
runtime service in this path and does not receive the browser data.

## What Sylin does not do

- **No developer-operated runtime service.** The extension does not send browsing data to a Sylin
  server.
- **No analytics or telemetry.** The extension contains no analytics SDK and sends no usage
  reports.
- **No advertising or profiling.** Browser data is not used for advertising, profiling,
  creditworthiness, or lending.
- **No sale or developer transfer.** Sylin does not receive, sell, or transfer data accessed by
  the extension.
- **No remotely hosted extension logic.** The service worker, content scripts, and supporting
  JavaScript that implement the extension ship in the reviewed extension package. The extension
  does not fetch or dynamically import code that changes its own behavior. The explicitly
  requested page JavaScript described above is an automation input from the local MCP client; it
  is evaluated only in the attached page and does not become extension logic.

## Limited Use

The use of information received from Google APIs will adhere to the Chrome Web Store User Data
Policy, including the Limited Use requirements.

Ghostlight uses browser information only to provide the browser operation the user requested. It
does not use or transfer that information for advertising, profiling, creditworthiness, lending,
or any purpose unrelated to its single browser-automation purpose.

## Local governance and user control

The native application evaluates tool calls against its configured capability policy and writes a
local structured audit record. Governance runs on the user's machine, not inside the extension or
on a Sylin service.

The user controls the complete local chain:

- whether the native application is installed and running;
- which MCP client and policy configuration it uses;
- whether automation is active;
- whether to pause, stop, or panic-kill the session; and
- whether the extension remains installed.

Removing the extension or stopping the local application ends its browser access.

## Policy changes

If the extension's data access, use, or destination changes, this policy will be updated before the
changed version is published.

## Contact

Questions or concerns can be raised at https://github.com/sylin-org/ghostlight or by email at
hello@sylin.org.
