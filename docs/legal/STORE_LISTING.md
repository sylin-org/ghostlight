# Ghostlight in Browser: candidate store listing

Last updated: 2026-10-16

This is repository-local candidate copy. Do not change the public listing or submit a package until
the owner approves the provenance-verified release artifacts and compatibility evidence. Recheck the
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
Page messages and animations are passive, with no buttons or interference with your input.
Use the Ghostlight window and the extension popup for human controls.

Credential input requires your explicit instruction, acknowledged by the agent for each request.
Without that acknowledgement, the call returns guidance without pausing the session. Configured
policy and your Pause/Stop controls still apply.

The extension is a thin adapter for the separately installed Ghostlight application. It works
with the Ghostlight application installed on this computer, which supplies the page code the
extension runs. Policy, terminal results, history, and model-facing tools stay in the local native
orchestrator. The extension owns only Chromium mechanisms, page-local access, observation, and
content-free visual feedback.

If Ghostlight is not on this computer yet, the extension opens its own setup page with the one
command to run.

Ghostlight is local-first. There is no developer-operated runtime service, account, telemetry,
advertising, tracking, activation, or data sale.

Requires a compatible Ghostlight desktop application:
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
`downloads`, `nativeMessaging`, `offscreen`, `storage`, `tabGroups`, `tabs`, `webNavigation`,
`windows`, HTTP/HTTPS host permissions, and explicit page-context JavaScript.

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
required. Describe client-supplied form values as input for the requested edit. Select data
categories from the actual collection and use; do not infer additional categories solely from
what an arbitrary input string could contain.
Ghostlight does not request Chrome history, cookies, geolocation, or sync-storage permissions and
does not maintain a browsing-history database. It does not read the browser's saved-password store.

## Assets

- Use `extension/icons/icon128.png` as the store icon. Do not redraw or recolor it.
- A version change alone does not require new screenshots or promotional assets. Reuse the current
  store assets while they remain accurate; update only an asset that materially misrepresents the
  submitted extension.
- If an asset does need replacement, capture it externally so the extension's intentional
  screenshot suppression does not hide its visible cursor, highlights, receipts, and ribbons.
  Show the real browser chrome, named Ghostlight group, a safe browser action, and a blocked
  action with its visible explanation.
- The popup, options page and setup page now use the Ghostlight kit: the sylin.org night ground
  and the #5eead4 accent. Recapture every popup and options screenshot in the store listing, and
  add the setup page (the one command, then the meeting) if the dashboard takes a third image.
  A store asset that still shows the old sky-blue popup misrepresents the submitted extension.
- Use only safe demo content. Remove accounts, notifications, personal tabs, paths, ids, and other
  private material. Do not invent a demo CLI; record the real packaged product through a supported
  MCP harness and the public safe demo forms.

## Submission gate

Before owner submission:

1. Produce the store zip from the approved release commit without the pinned development key or
   repository-only test material.
2. Compare the zip's manifest, icons, popup, options, setup page, permissions, and scripts with
   the approved source and exact candidate adapter version. The packaged file set is frozen by
   `tests/golden/extension-2.0.0.files.sha256`.
3. Complete the extension product and visible-browser gates in `docs/1.0/ACCEPTANCE.md`.
4. Verify the privacy policy public URL already discloses the candidate's authorized credential
   handling and passive page feedback. A source-file edit alone does not update that public URL.
5. Review the existing assets and listing fields, change only what the submitted extension makes
   inaccurate, review every disclosure in the live dashboard, then submit with the owner's
   explicit approval.
6. Answer the remote-code question as [`store-disclosure-2.0.md`](../release/store-disclosure-2.0.md)
   records it: No, because the page code ships with the application installed on this computer
   and reaches the extension over native messaging. Say plainly, in the listing and the
   permission justifications, that the extension works with that application and that
   `browser_execute` runs a script the person's agent wrote.
7. Submit for staged publication and publish in the order recorded in
   [`store-disclosure-2.0.md`](../release/store-disclosure-2.0.md), after the service release is
   public and the public privacy URL carries this text.

After approval, independently download the public package and compare it with the submitted zip,
allowing only store-injected metadata. Update `docs/public-status.json` from observed public state.
