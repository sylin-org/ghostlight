# ADR-0052: gif_creator capture via screencast, durable frames, and a breathing export

Status: Accepted (2026-07-09; owner picked screencast + all three pieces after a design pause during
the v0.5.0 local test). Amends ADR-0050 Decision 5's INTERNAL capture design (screenshot-per-action,
in-memory buffer, synchronous export). The tool's schema, governance classification, and advertised
surface do not change; everything here is extension-internal.

## Context

The first live 0.5.0 test broke gif_creator's export twice, and the failure decomposes into three
independent defects:

1. **Synchronous export stalls the service worker.** `encodeRecording` decodes, composites, and
   encodes every frame in one uninterrupted block on the MV3 service-worker thread. The measured
   hot spot is NeuQuant's per-pixel `lookupRGB` (a network search per pixel): 790 ms for just two
   875x1400 frames, ~2.8 s for six. The stall is long enough that the extension side goes silent,
   the hub reports "Browser extension disconnected before responding", and Chrome restarts the
   worker. A per-frame color cache (screenshots hold only a few thousand distinct colors) cuts the
   same two-frame quantization to 23 ms -- a 34x measured win with byte-identical output.
2. **Frames are volatile.** The recording buffer (lib/recbuffer.js) is in-memory. The export crash
   restarted the worker, which erased all captured frames; the suggested "retry the call" found
   nothing to export. A failure that should have been retryable was destructive.
3. **Per-action capture misses the page's actual motion.** One screenshot after each computer/
   navigate call samples the page at an arbitrary post-action instant -- mid-transition, pre-load,
   or after the interesting animation already finished. The owner proposed settle-driven capture
   (poll mutation counts, screenshot while the page is still changing). Prior-art research found the
   browser-native form of exactly that idea: CDP `Page.startScreencast` emits a frame ONLY when the
   page visually changes (no change, no frame), with built-in downscaling (`maxWidth`/`maxHeight`),
   JPEG compression at the source, `everyNthFrame` thinning, and ack-based flow control
   (`Page.screencastFrameAck`). It is the mechanism Playwright's video recording is built on. The
   compositor is a better change detector than a DOM-mutation poll: it sees CSS animations, canvas,
   and scroll, and it pushes instead of being polled.

## Decision

1. **Capture is screencast-driven.** `start_recording` starts `Page.startScreencast` (format jpeg,
   quality ~70, maxWidth/maxHeight at the screenshot token-budget cap) on the recorded tab;
   `stop_recording` stops it. Every `Page.screencastFrame` event is ack'd immediately; a keep-filter
   stores at most one frame per MIN_FRAME_INTERVAL_MS (200 ms) up to the existing 100-frame cap
   (oldest evicted). If no initial frame arrives shortly after start, one seed screenshot is taken
   as a fallback so an idle page still yields a first frame. The per-action screenshot in `dispatch`
   is DELETED; screencast frames carry their own arrival timestamp and viewport metadata.
2. **Frames are durable.** A new IndexedDB store (extension/lib/framestore.js) holds frames as raw
   JPEG Blobs keyed `[tabId, seq]`, plus a small per-tab state record (active flag, seq counter,
   viewport width, pending action tags). Lifecycle: `start_recording` clears the tab's prior frames;
   `stop_recording` freezes (frames kept); `clear` and tab-close purge. A crashed or idle-killed
   service worker no longer loses the recording: export re-reads from IndexedDB, so retry works.
3. **Export breathes and keeps real time.** Quantization memoizes `lookupRGB` per frame (the 34x
   fix). `encodeRecording` yields to the event loop between frames so keepalives flow. Frame delays
   come from the frames' real timestamps (successive deltas clamped to [100, 4000] ms) instead of a
   flat 500 ms, and the last frame holds +2000 ms (the official extension's viewing pause).
   `encodeGif` accepts an optional per-frame `delays` array (additive; `delayMs` still works).
4. **Action overlays tag frames by time, not by capture site.** The dispatch hook shrinks to
   metadata-only: while recording, each computer/navigate call records `{ts, type, coordinate,
   start_coordinate, description}` (coordinates already rescaled to CSS viewport px). Each pending
   action tags the FIRST kept frame whose timestamp is at or after the action's -- the frame where
   the click actually painted. The tagging helper is pure and node-tested.
5. **recbuffer is repealed.** lib/recbuffer.js and its tests are deleted; the bounded-buffer and
   freeze semantics live in framestore + the service worker's small in-memory mirror, and the pure,
   testable logic (delay computation, action tagging) lives beside the overlay helpers.

## Consequences

- The export failure mode observed live (stall -> worker death -> frames lost) is addressed on all
  three axes: the stall shrinks ~34x and yields, and even a killed worker keeps its frames.
- GIFs show transitions as they happened, with true timing, instead of one arbitrary still per
  action; quiet pages cost zero frames.
- New moving parts: one CDP event handler on the existing debugger-event router, one thin IndexedDB
  wrapper. Deleted: the per-action screenshot hook, the capture-time downscale, recbuffer. Net
  moving parts roughly even; capture fidelity and durability strictly better.
- IndexedDB is worker-only, so the frame store itself is live-verified, not node-tested; the pure
  helpers around it are node-tested. No Rust, schema, or pin change anywhere.

## Provenance

Owner direction (2026-07-09, mid live-test): "more meaningful results, less moving parts"; suggested
settle-driven capture and a local disk cache; authorized prior-art research; then chose the CDP
screencast architecture and the full three-piece scope from the presented comparison. Prior art:
CDP Page domain (startScreencast/screencastFrame/screencastFrameAck), Playwright video recording
(screencast-based), the official Claude-in-Chrome v1.0.80 (per-frame delays and last-frame hold;
offscreen-document encoding noted and deliberately not copied -- the color cache + yields keep our
lean inline encoder viable).
