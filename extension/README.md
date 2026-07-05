# Ghostlight in Browser: Extension (Manifest V3)

The thin, **policy-free** Chromium extension: a CDP executor + native-messaging endpoint. It holds
mechanism only; all governance lives in the `ghostlight` binary. Not a port: a clean
re-implementation that harvests proven mechanics (MV3 keepalive, live-state tab-group recovery, the
DPR-probe + downscale + coordinate-rescale screenshot model, JPEG 55->30 fallback, shadow-DOM
`form_input` traversal, the phantom-cursor UI) reimplemented from the observed technique, not
copied. See [../docs/adr/](../docs/adr/) for the decisions behind it.

## Files
- `manifest.json`: MV3 manifest (permissions, native-messaging host, background SW, content script).
- `service-worker.js`: native messaging, CDP tool execution, tab-group management, keepalive/recovery.
- `content.js`: DOM reads (accessibility tree, `find`, `form_input` (shadow DOM), `get_page_text`).
- `native-messaging-host.json`: host-manifest template (fill in the binary path + extension ID).

## Setup

The binary self-registers everything:

1. **Build:** `cargo build --release` (or `--debug`).
2. **Load the extension:** open `chrome://extensions` (or `brave://`, `edge://`), enable Developer
   mode, click **Load unpacked**, and select this `extension/` directory. The extension ID is
   pinned by a committed manifest `key`, so it is deterministic across machines.
3. **Register + wire clients:** run `ghostlight install`; it registers the native-messaging host
   and configures detected MCP clients via an idempotent value-level JSON merge (see
   [../docs/adr/0015-idempotent-merge-installer.md](../docs/adr/0015-idempotent-merge-installer.md)).
   `ghostlight doctor` verifies the setup; `ghostlight uninstall` reverses it.
4. **Restart the browser** (native-messaging host configs are read at startup).

Prefer to wire it by hand? See
[../docs/research/11-install-detection.md](../docs/research/11-install-detection.md) for the exact
host-manifest paths per OS/browser.

## Verify
Ask the agent to *navigate to a page and take a screenshot*: the 👻Ghostlight tab group opens
and the screenshot returns.
