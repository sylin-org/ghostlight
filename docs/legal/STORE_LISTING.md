# Ghostlight in Browser: Chrome Web Store Listing

Last updated: 2026-07-04

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
system. The other half is a local native application (a single Rust binary) that you install and
run separately -- it is not distributed through the Chrome Web Store. Without that native
application installed and registered, the extension is inert: it cannot connect to anything,
receive instructions, or take any action. Install instructions are in the project repository.

What it can do, on instruction from that local application:
- Read page content and structure (text, accessibility tree, shadow DOM).
- Take screenshots of the automated tab, with an on-page cursor showing where input lands.
- Dispatch clicks, keystrokes, scrolling, and drags with real-input fidelity.
- Fill in forms, find elements, and run in-page JavaScript.
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
- All code the extension runs ships inside the extension package. Manifest V3 forbids remotely
  hosted code, so this is enforced by the platform, not only promised.

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
out browser actions -- reading page content, taking screenshots, dispatching input, and managing
tabs -- on the automated tab, on instruction from that host over Chrome native messaging, so a
connected AI agent can operate the user's own authenticated browser session. Everything the
extension does serves that single purpose; it makes no access-control decisions of its own and
holds no policy or allowlist logic.
```

**Permission justifications**: copy each paragraph from
[PERMISSION_JUSTIFICATIONS.md](PERMISSION_JUSTIFICATIONS.md) into the matching box (tabs, debugger,
scripting, nativeMessaging, tabGroups, windows, storage, alarms, and the `<all_urls>` host
permission). They are written to paste one-to-one.

**Privacy policy URL** (interim, stable, on the release branch; upgrade to a GitHub Pages URL when
the site skeleton lands)

```
https://github.com/sylin-org/ghostlight/blob/main/docs/legal/PRIVACY.md
```

**Data usage disclosure** -- recommended answers. This is a compliance attestation the founder
signs at submission; confirm each answer against current dashboard wording before submitting.

- Does this item collect or use user data? Recommend YES, and disclose "Website content" only. The
  extension reads page content, screenshots, and console/network metadata of the automated tab. It
  transmits that data ONLY to the local native application on the same device (never off-device,
  never to the developer, never to a third party). Disclosing it is the defensible choice given the
  broad content-access permissions a reviewer sees; the privacy policy explains the local-only path.
- Do NOT check: personally identifiable information, health, financial/payment, authentication
  information, personal communications, location, web history. The extension does not read the
  credential/cookie store (it has no `cookies` permission), does not collect any of these as data
  types, and does not build a browsing history. It only acts on the specific tab being automated.
- Certifications (all TRUE):
  - I do not sell or transfer user data to third parties outside of the approved use cases.
  - I do not use or transfer user data for purposes unrelated to the item's single purpose.
  - I do not use or transfer user data to determine creditworthiness or for lending purposes.

## Graphic assets checklist

- Store icon: 128x128 PNG. Present at `extension/icons/icon128.png` (also in the package).
- Screenshots: at least one required; 1280x800 or 640x400, PNG or JPEG (a 24-bit PNG is safest).
  The shot list and how to capture each are below.
- Small promo tile: 440x280 PNG. Optional; helps search placement. Not yet produced.
- Marquee promo tile: 1400x560 PNG. Optional; only used if the item is featured.

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

1. Close this session's Ghostlight connection first (the capture script must own the IPC endpoint).
2. Run `pwsh -File scripts/capture-readme-tour.ps1` and record the 1280x800 window with OBS (or any
   screen recorder). The tour self-narrates: nav pill, click ripple and target glow, type shimmer,
   scroll chevrons, the read scan-line.
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
3. Fill the Store listing and Privacy tabs from this file; upload at least one screenshot.
4. Submit for review. Expect extra scrutiny on `debugger` + `<all_urls>` + `nativeMessaging`; the
   justifications and privacy policy are written to answer exactly that.

## Published extension id

The item exists (draft). The store assigned the id
**`lejccfmoeogmhemakeknjjdhkfkgncdl`** -- this is NOT the unpacked-dev id
`cjcmhepmagomefjggkcohdbfemacojoa` (the dev id comes from the pinned manifest `key`, which is
stripped from the store package).

Store-install onboarding therefore points the installer at the published id, so the native host's
`allowed_origins` matches the extension a store user actually runs:

```
ghostlight install --extension-id lejccfmoeogmhemakeknjjdhkfkgncdl
```

The installer already takes `--extension-id` as a validated parameter, so no code change is required
for this. (Optional future step: put the store's public key into `extension/manifest.json`'s `key`
field so unpacked-dev builds share the published id; if done, update the pinned id in the docs and
ADR-0016.)
