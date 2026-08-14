# Ghostlight in Browser: planned 1.0 store listing

Last updated: 2026-08-10

This is repository-local candidate copy. Do not change the public listing or submit a package until
the owner approves the provenance-verified 1.0 artifacts and compatibility evidence. Recheck the
store's current fields, asset sizes, and policy wording at submission time.

The public item id is `lejccfmoeogmhemakeknjjdhkfkgncdl`. The pinned source-development key and
unpacked id are preserved separately by `extension/manifest.json`; release packaging must follow
the existing store-identity mechanism rather than inventing a new extension identity.

## Listing

**Name**

```text
Ghostlight in Browser
```

**Summary**

```text
Governed browser automation over your own authenticated session, for AI agents.
```

**Category**

```text
Developer Tools
```

**Detailed description**

```text
Ghostlight gives compatible AI agents a visible workspace in the Chromium browser you already
use, with your existing signed-in sessions and local human control.

Browser work stays together in a clearly named blue tab group. You can watch page reading,
navigation, clicks, typing, form work, file upload, screenshots, dialogs, and other requested
actions; pause or end the session at any time; and preserve controlled tabs as visible evidence.

The extension is a thin adapter for the separately installed Ghostlight application. Policy,
terminal results, history, and model-facing tools stay in the local native orchestrator. The
extension owns only Chromium mechanisms, page-local access, observation, and content-free visual
feedback.

Ghostlight is local-first. There is no developer-operated runtime service, account, telemetry,
advertising, tracking, activation, or data sale.

Requires the matching Ghostlight 1.0 desktop application:
https://sylin.org/ghostlight/

Source and documentation:
https://github.com/sylin-org/ghostlight
```

**Homepage and support**

```text
https://github.com/sylin-org/ghostlight
```

## Privacy

**Single purpose**

```text
Ghostlight in Browser is the browser adapter for a separately installed local AI-browser
automation application. On typed instructions from that local application, it observes and acts in
visible Ghostlight-controlled HTTP(S) tabs, manages their windows and groups, and renders local
content-free feedback. Every permission supports that single purpose. The extension makes no
policy decision and sends no telemetry or browser data to Sylin.
```

Use the exact blocks in
[`PERMISSION_JUSTIFICATIONS.md`](PERMISSION_JUSTIFICATIONS.md) for `alarms`, `debugger`,
`nativeMessaging`, `storage`, `tabGroups`, `tabs`, `webNavigation`, `windows`, HTTP/HTTPS host
permissions, and explicit page-context JavaScript.

**Privacy policy**

```text
https://sylin.org/ghostlight/privacy/
```

**Limited Use disclosure**

```text
The use of information received from Google APIs will adhere to the Chrome Web Store User Data
Policy, including the Limited Use requirements.
```

At submission, disclose on-device handling of website content and user activity as required by the
dashboard's then-current definitions. Do not claim that local processing means no disclosure is
required. Ghostlight does not request Chrome history, cookies, credentials, payment, geolocation,
or sync-storage access and does not maintain a browsing-history database.

## Assets

- Use `extension/icons/icon128.png` as the store icon. Do not redraw or recolor it.
- Capture screenshots externally so the extension's intentional screenshot suppression does not
  hide its visible cursor, highlights, receipts, and ribbons.
- Show the real browser chrome, named Ghostlight group, current popup/options visual identity, a
  safe browser action, and a blocked action with its visible explanation.
- Use only safe demo content. Remove accounts, notifications, personal tabs, paths, ids, and other
  private material.
- Do not invent a demo CLI. Record the real packaged product through a supported MCP harness and
  the public safe demo forms.

## Submission gate

Before owner submission:

1. Produce the store zip from the approved release commit without the pinned development key or
   repository-only test material.
2. Compare the zip's manifest, icons, popup, options, permissions, and scripts with the approved
   source and exact 1.0 adapter version.
3. Complete the extension product and visible-browser gates in `docs/1.0/ACCEPTANCE.md`.
4. Verify the privacy policy public URL already carries the matching 1.0 text.
5. Upload assets and copy, review every disclosure in the live dashboard, then submit with the
   owner's explicit approval.
6. Use deferred publication where available. Publish the service and adapter in the compatibility
   order recorded by the final release plan.

After approval, independently download the public package and compare it with the submitted zip,
allowing only store-injected metadata. Update `docs/public-status.json` from observed public state.
