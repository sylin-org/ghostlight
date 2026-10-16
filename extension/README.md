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
- `manifest.json`: MV3 manifest (permissions, native-messaging host, background worker).
- `worker.js`: the protocol-4 worker. It opens the native port, sends the hello first, answers each
  request with one receipt, keeps the set of controlled tabs, forwards events for those tabs only,
  and feeds Chrome's events to the modules in `lib/`. It makes no policy decision.
- `lib/protocol.js`: the closed protocol-4 vocabulary and bounds, pinned by
  `../tests/golden/protocol4/`.
- `lib/primitives.js`: the primitive dispatcher. One request is one primitive and one receipt.
- `lib/sessions.js`: debugger attachment, flat child sessions and their lifetime.
- `lib/world.js`, `lib/inventory.js`: the service-supplied isolated world and the frame inventory
  with its document guard.
- `lib/tabs.js`, `lib/input.js`, `lib/capture.js`, `lib/evaluate.js`: tab, input, view and script
  mechanisms. `lib/script-evaluator.js` and `vendor/acorn.js` parse a script before any page effect.
- `lib/chunks.js`: bounded SHA-256-verified reassembly for large service-to-extension commands.
- `lib/notice.js`, `lib/presentation-queue.js`: content-free presentation notices and the unseen
  denial queue.
- `lib/buffers.js`, `lib/diagnostics.js`, `lib/recording.js`, `lib/gif.js`: opt-in, memory-only
  console and network evidence, and plural bounded recordings.
- `lib/connection-log.js`, `lib/state.js`, `lib/shared.js`: local connection evidence, stored
  preferences, and the small helpers the worker and the other modules share.
- `lib/setup-view.js`: the setup page's words, commands and three steps as a pure function of the
  connection snapshot and the computer.
- `popup.html`, `options.html`, `setup.html` and their scripts: the extension's own pages. On
  install, the worker opens `setup.html` once, and only when the first connection finds the
  Ghostlight service absent. The page shows the step for this computer and follows the connection
  live. It fetches nothing.
- `offscreen.html` and `offscreen.js`: bundled browser-local GIF encoding that closes after use.

The page code that runs inside tabs is not in this directory. The service ships it in the hello
answer, and the extension verifies its checksum before installing it.

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

## Adapter protocol 4

The worker speaks adapter protocol 4 only (`../docs/design/adapter-protocol-4.md`). A service that
cannot answer a protocol-4 hello is outdated, and the popup says "Update Ghostlight on this
computer." There is no fallback dialect.

The hello is the first frame. It carries this installation's persistent browser id, the browser's
own product name, and whether it currently holds a focused window. The service answers with
`hello_accepted`: its epoch, the page assets with an id, and the control state. The extension checks
the assets against their checksum and installs them into the isolated world `ghostlight` in every
session, the root page and each out-of-process child. A service that sends assets which fail the
checksum is refused.

Each request is one primitive and gets exactly one receipt of facts: whether it dispatched, which
URLs committed, and a closed failure code. The service classifies effects. An unknown primitive is
answered `unsupported`. Both sides ignore unknown fields, frame kinds and events.

The service keeps one connection per browser id, so several browsers can be connected at once, and
a second connection from this installation replaces the first. Events are sent only for tabs
Ghostlight controls. When a browser window gains focus the extension reports attention, which is how
the service decides where new work goes when a call does not name a browser. Only the gain is
reported, it carries no page or window content, and it is an ergonomic hint rather than proof that a
person did anything.

The extension answers a content-free heartbeat independently of browser work; the service sends
one every 20 seconds and stops treating the adapter as available after 45 seconds without an
acknowledgement. Raw CDP is private adapter technique and never a wire command. The native host
remains an opaque byte relay.

Large service-to-extension commands are divided below Chrome's directional message ceiling, then
verified and dispatched once in the extension. Partial transfers are memory-only, concurrency- and
byte-bounded, and erased on expiry or native disconnect. Raw chunks are at most 512 KiB. One
transfer is at most 8 MiB and 64 chunks; at most two transfers and 12 MiB of partial data are held
for 15 seconds.

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
