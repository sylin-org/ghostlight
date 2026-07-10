# ADR-0053: The thin-extension rule; the GIF pipeline moves into the service

Status: Accepted (2026-07-09; owner directive during the ADR-0052 review: the extension was meant to
be "as thin as possible ... the least responsibilities the extension had, the better", and the GIF
work had drifted the other way). Sharpens ADR-0005 (policy-free extension). Amends ADR-0050 Decision
5 (the vendored JS GIF encoder placement) and ADR-0052 (whose capture SEMANTICS -- change-driven
screencast frames, durable storage, real timing, action tagging -- are all KEPT, but whose state and
computation relocate from the extension into the binary). Ship gate: this lands BEFORE the v0.5.0
release is tagged, so the fat extension is never published.

## Context

ADR-0005 made the extension policy-free but "not necessarily minimal". ADR-0050 D5 then placed a
vendored JS GIF encoder in the extension -- copying the OFFICIAL extension's placement without
noticing the official has no binary to put it in. We do; the single-binary-plus-thin-extension split
is the project's founding architecture. ADR-0052 compounded the drift (IndexedDB frame store,
NeuQuant, overlay compositing, timing, and tagging all extension-side), engineering around
service-worker fragility (kill risk, in-memory volatility, weak testability) that only exists
BECAUSE the computation sits in the extension.

The operational costs of a fat extension are concrete:

1. **Distribution.** Every extension behavior change ships through Chrome Web Store review (days,
   rejection risk). The binary ships in minutes. A fat extension moves the fastest-changing code
   behind the slowest channel.
2. **Version skew.** Fat extension + fat service is a compatibility matrix; a thin extension keeps
   the wire protocol stable and makes skew harmless. The live 0.5.0 test hit stale-extension
   confusion twice in one day.
3. **Testability.** The binary has first-class cargo tests; extension logic gets node tests for the
   pure parts and "live-verified" for the rest (the canvas compositor was exactly that).
4. **Runtime fragility.** MV3 service workers are killable and single-threaded; Rust gets
   spawn_blocking and never loses state to a worker restart.

The counterweight, stated honestly: OffscreenCanvas gives the extension free JPEG decode, text
rendering, and compositing, which Rust replaces with a decoder crate and an embedded font. Frames
crossing the native-messaging pipe are a non-cost -- screenshots already cross it constantly, and
the framing layer caps messages at 128 MiB.

## Decision

1. **The thin-extension rule (sharpens ADR-0005).** The extension contains only what must touch a
   Chrome API (chrome.debugger/CDP, content-script DOM access, tab/window/group lifecycle, native
   messaging itself). Anything that is pure computation or durable state belongs to the binary.
   Future feature work applies this test before placing code.
2. **Capture stays extension-side, as a relay.** `Page.startScreencast` / frame ack / stop are
   chrome.debugger mechanics. The service's gif_creator handler COMMANDS capture via two small
   internal extension operations (start: {tabId, quality, maxSide, minIntervalMs}; stop: {tabId});
   the extension acks every compositor frame, applies the service-chosen minimum interval (transport
   thinning, policy chosen by the service), and forwards each kept frame to the binary as an
   unsolicited `gif_frame` event: {tabId, data (base64 JPEG), ts, deviceWidth}. The seed screenshot
   at start reuses the existing screenshot path. The extension holds NO recording buffer.
3. **Recording state and frames live in the service.** A per-tab recording session (active flag,
   frame index) with frames written to disk under the instance's data directory
   (`recordings/<tabId>/`). Lifecycle: start clears the tab's prior recording; stop freezes; clear
   and a startup sweep purge; frames survive service restarts incidentally (disk), but the
   authoritative resilience story is now "the service is a normal process", not "engineer around
   worker death".
4. **Action tagging moves to the pipeline.** The service dispatches every tool call, so it notes
   {ts, type, coordinate, start_coordinate, description} for computer/navigate calls on recording
   tabs itself -- the extension's dispatch hook is deleted. Model-space coordinates rescale to CSS
   viewport px in Rust: the extension's screenshot RESPONSE gains its ScreenshotContext snapshot
   ({vpW, vpH, shotW, shotH, offX, offY, regionW, regionH} -- data the extension already computes),
   and the service mirrors the latest per tab. `rescaleCtxCoord` ports to Rust with its tests.
   Tagging semantics are ADR-0052 D4 unchanged: each action tags the first kept frame at-or-after
   its timestamp.
