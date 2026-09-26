# Withdrawn Firefox Add-ons Candidate

Last updated: 2026-09-26

Status: Historical, withdrawn. Never submit or publish this copy. ADR-0184 makes no
commitment to a future Firefox channel.

ADR-0183 withdrew Firefox from the active product because this partial adapter could not deliver
Ghostlight's normal page capabilities in the user's ordinary visible session. The former listing
copy is retained below as historical evidence only. Its source path and publisher tooling were
removed from the active tree and remain recoverable from Git history.

The historical extension ID was `ghostlight@sylin.org`.

## Listing details

**Name**

```text
Ghostlight in Browser
```

**Summary**

```text
Governed browser automation over your own authenticated session, for AI agents.
```

**Categories**

```text
Developer Tools, Web Development
```

**Detailed description**

```text
Ghostlight gives compatible AI agents a controlled, visible workspace in the Firefox browser you
already use, with your existing signed-in sessions and local human control.

The Firefox adapter provides the browser-shell mechanisms it can execute completely: listing and
focusing controlled tabs, navigation, history, reload, zoom, window sizing, attention, and tab
closure when your local preserve-tabs choice allows it. Ghostlight negotiates these capabilities
at connection time instead of pretending that unsupported page operations are available.

Page reading, clicks, typing, form work, screenshots, scripts, and other document-local operations
are not advertised by this Firefox adapter. Use a compatible Chromium adapter when those
capabilities are needed.

The extension is a thin, policy-free adapter for the separately installed Ghostlight native service.
Policy, terminal receipts, governance, audit history, and model-facing tools stay in the local
orchestrator. The Firefox extension owns only Gecko WebExtension mechanisms, tab/window observation,
and native messaging communication.

Ghostlight is strictly local-first. There is no developer-operated cloud service, account
requirement, advertising, tracking, telemetry, activation ping, or third-party data transmission.

Requires the matching Ghostlight desktop service:
https://sylin.org/ghostlight/

Source code and documentation:
https://github.com/sylin-org/ghostlight
```

**Homepage and support**

```text
https://github.com/sylin-org/ghostlight
```

## Data collection and privacy disclosure

**Data collection disclosure (`data_collection_permissions`)**

```json
"data_collection_permissions": {
  "required": ["none"]
}
```

Ghostlight does not collect, record, or transmit personal data or telemetry of any kind (ADR-0028).
All operations are executed strictly between the local AI client, the local Ghostlight orchestrator
daemon, and the user's local Firefox browser over native messaging IPC on loopback/standard IO.

**Privacy policy URL**

```text
https://sylin.org/ghostlight/privacy/
```

## Permission justifications for Mozilla reviewers

| Permission | Purpose and justification |
|---|---|
| `nativeMessaging` | Required to exchange typed 32-bit framed JSON commands with the local Ghostlight service (`ghostlight-browser-connector`). This is the sole communication channel. |
| `tabs` | Required to enumerate, focus, navigate, reload, traverse history, zoom, and close tabs explicitly controlled by the AI agent. The adapter does not read page content or capture screenshots. |
| `webNavigation` | Required to observe document commitment and URL changes in controlled tabs to ensure governance preflights land on authorized origins. |
| `storage` | Required to persist the opaque local extension-minted browser ID and the preserve-tabs preference across restarts. It does not store URLs, titles, page content, or history. |

## Historical source submission note

The removed `extension-firefox/` package consisted entirely of human-readable vanilla JavaScript
without transpilation, minification, or obfuscation. This note does not authorize or describe a
current submission.
