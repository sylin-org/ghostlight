# Ghostlight in Browser: Chrome Web Store Listing

Last updated: 2026-07-13

Paste-ready copy for every text field in the Chrome Web Store developer dashboard, plus the
non-text asset checklist and the submission steps only the founder can take. Permission
justifications live in [PERMISSION_JUSTIFICATIONS.md](PERMISSION_JUSTIFICATIONS.md) and the
privacy policy in [PRIVACY.md](PRIVACY.md); this file does not restate them.

The package to upload is produced by `scripts/package-extension.ps1` at
`dist/ghostlight-extension-v<version>.zip`. That zip already has the local-dev `key` stripped, so
it is valid for a first upload (the store rejects a `key` field on the first upload and assigns the
extension id itself).

## Store listing tab

**Item name**

```
Ghostlight in Browser
```

**Summary** (short description, 132 char max; matches the manifest `description`)

```
Governed browser automation over your own authenticated session, for AI agents.
```

**Category**

```
Developer Tools
```

**Language**

```
English (United States)
```

**Detailed description** (plain text; the store does not render Markdown)

```
Ghostlight in Browser gives an AI agent controlled access to your real, authenticated browser
session. It drives the browser you are already logged in to, so the agent can observe and act on
the web apps you already use, through any Model Context Protocol (MCP) client such as Claude Code,
Cursor, or VS Code.

IMPORTANT: This extension does nothing on its own. It is the browser-side half of a two-part
system. The other half is a local native application that you install and run separately -- it is
not distributed through the Chrome Web Store. Without that native
application installed and registered, the extension is inert: it cannot connect to anything,
receive instructions, or take any action. Install instructions are in the project repository.

What it can do, on instruction from that local application:
- Read page content and structure (text, accessibility tree, shadow DOM).
- Take screenshots of the automated tab, with an on-page cursor showing where input lands, and,
  when you ask for a session recording, relay screen-capture frames of that tab so the local
  application can assemble an annotated animated GIF of what the agent did.
- Dispatch clicks, keystrokes, scrolling, and drags with real-input fidelity.
- Fill in forms, find elements, and run in-page JavaScript.
- Place files and captured screenshots into a page's file inputs and drop targets, using data the
  local application supplies (the extension never reads your disk).
- Inspect console messages and network request metadata.
- Open, navigate, group, and manage tabs in a dedicated, clearly labeled automation window.

Governed by design:
- The native application is the policy and audit layer. It classifies every action (read, act,
  write, execute), can restrict which domains the agent may touch, honors a "take the wheel" pause
  and a panic kill switch, and writes a structured audit record of what ran.
- All of that governance runs on your own machine, inside the native application, never inside this
  extension and never on any remote server.

Local-first and private:
- No developer-operated server. No analytics or telemetry. No ad tracking. No data sale.
- Data the extension reads is sent only to the local native application on your own machine, over
  Chrome native messaging (a direct, on-device, process-to-process channel). Nothing is transmitted
  over the network to reach it.
- All extension logic ships inside the reviewed extension package. The extension does not fetch or
  dynamically import code that changes its own behavior. JavaScript supplied for an explicitly
  requested javascript_tool call runs only in the attached page, not in the extension origin.

Open and inspectable:
- Source, install scripts, the governance policy engine, and full documentation are at
  https://github.com/sylin-org/ghostlight.

You stay in control: you choose whether the native application runs at all, which policy (if any)
it enforces, and you can pause or kill automation, or remove the extension, at any time.
```

**Homepage / support URL**

```
https://github.com/sylin-org/ghostlight
```

## Privacy tab

**Single purpose** (required)

```
Ghostlight in Browser is a thin executor for a separately installed local automation host. It carries
out browser actions -- reading page content, taking screenshots and (during a user-requested session
recording) screen-capture frames, dispatching input, placing host-supplied files into page inputs,
and managing tabs -- on the automated tab, on instruction from that host over Chrome native
messaging, so a connected AI agent can operate the user's own authenticated browser session. Everything the
extension does serves that single purpose; it makes no access-control decisions of its own and
holds no policy or allowlist logic.
```

**Permission justifications**: copy each fenced block from
[PERMISSION_JUSTIFICATIONS.md](PERMISSION_JUSTIFICATIONS.md) into the matching box (tabs, debugger,
scripting, nativeMessaging, tabGroups, windows, storage, alarms, and the `<all_urls>` host
permission). Each paste-ready block is below the dashboard's 1,000-character limit.

**Privacy policy URL**

```
https://sylin.org/ghostlight/privacy/
```

**Remote code use justification**: copy the fenced block under "Remote code use / page-context
JavaScript" in [PERMISSION_JUSTIFICATIONS.md](PERMISSION_JUSTIFICATIONS.md). That file is the single
source for dashboard justification text.

