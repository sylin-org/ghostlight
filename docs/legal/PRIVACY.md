# Ghostlight in Browser: Privacy Policy

Last updated: 2026-07-03

This document covers the "Ghostlight in Browser" Chrome extension (Manifest V3), published by
Sylin (github.com/sylin-org). It explains what data the extension can access, why, and where
that data goes and does not go.

## What Ghostlight in Browser is

Ghostlight in Browser is one half of a two-part system:

1. A local native application (a Rust binary) that runs on the user's own machine. This is the
   MCP server that an AI coding tool (for example Claude Code) talks to, and it is also the
   policy engine and audit log for the whole system.
2. This Chrome extension, which is a thin executor. It carries out browser actions (reading
   page content, taking screenshots, dispatching clicks and keystrokes, managing tabs) on
   instruction from the native application, over Chrome's native messaging channel.

The extension does not make access-control decisions on its own. It has no per-domain
allowlist, no concept of "sensitive site," and no audit log of its own. All of that lives in
the native application described below. See "How governance works" for what that means in
practice.

**The extension requires the separately installed native application to do anything at all.**
If you install only the Chrome extension, with no native host running and registered, the
extension is inert: it cannot connect to anything, receive instructions, or take any action.
The native application is not distributed through the Chrome Web Store; it is installed and
configured separately by the user, using the install scripts in the project repository.

## What data the extension can access, and why

The extension only acts on the browser tab(s) that the connected native application/AI agent
is automating, or that you yourself are using while automation is active. For that tab, the
extension can access:

- **Page content and structure** (DOM, accessibility tree, shadow DOM). This backs the
  page-reading and page-interaction tools: reading page text, finding elements, and filling in
  forms.
- **Screenshots of the tab.** This backs the visual "look at the screen" tool used for
  screenshot, scroll, and zoom actions.
- **Console messages** logged by the page. This backs the tool that lets an agent read
  JavaScript console output for debugging.
- **Network request metadata** (URLs, status codes, timing; not full response bodies beyond
  what the tool contract specifies). This backs the tool that lets an agent inspect network
  activity on the page.
- **Synthetic input dispatch.** The extension can send clicks, keystrokes, scrolling, and
  similar input events to the page, and can navigate tabs, open new tabs, and resize windows.
  This is how the automation tools actually act on a page, not a read capability, but it is
  listed here because it is a capability the extension mechanically holds.

Every one of these capabilities exists to back a specific tool in the underlying MCP tool
set (for example `read_page`, `computer`, `form_input`, `get_page_text`,
`read_console_messages`, `read_network_requests`, `navigate`, tab management). The extension
does not collect this data in the background or on pages you are not automating; it acts only
when the native application instructs it to, in service of a specific tool call.

## Where this data goes

Data read or captured by the extension is sent to the local native application on your own
machine, over Chrome's native messaging protocol (a local, direct process-to-process
connection; nothing is transmitted over the network to reach the native host). What the native
application and the connected MCP client (for example Claude Code) do with that data afterward
is governed by that separate application, not by this extension. See "How governance works"
below.

## Where this data does NOT go

- **No developer-operated server.** Sylin does not run a backend service that this extension
  talks to. There is no cloud component receiving your browsing data.
- **No analytics or telemetry.** The extension does not include an analytics SDK and does not
  phone home usage statistics.
- **No ad tracking.** The extension does not track you for advertising purposes and is not
  monetized through your data.
- **No data sale.** Data this extension can access is never sold or shared with third parties
  for any purpose.
- **No remotely hosted code.** All JavaScript the extension runs ships inside the extension
  package itself. Manifest V3 disallows remotely hosted code for extensions in the first place,
  so this is a structural guarantee enforced by the Chrome platform, not only a policy promise
  from the developer.

## How governance works

The native application is more than a relay: it is the policy and audit layer for the whole
system. It evaluates each tool call against a capability manifest (which actions are allowed,
which are read-only versus state-changing versus destructive), can block access to
specifically protected domains, and writes a structured audit record of what was executed. This
governance logic runs entirely on your own machine, inside the native application, not inside
this extension and not on any remote server. This document covers the browser extension's data
handling; the native application's governance behavior is documented separately in the project
repository and is configured by whoever installs and runs it on that machine.

## Your control over this system

Because both components run locally, you control the whole pipeline:

- You choose whether the native application is installed and running at all.
- You choose which manifest or policy configuration (if any) the native application enforces.
- You can stop automation at any point (including a "take the wheel" pause and a panic kill
  switch built into the extension), and you can remove the extension or stop the native
  application to end all access immediately.

## Changes to this policy

If the data this extension accesses, or where that data goes, changes in a future version,
this document will be updated to reflect it before that version is published.

## Contact

Questions or concerns about this extension can be raised via the project repository:
https://github.com/sylin-org/ghostlight, or by email at hello@sylin.org.
