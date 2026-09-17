# Ghostlight in Browser: Mozilla Firefox Add-ons (AMO) listing

Last updated: 2026-09-17

This is repository-local candidate copy for Mozilla Add-ons (AMO) and self-distributed signed XPI
packages. Do not change the public listing or submit a package until the owner approves the
provenance-verified artifacts and compatibility evidence.

The extension ID is pinned as `ghostlight@sylin.org` in `extension-firefox/manifest.json`.

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

You can watch tab management, page reading, navigation, clicks, typing, form work, screenshots, and
other requested actions; pause or resume the session at any time with keyboard shortcuts; and
preserve controlled tabs as visible evidence.

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
| `tabs` | Required to enumerate, open, focus, navigate, and close tabs explicitly controlled by the AI agent, and to capture tab screenshots via `tabs.captureTab`. |
| `webNavigation` | Required to observe document commitment and URL changes in controlled tabs to ensure governance preflights land on authorized origins. |
| `scripting` | Required to register preload content scripts and inject Ghostlight Web Component visual cues (indicator ribbon, controlled border) at document start. |
| `storage` | Required to persist the local extension-minted browser ID and user preferences (such as user hold states) across restarts. |
| `activeTab` | Required to interact with the active tab when triggered by user toolbar popup or shortcut actions. |
| `alarms` | Required for periodic reconnection and heartbeat retries to the local native connector when idle. |
| `<all_urls>` | Required because the user may instruct their AI agent to navigate and automate tasks across arbitrary user-specified websites. |

## Source code submission

The `extension-firefox/` package consists entirely of clean, human-readable vanilla JavaScript
without transpilation, minification, or obfuscation. No supplementary source build archive is
required by Mozilla policies.
