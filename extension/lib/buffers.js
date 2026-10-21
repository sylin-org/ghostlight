// Ghostlight -- the diagnostics and recording buffers of adapter protocol 4.
(function installGhostlightBuffers(root, factory) {
  const node = typeof module !== "undefined" && module.exports;
  const api = factory(node
    ? {
      args: require("./arguments.js"),
      capture: require("./capture.js"),
      diagnosticsApi: require("./diagnostics.js"),
      messages: require("./messages.js"),
      protocol: require("./protocol.js"),
      recordingApi: require("./recording.js"),
      sessions: require("./sessions.js"),
      shared: require("./shared.js"),
      worldApi: require("./world.js")
    }
    : {
      args: root.GhostlightArguments,
      capture: root.GhostlightCapture,
      diagnosticsApi: root.GhostlightDiagnostics,
      messages: root.GhostlightMessages,
      protocol: root.GhostlightProtocol,
      recordingApi: root.GhostlightRecording,
      sessions: root.GhostlightSessions,
      shared: root.GhostlightShared,
      worldApi: root.GhostlightWorld
    });
  root.GhostlightBuffers = api;
  if (node) module.exports = api;
})(globalThis, function createGhostlightBuffersApi(deps) {
  "use strict";

  const { args: check, capture: captureApi, diagnosticsApi, messages, protocol, recordingApi, sessions, shared, worldApi } = deps;
  const { CDP } = sessions;

  // The `diagnostics` and `recording` primitives. Both keep volatile, bounded state inside the
  // browser (the console and network ring of a tab, and screencast frames) and give it out only when
  // the service asks. Nothing here decides whether to record or what a read may show: the service
  // supplies the document set a read is bound to (`binding`), the documents a recording is allowed
  // to span (`watch`), and the guard each read and delivery is checked against.
  //
  // A diagnostics read resets the tab's ring when its binding differs from the last one, and a
  // top-level navigation begun on a bound tab starts a fresh ring. A recording is interrupted with
  // `document_boundary` the moment a watched tab commits any document, or when its frames no longer
  // show the watched set. Exporting runs the retained frames through the offscreen encoder, which
  // yields one GIF that is downloaded, returned to the service, or handed to a page world call as
  // that call's attachment.

  const DOMAIN = Object.freeze({ RUNTIME: "Runtime", NETWORK: "Network" });
  const SOURCE = Object.freeze({ BOTH: "both", CONSOLE: "console", NETWORK: "network" });
  const DIAGNOSTICS_OP = Object.freeze({ READ: "read", CLEAR: "clear" });
  const RECORDING_OP = Object.freeze({ START: "start", STATUS: "status", STOP: "stop", EXPORT: "export", DISCARD: "discard" });
  const DESTINATION = Object.freeze({ DOWNLOAD: "download", CLIENT: "client", PAGE: "page" });
  const DELIVERY = Object.freeze({ ATTACHED: "attached", DOWNLOADED: "downloaded", RETURNED: "returned" });
  const RESULT = Object.freeze({
    STARTED: "started",
    STATUS: "status",
    STOPPED: "stopped",
    EXPORTED: "exported",
    EXPORT_FAILED: "export_failed",
    DISCARDED: "discarded",
    NOT_FOUND: "not_found"
  });
  const STOP_REASON = Object.freeze({
    EXPLICIT: "explicit",
    DOCUMENT_BOUNDARY: "document_boundary",
    BROWSER_DETACHED: "browser_detached"
  });
  const RECORDING_STATE = "recording";
  const FRAME_KIND = Object.freeze({ SEED: "seed", SCREENCAST: "screencast", FINAL: "final" });
  const FAILURE = Object.freeze({ DOCUMENT_CHANGED: "document_changed" });
  const ENCODER_DOCUMENT = "offscreen.html";
  const ENCODER_REASON = "BLOBS";
  const ENCODER_JUSTIFICATION = "Decode recording frames and encode one animated GIF away from the service worker.";
  const { OFFSCREEN, TRANSFER } = messages;
  const DOWNLOAD_SETTLE_MS = 30_000;
  const PAGE_DELIVERY_MS = 30_000;
  const REASON_CHARS = 200;
  const RECORDING_ID_CHARS = 160;
  const BINDING_CHARS = 512;
  const FILE_NAME_CHARS = 255;
  const MIN_CAPTURE_SCALE = 0.05;
  const JPEG = "jpeg";

  /** The DiagnosticsFacts of a clear: how many tab rings were dropped. */
  function clearedFacts(count) {
    return { cleared_count: count };
  }

  /**
   * Creates the diagnostics and recording buffers.
   *
   * `lifecycle` is a GhostlightSessions instance, `inventory` a GhostlightInventory and `worldCalls`
   * a GhostlightWorld instance. `diagnostics` is a GhostlightDiagnostics instance, whose events the
   * worker feeds, and `recording` a GhostlightRecording instance created with `onStop` set to this
   * object's `recordingEnded`. `imageSize(base64)` measures an encoded frame. `changed()` is called
   * after a recording starts or ends, so the worker can refresh the popup and badge. `chromeApi`
   * supplies `tabs`, `runtime`, `offscreen` and `downloads`.
   */
  function create({
    chromeApi = globalThis.chrome,
    lifecycle,
    inventory,
    worldCalls,
    diagnostics,
    recording,
    imageSize = captureApi.decodeImageSize,
    now = () => Date.now(),
    changed = () => {}
  } = {}) {
    if (!lifecycle?.cdp || !inventory?.snapshot || !worldCalls?.ambientAll) {
      throw new TypeError("the buffers require debugger sessions, an inventory and the world");
    }
    if (!diagnostics?.read || !recording?.start) throw new TypeError("the buffers require the diagnostics and recording stores");
    if (!chromeApi?.tabs?.get) throw new TypeError("the buffers require chrome.tabs");
    const cdp = lifecycle.cdp;
    const rootTarget = (tabId) => ({ tabId });
    // tab id -> the documents a recording may span; tab id -> the document set a read is bound to.
    const watches = new Map();
    const bindings = new Map();

    // ---- diagnostics --------------------------------------------------------------------------

    async function disableCapture(tabIds) {
      await Promise.all(tabIds.flatMap((tabId) => [DOMAIN.RUNTIME, DOMAIN.NETWORK]
        .map((domain) => lifecycle.disableDomain(tabId, domain).catch(() => {}))));
    }

    async function readDiagnostics(op) {
      const tabId = op.tab_id;
      return lifecycle.withLease(tabId, async () => {
        if (!check.absent(op.binding) && bindings.get(tabId) !== op.binding) {
          // A different binding means the admitted documents changed without a tracked boundary.
          if (bindings.has(tabId)) {
            diagnostics.forget(tabId);
            await disableCapture([tabId]);
          }
          bindings.set(tabId, op.binding);
        }
        const captureStarted = diagnostics.enable(tabId);
        let sourceStarted = false;
        try {
          if (op.source !== SOURCE.NETWORK) sourceStarted = await lifecycle.enableDomain(tabId, DOMAIN.RUNTIME) || sourceStarted;
          if (op.source !== SOURCE.CONSOLE) sourceStarted = await lifecycle.enableDomain(tabId, DOMAIN.NETWORK) || sourceStarted;
        } catch (error) {
          if (captureStarted) {
            diagnostics.forget(tabId);
            await disableCapture([tabId]).catch(() => {});
          }
          throw error;
        }
        inventory.checkGuard(op.guard, await inventory.snapshot(tabId));
        let result;
        try {
          result = diagnostics.read(tabId, {
            source: op.source,
            detail: op.detail,
            match_text: op.match_text ?? undefined,
            after: op.after ?? undefined,
            limit: op.limit
          });
        } catch (error) {
          // The store rejects a cursor that does not belong to this tab's ring.
          if (error instanceof RangeError || error instanceof TypeError) throw check.invalid(error.message);
          throw error;
        }
        return { ...result, capture_started: result.capture_started || sourceStarted, ...clearedFacts(0) };
      });
    }

    async function clearDiagnostics(op) {
      for (const tabId of op.tab_ids) bindings.delete(tabId);
      const cleared = diagnostics.forgetMany(op.tab_ids);
      await disableCapture(op.tab_ids);
      return clearedFacts(cleared);
    }

    const diagnosticsPrimitive = Object.freeze({
      addresses: (request) => request.op === DIAGNOSTICS_OP.READ,
      tabOf: (request) => request.tab_id,
      validate(request) {
        check.requireArgs(request);
        if (!protocol.isKnown("diagnostics_ops", request.op)) throw check.invalid("op must name a diagnostics operation.");
        if (request.op === DIAGNOSTICS_OP.CLEAR) {
          const ids = request.tab_ids;
          if (!Array.isArray(ids) || ids.length < 1 || ids.length > diagnosticsApi.MAX_CLEAR_TABS) {
            throw check.invalid(`tab_ids must list from 1 to ${diagnosticsApi.MAX_CLEAR_TABS} tabs.`);
          }
          ids.forEach((id) => check.requireTabId(id, "tab_ids entry"));
          if (new Set(ids).size !== ids.length) throw check.invalid("tab_ids must not repeat a tab.");
          return;
        }
        const limits = protocol.vocabulary.limits.diagnostics_limit;
        check.requireTabId(request.tab_id);
        if (!protocol.isKnown("diagnostic_sources", request.source)) throw check.invalid("source must name a diagnostic source.");
        if (!protocol.isKnown("diagnostic_details", request.detail)) throw check.invalid("detail must name a diagnostic detail.");
        check.requireInteger(request.limit, "limit", { minimum: limits.min, maximum: limits.max });
        if (!check.absent(request.match_text)) check.requireText(request.match_text, "match_text", diagnosticsApi.MAX_MATCH_CHARS);
        if (!check.absent(request.after)) check.requireText(request.after, "after", diagnosticsApi.MAX_CURSOR_CHARS);
        if (!check.absent(request.binding)) check.requireText(request.binding, "binding", BINDING_CHARS);
        check.optionalGuard(request.guard);
      },
      /**
       * Resolves `DiagnosticsFacts`. A read opts the tab in (Runtime and Network, as the source asks),
       * checks the guard, and returns the entries after the cursor. A clear forgets the rings of the
       * named tabs and turns their capture off.
       */
      run(request) {
        return request.op === DIAGNOSTICS_OP.READ ? readDiagnostics(request) : clearDiagnostics(request);
      }
    });

    // ---- recording ----------------------------------------------------------------------------

    function sameDocuments(admitted, snap) {
      if (snap.owner.size !== admitted.length) return false;
      return admitted.every((document) => {
        const current = snap.owner.get(document.frame_id);
        return current && current.loaderId === document.loader_id && current.fact.url === document.url;
      });
    }

    // Every document the tab holds goes into the recording's source evidence, watched or not, so a
    // later export can recheck what the frames showed. A snapshot that is incomplete, has unseen
    // documents or cannot be taken leaves that evidence marked incomplete.
    function noteDocuments(tabId, snap) {
      if (!snap) { recording.noteUrl(tabId, null); return; }
      for (const entry of snap.owner.values()) recording.noteUrl(tabId, entry.fact.url);
      if (!snap.complete || snap.unseen.length > 0) recording.noteUrl(tabId, null);
    }

    async function snapshotOrNull(tabId) {
      try {
        return await inventory.snapshot(tabId);
      } catch (_error) {
        return null;
      }
    }

    // True while the tab's recording may keep taking frames. A recording with no watch is never
    // scoped, but it still collects the url of every document it sees. With a watch, the tab's
    // documents must still be exactly the watched set, completely seen; anything else, including
    // not being able to tell, ends the recording at the last verified frame.
    async function scopeCurrent(tabId) {
      if (!recording.activeForTab(tabId)) return false;
      const admitted = watches.get(tabId);
      const snap = await snapshotOrNull(tabId);
      if (!admitted) {
        noteDocuments(tabId, snap);
        return true;
      }
      if (snap && snap.complete && snap.unseen.length === 0 && sameDocuments(admitted, snap)) {
        noteDocuments(tabId, snap);
        return true;
      }
      recording.interruptTab(tabId, STOP_REASON.DOCUMENT_BOUNDARY);
      return false;
    }

    async function frameSize(data, clip) {
      try {
        return await imageSize(data);
      } catch (_error) {
        return { width: Math.round(clip.width * clip.scale), height: Math.round(clip.height * clip.scale) };
      }
    }

    // One still frame of the visible viewport, taken inside the caller's lease with Ghostlight's own
    // presentation hidden. Resolves true when the frame was kept.
    async function captureFrame(tabId, frameKind) {
      // A dialog blocks the renderer, so no frame can be taken until it is handled.
      if (lifecycle.currentDialog(tabId) || !(await scopeCurrent(tabId))) return false;
      return worldCalls.withHidden(tabId, async () => {
        const metrics = await cdp(rootTarget(tabId), CDP.PAGE_GET_LAYOUT_METRICS);
        const visual = metrics.cssVisualViewport || metrics.visualViewport;
        const clip = {
          x: visual.pageX ?? 0,
          y: visual.pageY ?? 0,
          width: Math.max(1, visual.clientWidth),
          height: Math.max(1, visual.clientHeight),
          scale: Math.max(MIN_CAPTURE_SCALE, Math.min(1, recordingApi.MAX_WIDTH / visual.clientWidth, recordingApi.MAX_HEIGHT / visual.clientHeight))
        };
        const shot = await cdp(rootTarget(tabId), CDP.PAGE_CAPTURE_SCREENSHOT,
          { format: JPEG, quality: recordingApi.JPEG_QUALITY, clip, captureBeyondViewport: true, fromSurface: true });
        const size = await frameSize(shot.data, clip);
        if (size.width > recordingApi.MAX_WIDTH || size.height > recordingApi.MAX_HEIGHT) {
          throw new Error("recording frame exceeded its negotiated dimensions");
        }
        return await scopeCurrent(tabId) && recording.append(tabId, shot.data, frameKind, now());
      });
    }

    function requireTab(tabId) {
      return check.requireTab(chromeApi.tabs, tabId);
    }

    async function startRecording(op) {
      const tabId = op.tab_id;
      const tab = await requireTab(tabId);
      return lifecycle.withLease(tabId, async () => {
        const started = recording.start(tabId, tab.url);
        if (started.existing) return { result: RESULT.STARTED, summary: started.existing, existing: true };
        const state = recording.activeForTab(tabId);
        if (op.watch) watches.set(tabId, op.watch);
        try {
          await captureFrame(tabId, FRAME_KIND.SEED).catch(() => false);
          if (!recording.activeForTab(tabId)) throw new Error("recording ended during startup");
          await cdp(rootTarget(tabId), CDP.PAGE_START_SCREENCAST, {
            format: JPEG,
            quality: recordingApi.JPEG_QUALITY,
            maxWidth: recordingApi.MAX_WIDTH,
            maxHeight: recordingApi.MAX_HEIGHT,
            everyNthFrame: 1
          });
          await worldCalls.ambientAll(tabId);
          changed();
          return { result: RESULT.STARTED, summary: recording.status(state.id).summary, existing: false };
        } catch (error) {
          // Discarding an active recording ends it, which stops the screencast and forgets the watch.
          recording.discard(state.id);
          throw error;
        }
      });
    }

    function recordingStatus(op) {
      const selected = recording.status(op.recording_id);
      return selected.summary ? { result: RESULT.STATUS, summary: selected.summary } : { result: RESULT.NOT_FOUND };
    }

    async function stopRecording(op) {
      const selected = recording.beginStop(op.recording_id);
      if (!selected.state) return { result: RESULT.NOT_FOUND };
      const state = selected.state;
      if (state.state !== RECORDING_STATE) return { result: RESULT.STOPPED, summary: selected.summary, changed: false };
      try {
        await lifecycle.withLease(state.tabId, async () => {
          await captureFrame(state.tabId, FRAME_KIND.FINAL).catch(() => false);
          await cdp(rootTarget(state.tabId), CDP.PAGE_STOP_SCREENCAST);
        });
        const summary = recording.finishStop(state, STOP_REASON.EXPLICIT);
        return { result: RESULT.STOPPED, summary, changed: true };
      } catch (error) {
        recording.interruptTab(state.tabId, STOP_REASON.BROWSER_DETACHED);
        throw error;
      }
    }

    function discardRecording(op) {
      const discarded = recording.discard(op.recording_id);
      if (discarded.notFound) return { result: RESULT.NOT_FOUND };
      return { result: RESULT.DISCARDED, recording_id: discarded.recordingId, released_bytes: discarded.releasedBytes };
    }

    // The encoder lives in an offscreen document because Chrome may evict this worker mid-encode,
    // and because object URLs, the way a GIF reaches the download mechanism without anyone reading
    // its bytes, do not exist in a service worker at all.
    let encoderSetup = null;

    async function ensureEncoder() {
      if (await chromeApi.offscreen.hasDocument()) return;
      encoderSetup ??= chromeApi.offscreen.createDocument({
        url: ENCODER_DOCUMENT,
        reasons: [ENCODER_REASON],
        justification: ENCODER_JUSTIFICATION
      }).finally(() => { encoderSetup = null; });
      await encoderSetup;
    }

    async function askEncoder(message) {
      await ensureEncoder();
      const response = await chromeApi.runtime.sendMessage({ target: OFFSCREEN.TARGET, ...message });
      if (!response?.ok) throw new Error(response?.reason || "recording encoding failed");
      return response;
    }

    // An open offscreen document keeps this worker awake, and a worker that never sleeps never
    // forgets recording bytes. Closing it is part of the volatility promise, not tidiness.
    async function closeEncoder() {
      try {
        if (await chromeApi.offscreen.hasDocument()) await chromeApi.offscreen.closeDocument();
      } catch (_error) { /* the document is gone either way */ }
    }

    function downloadSettled(downloadId) {
      return new Promise((resolve, reject) => {
        const settle = (error) => {
          chromeApi.downloads.onChanged.removeListener(watch);
          clearTimeout(timer);
          if (error) reject(error); else resolve();
        };
        const observe = (state) => {
          if (state === "complete") settle();
          if (state === "interrupted") settle(new Error("the browser could not write the replay"));
        };
        const watch = (delta) => {
          if (delta.id === downloadId && delta.state) observe(delta.state.current);
        };
        const timer = setTimeout(() => {
          // The caller revokes the source blob URL the instant this settles. Stopping the download
          // first means the write has really stopped by the time the URL disappears.
          chromeApi.downloads.cancel(downloadId).catch(() => {});
          settle(new Error("the browser did not finish writing the replay"));
        }, DOWNLOAD_SETTLE_MS);
        chromeApi.downloads.onChanged.addListener(watch);
        // The download can finish before the listener attaches, and then no change ever arrives.
        chromeApi.downloads.search({ id: downloadId }).then(([item]) => observe(item?.state)).catch(() => {});
      });
    }

    async function deliverToPage(destination, encoded, fx, correlation) {
      const tabId = destination.tab_id;
      return lifecycle.withLease(tabId, async () => {
        const snap = await inventory.snapshot(tabId);
        inventory.checkGuard(destination.guard, snap);
        fx.watch(tabId);
        const answer = await worldCalls.callOne(tabId, destination.call, {
          correlation,
          token: `${correlation}:replay`,
          deadline: now() + PAGE_DELIVERY_MS,
          snap,
          attachment: { media_type: encoded.mime_type, data: encoded.data, size: encoded.measurements.byte_count }
        });
        fx.dispatched = worldApi.started(answer);
        if (!worldApi.okAnswer(answer)) throw new Error("the page did not accept the replay");
        return { delivery: DELIVERY.ATTACHED, tab_id: tabId };
      });
    }

    async function deliver(destination, encoded, fx, correlation) {
      if (destination.destination === DESTINATION.PAGE) return deliverToPage(destination, encoded, fx, correlation);
      if (destination.destination === DESTINATION.DOWNLOAD) {
        const downloadId = await chromeApi.downloads.download({ url: encoded.object_url, filename: destination.file_name, saveAs: false });
        await downloadSettled(downloadId);
        return { delivery: DELIVERY.DOWNLOADED };
      }
      return { delivery: DELIVERY.RETURNED, mime_type: encoded.mime_type, data: encoded.data };
    }

    // One command, one artifact. The service says where the result goes; the browser records,
    // encodes, and delivers. The frames never leave. A guard that fails right before a page delivery
    // is a failure of the primitive, because the service must learn that the page changed; any other
    // trouble encoding or delivering is an `export_failed` result with its reason.
    async function exportRecording(op, fx, correlation) {
      const selected = recording.retained(op.recording_id);
      if (selected.notFound) return { result: RESULT.NOT_FOUND };
      const destination = op.destination;
      let encoded = null;
      try {
        encoded = await askEncoder({
          kind: OFFSCREEN.ENCODE,
          frames: selected.frames,
          max_bytes: op.max_output_bytes,
          // Bytes take exactly one form, chosen by where they are going: an object URL the browser
          // downloads without anyone reading it, or base64 for a destination that must.
          transfer: destination.destination === DESTINATION.DOWNLOAD ? TRANSFER.OBJECT_URL : TRANSFER.BASE64
        });
        const delivery = await deliver(destination, encoded, fx, correlation);
        return { result: RESULT.EXPORTED, summary: selected.summary, encoded: encoded.measurements, delivery };
      } catch (error) {
        if (error?.code === FAILURE.DOCUMENT_CHANGED) throw error;
        return { result: RESULT.EXPORT_FAILED, reason: shared.bounded(error?.message ?? error, REASON_CHARS) };
      } finally {
        if (encoded?.object_url) {
          await chromeApi.runtime.sendMessage({ target: OFFSCREEN.TARGET, kind: OFFSCREEN.RELEASE, object_url: encoded.object_url }).catch(() => {});
        }
        await closeEncoder();
      }
    }

    const recordingPrimitive = Object.freeze({
      addresses: (request) => request.op === RECORDING_OP.START
        || (request.op === RECORDING_OP.EXPORT && request.destination?.destination === DESTINATION.PAGE),
      tabOf: (request) => (request.op === RECORDING_OP.EXPORT ? request.destination.tab_id : request.tab_id),
      validate(request) {
        check.requireArgs(request);
        if (!protocol.isKnown("recording_ops", request.op)) throw check.invalid("op must name a recording operation.");
        if (request.op === RECORDING_OP.START) {
          check.requireTabId(request.tab_id);
          if (check.absent(request.watch)) return;
          if (!Array.isArray(request.watch) || request.watch.length > protocol.MAX_WORLD_CALLS) {
            throw check.invalid(`watch must list at most ${protocol.MAX_WORLD_CALLS} documents.`);
          }
          request.watch.forEach((document, index) => check.requireDocument(document, `watch[${index}]`));
          return;
        }
        check.requireText(request.recording_id, "recording_id", RECORDING_ID_CHARS);
        if (request.op !== RECORDING_OP.EXPORT) return;
        check.requireInteger(request.max_output_bytes, "max_output_bytes", { minimum: 1 });
        const destination = request.destination;
        if (!check.isObject(destination) || !protocol.isKnown("recording_destinations", destination.destination)) {
          throw check.invalid("destination must name a recording destination.");
        }
        if (destination.destination === DESTINATION.DOWNLOAD) check.requireText(destination.file_name, "destination.file_name", FILE_NAME_CHARS);
        if (destination.destination === DESTINATION.PAGE) {
          check.requireTabId(destination.tab_id, "destination.tab_id");
          check.requireWorldCall(destination.call, "destination.call");
          check.optionalGuard(destination.guard, "destination.guard");
        }
      },
      /**
       * Resolves `RecordingFacts`. `start` takes a seed frame and starts the screencast, reporting
       * the recording already running on the tab when there is one. `status`, `stop` and `discard`
       * act on the named recording and answer `not_found` for one that is gone. `export` encodes the
       * retained frames and delivers the GIF to its destination.
       */
      run(request, fx, correlation) {
        switch (request.op) {
          case RECORDING_OP.START: return startRecording(request);
          case RECORDING_OP.STATUS: return recordingStatus(request);
          case RECORDING_OP.STOP: return stopRecording(request);
          case RECORDING_OP.EXPORT: return exportRecording(request, fx, correlation);
          default: return discardRecording(request);
        }
      }
    });

    // ---- events the worker feeds --------------------------------------------------------------

    /**
     * Takes one `Page.screencastFrame` of a tab: acknowledges it so the compositor keeps sending,
     * and keeps it when the recording's watched documents are still the tab's documents.
     */
    async function screencastFrame(tabId, params) {
      try {
        await cdp(rootTarget(tabId), CDP.PAGE_SCREENCAST_FRAME_ACK, { sessionId: params.sessionId });
      } catch (_error) { /* a detached target has no compositor flow left to unblock */ }
      if (await scopeCurrent(tabId)) recording.append(tabId, params.data, FRAME_KIND.SCREENCAST, now());
    }

    /**
     * Takes a `webNavigation.onCommitted` event. Any frame committing on a tab whose recording is
     * watched ends the recording at a document boundary. A top-frame commit adds its url to the
     * recording's source urls.
     */
    function committed({ tabId, frameId, url }) {
      if (watches.has(tabId)) recording.interruptTab(tabId, STOP_REASON.DOCUMENT_BOUNDARY);
      if (frameId === 0) recording.noteUrl(tabId, url);
    }

    /**
     * Takes a top-frame `webNavigation.onBeforeNavigate` event. A tab with diagnostics capture,
     * bound or not, starts a fresh ring for the new document and keeps its Runtime and Network
     * capture on, so the navigation itself stays observable. Its binding, if any, ends with the
     * document it named.
     */
    function beforeNavigate(tabId) {
      bindings.delete(tabId);
      if (diagnostics.forget(tabId)) diagnostics.enable(tabId);
    }

    /**
     * Takes the end of a recording (the `onStop` hook of the recording store). Forgets its watch,
     * stops the screencast unless the stop did that itself, and refreshes the page flag.
     */
    function recordingEnded(tabId, _recordingId, reason) {
      watches.delete(tabId);
      if (reason !== STOP_REASON.EXPLICIT) cdp(rootTarget(tabId), CDP.PAGE_STOP_SCREENCAST).catch(() => {});
      worldCalls.ambientAll(tabId).catch(() => {});
      changed();
    }

    /** Takes a debugger detach: the tab's recording is interrupted and its diagnostics forgotten. */
    function tabDetached(tabId) {
      recording.interruptTab(tabId, STOP_REASON.BROWSER_DETACHED);
      diagnostics.forget(tabId);
      bindings.delete(tabId);
    }

    /**
     * Settles the state that must not outlive a boundary, such as the service going or the person
     * holding runtime: every recording is interrupted with `reason`, and every diagnostics ring is
     * dropped with its capture turned off and its read binding forgotten.
     */
    async function settle(reason) {
      recording.interruptAll(reason);
      await disableCapture(diagnostics.clearAll());
      bindings.clear();
    }

    /** Turns off the capture of one tab whose diagnostics ring expired. */
    async function captureExpired(tabId) {
      await disableCapture([tabId]);
    }

    return Object.freeze({
      diagnostics: diagnosticsPrimitive,
      recording: recordingPrimitive,
      screencastFrame,
      committed,
      beforeNavigate,
      recordingEnded,
      tabDetached,
      settle,
      captureExpired
    });
  }

  return Object.freeze({ create });
});
