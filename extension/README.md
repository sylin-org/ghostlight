# Ghostlight in Browser: Extension (Manifest V3)

The thin, **policy-free** Chromium extension: a CDP executor + native-messaging endpoint. It holds
mechanism only; all governance lives in the persistent `ghostlight` service. The browser-only
`ghostlight-browser-connector` carries its native-messaging frames; MCP JSON-RPC ends separately in
`ghostlight-mcp-connector`. Not a port: a clean
re-implementation that harvests proven mechanics (MV3 keepalive, live-state tab-group recovery, the
DPR-probe + downscale + coordinate-rescale screenshot model, JPEG 55->30 fallback, shadow-DOM
`form_input` traversal, the phantom-cursor UI) reimplemented from the observed technique, not
copied. See [../docs/adr/](../docs/adr/) for the decisions behind it.

## Files
- `manifest.json`: MV3 manifest (permissions, native-messaging host, background SW, content script).
- `lib/presentation.js`: bounded document-aware state/effect delivery, exact acknowledgements,
  navigation replay, and content-free visual rendering.
- `service-worker.js`: native messaging, CDP primitive execution, tab-group management, and
  keepalive/recovery.
- `lib/chunks.js`: bounded SHA-256-verified reassembly for large service-to-extension commands.
- `lib/diagnostics.js`: opt-in, memory-only console and sanitized network observation.
- `lib/recording.js`: plural bounded volatile recording registry and capture lifecycle.
- `offscreen.html` and `offscreen.js`: bundled browser-local GIF encoding that closes after use.
- `content.js`: DOM reads (accessibility tree, `find`, `form_input` (shadow DOM), `get_page_text`).

Native-messaging manifests are generated and ownership-checked by the native installer. They do
not belong in the extension source or Chrome Web Store package.

## Source-development setup

The binary self-registers everything:

1. **Build:** `cargo build --release`. This produces `ghostlight-mcp-connector`, `ghostlight`, and
   `ghostlight-browser-connector` side by side.
2. **Load the extension:** open `chrome://extensions` (or `brave://`, `edge://`), enable Developer
   mode, click **Load unpacked**, and select this `extension/` directory. The extension ID is
   pinned by a committed manifest `key`, so it is deterministic across machines.
3. **Register + wire clients:** run `ghostlight install`; it registers the native-messaging host
   and configures detected MCP clients via an idempotent value-level JSON merge (see
   [../docs/adr/0015-idempotent-merge-installer.md](../docs/adr/0015-idempotent-merge-installer.md)).
   `ghostlight doctor` verifies the setup; `ghostlight uninstall` reverses it.
4. **Restart the browser** (native-messaging host configs are read at startup).

See the public [installation guide](../docs/guides/installation.md) for packaged installation and
troubleshooting. Platform-specific registration details remain in the installer source and ADRs.

## Verify
Ask the agent to *navigate to a page and take a screenshot*: the Ghostlight tab group opens
and the screenshot returns.

## Adapter protocol 2 mechanisms

Protocol 2 adds independent physical capabilities for window resize, diagnostics, recording,
chunked commands, end-to-end adapter liveness, and reported attention. The native host remains an
opaque byte relay.
The extension answers a content-free heartbeat independently of browser work; the service sends
one every 20 seconds and stops treating the adapter as available after 45 seconds without an
acknowledgement.

The hello carries this installation's persistent browser id, and now also the browser's own product
name and whether it currently holds a focused window. The service keeps one connection per browser
id, so several browsers can be connected at once, and a second connection from this installation
replaces and closes the first rather than leaving two live ports. When a browser window gains focus
the extension reports attention, which is how the service decides where new work goes when a call
does not name a browser. Only the gain is reported, it carries no page or window content, and it is
an ergonomic hint rather than proof that a person did anything.

Large service-to-extension
commands are divided below Chrome's directional message ceiling, then verified and dispatched once
in the extension. Partial transfers are memory-only, concurrency- and byte-bounded, and erased on
expiry or native disconnect. Raw chunks are at most 512 KiB. One transfer is at most 8 MiB and 64
chunks; at most two transfers and 12 MiB of partial data are held for 15 seconds.

Diagnostics are off until the service requests them for a controlled tab. The first read can be
empty because earlier console and network activity was not captured. Entries are bounded and
volatile; network URLs contain only origin and path. A tab close, runtime hold, idle expiry, worker
loss, native disconnect, or ended session erases the capture. Each tab holds at most 1,000 entries
or 2 MiB for five idle minutes. Reads default to 50 problem entries and cannot request more than
200; console text is capped at 2,000 characters.

Recording uses `Page.startScreencast` and acknowledges every compositor frame. The extension owns
the plural in-memory registry, opaque recording ids, compressed-byte bounds, the 120-second hard
stop, final screenshot, five-minute frozen retention, erase, and truthful REC badge. Before
retention it folds each byte-identical JPEG sample into the preceding frame and extends that
frame's visual duration. While recording it disables only the perpetual controlled-scope glow;
transient action feedback remains available. The service sends only
start/status/stop/export/discard requests. The extension encodes the GIF in an offscreen document
and delivers it to the client, a page target, or a browser download as requested. Browser loss,
service disconnect, runtime hold, hard deadline, memory limit, or a JPEG larger than 2 MiB stops
capture locally. No frame crosses a process boundary, and no pixels enter extension storage.
