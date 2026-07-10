# LEDGER -- official-rebaseline batch (ADR-0050)

Durable progress log. One task = one entry. The executor updates this file as the LAST step of each
task (or when marking BLOCKED). A human reads RESUME HERE to pick up.

## RESUME HERE

- Status: **BATCH FULLY COMPLETE** -- T1..T5 + T4 Phase 2 all DONE (count 21). Everything in ADR-0050
  is shipped. **gif_creator REFINEMENTS (post-batch, owner-requested): BOTH TRACKS DONE** -- TRACK 1
  (adaptive NeuQuant palette) + TRACK 2 (visual overlays) shipped, extension-only, all node-tested.
  **ADR-0052 capture redesign EXECUTED on top (2026-07-09): see the entry below.**

- **ADR-0052 -- gif_creator capture redesign: EXECUTED** (three commits, each independently landable;
  extension-only, no Rust/schema/pin change). Origin: the FIRST LIVE 0.5.0 TEST broke export twice --
  the synchronous encode stalled the worker (NeuQuant per-pixel lookupRGB: 790ms for 2 frames), the
  native-messaging link dropped, Chrome restarted the worker, and the IN-MEMORY frames died with it
  (retry found "No frames"). Owner paused the debugging and asked for "more meaningful results, less
  moving parts" (suggested settle-driven capture + a disk cache; authorized prior-art research).
  Research found the browser-native form of the settle idea: CDP `Page.startScreencast` emits frames
  ONLY on visual change (Playwright video's mechanism). Owner chose screencast + all three pieces.
  - Piece 1 (`b577307`) export fixes (D3): color-cache memoizing lookupRGB per distinct color
    (full 6-frame encode 2830ms -> 385ms, byte-identical); event-loop yields between frames;
    real per-frame delays via `computeFrameDelays` (deltas clamped [100,4000]ms, last frame
    800+2000ms hold); `encodeGif` gains optional `delays` (additive). New GCE-delay pin test
    (structure-aware parser) + delay-oracle tests.
  - Piece 2 (`fc045f4`) durable frames (D2/D5): new `extension/lib/framestore.js` (thin IndexedDB
    wrapper; frames as JPEG Blobs keyed [tabId,seq] + per-tab state record; seq bookkeeping DERIVES
    from stored keys on rehydration). Worker keeps a hot-path mirror (`gifRec`, null-cached), export
    re-reads the store (crash-retryable), tab close purges (also fixes a latent leak: gif state was
    never cleaned on tab removal). `lib/recbuffer.js` + its tests DELETED; ci.yml + BOOTSTRAP node
    lines updated. IndexedDB is worker-only -> framestore is live-verified, pure logic node-tested.
  - Piece 3 (`c1c475b`) screencast capture (D1/D4): seed screenshot then `Page.startScreencast`
    (jpeg q70, maxWidth/maxHeight MAX_SIDE); `Page.screencastFrame` joins the debugger event router
    (ack immediately; keep <=1 frame per 200ms; cap 100); dispatch hook is METADATA-ONLY and runs
    BEFORE the action (pendingActions queue; `takeActionForFrame` pure helper: each action tags the
    first kept frame at-or-after its ts); stop/clear stop the screencast; per-frame vpW from
    screencast metadata deviceWidth (probed fallback).
  - Extension node --test green after each piece. LIVE VERIFICATION of the full loop (record ->
    actions -> export with overlays) is the NEXT step; the loaded extension must be reloaded first.
  - Known edges (documented, acceptable): a mid-recording debugger detach (kill switch) freezes
    capture until stop; a pre-noted action whose handler then throws may tag a frame anyway; pending
    action tags live in the mirror only (a worker death loses un-drained tags, never frames).

- **ADR-0053 -- thin-extension rule + the GIF pipeline moves into the binary: EXECUTED** (same day,
  before ADR-0052's live verification ever ran; the owner's review of the fat extension triggered
  the course correction, and the v0.5.0 ship gate holds until this landed). Commits: ADR `0009955`,
  piece A `626bd5f` (Rust pipeline: src/gif/{writer,quantize,overlay,font,mod}.rs -- LZW/GIF89a
  hand-ported from the oracle-tested JS, color_quant NeuQuant, reference overlay geometry recolored
  sky-blue, embedded public-domain font8x8, jpeg-decoder; 22 ported oracle tests), piece C
  `b134bb4` (the move: hub/outbound/recording.rs per-tab sessions + disk frames w/ startup sweep;
  Browser routes the first unsolicited ext event `gif_frame` + notes actions pre-dispatch with
  rescale-by-QUERY (`rescale_coords` internal op -- replaces the planned ctx-mirror piece B;
  mechanism data stays extension-side, queried not mirrored); mcp/gif_creator.rs Handler::Local
  orchestrator (first action_key Local tool -- lands at the existing post-grant dispatch, ZERO
  pipeline change; export under spawn_blocking; drag-drop reuses upload_image_exec); b64.rs
  std-only base64 w/ RFC 4648 pins; directory handler flip = wiring only, schema untouched;
  service-worker.js reduced to the capture relay: gif_capture_start/stop + ack/thin/forward +
  rescale_coords, ZERO stored state), piece D `249e017` (deleted lib/{gifenc,neuquant,gifoverlay,
  framestore}.js + their node tests -- oracles ported to cargo first; ci.yml + BOOTSTRAP lines
  shrunk). **V-ALL GREEN: workspace fast tier 688/0, fmt + clippy -D warnings clean, extension
  node 38/38.** Deliberate divergences recorded in the ADR: bitmap-font labels (upgrade only if
  the live look disappoints), no soft label shadow. Known edges: the service cannot see tab-close
  (dead-tab frames purge at the startup sweep or the next start_recording); recordings do not
  survive a service restart's sweep (the service is a normal process -- ADR-0053 Consequences).
  LIVE VERIFICATION still owed: this move CHANGES THE BINARY, so the live test needs BOTH a
  rebuilt+reinstalled release binary AND an extension reload, then record -> actions -> export.

- **TRACK 2 -- visual overlays: DONE.** New `extension/lib/gifoverlay.js` (pure geometry + routing:
  `describeAction`/`resolveOverlayOptions`/`scaleFactorFor`/`clickRadii`/`labelBox`/`progressBarRect`/
  `overlayPlan`, harvested from the reference offscreen.js). service-worker.js: added the canvas draw
  halves (`drawClickIndicator`/`drawDragPath`/`drawActionLabel`/`drawProgressBar`/`drawWatermark`/
  `compositeOverlays`) recolored to Ghostlight sky-blue (#38BDF8; the watermark is a "Ghostlight" pill,
  NOT Claude's logo); `encodeRecording(frames, delayMs, options)` now composites overlays before
  quantizing; `maybeCaptureGifFrame(tabId, meta)` + `gifFrameMeta(tool,args,tabId)` store per-frame
  action metadata with coordinates rescaled to CSS viewport px (via `rescaleCoord`, read BEFORE the
  capture screenshot overwrites the ctx) + `vpW`; `dispatch` builds+passes the meta; `start_recording`
  seeds an object frame; `export` threads `a.options` (the existing schema param -- no schema change).
  recbuffer frames are now objects `{base64, vpW?, type?, coordinate?, start_coordinate?, description?}`
  (recbuffer stays generic). Overlays gated by `options.{showClickIndicators,showDragPaths,
  showActionLabels,showProgressBar,showWatermark}`, all default true. New `tests/extension/
  gifoverlay.test.js` (7) wired into ci.yml + BOOTSTRAP; extension node --test 58/58 green; the canvas
  draw itself is live-verified (OffscreenCanvas, not node-testable). NO Rust/schema/pin change. Commit:
  `feat(gif): visual overlays for gif_creator (click cues, labels, progress, watermark)`.

- **TRACK 1 -- adaptive palette: DONE.** Per the owner's standing "check the reference original code
  for standards/patterns" directive, the official Claude-in-Chrome v1.0.80 ships gif.js 0.2.0, whose
  quantizer is **NeuQuant** (Dekker's neural-net, deterministic -- `Math.random` count 0). We adopted
  THAT reference standard instead of the median-cut sketched below. New `extension/lib/neuquant.js`
  (vendored, de-minified faithfully from the reference `gif.worker.js`, MIT). `extension/lib/gifenc.js`
  now builds ONE adaptive 256-color GLOBAL palette from all frames (bounded 500k-pixel deterministic
  training sample) via NeuQuant and maps every pixel with the network's own `lookupRGB`; the fixed
  3-3-2 `palette332`/`frameToIndices` are gone; LZW + GIF framing unchanged. `encodeGif` signature
  preserved (added optional `sampleFac`, default 10 = gif.js's default quality). Tests: new
  `tests/extension/neuquant.test.js` (4) + rewrote the gifenc oracle tests (2x2 solid now asserts
  4 identical indices via the independent decoder roundtrip, not a fixed index; gradient roundtrips
  against `quantizeFrame`; added a determinism + primaries-converge test with a MEASURED threshold --
  32x32 frames clear NeuQuant's 1509-byte `minpicturebytes` floor and converge to <=1, so slack 16).
  Both test files added to `ci.yml` + BOOTSTRAP node --test lines. Extension node --test 51/51 green;
  `node --check` on neuquant/gifenc/service-worker clean; Rust untouched (extension-only). Commit:
  `feat(gif): adaptive NeuQuant palette for gif_creator (reference-standard quantization)`.

## Post-batch: gif_creator refinements plan (owner-requested)

Two independent tracks; each is EXTENSION-ONLY (no Rust/schema/pin change -- the schema + governance
already exist), keep everything self-contained ASCII JS (MV3: no runtime fetch), and DETERMINISTIC
(no Math.random -- it throws in our test harness context and breaks reproducibility). Land each as its
own commit with node --test coverage where the logic is pure.

- TRACK 1 -- richer color QUANTIZATION (do FIRST; contained, node-testable, biggest visual win).
  Today `extension/lib/gifenc.js` uses a FIXED 3-3-2 uniform palette (`palette332` + `frameToIndices`
  by bit-truncation) -- coarse for photographic screenshots. Replace with an ADAPTIVE palette built
  from the actual frames: sample all frames' pixels, median-cut (deterministic, ~100 lines, no RNG)
  to <=256 colors, then map each pixel to the nearest palette entry (cache/box-lookup for speed).
  GIF uses ONE global color table in our encoder, so build ONE global palette across all frames.
  Keep `encodeGif(frames,{width,height,delayMs})` signature; the LZW stays as-is (already
  roundtrip-tested). New tests in `tests/extension/gifenc.test.js`: feed known N<=256 distinct colors
  -> palette reproduces them exactly + indices round-trip; feed a gradient -> bounded average error.
  The existing exact-2x2 oracle test assumes the 3-3-2 mapping (black->index 0) -- UPDATE it for the
  adaptive palette (recompute the oracle, or assert via decode round-trip instead of exact bytes).

- TRACK 2 -- visual OVERLAYS (larger; part live-only). **Reference spec harvested** from the official
  `offscreen.js` (`.../fcoeoabgfenejglbffodgkkbkcdhcgfn/1.0.80_0/offscreen.js`, functions
  `drawClickIndicator`/`drawDragPath`/`drawActionLabel`/`drawProgressBar`/`drawWatermark`/
  `applyActionIndicators`/`generateGif`). Adopt its GEOMETRY and OPTIONS schema; DIVERGE on two points
  by our own memories: keep our lean inline OffscreenCanvas encoder (the reference uses an offscreen
  document + gif.js + Web Workers -- see [[not-a-port-lean-internals]]), and RECOLOR the Claude-coral
  overlays to Ghostlight **sky-blue #38BDF8** (see [[ghostlight-visual-fx]]; the ref click ring is
  rgba(207,107,60,*) = #CF6B3C coral, progress #C96442, watermark = Claude logo -- swap all for our
  brand + our ghost mascot, NOT Claude's logo). Exact ref values to port (then recolor): click ring =
  outer arc r=15 (0.3 alpha), inner arc r=11 (0.5 alpha), border arc r=11 stroke lw=2; action label =
  14px system-ui, rounded-rect bg rgba(0,0,0,0.85) + drop shadow, white text, edge-aware placement;
  progress bar = bottom, height 4, bg rgba(0,0,0,0.3), fill brand; watermark = bottom-right, 32px,
  rounded bg. All sizes * `scaleFactor` where `scaleFactor = canvas.width / frame.viewportWidth`. The
  `options` object (open `{"type":"object"}` in our schema) gates via fields
  `showClickIndicators`/`showDragPaths`/`showActionLabels`/`showProgressBar`/`showWatermark`, ALL
  default true (ref `?? true`). Last frame gets +2000ms delay (ref). Needs TWO parts: (a) capture
  action METADATA per frame -- `recbuffer` frames currently store only base64; extend the frame shape
  to `{base64, action, coordinate, start_coordinate, description, viewportWidth}` and have
  `maybeCaptureGifFrame(tabId, meta)` (called from `dispatch` after computer/navigate) pass the tool
  type + coordinate(s) + a human label; (b) COMPOSITE overlays onto each frame in `encodeRecording`
  (service-worker.js) via OffscreenCanvas BEFORE quantizing -- port the ref draw fns (arc/fillText/
  fillRect/quadraticCurveTo/Path2D all work on OffscreenCanvasRenderingContext2D). Gate the ref's
  action-type routing: click/scroll -> ring (+label); left_click_drag -> drag path (+label); type/key/
  wait -> top-left label. Extract the pure per-frame overlay math (ring/label/bar positions, edge
  clamping, scaleFactor) into a testable module and node-test it against numbers; the canvas draw
  itself is live-verified. Honor `options.*` (default all on). Update the T4 LEDGER "deferred" note as
  it ships.
- (Historical) T4 Phase 2 plan, now done: the
  `gif_creator` `export` handler's `coordinate` branch (currently returns the Phase-1 "not yet
  supported (Phase 2)" text at service-worker.js) must instead ENCODE the GIF (`encodeRecording`)
  and drag-drop it as an `image/gif` File at the coordinate by REUSING T3's `content(tabId,
  {type:"setImage", coordinate, data, filename, mimeType})` path (content.js `setImage` already does
  the DragEvent drop for a coordinate). NO schema change (the `coordinate` param is already declared),
  NO Rust change, NO new count/variant pins -- `export` already classifies Write (covers download +
  coordinate). It is EXTENSION-ONLY + live-verified (the DragEvent path is not node-testable, same as
  T3's setImage coordinate branch); verify service-worker.js parses (`node --check`) + extension node
  tests still 12/12. Commit: `feat(tools): gif_creator phase 2 -- drag-drop GIF export`. Overlays +
  richer color quantization stay DEFERRED (out of scope for Phase 2).
- Base commit for the batch: `d52e0df`. T2 = `72f9b8a`, T3 = `b9b5dbb`, T4 = (this commit).
- Advertised tool count is now **21** (`file_upload`, `browser_batch`, `upload_image`, `gif_creator`,
  then `explain`); `total_variants == 37`, `with_action_key == 3`. Before T5, re-read the tree.
- **T4 PRE-FLIGHT FINDINGS (2026-07-09, before starting T4):**
  - **Part A schema source is GONE:** `scratchpad/harvest/HARVEST-1.0.80.md` (the ephemeral harvest
    that pinned gif_creator's description + params) no longer exists (a prior session's scratchpad).
    ADR-0050 D5 does NOT carry the full schema (it defers to "the T4 task prompt", which defers to the
    now-missing harvest). So T4 must EITHER re-extract from the installed official extension (id
    `fcoeoabgfenejglbffodgkkbkcdhcgfn`, `assets/mcpPermissions-*.js`, search `name:"gif_creator"` --
    an interactive/founder step) OR RECONSTRUCT a reasonable schema (acceptable: gif_creator is a NEW
    additive tool, NOT one of the 13 trained, and the fidelity test is a regression snapshot, so
    exact-official match is not load-bearing -- but flag the `options` gap per the prompt's fallback).
  - **Capability discrepancy RESOLVED to the PROMPT:** ADR-0050 D5 says a download-export classifies
    Read; the T4 prompt Part B classifies `export` as `[Write]` (fail-closed -- the variant keys on
    `action`, not the `download` flag, and Phase-2 coordinate export IS a page write). Use `[Write]`
    (the prompt's `EXPECTED` table pins `("gif_creator", Some("export"), &[Capability::Write])`); the
    ADR's "Read" was the earlier thought. Note the discrepancy in the T4 entry.
  - **The RISK is the vendored LZW GIF89a encoder** (`extension/lib/gifenc.js`): a from-scratch port
    of an omggif/gif.js-style encoder (~200 lines of bit-packed LZW). The Part E test oracle is WEAK
    (only checks the `GIF89a` header bytes + non-trivial length -- it would NOT catch an LZW bug that
    produces a corrupt-but-header-valid GIF). Consider strengthening it (a decode round-trip, or port
    an encoder that ships known test vectors) rather than trusting the header check.
  - Frame capture reuses `service-worker.js`'s `async function screenshot(tabId)` (line ~749).
  - Post-T3 STOP numbers to re-confirm before applying Part D deltas: `total_variants == 33`,
    `with_action_key.len() == 2`. Deltas: total_variants 33->37 (+4 variants), with_action_key 2->3,
    count 20->21 (only directory.rs pins + tool_schema_fidelity + all_open_golden + pipeline.rs
    explain literal; the mcp_protocol/hub-outbound/tool_enforcement count asserts DERIVE -- skip).
- BUILD NOTE (post dev re-install): live MCP clients continuously respawn `ghostlight-relay` and lock
  the normal `target/debug`, so the FULL V-ALL (which builds relay + spawns for the e2e tier) must run
  in an ISOLATED `CARGO_TARGET_DIR` (`CARGO_TARGET_DIR=$TMP/gl-target cargo build --workspace && cargo
  test --workspace -- --include-ignored --test-threads=1 < /dev/null`). Kill orphan `ghostlight.exe`
  first if a prior isolated run left a service locking the isolated dir. Core-lib-only checks
  (`cargo test -p ghostlight-core --lib`) run fine in the normal dir (no exe link).
- IMPORTANT verification note (see ADR-0051 + docs/design/verification-topology-evaluation.md): the
  advertised count/name set is pinned in MANY scattered spawn tests the prompts do NOT all enumerate
  (adapter_override, adapter_reconnect x3, hot_reload's expanded+full_set, pipeline.rs's explain
  literal, plus the 8 count sites). Before committing an additive-tool task, grep the WHOLE tree for
  the old count AND the tail name pair (`"form_fill"`, `"explain"`) and `Some(<oldcount>)`. Run the
  local spawn tier serially with closed stdin and no live `ghostlight service`
  (`cargo test ... -- --test-threads=1 < /dev/null`), else it hangs/flakes environmentally.

- **RE-PIN (ADR-0051 P1.1/P4.2 landed AFTER this batch was authored; supersedes every task's
  count-bump steps):** the advertised count now DERIVES from
  `directory::advertised_tool_count()` / `advertised_tool_names()` at ALL behavior sites, which no
  longer carry a literal to bump: `tests/mcp_protocol.rs`, `tests/tool_enforcement.rs`,
  `crates/core/src/hub/outbound/mod.rs` (x2), `tests/adapter_override.rs`,
  `tests/adapter_reconnect.rs`, `tests/hot_reload.rs`. So T2 Part D items 6/8/9, and the analogous
  count-assert steps in T3/T4/T5, are OBSOLETE -- do NOT edit those assertions. The ONLY sites an
  additive tool still hand-edits are: (1) `crates/core/src/browser/directory.rs` -- the REGISTRY row,
  the `EXPECTED` + `EXPECTED_TOOLS` `#[cfg(test)]` tables, the `total_variants` literal, and the two
  doc-comment counts (`N descriptors`, `N rows`); (2) `tests/tool_schema_fidelity.rs` -- `names.len()`
  + `all.len()` literals and the tail position asserts; (3) `tests/all_open_golden.rs` --
  `GOLDEN_TOOL_NAMES` array + its `[&str; N]` len + count message + doc; (4)
  `crates/core/src/mcp/pipeline.rs` -- the frozen `pinned_explain_text()` literal (the prompts OMIT
  this; add the new tool's `"<tool>: requires <cap>. <directory_description>"` line before `explain`).
  Stale DOC-COMMENT counts elsewhere (e.g. `tool_enforcement.rs`'s "18 tools" narration,
  hub/outbound's "N-declaration REGISTRY" prose) are cosmetic -- update for accuracy, but they are not
  assertions and never block V-ALL.

## Task log

(Each entry, filled on completion or BLOCK:)

### T1 -- file_upload
- Status: DONE
- Commit(s): (filled at commit)
- V-ALL: pass. fmt/clippy/build clean; ~600 unit tests + directory/hub pins (32) + the four oracle
  suites (tool_schema_fidelity, all_open_golden incl. the new governance test, mcp_protocol,
  tool_enforcement) + both pipeline.rs explain-text pins + the extension node --test (fileset 4/4)
  all green. The spawn tests that initially failed were fixed (see deviations) and re-run to green in
  isolation (adapter_override 2, adapter_reconnect 2, hot_reload 1). Local full-workspace green
  requires the Phase-1 procedure (serial + closed stdin + no live service; ADR-0051).
- Deviations:
  1. The prompt did not enumerate `crates/core/src/mcp/pipeline.rs`'s frozen `pinned_explain_text()`
     literal. `explain`'s output is DERIVED from the directory, so adding file_upload changed it.
     Added the `"file_upload: requires write. Upload files (base64 bytes) ..."` line before explain,
     matching the real formatter (`requires.first()` -> "write").
  2. The prompt AND the C1 red-team both missed four hardcoded advertised-COUNT asserts in spawn
     tests: `tests/adapter_override.rs:227` and `tests/adapter_reconnect.rs:{174,200,307}`, all
     `Some(17)` -> `Some(18)`. (These only fail through the E2E tier, which is why they were missed.)
  3. The prompt missed two advertised-NAME-set arrays in `tests/hot_reload.rs`: the `expanded`
     write-grant set and the `full_set` all-open set both needed `"file_upload"` before `"explain"`
     (file_upload requires [write], a subset of the [read,action,write] grant). `governed_read_only`
     correctly excludes it. Also bumped two stale doc counts (a "(17 tools)" -> 18 and a pre-existing
     stale "all-open 14" -> 18).
  4. (Process, not code) Local V-ALL's spawn tier is environment-sensitive: it hangs on interactive
     stdin and flakes on a relaunching persistent service / Chrome exe-lock. Ran it serially with
     `< /dev/null` and no live service. This fragility motivated ADR-0051 + the eval doc (authored in
     the same working session but a SEPARATE track from the ADR-0050 batch).
- Notes: file_upload is ExtensionForward (no new Rust arg struct/wire type); extension path is
  `lib/fileset.js decodeFiles` -> content.js `setFiles` -> service-worker `file_upload` handler;
  `paths` advertised-but-rejected (no host FS). New `tests/extension/fileset.test.js` added to ci.yml
  + BOOTSTRAP V-ALL. Two ADR-0050-unrelated files also landed for the verification eval
  (docs/design/verification-topology-evaluation.md, docs/adr/0051-*.md, README index) -- these are
  the owner-requested architecture evaluation, committed separately from T1.

### T2 -- browser_batch (overload; script kept)
- Status: DONE
- Commit(s): (filled at commit)
- V-ALL: pass (isolated CARGO_TARGET_DIR -- a live client relay locks the normal target/debug after
  the dev re-install). fmt --check clean; clippy --workspace --all-targets -D warnings clean; full
  workspace `cargo test -- --include-ignored --test-threads=1` = 44/44 binaries green (core lib 483
  incl. the 5 new browser_batch tests + all script.rs regression tests unchanged; the four oracle
  suites; the batch-reject test now asserting `browser_batch`; and the e2e tier).
- Deviations:
  1. Per the RESUME-HERE RE-PIN (ADR-0051 P1.1/P4.2 landed after authoring): Part D items 6/8/9 were
     OBSOLETE -- mcp_protocol/hub-outbound/tool_enforcement count asserts DERIVE from
     `advertised_tool_count()` now and carry no literal to bump. Left untouched (only their cosmetic
     doc-comment "18 tools" narration updated to 19).
  2. The prompt (like T1) omitted `crates/core/src/mcp/pipeline.rs`'s frozen `pinned_explain_text()`
     literal. Added the `browser_batch: requires nothing. Run a sequence of tool calls ...` line
     before explain (matching the real formatter: `&[]` -> "requires nothing").
  3. The prompt omitted `crates/core/src/browser/advertise.rs`'s OWN inline unit tests (the
     read-only + empty-grants advertised-set goldens the tool_advertisement.rs integration test defers
     to). browser_batch requires nothing, so it joins EVERY advertised set; added it to both.
  4. The prompt omitted the scattered advertised-set pins in the e2e/spawn tests: `hot_reload.rs`
     (`governed_read_only` + `expanded`), `manifest_validation.rs` (read-only), and
     `tool_advertisement.rs` (read-only + empty-grants). Added `browser_batch` before `explain` in all.
     (This is exactly the class the RESUME-HERE note warns about; the grep-the-whole-tree step found
     them.)
  5. SANCTIONED design deviation: `run_batch`'s signature gained `orchestrator: &'static str` (the
     prompt's A1 signature omitted it, hardcoding "script"). browser_batch's internal step audit
     records must be attributed to `"browser_batch"`, not `"script"` -- honest audit attribution in a
     governance tool. `interpret` (script) passes "script", so script's audit + compact output are
     byte-identical (proven by the unchanged script.rs regression suite).
- Notes: Part A refactor is behavior-preserving for `script`: the shared loop is now
  `run_batch -> BatchRun{steps: Vec<StepOutcome>, summary, duration_ms, batch_id}`, where
  `StepOutcome.result` keeps each step's FULL MCP result (content + structuredContent) so
  browser_batch preserves images; `build_compact(BatchRun)` derives the compact text/structured from
  it. `interpret = build_compact(run_batch(.., "script"))`. `StepRunner`/`PipelineRunner` are now
  `pub(crate)` so browser_batch wires the same engine. Nesting is symmetric (a `script` OR
  `browser_batch` step is rejected in either batcher).

### T3 -- upload_image (screenshot cache + drag-drop)
- Status: DONE (NOT split -- one commit)
- Commit(s): (filled at commit)
- V-ALL: pass (isolated CARGO_TARGET_DIR). fmt --check + clippy --all-targets -D warnings clean;
  core lib 487 (the screenshot-cache test + 3 upload_image arg-guard tests); extension node --test
  7/7; full workspace `cargo test -- --include-ignored --test-threads=1` = 44/44 binaries green
  (incl. the e2e tier with `upload_image` in the all-open + write-grant advertised sets).
- Deviations:
  1. Re-pin (RESUME note): Part E items for `mcp_protocol` / `hub/outbound/mod.rs` / `tool_enforcement`
     count asserts are OBSOLETE (they derive post-ADR-0051); left untouched (cosmetic doc counts only).
  2. Prompt omitted `pipeline.rs`'s `pinned_explain_text()` literal (same gap as T1/T2). Added
     `"upload_image: requires write. Upload a previously captured screenshot ..."` before explain.
  3. Advertised-SET goldens (prompt lists only the count pins): `upload_image` requires `[Write]`, so
     it joins `all_open_golden` (all-open full set) and `hot_reload.rs`'s `expanded` (write grant); it
     is correctly ABSENT from the read-only / empty-grants sets (advertise.rs, tool_advertisement,
     hot_reload governed_read_only, manifest_validation) -- no edit there.
  4. The Part F Browser test is named `screenshot_cache_round_trips_and_injects_image_id` (snake_case)
     rather than the prompt's `..._imageId`, to satisfy `-D warnings` (non_snake_case).
  5. No NEW extension node test: `setImage`'s decode REUSES `lib/fileset.js`'s `decodeFiles` (already
     covered by `fileset.test.js`); `setImage` itself is DOM-only (DataTransfer/DragEvent), not
     node-testable. The arg guard is tested via the pure `validate_target` in upload_image.rs.
- Notes: injection site = `Browser::call`, AFTER `send_and_await` succeeds, for `tool == "computer"`
  with an `image` content block `{type:"image", data:<base64>, mimeType:<...>}` (confirmed shape;
  service-worker `textImage`). No pre-existing test pinned the computer screenshot content shape, so a
  Browser-level test was added. Cache: per-guid `VecDeque` bound N=8 on `Browser`, imageId =
  `"img_" + uuid::simple`. upload_image handler forwards to the extension's `upload_image_exec`
  (not advertised), mirroring form_fill's internal-call idiom; the parent call is governed once
  (requires Write). `computer` INPUT schema + descriptor row UNCHANGED (only the output gains the
  trailing imageId text block, ADR-0050 D4's one sanctioned trained-output change).

### T4 -- gif_creator (phased; Phase 1 + Phase 2 done)
- Status: PHASE 1 + PHASE 2 DONE. (Overlays + richer color quantization still DEFERRED.)
- Phase 2 commit: (filled at commit). Phase 2 = the `export` `coordinate` branch in
  service-worker.js now ENCODES the GIF and drag-drops it as an `image/gif` File at the coordinate
  via T3's `content(tabId, {type:"setImage", coordinate, data, filename, mimeType})` path
  (content.js `setImage`, already the DragEvent drop). EXTENSION-ONLY: no Rust/schema/pin change (the
  `coordinate` param + `export`=Write were declared in Phase 1). Verified: `node --check` on all four
  extension files + extension node --test 12/12; the Rust suite is untouched (44/44 stands). The
  drag-drop DragEvent itself is live-verified (not node-testable, same as T3's setImage coordinate
  branch). Original Phase-1 status/deviations below.
- Commit(s): (filled at commit)
- V-ALL: pass (isolated CARGO_TARGET_DIR). fmt --check + clippy --all-targets -D warnings clean;
  core lib 487; extension node --test 12/12 (incl. the new gifenc 4 + recbuffer 4); full workspace
  `cargo test -- --include-ignored --test-threads=1` = all binaries green (incl. e2e tier with
  gif_creator's 4 variants in every advertised set).
- Deviations:
  1. Part A schema: the harvest note is gone, but the official schema was RE-EXTRACTED VERBATIM from
     the installed official extension (`.../Extensions/fcoeoabgfenejglbffodgkkbkcdhcgfn/1.0.80_0/
     assets/mcpPermissions-DCTt63hZ.js`, `name:"gif_creator"`) -- the prompt's preferred fallback.
     Initially reconstructed by hand, then corrected to the verbatim official description + parameter
     text after the owner flagged that "approximate" was a shortcut when the real schema was on disk.
     Structural bits (`enum`/`required`/`additionalProperties`) follow our house JSON-Schema style
     (official uses Anthropic's `parameters` format). The official `options` is an open object there
     too. NOTE for T5: the SAME on-disk file is the reference for re-baselining the 13 trained schemas.
  2. `export` classified `[Write]` per the T4 prompt Part B (NOT ADR-0050 D5's "Read"): the variant
     system keys on `action`, not the `download` flag, and Phase-2 coordinate export IS a page write,
     so over-classifying download-export as Write is the fail-closed direction. `EXPECTED` pins it.
  3. Re-pin: the mcp_protocol / hub-outbound / tool_enforcement count asserts DERIVE (untouched). Hand-
     edited: directory.rs (REGISTRY + EXPECTED 4 rows + EXPECTED_TOOLS + total_variants 33->37 +
     with_action_key 2->3 + the "exactly one variant" exemption + doc counts 21), tool_schema_fidelity,
     all_open_golden, pipeline.rs's 4 gif_creator explain lines (prompt-omitted, oracle taken from the
     failing test's real-formatter output), AND every advertised-SET golden -- gif_creator has
     `[]`-requiring variants (stop/clear) so it is advertised under EVERY grant, like computer
     (advertise.rs x2, tool_advertisement x2, hot_reload x2, manifest_validation, all_open_golden).
  4. Test strengthened beyond the prompt's header-only oracle: `gifenc.test.js` pins a hand-computed
     exact-LZW oracle for a 2x2 frame AND round-trips a 32x32 table-growth frame through an INDEPENDENT
     GIF LZW decoder -- which caught the classic code-size-bump off-by-one during development (the
     encoder follows the Poskanzer/giflib rule; the decoder lags one entry). Header-only would have
     shipped a corrupt-but-valid-header encoder silently.
- Notes: encoder = `extension/lib/gifenc.js` (vendored ASCII, MIT; fixed 3-3-2 uniform 256-color
  palette -- a coarse but always-valid FLOOR; richer quantization + overlays DEFERRED). Recording
  buffer = `extension/lib/recbuffer.js` (pure, bounded N=100, per-tab). Both loaded via the service
  worker's `importScripts`; the `gif_creator` handler (record/stop/clear/export-download) + a
  `maybeCaptureGifFrame` hook after computer/navigate live in service-worker.js; export decodes the
  JPEG frames to RGBA via OffscreenCanvas and returns the GIF as an `image/gif` content block.
  **DEFERRED to Phase 2: `export` with `coordinate` (drag-drop the GIF via T3's setImage DragEvent);
  richer color quantization; overlays (click cues/labels/watermark/progress).**

### T5 -- 13-tool re-baseline vs 1.0.80 + retire reference/
- Status: DONE (Half A + Half B; two commits).
- Commit(s): Half A `fc2ed64`; Half B (this commit).
- V-ALL: pass. fmt --check + core lib 487 + the description-sensitive integration guards
  (tool_schema_fidelity, all_open_golden, tool_advertisement, mcp_protocol) all green. Half B is a
  DESCRIPTION-ONLY change (no e2e/spawn assertion touches trained descriptions), so it cannot affect
  the e2e tier (already 44/44 at T4). No non-test file references `reference/open-claude-in-chrome`.
- Deviations / notes:
  * Half A: the repo tracked ONLY `reference/ANALYSIS.md` (the open-claude-in-chrome clone was
    local/untracked -- likely gitignored), so `git rm` was just ANALYSIS.md. One code comment cited
    it (`crates/transport/src/host.rs`) -- a prose pointer, not a code dep; repointed at docs/research/12
    + ADR-0050 D1. SPEC.md had TWO upload_image exclusions: section-4-ish (line ~206, already annotated
    superseded) + section-10 (line ~590, annotated now). Historical Implementation-Phases / Repository-
    Structure prose left as-is (the CLAUDE.md preamble already labels it historical, and A5 fences it).
  * Half B re-harvested the 13 official schemas from the ON-DISK v1.0.80
    (`.../Extensions/fcoeoabgfenejglbffodgkkbkcdhcgfn/1.0.80_0/assets/mcpPermissions-DCTt63hZ.js`,
    `name:"<tool>",description:...,parameters:...`). APPLIED 3 DESCRIPTION-ONLY deltas, each also
    fixing an inaccuracy (our impl already does the v1.0.80 behavior; only the prose lagged):
      - `form_input`: "the read_page tool" -> "the read_page or find tools" (desc + `ref` param).
      - `get_page_text`: over-limit prose "you will receive an error suggesting alternatives" ->
        "truncated with a note giving the full size" (content.js:441 already TRUNCATES, never errored).
      - `read_page`: over-limit prose "you will receive an error asking..." -> "truncated at a line
        boundary with a note giving the full size; pass a larger max_chars, or use depth/ref_id to
        focus" + the official's richer optional-filter sentence (content.js:381 already TRUNCATES with
        a "[showing X of Y elements...]" note).
    VERIFIED NO delta needed (already matches v1.0.80) for: navigate (`force` desc verbatim),
    computer (action enum order + duration max 10), javascript_tool (`action`="Must be set to
    'javascript_exec'", no const; REPL `text`), get_page_text `max_chars`, tabs_context_mcp/
    tabs_create_mcp/find/read_console_messages/read_network_requests/resize_window/update_plan descs.
    NO NEVER-list touch: zero renames, zero removals, zero type changes -- `EXPECTED_TRAINED` untouched.
    The whole doc-12 section-A checklist is now either confirmed-applied or the over-limit-message
    items updated here.

## Deviation index (cross-task, for the next-batch review)

(Append one line per numbered deviation as they occur: `T<n>.<k>: <what and why>`.)

- T1.1: pipeline.rs `pinned_explain_text()` frozen literal not in the prompt; explain derives from the directory, so added the file_upload line (formatter uses `requires.first()` -> "write").
- T1.2: four `Some(17)` advertised-count asserts in adapter_override/adapter_reconnect not in the prompt or C1 red-team -> Some(18); only observable via the E2E tier.
- T1.3: hot_reload `expanded` (write-grant) + `full_set` (all-open) name arrays needed file_upload before explain; two stale doc counts corrected.
- T1.4: (process) local spawn-tier V-ALL is environment-sensitive (interactive stdin hang; persistent-service/Chrome exe-lock); ran serial + closed-stdin; motivated ADR-0051 (separate track).