Policy reference: [Chrome Web Store Manifest V3 requirements](https://developer.chrome.com/docs/webstore/program-policies/mv3-requirements).

**Limited Use disclosure** (must also appear at the privacy policy URL)

```
The use of information received from Google APIs will adhere to the Chrome Web Store User Data
Policy, including the Limited Use requirements.
```

**Data usage disclosure** -- recommended answers. This is a compliance attestation the founder
signs at submission; confirm each answer against current dashboard wording before submitting.

Policy reference: [Chrome Web Store privacy fields](https://developer.chrome.com/docs/webstore/cws-dashboard-privacy).

- Does this item collect or use user data? Select YES. Chrome requires disclosure even when data is
  processed only on the user's device.
- Select **Website content**. The extension handles page text and structure, images and screenshots,
  console output, hyperlinks, and other content of the automated tab.
- Select **User activity**. The dashboard definition includes network monitoring, clicks, mouse
  position, scrolling, and keystrokes; Ghostlight handles network-request metadata and the
  agent-directed interaction signals used in the visible automation session.
- Do not select **Web history**: the extension does not request the history permission or maintain a
  list of pages visited with visit times. Current URLs and titles are transient automation-tab state,
  disclosed in the privacy policy under browser state.
- Do not select personally identifiable information, health information, financial and payment
  information, authentication information, personal communications, or location as separately
  collected categories. Ghostlight does not target or extract those semantic data types. Content a
  user explicitly asks it to handle remains covered by Website content; it does not read Chrome's
  cookie, credential, payment, location, or communication stores.
- Certifications (all TRUE):
  - I do not sell user data to third parties.
  - I do not use or transfer user data for purposes that are unrelated to my item's single purpose.
  - I do not use or transfer user data to determine creditworthiness or for lending purposes.

## Graphic assets checklist

- Store icon: 128x128 PNG. Present at `extension/icons/icon128.png` (also in the package).
- Screenshots: at least one required; 1280x800 or 640x400, PNG or JPEG (a 24-bit PNG is safest).
  The shot list and how to capture each are below.
- Promotional video: a YouTube URL that shows the extension's features. Required by the current
  dashboard listing guidance. The recording recipe is below.
- Small promo tile: 440x280 PNG. Required. Its deterministic capture source is
  `https://sylin.org/ghostlight/store-assets/promo/`.
- Marquee promo tile: 1400x560 PNG. Optional; only used if the item is featured.

Policy references: [complete the listing](https://developer.chrome.com/docs/webstore/cws-dashboard-listing)
and [supply images](https://developer.chrome.com/docs/webstore/images).

### How to capture the promotional tiles

The website route `https://sylin.org/ghostlight/store-assets/promo/` is a noindex, static capture
surface. It uses the Card Foundry visual system but has no animation, personal data, or timing
dependency. The same document selects the small or marquee composition from the exact viewport.

1. Open the route in Chrome and open DevTools (F12).
2. Toggle the Device Toolbar (Ctrl+Shift+M), set DPR to 1, and choose one required viewport:
   - 440 x 280 for the required small promo tile.
   - 1400 x 560 for the optional marquee promo tile.
3. Choose "Capture screenshot" from the device-toolbar menu. Do not use the full-size screenshot
   command; the viewport itself is the asset boundary.
4. Confirm the resulting PNG has the exact pixel dimensions before uploading it.

Chrome does not define a 1400x650 marquee asset. Use 1400x560 exactly; the dashboard rejects the
wrong dimensions.

### Promotional video: the shortest honest story

Use the built-in `ghostlight demo` tour. It drives the public demo stage through the same MCP relay
and tool surface an agent uses. It shows the dedicated Ghostlight tab, visible actions, form work,
console and network observation, page reading, and a tighten-only session policy refusing an
off-domain navigation. The default pacing is designed for a roughly 90-second recording.

1. Run `target\release\ghostlight.exe doctor`. Do not record until the verdict is OK and it says
   `extension connected (live)`.
2. In the extension popup, turn on **Show action captions**. Close or hide unrelated tabs and any
   notification surface that could reveal personal information.
3. In OBS, capture only the Chrome window at 1920x1080. Record without microphone or desktop audio.
   Keep the browser chrome visible so the Ghostlight tab group and real-browser context are clear.
4. Start recording, then run:

   ```powershell
   target\release\ghostlight.exe demo --setup-pause 10 --pause 3
   ```

   Use the setup pause to bring Chrome to the front. Do not interact until the terminal reports
   `Demo complete -- every tool ran, and the guardrail held.`
5. Stop recording. Trim only the idle setup and tail; keep the denial ribbon and its plain-language
   explanation on screen for at least three seconds. Do not add claims, stock footage, or a sales
   voice-over. The product behavior is the pitch.
6. Upload the MP4 to YouTube as **Unlisted** with this metadata:

   **Title**

   ```text
   Ghostlight: governed browser automation in your real browser
   ```

   **Description**

   ```text
   Ghostlight gives MCP agents visible, local access to the Chromium session you already use.
   This uncut product tour shows real browser actions and a session policy refusing an off-domain
   request. No cloud browser and no developer-operated service.

   Project and source: https://github.com/sylin-org/ghostlight
   Privacy: https://sylin.org/ghostlight/privacy/
   ```

7. Paste the YouTube share URL into **Promotional video** in the Store listing tab. Watch the
   uploaded video once from a private window before submitting so its visibility and playback are
   proven.

### How to capture an exact 1280x800 still

Do NOT use the agent's own `computer` screenshot tool for store assets. That path is built for the
model, not for marketing: it hides the phantom cursor, the ripples, and every per-action effect
during capture (by design; see docs/design/visual-feedback.md), and it downscales to a token budget,
so its output is neither on-brand nor a predictable pixel size. Capture externally instead. (The
agent also cannot open a `chrome-extension://` page: `navigate` forces `https://`, so the options
page must be opened by hand, as in Shot 2.)

The reliable, display-DPR-independent method for any web or extension page:

1. Open the page in Chrome (see each shot below for how).
2. Open DevTools (F12) and toggle the Device Toolbar (Ctrl+Shift+M).
3. Set the dimensions to 1280 x 800 and the device pixel ratio to 1 (the DPR field; add it from the
   device-toolbar overflow menu if it is hidden).
4. In the device-toolbar three-dot menu, choose "Capture screenshot". Chrome writes an exact
   1280x800 PNG regardless of your monitor's scaling. (Use "Capture screenshot", not "Capture full
   size screenshot", so you get the viewport, not the whole scroll height.)

### Shot 1 (recommended hero): the agent driving a real page

The hook: the sky-blue phantom cursor plus a click ripple, on a recognizable site, inside the
ghost-marked "Ghostlight" tab group. Because the effects are hidden from the agent's own captures,
this one is recorded from the outside:

1. Run `target\release\ghostlight.exe doctor`; continue only when the extension is connected.
2. Run the `ghostlight demo` command from the promotional-video recipe and record the browser window
   with OBS. The tour self-narrates through timed Agent ribbons, action effects, and the purpose-built
   demo pages. The current package includes ADR-0072 `narrate`.
3. Extract the peak frame of an effect from the recording (VLC "Take Snapshot", or ffmpeg). Crop to
   1280x800 if the recorder added window chrome. Turning on "Show action captions" in the extension
   popup before recording adds the subtitle line, which reads well in a still.

### Shot 2: the settings page (governed, and yours to configure)

The on-brand dark options page: the "Agent activity effects" and "Action captions" toggles and the
governance boundary card that says policy lives in the binary, not the extension. Open it the way a
user would:

1. Click the Ghostlight extension icon, then "More settings"; or open chrome://extensions, find
   Ghostlight, click "Details", then "Extension options".
2. Capture at exactly 1280x800 with the DevTools method above.

### Shot 3 (optional): the governance is real

A split of an MCP client hitting a governed denial next to the matching JSON-Lines audit record, or
the output of `ghostlight policy explain`. This is the differentiator shot for the enterprise
audience. Capture the terminal window with the OS and crop to 1280x800.

## Submission steps (founder actions)

1. Create a Chrome Web Store developer account (one-time 5 USD fee). Agent cannot do this.
2. Add new item; upload `dist/ghostlight-extension-v<version>.zip`.
3. Fill the Store listing and Privacy tabs from this file; upload at least one screenshot and the
   required small promo tile, then paste the YouTube promotional-video URL.
4. Submit for review. Expect extra scrutiny on `debugger` + `<all_urls>` + `nativeMessaging`; the
   justifications and privacy policy are written to answer exactly that.

## Submitted extension id

The item was submitted for review on 2026-07-13. The store assigned the id
**`lejccfmoeogmhemakeknjjdhkfkgncdl`** -- this is NOT the unpacked-dev id
`cjcmhepmagomefjggkcohdbfemacojoa` (the dev id comes from the pinned manifest `key`, which is
stripped from the store package).

Ordinary `ghostlight install` already registers both the Web Store id and the pinned unpacked-dev
id in the native host's `allowed_origins`. A store user therefore runs the normal command:

```
ghostlight install
```

`--extension-id` appends one additional validated origin for a fork or enterprise-packaged build;
it is not required for either official Ghostlight extension id. The store and unpacked-dev ids are
intentionally distinct.