5. **The GIF pipeline is Rust.** A `gif` module in ghostlight-core: JPEG decode via a pure-Rust
   decoder crate (`jpeg-decoder`); adaptive palette via `color_quant` (the image-rs NeuQuant -- the
   same algorithm ADR-0050 vendored, now as a maintained dependency); the GIF89a writer + LZW via
   the image-rs `gif` crate (weezl LZW). [Amended during the live test: this was first hand-ported
   from the JS, but the port carried a latent code-width off-by-one at the first 9->10 bit
   transition -- it round-tripped through its own matched decoder yet strict third-party decoders
   rejected it. Replaced with the library per this ADR's own Decision 1 rationale and ADR-0008: do
   not hand-roll a codec.] Overlay
   compositing (click ring, drag path, action label, progress bar, Ghostlight watermark pill) as
   pure RGBA-buffer drawing with the geometry ported from lib/gifoverlay.js; label/watermark text
   via an embedded ASCII bitmap font (zero-dep, deterministic; upgrade to a rasterized TTF only if
   the live look disappoints). Per-frame delays keep ADR-0052 D3 semantics (deltas clamped
   [100, 4000] ms, last frame 800+2000 ms). Encoding runs under spawn_blocking. Every JS test
   oracle (GCE delay bytes, solid-frame roundtrip via an independent decoder, delay clamping,
   overlay routing/geometry, palette determinism) ports to cargo tests.
6. **gif_creator becomes a service-side orchestrator tool** (the form_fill/ADR-0036 precedent: a
   binary-side handler dialing internal extension operations). start/stop/clear command the relay;
   export reads frames from disk and runs the Rust pipeline; the download branch returns the GIF as
   MCP image content exactly as today; the coordinate branch hands the encoded bytes to the
   EXISTING internal setImage path (T3's upload_image machinery) for the drag-drop. The advertised
   schema, classification, and audit surface do not change.
7. **The extension sheds the pipeline.** Deleted: lib/gifenc.js, lib/neuquant.js, lib/gifoverlay.js,
   lib/framestore.js, the canvas overlay drawing, the IndexedDB mirror, the dispatch action hook,
   and their node test files (oracles ported to Rust first). The extension's remaining gif surface
   is the capture relay of Decision 2.
8. **Protocol notes.** The `gif_frame` event is the first unsolicited extension-to-binary message
   beyond the connection handshake; the binary's reader must route it by type and IGNORE unknown
   event types (pin with a test), so old-binary/new-extension skew degrades to "recording captures
   nothing" rather than an error. New crates (jpeg-decoder, color_quant) are pure Rust, image-rs
   maintained, dual-licensed -- compatible with the workspace's licensing and its zero-C-deps
   posture.

## Consequences

- The extension returns to being a stable, thin executor; GIF behavior fixes ship with the binary
  (minutes) instead of through CWS review (days).
- The whole class of worker-death failures engineered around in ADR-0052 (stall -> kill -> loss)
  stops being reachable: encoding happens in a real process on a blocking thread, state on disk.
- The pipeline gains real cargo tests; the JS oracles transfer rather than being rewritten.
- Costs accepted: one JPEG-decoder dependency + color_quant; an embedded bitmap font renders labels
  more plainly than canvas system-ui text (revisit only if the live look disappoints); frames
  transit the pipe during recording (bounded by the min-interval and the per-frame JPEG size, the
  same order as routine screenshots).
- ADR-0052's commits remain correct history; its semantics survive relocated. The v0.5.0 release
  ships the thin extension -- no published artifact ever carries the extension-side pipeline.

## Provenance

Owner directive and review question (2026-07-09): "I wanted the chrome extension to be as thin as
possible ... the least responsibilities the extension had, the better. Am I wrong here?" -- decided
as: not wrong; adopt the rule, move the pipeline, move BEFORE tagging v0.5.0. Options and trade-offs
(including the OffscreenCanvas counterweight and a storage-only half-step, rejected as splitting the
pipeline across two processes) were presented and chosen in-session. Prior art for placement: the
official extension encodes extension-side only because it has no binary; Ghostlight's founding
architecture (ADR-0001) is the binary.
