// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- background service worker.
//
// Policy-free CDP executor + native-messaging endpoint + tab-group manager. It holds MECHANISM
// only; all governance (domains, tool classification, audit) lives in the Rust binary. It receives
// { id, type: "tool_request", tool, args } and replies { id, type: "tool_response", result } or
// { id, type: "tool_error", error, hop?, detail? }. `hop` (only ever "cdp" or "page") and `detail`
// are optional and are mechanism tags (which layer threw), never policy; an absent `hop` means the
// binary attributes the failure to the extension itself. Chrome frames native messages (4-byte LE)
// for us via the Port.
//
// Tab-URL query (g13): { id, type: "tab_url_request", tabId } gets
// { id, type: "tab_url_response", result: { url } }, reporting chrome.tabs.get(tabId).url (or
// null) with no matching or interpretation -- the binary's grant enforcement decides.
//
// Tab-group-per-session request (H7, ADR-0030 Decision 6/7): { type: "group_request", guid,
// tabIds, title, workspace? } gets { type: "group_response", guid, ok } (both id-less;
// fire-and-forget). The
// grouping DECISION (which tabIds get grouped and how) lives in the pure lib/grouping.js module
// this worker calls on receipt, ADDITIVE to (never replacing) the existing single-group
// ensureGroup/groupTabs/inGroup access-control mechanism below, which this path never touches.

importScripts("lib/constants.js", "lib/geometry.js", "lib/keys.js", "lib/input-events.js", "lib/drag-session.js", "lib/workspace.js", "lib/grouping.js", "lib/debug.js", "lib/identity.js", "lib/presentation-broker.js", "lib/action-signature.js", "lib/find-visual.js", "lib/dialog.js", "lib/tab-control.js", "lib/wire-chunks.js", "lib/surface-executor.js", "lib/execution-response.js");

// gif_creator capture relay (ADR-0053 D2): the BINARY owns recording state, frames, and the GIF
// pipeline; this worker only drives the Chrome APIs -- start/stop the tab's screencast, ack every
// compositor frame, thin to the service-chosen interval, and forward kept frames as unsolicited
// gif_frame events. Transient identity prevents delayed frames crossing recording generations.
const gifCast = new Map();

// ADR-0074: bounded, memory-only reassembly for large host-to-extension requests. The ordinary
// request is dispatched only after every ordered chunk and its SHA-256 digest verify.
const wireChunkStore = self.GhostlightWireChunks.createWireChunkStore({
  decodeBase64: bytesFromBase64,
  decodeUtf8: (bytes) => new TextDecoder("utf-8", { fatal: true }).decode(bytes),
  sha256Hex: async (bytes) => {
    const digest = await crypto.subtle.digest("SHA-256", bytes);
    return Array.from(new Uint8Array(digest), (byte) =>
      byte.toString(16).padStart(2, "0")
    ).join("");
  },
});

// ADR-0080: one worker generation owns the bounded resource executor. The service supplies the
// execution class and resource identity; this mechanism only preserves FIFO and isolation.
const EXECUTOR_GENERATION = typeof crypto.randomUUID === "function"
  ? crypto.randomUUID()
  : `${Date.now()}-${Math.random().toString(16).slice(2)}`;
let executorBrowserSlot = null;
const toolResponder = self.GhostlightExecutionResponse.createToolResponder(EXECUTOR_GENERATION);

function executorPost(item, message) {
  try { item.response.port.postMessage(message); } catch { /* connection generation is gone */ }
}

function wireCommandId(item) {
  return item.response.commandId;
}

function postExecutorTerminal(item) {
  executorPost(item, {
    type: "tool_terminal",
    id: item.response.requestId,
    commandId: wireCommandId(item),
    executorGeneration: EXECUTOR_GENERATION,
    resource: item.resource,
  });
}

const surfaceExecutor = self.GhostlightSurfaceExecutor.createSurfaceExecutor({
  execute: async (item) => {
    await dispatch(item);
  },
  onAccepted: (item, duplicate) => executorPost(item, {
    type: "tool_accepted",
    id: item.response.requestId,
    commandId: wireCommandId(item),
    executorGeneration: EXECUTOR_GENERATION,
    duplicate: duplicate === true,
  }),
  onRejected: (item, reason) => {
    if (!item) return;
    fail(item.response, hopError("extension", `Command not executed: ${reason}`));
    postExecutorTerminal(item);
  },
  onTerminal: postExecutorTerminal,
});

function requestBytes(msg) {
  try { return new TextEncoder().encode(JSON.stringify(msg)).byteLength; } catch { return 0; }
}

function executionItem(msg, port, connectionGeneration) {
  const execution = msg.execution || {};
  const resource = execution.resource || null;
  const wireId = execution.commandId === undefined ? msg.id : String(execution.commandId);
  let key = "legacy:global";
  if (resource && resource.kind === "surface") {
    executorBrowserSlot = resource.browserSlot;
    key = `surface:${resource.browserSlot}:${resource.nativeTab}`;
  } else if (resource && resource.kind === "client_topology") {
    key = `topology:${resource.browserSlot}:${resource.clientKey}`;
  } else if (resource && resource.kind === "browser") {
    key = `browser:${resource.browserSlot}`;
  } else if (Number.isSafeInteger(msg.args && msg.args.tabId)) {
    key = `legacy-surface:${msg.args.tabId}`;
  }
  const bypass = execution.class === "presentation" || execution.class === "local" ||
    execution.class === "safety_protocol";
  const response = self.GhostlightExecutionResponse.createResponseScope(msg.id, port, wireId);
  return {
    // A retained semantic intent sends several distinct extension requests under one service
    // scheduler command. Deduplicate exact request deliveries, not the whole retained lease.
    commandId: self.GhostlightSurfaceExecutor.executionIdentity(
      connectionGeneration,
      wireId,
      msg.id
    ),
    request: msg,
    resource,
    key,
    bypass,
    bytes: requestBytes(msg),
    response,
  };
}

// Operational tunables (lib/constants.js), destructured once for use throughout this worker.
const {
  MAX_SCREENSHOT_B64, JPEG_QUALITY, JPEG_QUALITY_FALLBACK, JPEG_QUALITY_FULL,
  KEEPALIVE_PERIOD_MINUTES, RECONNECT_DELAY_MS, HOLD_REQUEST_TIMEOUT_MS,
  CAPTURE_SETTLE_MS, CLICK_GAP_MS, DRAG_INTERCEPT_GRACE_MS, DRAG_INTERCEPT_WAIT_MS,
  NAV_SETTLE_TIMEOUT_MS, MAX_SIDE,
} = self.GhostlightConstants;
// The H7 grouping DECISION (lib/grouping.js): pure, unit-tested in isolation
// (tests/extension/grouping.test.js), given an injected chrome so it never touches policy.
const { groupSessionTabs, managedGroupIds, isManagedGroupId, pruneDeadGroups, reclaimGroupsByTitle } =
  self.GhostlightGrouping;
const {
  workspaceGroupKey,
  resolveWorkspaceWindow,
  resolveWorkspaceGroup,
  rememberFocusedWindow,
  forgetWorkspaceWindow,
  reconcileWorkspaceGroups,
  tabsInWindow,
} = self.GhostlightWorkspace;
// ADR-0081: one policy-free broker owns current-document readiness, presentation replacement,
// expiry, replay, acknowledgements, and bounded transient events. Chrome APIs remain adapter
// callbacks; the broker contains no policy or page-content interpretation.
const presentationBroker = self.GhostlightPresentationBroker.createPresentationBroker({
  deliver: deliverPresentation,
  activate: activatePresentation,
  onStateChange: persistPresentationSnapshot,
});
// ADR-0078 D7: the minimum current CDP dialog state per tab. It is mechanism-only, memory-only,
// and never resolves anything without an explicit model-facing dialog action.
const dialogStore = self.GhostlightDialog.createDialogStore();

// Native-messaging host name (ADR-0065: one stack). Every build of this extension -- the Web
// Store release and the unpacked dev copy alike -- talks to the ONE host `org.sylin.ghostlight`,
// whose manifest allows both extension ids. Whatever engine currently holds the endpoint behind
// that host (the installed release, or a fresh build a developer just started) serves everyone;
// the extension never picks an engine.
const NATIVE_HOST = "org.sylin.ghostlight";
const BROWSER_GENERATION_KEY = "ghostlight_browser_generation";
const PRESENTATION_STATE_KEY = "ghostlight_presentation_state";
const VISUAL_SCRIPT_FILES = ["lib/presentation-placement.js", "lib/action-signature.js", "lib/find-visual.js", "lib/keys.js", "agent-visual-indicator.js"];
const PRESENTATION_EVENT_TTL_MS = 2500;
const SIGNATURE_EVENT_TTL_MS = 1500;
const SIGNATURE_DELIVERY_WAIT_MS = 250;
const FIND_EVENT_TTL_MS = 3500;
const FIND_DELIVERY_WAIT_MS = 250;
const NARRATION_DEFAULT_DURATION_MS = 5000;
// The MCP tab group label shown in Chrome: a ghost emoji (U+1F47B) followed by the brand
// name. The emoji is written as an escape so this source file stays ASCII; it renders as
// the glyph at runtime.
const GROUP_TITLE = "\u{1F47B}Ghostlight";

let nativePort = null;
let nativeConnectionSeq = 0;
let groupId = null;
// Extension-owned browser identity (ADR-0061): a UUID minted once and persisted in
// chrome.storage.local, announced to the service as the opening frame of every native-messaging
// connection. This -- not the relay's guessed parent pid -- is what the service keys a browser's
// session (and its composite tab ids) by, so identity survives relay reconnects and worker deaths
// and never collides on a degraded pid=0.
const browserIdentity = self.GhostlightIdentity.createBrowserIdentity(chrome.storage.local);
// ADR-0085 (amending ADR-0066 D1): the presentation map is keyed on the user-placed workspace,
// `browser window + clientKey -> Chrome tab-group id`. Sessions of the same client reuse one
// group IN A GIVEN WINDOW, while deliberate placement in another window gets another group instead
// of moving old tabs or spawning a new browser window. Legacy stored client-only keys are upgraded
// from the live group's window during rehydrate. The single-group access-control gate
// (groupTabs/inGroup/effectiveTabId) still CONSULTS this map through the managed-surface predicate
// (lib/grouping.js managedGroupIds/isManagedGroupId): a tab is in-surface when it sits in the
// global `groupId` group OR any group recorded here OR (ADR-0066 D5) in `managedTabs` below.
const clientGroups = new Map();
// ADR-0066 D5: the set of tabIds this extension has placed in a managed group. A tab the user
// drags OUT of the group (detached, or moved to another window -- both ungroup it in Chrome) stays
// drivable because it is still one of OUR tabs, while a tab we never managed (the user's own,
// named by a guessed id) is still refused -- the extension gate, not just the service, is what
// keeps the agent out of the user's personal tabs (the service first-touch-adopts any unowned id).
// Session-scoped only (persisted in chrome.storage.session; Chrome renumbers tab ids across a
// browser restart, so a stale id must never survive one), and re-seeded on rehydrate from the live
// members of every managed group. Pruned on tabs.onRemoved.
const managedTabs = new Set();
// ADR-0081 control-scope amendment: membership in `managedTabs` is also the exact lifecycle of
// the persistent viewport border. The broker state replays into every eligible document and
// survives a worker restart; it is not an action effect and has no deadline. Centralizing every
// add here prevents reachability and its user-visible disclosure from drifting apart.
function markTabManaged(tabId) {
  if (!Number.isSafeInteger(tabId)) return false;
  managedTabs.add(tabId);
  const hasControlState = presentationBroker.states().some(
    (state) => state.tabId === tabId && state.channel === "control"
  );
  if (!hasControlState) {
    presentationBroker.publishState(tabId, "control", {
      type: "SHOW_AGENT_INDICATORS",
    }, {
      clearMessage: { type: "HIDE_AGENT_INDICATORS" },
      waitForDelivery: false,
    });
  }
  return true;
}
// ADR-0066 D2/D4: the group title the service writes is the ghost glyph + a space + the clientKey
// (session_title). This prefix is what rehydrate strips to reclaim a group by title after a
// browser restart. Distinct from GROUP_TITLE (glyph + name, NO space) so the two never collide.
const CLIENT_TITLE_PREFIX = "\u{1F47B} ";
// Take-the-wheel hold (g10): pending id -> resolver, for get_hold/set_hold/toggle_hold replies.
// A separate sequence and map from tool_request ids; hold ids never collide with tool ids
// because tool ids are binary-chosen and hold ids are extension-chosen.
const holdPending = new Map(); // id -> { resolve }
let holdSeq = 0;
const attentionPending = new Map();
let attentionSeq = 0;
let holdBadgeState = null;
// Panic kill switch (g11): the hot-path mirror of the chrome.storage.session "session_killed"
// marker (the source of truth for reconnect gating). Set synchronously, before any await, at
// the start of killSession() and by startup recovery; kept in sync on every transition so
// nothing here ever drifts from storage across a service-worker restart.
let sessionKilled = false;
const attached = new Map(); // tabId -> { domains: Set<string> }
const consoleBuffer = new Map(); // tabId -> { host, items: [{ level, text }] }
const networkBuffer = new Map(); // tabId -> { host, items: [{ requestId, method, url, status, mimeType, errorText, canceled }] }
const screenshotCtx = new Map(); // tabId -> { vpW, vpH, shotW, shotH, offX, offY, regionW, regionH } (set on each screenshot/zoom)
const tabHost = new Map(); // tabId -> hostname of the tab's current URL ("" when none)
const tabUrl = new Map(); // tabId -> the tab's current full URL ("" when none); fallback location
const dragCoordinator = self.GhostlightDragSession.createDragCoordinator(DRAG_INTERCEPT_WAIT_MS);
const activeDragOperations = new Map(); // tabId -> transient pointer/native cleanup state
// context for exceptionText() when a CDP exceptionDetails/callFrame carries no url of its own
// (routine for exceptions thrown from a deferred callback rather than a freshly-parsed script).
// Set true by rehydrate() when a prior session was recovered; consumed (and cleared) by the next
// successful read of the corresponding buffer, so the model is told once that tracking restarted.
let consoleResetNotice = false;
let networkResetNotice = false;

// A rejected promise must not tear down the service worker.
self.addEventListener("unhandledrejection", (e) => e.preventDefault());

// --- Native messaging + Manifest V3 keepalive ---
chrome.alarms.create("keepalive", { periodInMinutes: KEEPALIVE_PERIOD_MINUTES });
chrome.alarms.onAlarm.addListener((a) => {
  if (a.name === "keepalive" && !nativePort) connect();
});

// Async so it can consult the kill-switch marker before ever opening a port (g11). All three
// callers -- the top-level startup path, the keepalive alarm, and the onDisconnect retry timer
// -- call this unchanged; none of them need to await it.
async function connect() {
  if (nativePort) return;
  const s = await chrome.storage.session.get("session_killed");
  if (s.session_killed) return; // killed: only an explicit user reconnect resumes
  if (nativePort) return; // re-check: another caller may have won an await above
  // ADR-0061: resolve the persistent browser id BEFORE opening the port, so it can be sent as the
  // very first frame (below) with no await interleaving another message ahead of it.
  const browserId = await browserIdentity.get();
  const browserGeneration = await browserProcessGeneration();
  if (nativePort) return; // re-check after the await above
  try {
    nativePort = chrome.runtime.connectNative(NATIVE_HOST);
    const connectedPort = nativePort;
    const connectedResponder = self.GhostlightExecutionResponse
      .createConnectionResponder(connectedPort);
    const connectionGeneration = `${EXECUTOR_GENERATION}:${++nativeConnectionSeq}`;
    // ADR-0061: announce identity FIRST, before any other frame. The relay forwards it verbatim, so
    // the service reads it as the extension's opening handshake frame (right after the relay's own
    // ROLE_BROWSER hello) and keys this browser's session by it. Fire-and-forget, mechanism only.
    try {
      nativePort.postMessage({
        type: "browser_hello",
        browserId,
        browserGeneration,
        features: ["chunkedHostMessagesV1", "surfaceExecutorV1"],
        executorGeneration: EXECUTOR_GENERATION,
      });
    } catch { /* port gone */ }
    sendDebugEvent("connect_attempt");
    flushPendingDebugEvents(); // deliver any notes buffered while no port was open (ADR-0059)
    const handleNativeMessage = (msg) => {
      if (msg && msg.type === "wire_chunk") {
        wireChunkStore.accept(msg, handleNativeMessage, (requestId, reason) => {
          fail(
            self.GhostlightExecutionResponse.createResponseScope(requestId, connectedPort),
            hopError("extension", `Large request rejected: ${reason}`)
          );
        });
        return;
      }
      if (msg && msg.type === "tool_request" && msg.id) {
        if (sessionKilled) {
          fail(
            self.GhostlightExecutionResponse.createResponseScope(msg.id, connectedPort),
            hopError("extension", "The user ended the browser session (kill switch)")
          );
          return;
        }
        surfaceExecutor.submit(executionItem(msg, connectedPort, connectionGeneration));
        return;
      }
      // Tab-URL query (g13): mechanism only. Reports chrome.tabs.get(tabId).url verbatim (or
      // null for an unknown/closed tab); the binary decides what it means. No matching, no
      // classification, no denial text here.
      //
      // Group-gated (SEC-MED-05): the URL is reported ONLY for a tab Ghostlight manages -- the
      // same `inGroup` membership mechanism the tool-dispatch path enforces via effectiveTabId().
      // A guessed tabId of one of the user's PERSONAL tabs returns null, indistinguishable from an
      // unknown/closed tab, so the binary's governance domain-resolution probe cannot be turned
      // into an enumeration of out-of-group browsing context. Still mechanism-only: membership is
      // a fact about our own group, not a policy decision.
      if (msg && msg.type === "tab_url_request" && msg.id) {
        inGroup(msg.tabId).then(
          async (managed) => {
            let url = null;
            if (managed) {
              try { const tab = await chrome.tabs.get(msg.tabId); url = tab.url || null; } catch { url = null; }
            }
            connectedResponder.post({
              id: msg.id,
              type: "tab_url_response",
              result: { url },
            });
          },
          () => {
            connectedResponder.post({
              id: msg.id,
              type: "tab_url_response",
              result: { url: null },
            });
          }
        );
        return;
      }
      // Tab-group-per-session request (H7, ADR-0030 Decision 6/7): mechanism only, out of band
      // from tool dispatch. Groups exactly the named tabIds (the pure lib/grouping.js decision,
      // given the injected `chrome`) and persists the updated per-session map; fire-and-forget --
      // neither this request nor its reply carries an `id`, so nothing here awaits a correlated
      // response.
      // ADR-0085: a pinned request keys presentation on client + window. Tabs the user moved to a
      // different window stay reachable through `managedTabs` but are not dragged back by this
      // presentation request. A legacy caller without workspace metadata keeps client/guid keying.
      const clientKey = (msg && (msg.clientKey || msg.guid)) || null;
      const workspaceWindowId = msg && msg.workspace && msg.workspace.windowId;
      const groupKey = Number.isSafeInteger(workspaceWindowId)
        ? workspaceGroupKey(clientKey, workspaceWindowId)
        : clientKey;
      if (msg && msg.type === "group_request" && typeof groupKey === "string" && groupKey) {
        const namedTabIds = Array.isArray(msg.tabIds) ? msg.tabIds : [];
        for (const t of namedTabIds) markTabManaged(t);
        Promise.resolve()
          .then(async () => {
            if (Number.isSafeInteger(workspaceWindowId)) {
              // Re-key a group the user moved before attempting reuse. Otherwise Chrome could
              // reject a cross-window group operation or undo the user's placement.
              await workspaceGroupId(clientKey, workspaceWindowId);
              return tabsInWindow(chrome, namedTabIds, workspaceWindowId);
            }
            return namedTabIds;
          })
          .then((tabIds) => groupSessionTabs(
            chrome,
            clientGroups,
            groupKey,
            tabIds,
            msg.title || GROUP_TITLE
          ))
          .then(() => persistSessionState())
          .then(() => {
            connectedResponder.post({ type: "group_response", guid: msg.guid, ok: true });
          })
          .catch(() => {
            connectedResponder.post({ type: "group_response", guid: msg.guid, ok: false });
          });
        return;
      }
      // On-screen notification (SAPS PRES-HIGH-01): mechanism only, out of band from tool
      // dispatch, the same fire-and-forget posture as group_request above. The binary has
      // already decided everything (class/icon/title/description); this only relays it to the
      // named tab's content script for rendering -- no policy decision, no interpretation here.
      if (msg && msg.type === "notification" && typeof msg.tabId === "number") {
        presentationBroker.publishState(
          msg.tabId,
          "notification",
          {
            type: "AGENT_NOTIFICATION",
            class: msg.class,
            icon: msg.icon,
            title: msg.title,
            description: msg.description,
            durationMs: msg.durationMs,
          },
          {
            ttlMs: Math.max(500, Number(msg.durationMs) || 3000),
            clearMessage: { type: "AGENT_NOTIFICATION_CLEAR" },
            waitForDelivery: false,
          }
        );
        return;
      }
      if (msg && msg.type === "attention_required" && typeof msg.tabId === "number") {
        const record = normalizeAttention(msg);
        renderAttention(msg.tabId, record);
        refreshActionBadge();
        return;
      }
      if (msg && msg.type === "attention_resolved" && typeof msg.guid === "string") {
        const prior = presentationBroker.states("attention:")
          .find((state) => state.channel === `attention:${msg.guid}`);
        if (prior) {
          presentationBroker.clearState(
            prior.tabId,
            prior.channel,
            { type: "AGENT_ATTENTION_CLEAR", guid: msg.guid }
          );
        }
        refreshActionBadge();
        return;
      }
      // ADR-0072/0078 session cleanup: the binary names only this MCP session's owned tabs. The
      // extension clears transient narration and cached dialog mechanism state for each tab.
      if (msg && msg.type === "narration_clear" && typeof msg.tabId === "number") {
        dialogStore.remove(msg.tabId);
        presentationBroker.clearState(
          msg.tabId,
          "narration",
          { type: "AGENT_NARRATION_CLEAR" }
        );
        return;
      }
      if (msg && msg.type === "gif_lease_renew" && typeof msg.tabId === "number") {
        const cast = gifCast.get(msg.tabId);
        if (cast && cast.recordingId === msg.recordingId && cast.generation === msg.generation) {
          cast.leaseDeadline = Date.now() + boundedGifTimeout(msg.leaseMs, 15000);
          armGifExpiry(msg.tabId, cast);
        }
        return;
      }
      if (msg && msg.type === "gif_capture_cancel" && typeof msg.tabId === "number") {
        const cast = gifCast.get(msg.tabId);
        if (cast && cast.recordingId === msg.recordingId && cast.generation === msg.generation) {
          stopGifCast(msg.tabId, cast, null);
        }
        return;
      }
      if (msg && (msg.type === "hold_state" || msg.type === "hold_error") && msg.id) {
        const pending = holdPending.get(msg.id);
        if (!pending) return; // late or duplicate reply
        holdPending.delete(msg.id);
        if (msg.type === "hold_state") {
          const held = msg.result && msg.result.held === true;
          updateHoldBadge(held);
          if (held) stopAllGifCasts();
          pending.resolve(msg.result || null);
        } else {
          pending.resolve(null);
        }
        return;
      }
      if (msg && (msg.type === "attention_state" || msg.type === "attention_error") && msg.id) {
        const pending = attentionPending.get(msg.id);
        if (!pending) return;
        attentionPending.delete(msg.id);
        if (msg.type === "attention_state") {
          pending.resolve(msg.result || { sessions: [] });
        } else {
          pending.resolve(null);
        }
      }
    };
    nativePort.onMessage.addListener(handleNativeMessage);
    nativePort.onDisconnect.addListener(() => {
      // chrome.runtime.lastError is the ONE piece of information this file otherwise has no way
      // to surface (ADR-0059): buffered (the port that would carry it just died) and delivered
      // on the next successful connect.
      const lastError = chrome.runtime.lastError;
      sendDebugEvent("connect_disconnect", lastError ? String(lastError.message || lastError) : null);
      // ADR-0073: a native-port loss revokes capture immediately. An MV3 timer is not the sole
      // safety mechanism for a privacy-sensitive screencast.
      stopAllGifCasts();
      wireChunkStore.clear();
      surfaceExecutor.clear();
      if (nativePort === connectedPort) nativePort = null;
      updateHoldBadge(null); // state unknown without a session
      setTimeout(connect, RECONNECT_DELAY_MS);
    });
    // Cold-start focus report (ADR-0058): a window can already be focused before this connect
    // ever completes (the common case -- the user was already looking at this browser), and
    // onFocusChanged below only fires on a FUTURE change, which might not happen again for a
    // while. Check once, right after attaching, so the service's focus-chain tie-breaker has a
    // real answer from the first tool call rather than only after the user later alt-tabs.
    reportFocusIfFocused();
    attentionRequest({ type: "get_attention" }).then(syncAttentionState);
  } catch {
    nativePort = null;
    setTimeout(connect, RECONNECT_DELAY_MS);
  }
}

async function browserProcessGeneration() {
  const stored = await chrome.storage.session.get(BROWSER_GENERATION_KEY);
  if (typeof stored[BROWSER_GENERATION_KEY] === "string" && stored[BROWSER_GENERATION_KEY]) {
    return stored[BROWSER_GENERATION_KEY];
  }
  const generation = typeof crypto.randomUUID === "function"
    ? crypto.randomUUID()
    : `${Date.now()}-${Math.random().toString(16).slice(2)}`;
  await chrome.storage.session.set({ [BROWSER_GENERATION_KEY]: generation });
  return generation;
}

// ADR-0081 adapter seams. Exact document targeting prevents a late message from landing in a
// replacement document. Activation injects only this package's committed visual scripts and the
// content script announces readiness after its listener exists.
async function deliverPresentation(tabId, documentId, envelope) {
  return chrome.tabs.sendMessage(tabId, envelope, { documentId });
}

async function activatePresentation(tabId) {
  try {
    await chrome.scripting.executeScript({
      target: { tabId, frameIds: [0] },
      files: VISUAL_SCRIPT_FILES,
    });
    return { ready: true };
  } catch (error) {
    return {
      ready: false,
      reason: "the visual layer is unavailable on this page",
      detail: (error && error.message) || String(error),
    };
  }
}

function persistPresentationSnapshot(snapshot) {
  chrome.storage.session.set({ [PRESENTATION_STATE_KEY]: snapshot }).catch(() => {});
}

function reply(response, result) {
  toolResponder.reply(response, result);
}

// --- Developer diagnostics (ADR-0059): mechanism only, fire-and-forget, the SAME posture as
// focus reporting below -- off by default (chrome.storage.local "ghostlight_debug"), and when
// on, purely a breadcrumb for `ghostlight doctor` / a raw debug-state file, never anything the
// dispatch path reads back. The decision logic (is debug on, buffer-while-no-port) lives in the
// pure lib/debug.js module (the SAME injected-dependency shape lib/grouping.js already
// established); this worker only supplies WHAT to post and WHERE.
const debugForwarder = self.GhostlightDebug.createDebugForwarder(chrome.storage.local);
function postToNativePort(msg) {
  nativePort.postMessage(msg);
}
function sendDebugEvent(event, detail) {
  return debugForwarder.send(nativePort ? postToNativePort : null, event, detail);
}
function flushPendingDebugEvents() {
  debugForwarder.flush(nativePort ? postToNativePort : null);
}

// --- Focus reporting (ADR-0058): mechanism only, fire-and-forget, the same posture as
// group_request/notification. Chosen over OS-level window z-order specifically because Chrome's
// own onFocusChanged/getLastFocused already answer "is THIS profile's window focused" from
// inside the one process that already knows it -- portably, with no unsafe native window
// enumeration. Only "gained focus" is ever reported: losing focus to another app (or to a
// DIFFERENT browser profile, which looks identical from here) carries no actionable signal, so
// there is no separate blurred/focused state to track or send.
function reportFocus() {
  try { nativePort && nativePort.postMessage({ type: "focus" }); } catch { /* port gone */ }
}

async function reportFocusIfFocused() {
  try {
    const win = await chrome.windows.getLastFocused();
    if (win && win.focused) {
      rememberFocusedWindow(chrome, win.id).catch(() => {});
      reportFocus();
    }
  } catch { /* no windows yet, or the API is unavailable on this platform */ }
}

chrome.windows.onFocusChanged.addListener((windowId) => {
  if (windowId !== chrome.windows.WINDOW_ID_NONE) {
    rememberFocusedWindow(chrome, windowId).catch(() => {});
    reportFocus();
  }
});
chrome.windows.onRemoved.addListener((windowId) => {
  forgetWorkspaceWindow(chrome, windowId).catch(() => {});
});

// --- Take-the-wheel hold (g10): mechanism only. The binary holds the flag and decides;
// this file only reports the user's gesture and renders the state the binary reports back.

// Send one get_hold/set_hold/toggle_hold request and resolve with its `result` object (or
// `null` on a hold_error, a 1500ms timeout, or no connected port). `null` means "no active
// session" to callers. Never gates tool_request dispatch on the outcome.
function holdRequest(payload) {
  return new Promise((resolve) => {
    if (!nativePort) {
      connect(); // attempt a reconnect for next time, but do not wait for it here
      resolve(null);
      return;
    }
    const id = `h${++holdSeq}`;
    const timer = setTimeout(() => {
      holdPending.delete(id);
      resolve(null);
    }, HOLD_REQUEST_TIMEOUT_MS);
    holdPending.set(id, {
      resolve: (result) => {
        clearTimeout(timer);
        resolve(result);
      },
    });
    try {
      nativePort.postMessage(Object.assign({ id }, payload));
    } catch {
      clearTimeout(timer);
      holdPending.delete(id);
      resolve(null);
    }
  });
}

function attentionRequest(payload) {
  return new Promise((resolve) => {
    if (!nativePort) {
      connect();
      resolve(null);
      return;
    }
    const id = `a${++attentionSeq}`;
    const timer = setTimeout(() => {
      attentionPending.delete(id);
      resolve(null);
    }, HOLD_REQUEST_TIMEOUT_MS);
    attentionPending.set(id, {
      resolve: (result) => {
        clearTimeout(timer);
        resolve(result);
      },
    });
    try {
      nativePort.postMessage(Object.assign({ id }, payload));
    } catch {
      clearTimeout(timer);
      attentionPending.delete(id);
      resolve(null);
    }
  });
}

function normalizeAttention(record) {
  if (!record || typeof record.guid !== "string" || !record.guid) return null;
  return {
    guid: record.guid,
    tabId: Number.isSafeInteger(record.tabId) ? record.tabId : null,
    label: String(record.label || "MCP client").slice(0, 80),
    category: record.category === "sacred" ? "sacred" : "policy",
    origin: typeof record.origin === "string" ? record.origin : null,
    threshold: record.threshold === "session" ? "session" : "matching",
    count: Number.isInteger(record.count) ? record.count : 0,
    title: String(record.title || "Agent browsing paused"),
    description: String(record.description || "Repeated blocked actions need your attention."),
    controls: Array.isArray(record.controls) ? record.controls.slice() : [],
  };
}

function renderAttention(tabId, record) {
  if (!record || !Number.isSafeInteger(tabId)) return;
  presentationBroker.publishState(tabId, `attention:${record.guid}`, {
    type: "AGENT_ATTENTION_REQUIRED",
    guid: record.guid,
    label: record.label,
    category: record.category,
    origin: record.origin,
    threshold: record.threshold,
    count: record.count,
    title: record.title,
    description: record.description,
    controls: record.controls,
  }, { waitForDelivery: false });
}

function syncAttentionState(result) {
  if (!result) return;
  const incoming = new Set((result.sessions || [])
    .filter((record) => record && typeof record.guid === "string")
    .map((record) => record.guid));
  for (const state of presentationBroker.states("attention:")) {
    const guid = state.message.guid;
    if (!incoming.has(guid)) {
      presentationBroker.clearState(
        state.tabId,
        state.channel,
        { type: "AGENT_ATTENTION_CLEAR", guid }
      );
    }
  }
  for (const raw of result.sessions || []) {
    const record = normalizeAttention(raw);
    if (record && Number.isSafeInteger(record.tabId)) renderAttention(record.tabId, record);
  }
  refreshActionBadge();
}

function refreshActionBadge() {
  if (presentationBroker.states("attention:").length > 0) {
    chrome.action.setBadgeText({ text: "!" });
    chrome.action.setBadgeBackgroundColor({ color: "#dc2626" });
  } else if (holdBadgeState === true) {
    chrome.action.setBadgeText({ text: "II" });
    chrome.action.setBadgeBackgroundColor({ color: "#38bdf8" });
  } else if (gifCast.size > 0) {
    chrome.action.setBadgeText({ text: "REC" });
    chrome.action.setBadgeBackgroundColor({ color: "#ef4444" });
  } else {
    chrome.action.setBadgeText({ text: "" });
  }
}

// `held` is `true`/`false` from a hold_state reply, or `null` when the session state is
// unknown (no connected port). Badge text/color only; renders state, decides nothing.
function updateHoldBadge(held) {
  holdBadgeState = held;
  refreshActionBadge();
}

chrome.commands.onCommand.addListener((command) => {
  if (command !== "toggle-hold") return;
  holdRequest({ type: "toggle_hold" });
});

// --- Panic kill switch (g11): mechanism only. The extension severs only its OWN debugger
// attachments and its OWN native port, at the user's direct gesture; it decides nothing about
// domains, tools, or grants. Distinct from the hold above: its own button, never a shared
// toggle, and it is never gated on or by any pause state.

// Detach every debugger attachment: the in-memory map first (the common case), then a sweep of
// chrome.debugger.getTargets() for attachments a prior service-worker instance made that this
// instance's map has forgotten. Errors are swallowed throughout: the tab may be gone, or the
// target may belong to something else (DevTools) and refuse to detach; either way there is
// nothing more useful to do here.
async function sweepDetachAll() {
  for (const tabId of attached.keys()) {
    try { await chrome.debugger.detach({ tabId }); } catch { /* tab may be gone */ }
  }
  try {
    const targets = await chrome.debugger.getTargets();
    for (const t of targets) {
      if (t.attached && t.tabId) {
        try { await chrome.debugger.detach({ tabId: t.tabId }); } catch { /* not ours, or already gone */ }
      }
    }
  } catch { /* getTargets unavailable; nothing more to sweep */ }
}

// One gesture, severs everything. Order is load-bearing (g11 constraint 10): marker first (so
// a service-worker death anywhere after this line is completed by startup recovery), signal the
// binary while the port is still open, detach every debugger, clear in-memory state, then tear
// down the port. Never closes, ungroups, or navigates any tab.
async function killSession() {
  sessionKilled = true; // set synchronously, before the first await: the hot-path refusal above
  await chrome.storage.session.set({ session_killed: true });

  if (nativePort) {
    try { nativePort.postMessage({ type: "session_killed" }); } catch { /* port gone */ }
    await sleep(100); // let the frame flush before the port is torn down
  }

  stopAllGifCasts();
  await sweepDetachAll();

  presentationBroker.clearPrefix("narration", { type: "AGENT_NARRATION_CLEAR" });
  presentationBroker.clearPrefix(
    "attention:",
    (message) => ({ type: "AGENT_ATTENTION_CLEAR", guid: message.guid })
  );
  for (const state of presentationBroker.states("notification")) {
    presentationBroker.clearState(
      state.tabId,
      state.channel,
      { type: "AGENT_NOTIFICATION_CLEAR" }
    );
  }

  attached.clear();
  attaching.clear();
  dragCoordinator.clear();
  activeDragOperations.clear(); // debugger detachment above releases every transient input state
  consoleBuffer.clear();
  networkBuffer.clear();
  screenshotCtx.clear();
  dialogStore.clear();

  if (nativePort) {
    // Chrome does not fire our own onDisconnect for a self-initiated disconnect, and even if a
    // reconnect timer were pending, the connect() guard above blocks it.
    try { nativePort.disconnect(); } catch { /* already gone */ }
    nativePort = null;
  }
  updateHoldBadge(null); // the session (and any hold state) is gone; render it as unknown
}

// First install: open the install walkthrough so whichever half the user found first
// (extension or binary) leads them to the other. Fires ONLY on reason "install" -- never on
// updates or browser restarts -- and holds no state. Mechanism only; no policy, no phoning
// home (a static page on the project's website, sylin.org).
chrome.runtime.onInstalled.addListener((details) => {
  if (details.reason === "install") {
    chrome.tabs.create({
      url: "https://sylin.org/ghostlight/chromium-extension/post-install/",
    });
  }
});

// Popup messages. Returns true to answer asynchronously; false for unrecognized types (the
// popup treats a false/undefined response the same as "no active session").
chrome.runtime.onMessage.addListener((msg, sender, sendResponse) => {
  if (msg && msg.type === "GHOSTLIGHT_PRESENTATION_READY") {
    const tabId = sender && sender.tab && sender.tab.id;
    const documentId = sender && sender.documentId;
    if (!Number.isSafeInteger(tabId) || sender.frameId !== 0 ||
        typeof documentId !== "string" || !documentId) {
      sendResponse({ accepted: false });
      return false;
    }
    ready.then(() => {
      const accepted = managedTabs.has(tabId) &&
        presentationBroker.documentReady(tabId, documentId);
      sendResponse({ accepted });
    });
    return true;
  }
  if (msg && msg.type === "getHoldState") {
    holdRequest({ type: "get_hold" }).then((result) => {
      sendResponse(
        result ? { session: true, held: result.held === true } : { session: false, held: false }
      );
    });
    return true;
  }
  if (msg && msg.type === "setHold") {
    holdRequest({ type: "set_hold", held: msg.held === true }).then((result) => {
      sendResponse(
        result ? { session: true, held: result.held === true } : { session: false, held: false }
      );
    });
    return true;
  }
  if (msg && msg.type === "GET_ATTENTION_STATE") {
    attentionRequest({ type: "get_attention" }).then((result) => {
      syncAttentionState(result);
      sendResponse(result || { sessions: [] });
    });
    return true;
  }
  if (msg && msg.type === "ATTENTION_ACTION") {
    attentionRequest({
      type: "attention_action",
      guid: msg.guid,
      disposition: msg.disposition,
    }).then(async (result) => {
      syncAttentionState(result);
      if (result && result.endSession === true) await killSession();
      sendResponse(result || { sessions: [] });
    });
    return true;
  }
  if (msg && msg.type === "GET_SESSION_STATE") {
    (async () => {
      const s = await chrome.storage.session.get("session_killed");
      sendResponse({
        killed: s.session_killed === true,
        connected: nativePort !== null,
        attachedTabs: attached.size,
        recordingTabs: gifCast.size,
      });
    })();
    return true;
  }
  if (msg && msg.type === "KILL_SESSION") {
    killSession().then(() => {
      sendResponse({ killed: true, connected: nativePort !== null, attachedTabs: attached.size });
    });
    return true;
  }
  if (msg && msg.type === "RECONNECT_SESSION") {
    (async () => {
      await chrome.storage.session.remove("session_killed");
      sessionKilled = false;
      connect();
      sendResponse({ killed: false, connected: nativePort !== null, attachedTabs: attached.size });
    })();
    return true;
  }
  return false;
});
// Tag an error with the hop (mechanism, not policy) that threw it, plus optional debug-only detail.
function hopError(hop, message, detail) {
  const err = new Error(message);
  err.hop = hop;
  if (detail) err.detail = String(detail);
  return err;
}
function fail(response, error) {
  toolResponder.fail(response, error);
}

// --- CDP ---
const attaching = new Map(); // tabId -> in-flight attach promise (prevents concurrent double-attach)
async function ensureAttached(tabId) {
  if (attached.has(tabId)) return;
  if (attaching.has(tabId)) return attaching.get(tabId);
  const p = (async () => {
    try {
      await chrome.debugger.attach({ tabId }, "1.3");
    } catch (e) {
      const msg = (e && e.message) || String(e);
      // A previous service-worker instance's attachment can survive a restart (Chrome keeps the
      // debugger session alive while the extension's worker itself dies); adopt it if it is still
      // there instead of failing a tool call over an attachment we already effectively own.
      if (/already attached/i.test(msg)) {
        const targets = await chrome.debugger.getTargets();
        const survivor = targets.find((t) => t.tabId === tabId && t.attached);
        if (!survivor) throw hopError("cdp", `debugger attach failed: ${msg}`);
      } else {
        throw hopError("cdp", `debugger attach failed: ${msg}`);
      }
    }
    attached.set(tabId, { domains: new Set() });
    try {
      const t = await chrome.tabs.get(tabId);
      tabHost.set(tabId, hostOf(t.url || ""));
      tabUrl.set(tabId, t.url || "");
    } catch { /* tab gone */ }
  })();
  attaching.set(tabId, p);
  try { await p; } finally { attaching.delete(tabId); }
}
// Coordinate model (harvest step 4, official v1.0.78): NO device-metrics override. Each screenshot
// probes the CSS viewport + DPR, captures at native resolution, downscales to a token budget, and
// records a per-tab ScreenshotContext. Model coordinates (read off that downscaled image) are then
// rescaled back to CSS viewport pixels before Input dispatch. ref-derived coordinates are already
// CSS px and are NOT rescaled.
const { targetDims, zoomScale, rescaleCtxCoord } = self.GhostlightGeometry;

async function probeViewport(tabId) {
  const r = await cdp(tabId, "Runtime.evaluate", {
    expression: "({w:innerWidth,h:innerHeight,d:window.devicePixelRatio||1,vis:document.visibilityState})",
    returnByValue: true,
  });
  const v = r && r.result && r.result.value;
  if (!v || !v.w || !v.h) throw hopError("page", "failed to probe viewport");
  // Chrome reports "hidden" for both background tabs and tabs in minimized windows, so this one
  // probe covers both cases; a missing value counts as visible so pages without the API are unaffected.
  return { vpW: v.w, vpH: v.h, dpr: v.d || 1, visible: (v.vis || "visible") === "visible" };
}
function bytesFromBase64(b64) {
  const bin = atob(b64), bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return bytes;
}
function base64FromBytes(bytes) {
  let bin = "";
  for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode.apply(null, bytes.subarray(i, i + 0x8000));
  return btoa(bin);
}
// Forward one captured frame to the binary as an unsolicited gif_frame event (ADR-0053 D2).
function sendGifFrame(tabId, cast, base64, deviceWidth, finalFrame) {
  try {
    nativePort && nativePort.postMessage({
      type: "gif_frame",
      tabId,
      recordingId: cast.recordingId,
      generation: cast.generation,
      sequence: cast.nextSequence++,
      data: base64,
      ts: Date.now(),
      deviceWidth: deviceWidth || undefined,
      final: finalFrame === true,
    });
  } catch (e) {
    /* port gone; this frame is lost, the stream continues */
  }
}
function boundedGifTimeout(value, fallback) {
  return Number.isFinite(value) && value >= 1000 ? Math.min(value, 10 * 60 * 1000) : fallback;
}
function stopGifCast(tabId, cast, reason) {
  const current = gifCast.get(tabId);
  if (!current || current !== cast) return;
  gifCast.delete(tabId);
  refreshActionBadge();
  if (cast.expiryTimer) clearTimeout(cast.expiryTimer);
  if (attached.has(tabId)) {
    chrome.debugger.sendCommand({ tabId }, "Page.stopScreencast", {}).catch(() => {});
  }
  if (reason) {
    try {
      nativePort && nativePort.postMessage({
        type: "gif_capture_ended",
        tabId,
        recordingId: cast.recordingId,
        generation: cast.generation,
        reason,
      });
    } catch { /* port gone */ }
  }
}
function armGifExpiry(tabId, cast) {
  if (cast.expiryTimer) clearTimeout(cast.expiryTimer);
  const deadline = Math.min(cast.leaseDeadline, cast.hardDeadline);
  cast.expiryTimer = setTimeout(() => {
    const current = gifCast.get(tabId);
    if (current !== cast) return;
    const now = Date.now();
    if (now < cast.leaseDeadline && now < cast.hardDeadline) {
      armGifExpiry(tabId, cast);
      return;
    }
    stopGifCast(tabId, cast, now >= cast.hardDeadline ? "hard_timeout" : "lease_expired");
  }, Math.max(0, deadline - Date.now()));
}
// One screencast frame (ADR-0053 D2): ack immediately (unacked frames stall the compositor's
// screencast pipeline), thin to the service-chosen minimum interval, and forward. The worker
// stores NOTHING -- recording state and frames live in the binary.
async function handleScreencastFrame(tabId, params) {
  try {
    await cdp(tabId, "Page.screencastFrameAck", { sessionId: params.sessionId });
  } catch (e) {
    /* ack is best-effort: a detaching tab has nothing left to stall */
  }
  const cast = gifCast.get(tabId);
  if (!cast) return;
  const now = Date.now();
  if (now >= cast.leaseDeadline || now >= cast.hardDeadline) {
    stopGifCast(tabId, cast, now >= cast.hardDeadline ? "hard_timeout" : "lease_expired");
    return;
  }
  if (now - cast.lastSentTs < cast.minIntervalMs) return;
  cast.lastSentTs = now;
  const deviceWidth = params.metadata && params.metadata.deviceWidth;
  sendGifFrame(tabId, cast, params.data, deviceWidth, false);
}

// Stop and forget all capture mechanics. Safe to call from disconnect and panic paths without
// awaiting completion; clearing the map first prevents any later compositor event from forwarding.
function stopAllGifCasts() {
  const casts = Array.from(gifCast.entries());
  gifCast.clear();
  for (const [tabId, cast] of casts) {
    if (cast.expiryTimer) clearTimeout(cast.expiryTimer);
    if (attached.has(tabId)) cdp(tabId, "Page.stopScreencast", {}).catch(() => {});
  }
}
async function encodeJpeg(bitmap, w, h, quality) {
  const canvas = new OffscreenCanvas(w, h);
  const ctx = canvas.getContext("2d");
  ctx.drawImage(bitmap, 0, 0, w, h);
  const blob = await canvas.convertToBlob({ type: "image/jpeg", quality });
  return base64FromBytes(new Uint8Array(await blob.arrayBuffer()));
}
// Map a model-provided coordinate (read off the downscaled screenshot) back to CSS viewport px.
// Passthrough when no screenshot has been taken for the tab (nothing to map against). A zoomed
// capture carries a region offset (offX, offY) that the mapped point is added back onto.
function rescaleCoord(tabId, x, y) {
  return rescaleCtxCoord(screenshotCtx.get(tabId), x, y);
}
async function cdp(tabId, method, params) {
  await ensureAttached(tabId);
  try {
    return await chrome.debugger.sendCommand({ tabId }, method, params || {});
  } catch (e) {
    throw hopError("cdp", `${method} failed: ${(e && e.message) || e}`);
  }
}
async function enableDomain(tabId, domain) {
  const state = attached.get(tabId);
  if (!state) throw new Error("not attached");
  if (state.domains.has(domain)) return;
  await chrome.debugger.sendCommand({ tabId }, domain + ".enable", {});
  state.domains.add(domain);
}
// Remove every transient mechanism record for one tab. Idempotent so explicit `tab_control.close`
// and Chrome's onRemoved event can both call it without widening the close to a group or window.
function clearTabState(tabId) {
  dragCoordinator.cancel(tabId);
  const drag = activeDragOperations.get(tabId);
  if (drag) drag.cancelled = true;
  activeDragOperations.delete(tabId);
  const cast = gifCast.get(tabId);
  if (cast) stopGifCast(tabId, cast, "browser_detached");
  if (attached.has(tabId)) {
    try { chrome.debugger.detach({ tabId }); } catch { /* already gone */ }
    attached.delete(tabId);
  }
  consoleBuffer.delete(tabId);
  networkBuffer.delete(tabId);
  screenshotCtx.delete(tabId);
  tabHost.delete(tabId);
  tabUrl.delete(tabId);
  dialogStore.remove(tabId);
  managedTabs.delete(tabId); // ADR-0066 D5: a closed tab is no longer ours to reach
  presentationBroker.destroyTab(tabId);
  persistSessionState();
}
chrome.tabs.onRemoved.addListener((tabId) => {
  clearTabState(tabId);
  if (executorBrowserSlot !== null) {
    surfaceExecutor.destroyKey(`surface:${executorBrowserSlot}:${tabId}`);
  }
  try {
    nativePort && nativePort.postMessage({
      type: "surface_destroyed",
      tabId,
      executorGeneration: EXECUTOR_GENERATION,
    });
  } catch { /* port gone */ }
});
chrome.debugger.onDetach.addListener((src) => {
  const cast = gifCast.get(src.tabId);
  if (cast) stopGifCast(src.tabId, cast, "browser_detached");
  attached.delete(src.tabId);
  dragCoordinator.cancel(src.tabId);
  const drag = activeDragOperations.get(src.tabId);
  if (drag) drag.cancelled = true;
  activeDragOperations.delete(src.tabId);
  dialogStore.remove(src.tabId);
});

// --- Console / network buffering (join network events by requestId, unlike the reference) ---
function hostOf(url) {
  try { return new URL(url).hostname; } catch { return ""; }
}
chrome.tabs.onUpdated.addListener((tabId, info) => {
  if (info.status === "loading" && managedTabs.has(tabId)) {
    void cancelActiveDrag(tabId);
    presentationBroker.documentLoading(tabId);
  }
  if (info.url !== undefined) {
    tabHost.set(tabId, hostOf(info.url));
    tabUrl.set(tabId, info.url);
    dialogStore.remove(tabId);
  }
  if (info.status === "complete" && managedTabs.has(tabId)) {
    presentationBroker.activateTab(tabId);
  }
});
// Render an uncaught-exception CDP event as one single-line string: base message, then an
// optional (url:line) location, then an optional compact [at frame, frame, ...] stack.
// fallbackUrl covers exceptions whose exceptionDetails/callFrames carry no url of their own
// (routine for a deferred callback rather than a freshly-parsed script): the tab's current URL
// beats an empty/misleading "@:1" location.
function exceptionText(details, fallbackUrl) {
  const exc = details.exception;
  let base;
  if (exc && typeof exc.description === "string" && exc.description) {
    base = exc.description.split("\n")[0];
  } else if (exc && exc.value !== undefined) {
    base = String(exc.value);
  } else if (typeof details.text === "string" && details.text) {
    base = details.text;
  } else {
    base = "Uncaught exception";
  }
  let out = base;
  const url = (typeof details.url === "string" && details.url) || fallbackUrl || "";
  if (url) {
    // CDP line numbers are 0-based; add 1 for the human-readable line reported here.
    out += typeof details.lineNumber === "number" ? ` (${url}:${details.lineNumber + 1})` : ` (${url})`;
  }
  const frames = details.stackTrace && Array.isArray(details.stackTrace.callFrames) ? details.stackTrace.callFrames : [];
  if (frames.length) {
    const rendered = frames.slice(0, 3).map((f) => `${f.functionName || "<anonymous>"}@${f.url || fallbackUrl || ""}:${f.lineNumber + 1}`);
    out += ` [at ${rendered.join(", ")}]`;
  }
  return out;
}
chrome.debugger.onEvent.addListener((src, method, params) => {
  const tabId = src.tabId;
  if (method === "Input.dragIntercepted") {
    dragCoordinator.intercepted(tabId, params && params.data);
    return;
  }
  if (method === "Page.javascriptDialogOpening") {
    dialogStore.opened(tabId, params);
    return;
  }
  if (method === "Page.javascriptDialogClosed") {
    dialogStore.remove(tabId);
    return;
  }
  if (method === "Page.screencastFrame") {
    // gif_creator capture (ADR-0052 D1): fire-and-forget; the handler acks + keeps/drops the frame.
    handleScreencastFrame(tabId, params);
    return;
  }
  if (method === "Runtime.consoleAPICalled") {
    // Single console source. Both the Runtime domain (Runtime.consoleAPICalled) and the
    // deprecated Console domain (Console.messageAdded) report the same console.* call, so
    // enabling and buffering both double-counts every message. We keep only the richer
    // Runtime event (structured args + method-accurate `type`) and never enable Console.
    const text = (params.args || []).map((a) => a.value !== undefined ? a.value : (a.description || "")).join(" ");
    pushCapped(consoleBuffer, tabId, { level: params.type || "log", text });
  } else if (method === "Runtime.exceptionThrown") {
    pushCapped(consoleBuffer, tabId, { level: "exception", text: exceptionText(params.exceptionDetails || {}, tabUrl.get(tabId)) });
  } else if (method === "Network.requestWillBeSent" && params.request) {
    pushCapped(networkBuffer, tabId, { requestId: params.requestId, method: params.request.method, url: params.request.url, status: 0 });
  } else if (method === "Network.responseReceived" && params.response) {
    const buf = bufferFor(networkBuffer, tabId, tabHost.get(tabId));
    const existing = buf.items.find((r) => r.requestId === params.requestId);
    if (existing) { existing.status = params.response.status; existing.mimeType = params.response.mimeType; }
    else pushCapped(networkBuffer, tabId, { requestId: params.requestId, method: "?", url: params.response.url, status: params.response.status, mimeType: params.response.mimeType });
  } else if (method === "Network.loadingFailed" && params.requestId) {
    const buf = bufferFor(networkBuffer, tabId, tabHost.get(tabId));
    const existing = buf.items.find((r) => r.requestId === params.requestId);
    if (existing) {
      existing.status = 503;
      if (params.errorText) existing.errorText = params.errorText;
      existing.canceled = !!params.canceled;
    }
  }
});
// Buffers are owned by the tab's current hostname, per the read_console_messages /
// read_network_requests schema contract; a hostname change replaces the buffer with a fresh one.
function bufferFor(map, tabId, host) {
  let buf = map.get(tabId);
  if (!buf || (host !== undefined && buf.host !== undefined && buf.host !== host)) {
    buf = { host, items: [] };
    map.set(tabId, buf);
  } else if (buf.host === undefined && host !== undefined) {
    buf.host = host; // entries captured before the host was known belong to the first host learned
  }
  return buf;
}
function pushCapped(map, tabId, item) {
  const buf = bufferFor(map, tabId, tabHost.get(tabId));
  buf.items.push(item);
  if (buf.items.length > 1000) buf.items.splice(0, buf.items.length - 1000);
}

// --- Tab group (created lazily; recovered from live state after a service-worker restart) ---
// chrome.storage.session survives a service-worker restart (extension reload, browser update,
// crash) but is cleared on a full browser restart -- exactly the durability window we want: a
// genuinely fresh browser session looks like a fresh install, never a false recovery notice.
async function persistSessionState() {
  let tabIds = [];
  if (groupId !== null) {
    try {
      tabIds = (await chrome.tabs.query({ groupId })).map((t) => t.id);
    } catch {
      tabIds = []; // the group vanished between the null check and the query
    }
  }
  try {
    await chrome.storage.session.set({
      sessionState: { groupId, tabIds },
      // ADR-0085: the workspace-key -> groupId map, persisted under its OWN key -- ADDITIVE
      // alongside `sessionState`, whose own shape is unchanged -- so a service-worker restart
      // recovers client groups too (a browser restart clears it, and rehydrate reclaims by title).
      clientGroupsState: Array.from(clientGroups.entries()),
      // ADR-0066 D5: the managed-tab set, so a detached-but-owned tab stays reachable across a
      // service-worker restart. Session-scoped only -- never storage.local -- because Chrome
      // renumbers tab ids across a browser restart and a stale id must not survive one.
      managedTabsState: Array.from(managedTabs),
    });
  } catch { /* persistence is best-effort; recovery still has the title-based fallback below */ }
}
async function ensureGroup(create) {
  if (groupId !== null) {
    try {
      await chrome.tabGroups.get(groupId);
      await persistSessionState();
      return;
    } catch { groupId = null; }
  }
  const groups = await chrome.tabGroups.query({ title: GROUP_TITLE });
  if (groups.length) {
    groupId = groups[0].id;
    await persistSessionState();
    return;
  }
  if (!create) {
    await persistSessionState();
    return;
  }
  const win = await chrome.windows.create({ focused: true, url: "about:blank" });
  const gid = await chrome.tabs.group({ tabIds: [win.tabs[0].id] });
  await chrome.tabGroups.update(gid, { title: GROUP_TITLE, color: "blue" });
  groupId = gid;
  markTabManaged(win.tabs[0].id); // ADR-0066 D5: track the global group's tab too
  await persistSessionState();
}

class WorkspaceWindowGoneError extends Error {}

function withWorkspaceResult(result, windowId) {
  if (result && typeof result === "object") {
    result._ghostlightWorkspace = { windowId };
  }
  return result;
}

async function workspaceGroupId(clientKey, windowId) {
  const state = await resolveWorkspaceGroup(chrome, clientGroups, clientKey, windowId);
  return { key: state.key, gid: state.groupId, changed: state.changed };
}

async function createTabInResolvedWorkspace(clientKey, resolved) {
  const windowId = resolved.window.id;
  const state = await workspaceGroupId(clientKey, windowId);
  let tab;
  if (resolved.created && resolved.window.tabs && resolved.window.tabs[0] && state.gid === null) {
    tab = resolved.window.tabs[0];
  } else {
    try {
      tab = await chrome.tabs.create({ active: true, windowId });
    } catch (error) {
      throw new WorkspaceWindowGoneError((error && error.message) || String(error));
    }
  }

  let gid = state.gid;
  if (gid === null) {
    gid = await chrome.tabs.group({ tabIds: [tab.id] });
    await chrome.tabGroups.update(gid, { title: GROUP_TITLE, color: "blue" });
  } else {
    await chrome.tabs.group({ tabIds: [tab.id], groupId: gid });
  }
  clientGroups.set(state.key, gid);
  markTabManaged(tab.id);
  return { tab, gid, windowId };
}

// Resolve the service-requested placement and create a tab without moving existing tabs. An
// automatic target may disappear between getLastFocused and tabs.create; retry that pre-mutation
// race once. A pinned target never silently fails over.
async function createTabInSessionGroup(clientKey, workspaceRequest, initialTarget) {
  const first = initialTarget || await resolveWorkspaceWindow(chrome, workspaceRequest);
  try {
    return await createTabInResolvedWorkspace(clientKey, first);
  } catch (error) {
    if (first.pinned || !(error instanceof WorkspaceWindowGoneError)) throw error;
    const second = await resolveWorkspaceWindow(chrome, workspaceRequest);
    return createTabInResolvedWorkspace(clientKey, second);
  }
}
async function groupTabs() {
  const ids = managedGroupIds(groupId, clientGroups);
  const all = [];
  for (const gid of ids) {
    try {
      all.push(...(await chrome.tabs.query({ groupId: gid })));
    } catch { /* a vanished group contributes no tabs */ }
  }
  return all;
}
// ADR-0066 D5: record every tab currently sitting in a managed group as a managed tab, so it stays
// reachable if the user later drags it out. Called on rehydrate to rebuild the set from live state
// after a browser restart (where the persisted set's tab ids are stale). Best-effort per group.
async function seedManagedTabsFromGroups() {
  for (const gid of managedGroupIds(groupId, clientGroups)) {
    try {
      for (const t of await chrome.tabs.query({ groupId: gid })) markTabManaged(t.id);
    } catch { /* a vanished group contributes no tabs */ }
  }
}
async function inGroup(tabId) {
  // Always consult live state; the in-memory groupId can be stale after a restart.
  try {
    const tab = await chrome.tabs.get(tabId);
    if (tab.groupId !== -1 && groupId === null) {
      const g = await chrome.tabGroups.get(tab.groupId);
      if (g.title === GROUP_TITLE) {
        groupId = g.id;
        await persistSessionState();
      }
    }
    // ADR-0066 D5: in-surface if the tab sits in a managed group OR it is one we manage but the
    // user has dragged out of the group (ungrouped / moved to another window). `managedTabs` only
    // ever holds tabs we grouped, so a never-managed user tab is still refused.
    return isManagedGroupId(tab.groupId, groupId, clientGroups) || managedTabs.has(tabId);
  } catch {
    return false;
  }
}
// Restore durable session state (if any) on service-worker startup. Never rejects: any internal
// failure degrades to the existing cold-start / title-based recovery path instead of wedging
// dispatch, which awaits this promise before running any tool.
async function rehydrate() {
  try {
    const stored = await chrome.storage.session.get([
      "sessionState",
      "clientGroupsState",
      "managedTabsState",
      PRESENTATION_STATE_KEY,
    ]);
    presentationBroker.restore(stored && stored[PRESENTATION_STATE_KEY]);
    const sessionState = stored && stored.sessionState;
    // ADR-0085: restore the workspace-group map independently of the legacy single-group
    // `sessionState` below -- a fresh install has neither, but either one being absent must not
    // block recovering the other.
    if (Array.isArray(stored && stored.clientGroupsState)) {
      for (const [key, gid] of stored.clientGroupsState) clientGroups.set(key, gid);
    }
    // ADR-0066 D5: restore the managed-tab set (a service-worker restart keeps tab ids stable).
    if (Array.isArray(stored && stored.managedTabsState)) {
      for (const id of stored.managedTabsState) markTabManaged(id);
    }
    // ADR-0047 D5: drop any restored groups whose Chrome group died while the worker was asleep,
    // so the managed surface never names a stale group id.
    await pruneDeadGroups(chrome, clientGroups);
    // ADR-0085: upgrade legacy client-only keys and repair a group's key after the user moved it.
    // Then re-attach to groups Chrome restored after a browser restart by combining each title's
    // client key with the group's CURRENT live window id.
    await reconcileWorkspaceGroups(chrome, clientGroups);
    await reclaimGroupsByTitle(
      chrome,
      clientGroups,
      CLIENT_TITLE_PREFIX,
      workspaceGroupKey
    );
    // ADR-0066 D5: re-seed managedTabs from the live members of every managed group (covers the
    // browser-restart case where Chrome renumbered tab ids), then drop any managed id that no
    // longer exists (tabs closed while the worker was asleep).
    await seedManagedTabsFromGroups();
    for (const id of Array.from(managedTabs)) {
      try { await chrome.tabs.get(id); } catch { managedTabs.delete(id); }
    }
    for (const state of presentationBroker.states()) {
      if (!managedTabs.has(state.tabId)) presentationBroker.destroyTab(state.tabId);
    }
    // reclaim/prune/seed all mutate durable state; persist the reconciled maps unconditionally.
    await persistSessionState();
    if (!sessionState) return; // genuinely fresh start: nothing more to recover
    const priorSession =
      sessionState.groupId !== null ||
      (Array.isArray(sessionState.tabIds) && sessionState.tabIds.length > 0);
    if (priorSession) {
      consoleResetNotice = true;
      networkResetNotice = true;
    }
    if (sessionState.groupId !== null) {
      try {
        await chrome.tabGroups.get(sessionState.groupId);
        groupId = sessionState.groupId; // stored id is authoritative even if the user renamed it
      } catch { /* group is gone; ensureGroup's title-query fallback recovers next */ }
    }
    await persistSessionState();
  } catch { /* rehydration must never wedge dispatch; degrade to cold-start behavior */ }
}
// Thrown when a tool call names a tab outside the group or the group has no usable tab.
// dispatch() converts it to a plain text tool result so the message reaches the model
// verbatim, matching how group-membership refusals are delivered today.
class TabAccessError extends Error {}

// Resolve the tab a tool call acts on. A provided tabId must be in the group; an omitted or
// null tabId falls back to the group's active tab, else its most recently accessed tab.
async function effectiveTabId(rawTabId) {
  if (rawTabId !== undefined && rawTabId !== null) {
    if (await inGroup(rawTabId)) return rawTabId;
    await ensureGroup(false);
    const tabs = await groupTabs();
    if (!tabs.length) {
      throw new TabAccessError(`Tab ${rawTabId} is not a tab Ghostlight manages, and there are no managed tabs yet. Create one with tabs_create_mcp.`);
    }
    throw new TabAccessError(`Tab ${rawTabId} is not a tab Ghostlight manages. Valid tab IDs: ${tabs.map((t) => t.id).join(", ")}. List them with tabs_context_mcp.`);
  }
  await ensureGroup(false);
  const tabs = await groupTabs();
  if (!tabs.length) {
    throw new TabAccessError(`No Ghostlight tabs yet. Create one with tabs_create_mcp, or call tabs_context_mcp with createIfEmpty: true.`);
  }
  const active = tabs.filter((t) => t.active);
  const pool = active.length ? active : tabs;
  let best = pool[0];
  for (const t of pool) {
    if ((t.lastAccessed || 0) > (best.lastAccessed || 0)) best = t;
  }
  return best.id;
}

// Resolve the tab for a `navigate` call, auto-creating the Ghostlight tab group + a tab when there
// is NONE yet (CAP-MED-02). navigate is the natural bootstrap action -- "go somewhere" implies
// "make a place to go" -- so a first-time agent that calls navigate before opening a group just
// works, instead of failing cold and having to discover tabs_create_mcp first. Bootstrap only when
// the managed surface is genuinely empty: if managed tabs DO exist and the named tabId is not one
// of them, effectiveTabId's helpful "not a tab Ghostlight manages" error stands -- a wrong tabId is
// a real mistake, not a bootstrap. Client-scoped when a `key` is present (ADR-0066: clientKey, or a
// legacy guid), else the legacy global group for guid-less native callers.
async function navigateTabId(rawTabId, key, workspaceRequest) {
  if (rawTabId !== undefined && rawTabId !== null) {
    return { tabId: await effectiveTabId(rawTabId), windowId: null };
  }
  if (typeof key === "string" && key) {
    const resolved = await resolveWorkspaceWindow(chrome, workspaceRequest);
    const state = await workspaceGroupId(key, resolved.window.id);
    if (state.changed) await persistSessionState();
    if (state.gid !== null) {
      const tabs = await chrome.tabs.query({ groupId: state.gid });
      if (tabs.length) {
        const active = tabs.find((tab) => tab.active) || tabs[0];
        return { tabId: active.id, windowId: resolved.window.id };
      }
    }
    const { tab, windowId } = await createTabInSessionGroup(key, workspaceRequest, resolved);
    await persistSessionState();
    return { tabId: tab.id, windowId };
  }

  await ensureGroup(false);
  const tabs = await groupTabs();
  if (tabs.length) return { tabId: await effectiveTabId(rawTabId), windowId: null };
  await ensureGroup(true);
  const tab = await chrome.tabs.create({ active: true });
  await chrome.tabs.group({ tabIds: [tab.id], groupId });
  await persistSessionState();
  return { tabId: tab.id, windowId: null };
}
function tabContext(tabs, reportGroupId) {
  const gid = reportGroupId === undefined ? groupId : reportGroupId;
  const available = tabs.map((t) => ({ tabId: t.id, title: t.title || "", url: t.url || "" }));
  const r = text(JSON.stringify({ mcpGroupId: gid, tabs: available }, null, 2));
  r.structuredContent = { mcpGroupId: gid, tabs: available };
  return r;
}

// --- Content-script bridge (inject on demand) ---
async function content(tabId, message) {
  try {
    return await chrome.tabs.sendMessage(tabId, message);
  } catch {
    try {
      await chrome.scripting.executeScript({ target: { tabId }, files: ["lib/settle.js", "lib/observation.js", "lib/receipt.js", "lib/treediff.js", "lib/fileset.js", "lib/actionable.js", "lib/keys.js", "lib/drag-session.js", "content.js"] });
      return await chrome.tabs.sendMessage(tabId, message);
    } catch (e) {
      throw hopError(
        "page",
        "content script unavailable on this page (script injection blocked)",
        (e && e.message) || e
      );
    }
  }
}

// --- MCP result helpers ---
function text(t) {
  return { content: [{ type: "text", text: t }] };
}
function textImage(t, base64) {
  return { content: [{ type: "text", text: t }, { type: "image", data: base64, mimeType: "image/jpeg" }] };
}

// --- Consequence digest wrapper (ADR-0037 D2, PINS.md SS10): wrap a mutating action so the page
// is sampled before the action and 300ms after, and the action's text confirmation gains an
// `observation:` block reporting what changed. `run` performs the action and returns the result
// (text/textImage); the snap is taken first, then `run`, then the sample. The existing
// confirmation text is untouched; the digest is appended after a "\n" separator. The structured
// twin merges into structuredContent. Best-effort: a content-script failure (e.g. the page
// navigated away) degrades to the plain confirmation -- the observation is additive, never
// load-bearing.
async function appendDialogBlocker(result, tabId, meta) {
  const open = dialogStore.current(tabId);
  if (!open) return result;
  if (!result.structuredContent) result.structuredContent = {};
  let receipt = result.structuredContent.interactionReceipt;
  if (!receipt) {
    receipt = {
      targetAssurance: meta.targetAssurance || "none",
      action: meta.action || "unknown",
      observedAfter: {},
      blockers: [],
      page: await pageMeta(tabId),
      more: false,
    };
    result.structuredContent.interactionReceipt = receipt;
  }
  if (!Array.isArray(receipt.blockers)) receipt.blockers = [];
  if (!receipt.blockers.some((blocker) => blocker.kind === "dialog_open")) {
    receipt.blockers.push({
      kind: "dialog_open",
      summary: "A JavaScript dialog is blocking the tab.",
      nextStep: "Inspect and resolve the dialog explicitly before continuing.",
    });
  }
  const rendered = "blocked: dialog_open: A JavaScript dialog is blocking the tab. " +
    "Next: Inspect and resolve the dialog explicitly before continuing.";
  if (!result.content[0].text.includes("dialog_open")) result.content[0].text += "\n" + rendered;
  return result;
}

async function withObservation(tabId, meta, run) {
  if (typeof meta === "function") {
    run = meta;
    meta = {};
  }
  const receiptMeta = Object.assign({ tabId, targetAssurance: "none" }, meta || {});
  try {
    await ensureAttached(tabId);
    await enableDomain(tabId, "Page");
    await sleep(0);
  } catch { /* dialog tracking is additive; the action's own mechanism reports hard failures */ }
  if (dialogStore.current(tabId)) {
    return appendDialogBlocker(
      text("Action not dispatched because a JavaScript dialog is already blocking the tab."),
      tabId,
      receiptMeta
    );
  }
  if (receiptMeta.ref) {
    try {
      const resolved = await content(tabId, { type: "elementSummary", ref: receiptMeta.ref });
      if (resolved && resolved.result && !resolved.result.error) receiptMeta.target = resolved.result;
    } catch { /* target detail is additive; dispatch still owns the real stale-ref decision */ }
  }
  let before = null;
  try { before = await content(tabId, { type: "observeSnap" }); } catch { /* page may be mid-load */ }
  const result = await run();
  if (before && before.result) {
    try {
      const sample = await content(tabId, { type: "observeSample", before: before.result, meta: receiptMeta });
      const obs = sample && sample.result;
      if (obs && obs.digest) {
        result.content[0].text += "\n" + obs.digest;
        if (obs.receipt) {
          result.structuredContent = Object.assign({}, result.structuredContent || {}, {
            interactionReceipt: obs.receipt,
          });
        }
      }
    } catch { /* observation never masks the action's own result */ }
  }
  return appendDialogBlocker(result, tabId, receiptMeta);
}

// --- Screenshot pipeline: capture native, downscale to the token budget, record ScreenshotContext ---
// Returns { base64, note }; note is "" on every clean path and carries a truthful warning when a
// non-visible tab could not be captured directly and the standard (possibly blank/stale) path ran.
async function screenshot(tabId) {
  await ensureAttached(tabId);
  const { vpW, vpH, dpr, visible } = await probeViewport(tabId);
  const { w, h } = targetDims(vpW, vpH);
  // Hide the phantom cursor / glow so they never appear in the model's screenshot.
  await presentationBroker.publishCapture(tabId, { type: "HIDE_FOR_TOOL_USE" });
  await sleep(CAPTURE_SETTLE_MS);
  let cap, note = "", clipMsg = null;
  try {
    if (!visible) {
      // Background/minimized tabs: clip + scale in one pass inside the browser (no canvas re-encode
      // needed), reading from the compositing surface so a non-presented tab still yields real pixels.
      const scale = w / vpW; // always <= 1: targetDims never grows past the CSS viewport
      const clipParams = { clip: { x: 0, y: 0, width: vpW, height: vpH, scale }, fromSurface: true, captureBeyondViewport: false };
      try {
        cap = await cdp(tabId, "Page.captureScreenshot", { format: "jpeg", quality: JPEG_QUALITY, ...clipParams });
        if (cap.data.length > MAX_SCREENSHOT_B64) {
          cap = await cdp(tabId, "Page.captureScreenshot", { format: "jpeg", quality: JPEG_QUALITY_FALLBACK, ...clipParams });
        }
        // The encoded image may differ from w x h by at most one rounding pixel per axis; recording
        // w/h (not the decoded bitmap) keeps rescaleCoord's mapping exact without a canvas pass.
        screenshotCtx.set(tabId, { vpW, vpH, shotW: w, shotH: h, offX: 0, offY: 0, regionW: vpW, regionH: vpH });
        return { base64: cap.data, note: "" };
      } catch (e) {
        clipMsg = (e && e.message) || String(e);
      }
    }
    try {
      cap = await cdp(tabId, "Page.captureScreenshot", { format: "jpeg", quality: JPEG_QUALITY_FULL, captureBeyondViewport: false });
    } catch (e) {
      if (clipMsg === null) throw e; // visible tab: propagate the standard-capture failure unchanged
      const fbMsg = (e && e.message) || String(e);
      throw new Error(`screenshot of non-visible tab failed: clipped capture: ${clipMsg}; fallback capture: ${fbMsg}`);
    }
    if (clipMsg !== null) {
      note = "Warning: this tab was not visible and direct background capture failed; the image was taken with the standard capture path and may be blank or stale.";
    }
  } finally {
    await presentationBroker.publishCapture(tabId, { type: "SHOW_AFTER_TOOL_USE" });
  }
  // Default to the raw native capture (dims = CSS viewport * DPR) if canvas downscaling is unavailable.
  let base64 = cap.data, shotW = Math.round(vpW * dpr), shotH = Math.round(vpH * dpr);
  try {
    const bitmap = await createImageBitmap(new Blob([bytesFromBase64(cap.data)], { type: "image/jpeg" }));
    base64 = await encodeJpeg(bitmap, w, h, JPEG_QUALITY / 100);
    if (base64.length > MAX_SCREENSHOT_B64) base64 = await encodeJpeg(bitmap, w, h, JPEG_QUALITY_FALLBACK / 100);
    shotW = w; shotH = h;
    if (bitmap.close) bitmap.close();
  } catch { /* OffscreenCanvas/createImageBitmap unavailable: keep the raw native capture */ }
  // A full screenshot resets the zoom offset: subsequent coordinates map against the whole viewport.
  screenshotCtx.set(tabId, { vpW, vpH, shotW, shotH, offX: 0, offY: 0, regionW: vpW, regionH: vpH });
  return { base64, note };
}

// --- Zoom: capture a clipped, magnified region and record it as the tab's coordinate context ---
async function zoomScreenshot(tabId, region) {
  await ensureAttached(tabId);
  const r = await cdp(tabId, "Runtime.evaluate", {
    expression: "({w:innerWidth,h:innerHeight,sx:window.scrollX||0,sy:window.scrollY||0})",
    returnByValue: true,
  });
  const v = r && r.result && r.result.value;
  if (!v || !v.w || !v.h) throw hopError("page", "failed to probe viewport");
  const vpW = v.w, vpH = v.h, sx = v.sx || 0, sy = v.sy || 0;
  // Rescale against the context as it was BEFORE this zoom, so a zoom issued against a previous
  // zoomed screenshot composes correctly (chained zooms).
  const [rx0, ry0] = rescaleCoord(tabId, region[0], region[1]);
  const [rx1, ry1] = rescaleCoord(tabId, region[2], region[3]);
  const x0 = Math.min(Math.max(rx0, 0), vpW), y0 = Math.min(Math.max(ry0, 0), vpH);
  const x1 = Math.min(Math.max(rx1, 0), vpW), y1 = Math.min(Math.max(ry1, 0), vpH);
  const clamped = x0 !== rx0 || y0 !== ry0 || x1 !== rx1 || y1 !== ry1;
  const w = x1 - x0, h = y1 - y0;
  if (w < 1 || h < 1) return { error: "zoom region is empty or entirely outside the visible viewport." };
  const s = zoomScale(w, h);
  await presentationBroker.publishCapture(tabId, { type: "HIDE_FOR_TOOL_USE" });
  await sleep(CAPTURE_SETTLE_MS);
  let cap;
  try {
    cap = await cdp(tabId, "Page.captureScreenshot", {
      format: "jpeg", quality: JPEG_QUALITY_FULL,
      // clip is document-relative CSS pixels, not viewport-relative, so the scroll offset is added.
      // captureBeyondViewport must be true for CDP to actually honor that: with it false, Chrome
      // treats clip as viewport-relative and the scroll offset added above gets double-counted,
      // clipping to a position outside the rendered surface (a blank capture) on any scrolled page.
      clip: { x: sx + x0, y: sy + y0, width: w, height: h, scale: s },
      captureBeyondViewport: true,
    });
  } finally {
    await presentationBroker.publishCapture(tabId, { type: "SHOW_AFTER_TOOL_USE" });
  }
  let shotW = Math.max(1, Math.round(w * s)), shotH = Math.max(1, Math.round(h * s));
  let base64 = cap.data;
  try {
    const bitmap = await createImageBitmap(new Blob([bytesFromBase64(cap.data)], { type: "image/jpeg" }));
    base64 = await encodeJpeg(bitmap, bitmap.width, bitmap.height, JPEG_QUALITY / 100);
    if (base64.length > MAX_SCREENSHOT_B64) base64 = await encodeJpeg(bitmap, bitmap.width, bitmap.height, JPEG_QUALITY_FALLBACK / 100);
    shotW = bitmap.width; shotH = bitmap.height;
    if (bitmap.close) bitmap.close();
  } catch { /* OffscreenCanvas/createImageBitmap unavailable: keep the raw native capture */ }
  screenshotCtx.set(tabId, { vpW, vpH, shotW, shotH, offX: x0, offY: y0, regionW: w, regionH: h });
  return { base64, x0, y0, x1, y1, clamped };
}

// --- Input helpers ---
function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}
// --- Visual indicator (best-effort; the content script is absent on chrome:// and similar pages) ---
function sendToTab(tabId, msg) {
  return presentationBroker.publishEvent(tabId, msg, {
    channel: "effect",
    ttlMs: PRESENTATION_EVENT_TTL_MS,
  });
}
// ADR-0083: fixed, content-free signature events. Starts wait only briefly for exact-document
// acknowledgement so the cue can paint before work begins; presentation failure never fails or
// meaningfully delays the browser operation. Finish/confirm remain ordered broker events but do
// not extend the model-facing response path.
async function startActionSignature(tabId, kind) {
  try {
    return await presentationBroker.publishEvent(
      tabId,
      self.GhostlightActionSignature.message(
        kind,
        self.GhostlightActionSignature.PHASES.START
      ),
      {
        channel: self.GhostlightActionSignature.CHANNEL,
        ttlMs: SIGNATURE_EVENT_TTL_MS,
        deliveryWaitMs: SIGNATURE_DELIVERY_WAIT_MS,
      }
    );
  } catch {
    return { shown: false, reason: "action signature unavailable" };
  }
}
function finishActionSignature(tabId, kind) {
  presentationBroker.publishEvent(
    tabId,
    self.GhostlightActionSignature.message(
      kind,
      self.GhostlightActionSignature.PHASES.FINISH
    ),
    {
      channel: self.GhostlightActionSignature.CHANNEL,
      ttlMs: SIGNATURE_EVENT_TTL_MS,
      waitForDelivery: false,
    }
  );
}
function confirmActionSignature(tabId, kind) {
  presentationBroker.publishEvent(
    tabId,
    self.GhostlightActionSignature.message(
      kind,
      self.GhostlightActionSignature.PHASES.CONFIRM
    ),
    {
      channel: self.GhostlightActionSignature.CHANNEL,
      ttlMs: SIGNATURE_EVENT_TTL_MS,
      waitForDelivery: false,
    }
  );
}
// ADR-0086: find uses its own broker channel. The start cue waits only long enough to make the
// agent's intent visible before DOM work begins. Result events carry only an aggregate count;
// matched text and geometry never leave the isolated page world for presentation.
async function startFindVisual(tabId) {
  try {
    return await presentationBroker.publishEvent(
      tabId,
      self.GhostlightFindVisual.message(self.GhostlightFindVisual.PHASES.START),
      {
        channel: self.GhostlightFindVisual.CHANNEL,
        ttlMs: FIND_EVENT_TTL_MS,
        deliveryWaitMs: FIND_DELIVERY_WAIT_MS,
      }
    );
  } catch {
    return { shown: false, reason: "find visual unavailable" };
  }
}
function finishFindVisual(tabId, count, more) {
  const phase = count > 0
    ? self.GhostlightFindVisual.PHASES.FOUND
    : self.GhostlightFindVisual.PHASES.EMPTY;
  presentationBroker.publishEvent(
    tabId,
    self.GhostlightFindVisual.message(phase, count, more),
    {
      channel: self.GhostlightFindVisual.CHANNEL,
      ttlMs: FIND_EVENT_TTL_MS,
      waitForDelivery: false,
    }
  );
}
function cancelFindVisual(tabId) {
  presentationBroker.publishEvent(
    tabId,
    self.GhostlightFindVisual.message(self.GhostlightFindVisual.PHASES.CANCEL),
    {
      channel: self.GhostlightFindVisual.CHANNEL,
      ttlMs: FIND_EVENT_TTL_MS,
      waitForDelivery: false,
    }
  );
}
// Move the phantom cursor to a (rescaled, CSS-px) point and wait for it to settle, so the user sees
// the pointer arrive before the action fires. Resolves immediately if no indicator is present.
function moveCursor(tabId, x, y) { return sendToTab(tabId, { type: "UPDATE_PHANTOM_CURSOR", x, y }); }
// Emit a click ripple: one expanding ring per click, so a double-click pings twice and a
// triple-click three times. Fire-and-forget (visual only).
function clickRipple(tabId, x, y, count, button) { sendToTab(tabId, { type: "AGENT_CLICK_RIPPLE", x, y, count, button }); }
// A comet-trail dot along a drag path, and a soft shimmer on the focused field when typing.
function dragTrail(tabId, x, y) { sendToTab(tabId, { type: "AGENT_DRAG_TRAIL", x, y }); }
function typeShimmer(tabId) { sendToTab(tabId, { type: "AGENT_TYPE_SHIMMER" }); }
// Extended vocabulary (the visual feedback dictionary): one treatment per action, all rendered by
// agent-visual-indicator.js and all hidden from the agent's own screenshots.
function targetGlow(tabId, x, y) { sendToTab(tabId, { type: "AGENT_TARGET_GLOW", x, y }); }
function semanticTargetCue(tabId, x, y, action) {
  return sendToTab(tabId, { type: "AGENT_SEMANTIC_TARGET", x, y, action });
}
function keystrokeCue(tabId, cue) {
  sendToTab(tabId, { type: "AGENT_KEYSTROKE", cue });
}
async function beginKeyCueObservation(tabId, expectedCount) {
  try {
    const response = await content(tabId, {
      type: KEY_CUE_OBSERVATION_MESSAGES.BEGIN,
      expectedCount,
    });
    return response && response.result && response.result.token;
  } catch {
    return null;
  }
}
async function finishKeyCueObservation(tabId, token) {
  if (token === null || token === undefined) return [];
  try {
    const response = await content(tabId, { type: KEY_CUE_OBSERVATION_MESSAGES.FINISH, token });
    const result = response && response.result;
    if (!result || result.overflow || !Array.isArray(result.targetStates)) return [];
    return result.targetStates;
  } catch {
    return [];
  }
}
async function beginDragObservation(tabId) {
  try {
    const response = await content(tabId, { type: DRAG_OBSERVATION_MESSAGES.BEGIN });
    return response && response.result && response.result.token;
  } catch {
    return null;
  }
}
async function finishDragObservation(tabId, token) {
  if (token === null || token === undefined) return { started: false, cancelled: false };
  try {
    const response = await content(tabId, { type: DRAG_OBSERVATION_MESSAGES.FINISH, token });
    const result = response && response.result;
    if (!result || typeof result.started !== "boolean" || typeof result.cancelled !== "boolean") {
      return { started: false, cancelled: false };
    }
    return result;
  } catch {
    return { started: false, cancelled: false };
  }
}
function scrollCue(tabId, direction) { sendToTab(tabId, { type: "AGENT_SCROLL_CUE", direction }); }
function readScan(tabId) { sendToTab(tabId, { type: "AGENT_READ_SCAN" }); }
function navigatePill(tabId, url) { sendToTab(tabId, { type: "AGENT_NAVIGATE_PILL", url }); }
function screenshotFx(tabId) { sendToTab(tabId, { type: "AGENT_SCREENSHOT_FX" }); }
function zoomFrameCue(tabId, x0, y0, x1, y1) { sendToTab(tabId, { type: "AGENT_ZOOM_FRAME", x0, y0, x1, y1 }); }
// ADR-0072/0081: publish one active narration state. The broker retains the absolute deadline,
// filters expired restore/replay state, and reports exact current-document acknowledgement.
function renderNarration(tabId, narration) {
  const durationMs = Math.max(1, Math.round(narration.durationMs));
  return presentationBroker.publishState(tabId, "narration", {
    type: "AGENT_NARRATION",
    text: narration.text,
    position: narration.position,
    durationMs,
  }, {
    deadline: narration.deadline,
    clearMessage: { type: "AGENT_NARRATION_CLEAR" },
  });
}
const { modifierBits, KEY_CUE_OBSERVATION_MESSAGES, keyCuePresentation, textDispatchPlan, keyDispatchPlan } = self.GhostlightKeys;
const { BUTTON_BITS, mouseMoveEvent, mouseButtonEvent, mouseWheelEvent, dragEvent } = self.GhostlightInputEvents;
const { DRAG_SESSION_PHASES, DRAG_OBSERVATION_MESSAGES } = self.GhostlightDragSession;
// CLICK_GAP_MS (press/release + inter-click spacing) comes from lib/constants.js.
async function click(tabId, x, y, opts) {
  const modifiers = opts.modifiers || 0, button = opts.button || "left", clickCount = opts.clickCount || 1;
  await cdp(tabId, "Input.dispatchMouseEvent", mouseMoveEvent(x, y, modifiers));
  clickRipple(tabId, x, y, clickCount, button);
  targetGlow(tabId, x, y); // glow the element under the point -- confirm WHAT was acted on
  await sleep(CLICK_GAP_MS);
  // Real N-clicks are N press/release pairs with clickCount incrementing 1..N, not one pair with
  // clickCount set to N.
  for (let i = 1; i <= clickCount; i++) {
    await cdp(tabId, "Input.dispatchMouseEvent", mouseButtonEvent("mousePressed", x, y, button, modifiers, i));
    await sleep(CLICK_GAP_MS);
    await cdp(tabId, "Input.dispatchMouseEvent", mouseButtonEvent("mouseReleased", x, y, button, modifiers, i));
    if (i < clickCount) await sleep(CLICK_GAP_MS);
  }
}
async function resolveCoords(tabId, args) {
  // Model-provided coordinates are read off the (downscaled) screenshot -> rescale to CSS px.
  if (args.coordinate) return rescaleCoord(tabId, args.coordinate[0], args.coordinate[1]);
  // ref coordinates come from getBoundingClientRect (already CSS viewport px) -> do NOT rescale.
  if (args.ref) {
    const r = await content(tabId, { type: "refCoordinates", ref: args.ref });
    if (r && r.result && !r.result.error) return [r.result.x, r.result.y];
    // The engine is truthful: a stale ref is a failure, never a silent [0, 0] substitution. A
    // stale-ref corrective message (render serial moved) is surfaced verbatim; a plain miss keeps
    // the generic wording.
    if (r && r.result && r.result.error) throw hopError("page", r.result.error);
    throw hopError("page", `Element ${args.ref} not found; the page may have changed since it was read`);
  }
  return null;
}
// Scrollable-ancestor predicate shared by probeScrollState and directScrollFallback: an element
// counts as scrollable when its computed overflow allows scrolling AND its content overflows.
const SCROLLABLE_FINDER_SNIPPET = `
function findScrollable(px, py) {
  let el = document.elementFromPoint(px, py);
  while (el) {
    const cs = getComputedStyle(el);
    const overflowScrollable = cs.overflowY === "auto" || cs.overflowY === "scroll" || cs.overflowX === "auto" || cs.overflowX === "scroll";
    const sizeScrollable = el.scrollHeight > el.clientHeight || el.scrollWidth > el.clientWidth;
    if (overflowScrollable && sizeScrollable) return el;
    el = el.parentElement;
  }
  return null;
}`;
// Reads the window scroll position plus the scrollable-ancestor state at (x, y), for a before/
// after comparison around a wheel dispatch. Resolves to null (never throws) on any failure.
async function probeScrollState(tabId, x, y) {
  const px = Math.round(x), py = Math.round(y);
  const expression = `(() => {${SCROLLABLE_FINDER_SNIPPET}
    const el = findScrollable(${px}, ${py});
    return {
      winX: window.scrollX, winY: window.scrollY,
      hasEl: !!el,
      elX: el ? el.scrollLeft : null,
      elY: el ? el.scrollTop : null,
    };
  })()`;
  try {
    const r = await cdp(tabId, "Runtime.evaluate", { expression, returnByValue: true });
    if (!r || r.exceptionDetails || !r.result || r.result.value === undefined) return null;
    return r.result.value;
  } catch {
    return null;
  }
}
// Direct scrollBy on the nearest scrollable ancestor (or window), used when a dispatched wheel
// event did not move anything (preventDefault, virtualized lists, etc). Resolves to null (never
// throws) on any failure. dx/dy must be the same deltaX/deltaY already computed for the wheel.
async function directScrollFallback(tabId, x, y, dx, dy) {
  const px = Math.round(x), py = Math.round(y), pdx = Math.round(dx), pdy = Math.round(dy);
  const expression = `(() => {${SCROLLABLE_FINDER_SNIPPET}
    const el = findScrollable(${px}, ${py});
    const target = el || window;
    const beforeX = el ? el.scrollLeft : window.scrollX;
    const beforeY = el ? el.scrollTop : window.scrollY;
    target.scrollBy({ left: ${pdx}, top: ${pdy}, behavior: "instant" });
    const afterX = el ? el.scrollLeft : window.scrollX;
    const afterY = el ? el.scrollTop : window.scrollY;
    // 5px threshold matches the moved-more-than-5px verification contract.
    return { moved: Math.abs(afterX - beforeX) > 5 || Math.abs(afterY - beforeY) > 5, usedWindow: !el };
  })()`;
  try {
    const r = await cdp(tabId, "Runtime.evaluate", { expression, returnByValue: true });
    if (!r || r.exceptionDetails || !r.result || r.result.value === undefined) return null;
    return r.result.value;
  } catch {
    return null;
  }
}
async function pressKey(tabId, combo) {
  const plan = keyDispatchPlan(combo);
  if (plan.reload) {
    await chrome.tabs.reload(tabId, plan.reload);
    return;
  }
  await cdp(tabId, "Input.dispatchKeyEvent", plan.keyDown);
  await cdp(tabId, "Input.dispatchKeyEvent", plan.keyUp);
  await sleep(20);
}
function dragInterceptUnsupported(error) {
  return /unknown method|wasn't found|not supported/i.test(String(error && error.message || error));
}
async function cancelActiveDrag(tabId) {
  dragCoordinator.cancel(tabId);
  const operation = activeDragOperations.get(tabId);
  if (!operation) return;
  operation.cancelled = true;
  activeDragOperations.delete(tabId);

  // Navigation destroys the old document and its structural observer. Retire CDP input state
  // directly, without the content() reinjection fallback that could install an observer in the
  // replacement document.
  const interceptEnabled = operation.interceptEnabled;
  const pressed = operation.pressed;
  operation.interceptEnabled = false;
  operation.pressed = false;
  if (!attached.has(tabId)) return;
  if (interceptEnabled) {
    try {
      await chrome.debugger.sendCommand({ tabId }, "Input.setInterceptDrags", { enabled: false });
    } catch { /* navigation or detach already retired interception */ }
  }
  try {
    await chrome.debugger.sendCommand({ tabId }, "Input.cancelDragging", {});
  } catch { /* no active native drag */ }
  if (pressed) {
    try {
      await chrome.debugger.sendCommand(
        { tabId },
        "Input.dispatchMouseEvent",
        mouseButtonEvent("mouseReleased", operation.x, operation.y, "left", operation.modifiers, 1)
      );
    } catch { /* navigation or detach already released the pointer */ }
  }
}
async function pointerDrag(tabId, sx, sy, ex, ey, modifiers) {
  let observationToken = await beginDragObservation(tabId);
  let interceptSession = dragCoordinator.begin(tabId);
  const operation = {
    cancelled: false,
    interceptEnabled: false,
    pressed: false,
    x: sx,
    y: sy,
    modifiers,
  };
  activeDragOperations.set(tabId, operation);
  try {
    try {
      await cdp(tabId, "Input.setInterceptDrags", { enabled: true });
      operation.interceptEnabled = true;
    } catch (error) {
      dragCoordinator.cancel(tabId);
      interceptSession = null;
      if (!dragInterceptUnsupported(error)) throw error;
    }

    await cdp(tabId, "Input.dispatchMouseEvent", mouseMoveEvent(sx, sy, modifiers));
    await sleep(CAPTURE_SETTLE_MS);
    await cdp(tabId, "Input.dispatchMouseEvent", mouseButtonEvent("mousePressed", sx, sy, "left", modifiers, 1));
    operation.pressed = true;
    await sleep(CAPTURE_SETTLE_MS);
    for (let i = 1; i <= 10; i++) {
      if (operation.cancelled) throw new Error("Drag cancelled because the tab document changed.");
      const tx = sx + ((ex - sx) * i) / 10;
      const ty = sy + ((ey - sy) * i) / 10;
      operation.x = tx;
      operation.y = ty;
      await cdp(tabId, "Input.dispatchMouseEvent", mouseMoveEvent(tx, ty, modifiers, BUTTON_BITS.left));
      dragTrail(tabId, tx, ty);
      await sleep(16);
    }

    const observation = await finishDragObservation(tabId, observationToken);
    observationToken = null;
    const nativeExpected = observation.started && !observation.cancelled;
    const interceptResult = interceptSession
      ? await dragCoordinator.finish(
        interceptSession,
        nativeExpected ? DRAG_INTERCEPT_WAIT_MS : DRAG_INTERCEPT_GRACE_MS
      )
      : { mode: DRAG_SESSION_PHASES.POINTER };
    interceptSession = null;
    if (operation.cancelled) throw new Error("Drag cancelled because the tab document changed.");
    if (operation.interceptEnabled) {
      await cdp(tabId, "Input.setInterceptDrags", { enabled: false });
      operation.interceptEnabled = false;
    }
    if (interceptResult.mode === DRAG_SESSION_PHASES.NATIVE) {
      await cdp(tabId, "Input.dispatchDragEvent", dragEvent("dragEnter", ex, ey, interceptResult.data, modifiers));
      await cdp(tabId, "Input.dispatchDragEvent", dragEvent("dragOver", ex, ey, interceptResult.data, modifiers));
      await cdp(tabId, "Input.dispatchDragEvent", dragEvent("drop", ex, ey, interceptResult.data, modifiers));
    }
    await cdp(tabId, "Input.dispatchMouseEvent", mouseButtonEvent("mouseReleased", ex, ey, "left", modifiers, 1));
    operation.pressed = false;
    if (activeDragOperations.get(tabId) === operation) activeDragOperations.delete(tabId);
    return { nativeDrag: interceptResult.mode === DRAG_SESSION_PHASES.NATIVE };
  } catch (error) {
    dragCoordinator.cancel(tabId);
    if (!operation.cancelled) await finishDragObservation(tabId, observationToken);
    observationToken = null;
    if (operation.interceptEnabled) {
      try { await cdp(tabId, "Input.setInterceptDrags", { enabled: false }); } catch { /* detached */ }
      operation.interceptEnabled = false;
    }
    try { await cdp(tabId, "Input.cancelDragging"); } catch { /* no active native drag */ }
    if (operation.pressed) {
      try {
        await cdp(tabId, "Input.dispatchMouseEvent", mouseButtonEvent("mouseReleased", operation.x, operation.y, "left", modifiers, 1));
      } catch { /* detached */ }
      operation.pressed = false;
    }
    if (activeDragOperations.get(tabId) === operation) activeDragOperations.delete(tabId);
    throw error;
  }
}
function waitForLoad(tabId) {
  return new Promise((resolve) => {
    const listener = (id, info) => {
      if (id === tabId && info.status === "complete") {
        chrome.tabs.onUpdated.removeListener(listener);
        resolve();
      }
    };
    chrome.tabs.onUpdated.addListener(listener);
    setTimeout(() => { chrome.tabs.onUpdated.removeListener(listener); resolve(); }, NAV_SETTLE_TIMEOUT_MS);
  });
}

// --- computer (13 actions; screenshots only on screenshot/scroll/zoom) ---
async function computer(a) {
  const tabId = await effectiveTabId(a.tabId);
  const modifiers = modifierBits(a.modifiers);
  switch (a.action) {
    case "screenshot": {
      const caption = "Screenshot captured (jpeg).";
      const shot = await screenshot(tabId);
      screenshotFx(tabId); // shutter flash + viewfinder, AFTER the capture (never in the image)
      confirmActionSignature(tabId, self.GhostlightActionSignature.KINDS.SCREENSHOT);
      return textImage(shot.note ? caption + " " + shot.note : caption, shot.base64);
    }
    case "zoom": {
      const r = a.region;
      if (!Array.isArray(r) || r.length !== 4 || !r.every((v) => Number.isFinite(v)))
        return text("region [x0, y0, x1, y1] is required for zoom.");
      if (!(r[2] > r[0]) || !(r[3] > r[1]))
        return text("zoom region is empty: x1 must be greater than x0 and y1 must be greater than y0.");
      const z = await zoomScreenshot(tabId, r);
      if (z.error) return text(z.error);
      zoomFrameCue(tabId, z.x0, z.y0, z.x1, z.y1); // magnifier frame on the region, AFTER the capture
      return textImage(`Zoom region (${z.x0}, ${z.y0}) -> (${z.x1}, ${z.y1}) captured (jpeg${z.clamped ? "; clamped to the visible viewport" : ""}).`, z.base64);
    }
    case "wait": {
      const s = Math.min(a.duration || 1, 30);
      await startActionSignature(tabId, self.GhostlightActionSignature.KINDS.WAIT);
      try {
        await sleep(s * 1000);
      } finally {
        finishActionSignature(tabId, self.GhostlightActionSignature.KINDS.WAIT);
      }
      return text(`Waited ${s}s.`);
    }
    case "left_click":
    case "right_click":
    case "double_click":
    case "triple_click":
    case "hover": {
      return withObservation(tabId, {
        action: a.action,
        ref: a.ref,
        targetAssurance: a.ref ? "ref" : "coordinate",
      }, async () => {
        // Keep every page-dependent preparation behind the dialog guard. Resolving a ref or
        // moving the visible cursor can otherwise wait forever on a page blocked by a modal.
        const c = await resolveCoords(tabId, a);
        if (!c) return text("coordinate or ref is required.");
        await moveCursor(tabId, c[0], c[1]); // show the pointer arrive before acting
        if (a.action === "hover") {
          await cdp(tabId, "Input.dispatchMouseEvent", mouseMoveEvent(c[0], c[1], modifiers));
          return text(`Hovered at (${c[0]}, ${c[1]}).`);
        }
        const button = a.action === "right_click" ? "right" : "left";
        const clickCount = a.action === "double_click" ? 2 : a.action === "triple_click" ? 3 : 1;
        await click(tabId, c[0], c[1], { button, clickCount, modifiers });
        return text(`${a.action} at (${c[0]}, ${c[1]}).`);
      });
    }
    case "type": {
      if (!a.text) return text("text is required for type.");
      return withObservation(tabId, { action: "type", targetAssurance: "none" }, async () => {
        await ensureAttached(tabId);
        typeShimmer(tabId);
        await startActionSignature(tabId, self.GhostlightActionSignature.KINDS.TYPING);
        const dispatch = textDispatchPlan(a.text);
        try {
          for (const operation of dispatch.operations) {
            await cdp(tabId, operation.method, operation.params);
            await sleep(8);
          }
        } finally {
          finishActionSignature(tabId, self.GhostlightActionSignature.KINDS.TYPING);
        }
        return text(`Typed ${dispatch.characterCount} character(s).`);
      });
    }
    case "key": {
      if (!a.text) return text("text is required for key.");
      return withObservation(tabId, { action: "key", targetAssurance: "none" }, async () => {
        await ensureAttached(tabId);
        const repeat = Math.min(a.repeat || 1, 100);
        const combos = a.text.split(" ").filter(Boolean);
        const observationToken = await beginKeyCueObservation(tabId, combos.length * repeat);
        let observedTargets = [];
        try {
          for (let i = 0; i < repeat; i++) {
            for (const combo of combos) await pressKey(tabId, combo);
          }
        } finally {
          observedTargets = await finishKeyCueObservation(tabId, observationToken);
          keystrokeCue(tabId, keyCuePresentation(a.text, observedTargets, repeat));
        }
        return text(`Pressed: ${a.text} (x${repeat}).`);
      });
    }
    case "scroll": {
      return withObservation(tabId, {
        action: "scroll",
        ref: a.ref,
        targetAssurance: a.ref ? "ref" : "coordinate",
      }, async () => {
        const c = (await resolveCoords(tabId, a)) || [0, 0];
        const dir = a.scroll_direction || "down";
        const amount = Math.min(a.scroll_amount || 3, 10);
        const deltaX = dir === "left" ? -amount * 100 : dir === "right" ? amount * 100 : 0;
        const deltaY = dir === "up" ? -amount * 100 : dir === "down" ? amount * 100 : 0;
        const before = await probeScrollState(tabId, c[0], c[1]);
        await moveCursor(tabId, c[0], c[1]);
        scrollCue(tabId, dir);
        await cdp(tabId, "Input.dispatchMouseEvent", mouseWheelEvent(c[0], c[1], deltaX, deltaY, modifiers));
        const scrolled = `Scrolled ${dir} by ${amount}.`;
        if (before === null) {
          // Verification unavailable (for example a mid-navigation page): same blind claim as before.
          await sleep(250);
          const shot = await screenshot(tabId);
          return textImage(shot.note ? scrolled + " " + shot.note : scrolled, shot.base64);
        }
        await sleep(200);
        const after = await probeScrollState(tabId, c[0], c[1]);
        // Re-read failed: do not run the fallback, a blind fallback risks double-scrolling.
        if (after === null) {
          const shot = await screenshot(tabId);
          return textImage(shot.note ? scrolled + " " + shot.note : scrolled, shot.base64);
        }
        // 5px threshold matches the moved-more-than-5px verification contract.
        const windowMoved = Math.abs(after.winX - before.winX) > 5 || Math.abs(after.winY - before.winY) > 5;
        const elementMoved = before.hasEl && after.hasEl &&
          (Math.abs((after.elX || 0) - (before.elX || 0)) > 5 || Math.abs((after.elY || 0) - (before.elY || 0)) > 5);
        if (windowMoved || elementMoved) {
          const shot = await screenshot(tabId);
          return textImage(shot.note ? scrolled + " " + shot.note : scrolled, shot.base64);
        }
        const fb = await directScrollFallback(tabId, c[0], c[1], deltaX, deltaY);
        if (fb === null) {
          const caption = `Scroll ${dir} had no effect at (${c[0]}, ${c[1]}); the direct scroll fallback could not run.`;
          const shot = await screenshot(tabId);
          return textImage(shot.note ? caption + " " + shot.note : caption, shot.base64);
        }
        if (fb.moved) {
          const caption = `Scrolled ${dir} by ${amount} (mouse wheel had no effect; used direct scroll fallback).`;
          const shot = await screenshot(tabId);
          return textImage(shot.note ? caption + " " + shot.note : caption, shot.base64);
        }
        const caption = `Scroll ${dir} had no effect at (${c[0]}, ${c[1]}); the page did not move at that position.`;
        const shot = await screenshot(tabId);
        return textImage(shot.note ? caption + " " + shot.note : caption, shot.base64);
      });
    }
    case "scroll_to": {
      if (!a.ref && !a.coordinate) return text("ref or coordinate is required for scroll_to.");
      return withObservation(tabId, {
        action: "scroll_to",
        ref: a.ref,
        targetAssurance: a.ref ? "ref" : "coordinate",
      }, async () => {
        if (a.ref) {
          const r = await content(tabId, { type: "scrollToRef", ref: a.ref });
          // The engine is truthful: a stale ref is a failure, never a false "Scrolled to target.".
          // A stale-ref corrective message (render serial moved) is surfaced verbatim.
          if (r && r.result && r.result.error) throw hopError("page", r.result.error);
          if (!(r && r.result === true)) {
            throw hopError("page", `Element ${a.ref} not found; the page may have changed since it was read`);
          }
        } else if (a.coordinate) {
          await cdp(tabId, "Runtime.evaluate", { expression: `window.scrollTo(${a.coordinate[0]}, ${a.coordinate[1]})` });
        }
        await sleep(250);
        return text("Scrolled to target.");
      });
    }
    case "left_click_drag": {
      if (!a.start_coordinate || !a.coordinate) return text("start_coordinate and coordinate are required.");
      // Both endpoints are model-provided (read off the screenshot) -> rescale to CSS px.
      const [sx, sy] = rescaleCoord(tabId, a.start_coordinate[0], a.start_coordinate[1]);
      const [ex, ey] = rescaleCoord(tabId, a.coordinate[0], a.coordinate[1]);
      return withObservation(tabId, { action: "left_click_drag", targetAssurance: "coordinate" }, async () => {
        await moveCursor(tabId, sx, sy);
        await pointerDrag(tabId, sx, sy, ex, ey, modifiers);
        await moveCursor(tabId, ex, ey);
        return text(`Dragged (${sx}, ${sy}) -> (${ex}, ${ey}).`);
      });
    }
    default:
      return text(`Unknown computer action: ${a.action}`);
  }
}

// --- Tool handlers ---
// Pre-0047 tabs_create_mcp behavior, kept verbatim for guid-less legacy/native callers
// (ADR-0047 D3): global-group birth via ensureGroup(true).
async function tabsCreateLegacy() {
  await ensureGroup(true);
  const tab = await chrome.tabs.create({ active: true });
  await chrome.tabs.group({ tabIds: [tab.id], groupId });
  markTabManaged(tab.id); // ADR-0066 D5
  await persistSessionState();
  const r = tabContext(await groupTabs());
  r.content[0].text = `Created tab ${tab.id}.\n` + r.content[0].text;
  r.structuredContent = { tabId: tab.id, tabs: r.structuredContent.tabs };
  return r;
}

// Pre-0047 tabs_context_mcp behavior, kept verbatim for guid-less legacy/native callers
// (ADR-0047 D3): the global group's view.
async function tabsContextLegacy(a) {
  await ensureGroup(a.createIfEmpty);
  if (groupId === null) return text(`No ${GROUP_TITLE} tab group. Call with createIfEmpty: true.`);
  return tabContext(await groupTabs());
}

async function pageMeta(tabId) {
  const tab = await chrome.tabs.get(tabId);
  let fromPage = null;
  // A JavaScript dialog blocks the page event loop, so do not await a content-script reply while
  // one is open. Chrome's tab metadata still gives the origin/title needed for provenance.
  if (!dialogStore.current(tabId)) {
    try {
      const response = await content(tabId, { type: "pageMeta" });
      fromPage = response && response.result;
    } catch (_error) {
      // Restricted browser pages may not host the content script. Browser metadata is still useful.
    }
  }
  let origin = "unknown";
  try { origin = new URL((fromPage && fromPage.url) || tab.url || "about:blank").origin; }
  catch (_error) { /* keep unknown */ }
  const meta = {
    tabId,
    url: (fromPage && fromPage.url) || tab.url || "",
    origin,
    title: (fromPage && fromPage.title) || tab.title || "",
  };
  if (fromPage && typeof fromPage.renderSerial === "number") meta.renderSerial = fromPage.renderSerial;
  return meta;
}

const handlers = {
  // `key` is the client's stable clientKey, falling back to the guid. `workspaceRequest` is a
  // private service instruction: either select the most recently focused normal window or use the
  // session's already-pinned window.
  async tabs_context_mcp(a, key, workspaceRequest) {
    if (typeof key !== "string" || !key) return tabsContextLegacy(a);
    const resolved = await resolveWorkspaceWindow(chrome, workspaceRequest);
    let workspaceWindowId = resolved.window.id;
    const group = await workspaceGroupId(key, resolved.window.id);
    let { gid } = group;
    if (group.changed) await persistSessionState();
    if (gid === null) {
      if (!a.createIfEmpty) {
        return withWorkspaceResult(
          text("No Ghostlight tab group in this session workspace. Call tabs_context_mcp with createIfEmpty: true, or create a tab with tabs_create_mcp."),
          resolved.window.id
        );
      }
      const created = await createTabInSessionGroup(key, workspaceRequest, resolved);
      gid = created.gid;
      workspaceWindowId = created.windowId;
      await persistSessionState();
    }
    return withWorkspaceResult(
      tabContext(await chrome.tabs.query({ groupId: gid }), gid),
      workspaceWindowId
    );
  },
  async tabs_create_mcp(_a, key, workspaceRequest) {
    if (typeof key !== "string" || !key) return tabsCreateLegacy();
    const { tab, gid, windowId } = await createTabInSessionGroup(key, workspaceRequest);
    await persistSessionState();
    const r = tabContext(await chrome.tabs.query({ groupId: gid }), gid);
    r.content[0].text = `Created tab ${tab.id}.\n` + r.content[0].text;
    r.structuredContent = { tabId: tab.id, tabs: r.structuredContent.tabs };
    return withWorkspaceResult(r, windowId);
  },
  async navigate(a, key, workspaceRequest) {
    const navigation = await navigateTabId(a.tabId, key, workspaceRequest);
    const { tabId } = navigation;
    if (a.url === "back") {
      await chrome.tabs.goBack(tabId);
    } else if (a.url === "forward") {
      await chrome.tabs.goForward(tabId);
    } else {
      let url = a.url;
      if (!/^https?:\/\//i.test(url) && !/^(about|chrome|edge|brave):/i.test(url)) {
        url = "https://" + url.replace(/^[a-z]{1,6}:\/+/i, "");
      }
      try { new URL(url); } catch { return text(`Invalid URL: "${a.url}".`); }
      await chrome.tabs.update(tabId, { url });
    }
    await waitForLoad(tabId);
    const tab = await chrome.tabs.get(tabId);
    navigatePill(tabId, tab.url); // destination pill on the freshly loaded page
    const r = text(`Navigated to ${tab.url}${tab.status !== "complete" ? " (still loading)" : ""}.`);
    r.structuredContent = { tabId, url: tab.url, title: tab.title || "" };
    return navigation.windowId === null ? r : withWorkspaceResult(r, navigation.windowId);
  },
  async dialog(a) {
    const tabId = await effectiveTabId(a.tabId);
    await ensureAttached(tabId);
    await enableDomain(tabId, "Page");
    await sleep(0);
    const open = dialogStore.current(tabId);
    if (a.action === "status") {
      const out = open
        ? text(`JavaScript ${open.type} dialog is blocking the tab: ${JSON.stringify(open.message)}.`)
        : text("No JavaScript dialog is currently blocking the tab.");
      out.structuredContent = open
        ? { open: true, type: open.type, message: open.message, page: await pageMeta(tabId) }
        : { open: false, page: await pageMeta(tabId) };
      return out;
    }
    if (!open) {
      const out = text("No JavaScript dialog is currently open; nothing was resolved.");
      out.structuredContent = { open: false, resolved: false, page: await pageMeta(tabId) };
      return out;
    }
    const params = self.GhostlightDialog.resolutionCommand(a.action, a.text);
    await cdp(tabId, "Page.handleJavaScriptDialog", params);
    dialogStore.remove(tabId);
    const out = text(`JavaScript dialog ${a.action} dispatched.`);
    out.structuredContent = {
      open: false,
      resolved: true,
      action: a.action,
      type: open.type,
      page: await pageMeta(tabId),
    };
    return out;
  },
  async tab_control(a) {
    const tabId = await effectiveTabId(a.tabId);
    const page = { tabId };
    if (a.action === "focus") {
      const tab = await chrome.tabs.get(tabId);
      await chrome.windows.update(tab.windowId, { focused: true });
      await chrome.tabs.update(tabId, { active: true });
    } else if (a.action === "reload") {
      await chrome.tabs.reload(tabId);
      await waitForLoad(tabId);
    } else if (a.action === "close") {
      await chrome.tabs.remove(tabId);
      clearTabState(tabId);
    } else {
      throw hopError("extension", `unsupported tab_control action: ${a.action}`);
    }
    const labels = {
      focus: "Tab focus observed.",
      reload: "Tab reload observed.",
      close: "Tab close observed.",
    };
    const out = text(labels[a.action]);
    out.structuredContent = {
      interactionReceipt: self.GhostlightTabControl.makeReceipt(a.action, page),
    };
    return out;
  },
  async computer(a) {
    const out = await computer(a);
    if (!out.structuredContent) out.structuredContent = {};
    return out;
  },
  async read_page(a) {
    const tabId = await effectiveTabId(a.tabId);
    readScan(tabId);
    const r = await content(tabId, { type: "accessibilityTree", options: a });
    const out = text((r && r.result) || "Could not read the page.");
    out.structuredContent = { page: await pageMeta(tabId) };
    if (a.ref_id) {
      const target = await content(tabId, { type: "elementSummary", ref: a.ref_id });
      if (target && target.result && !target.result.error) out.structuredContent.target = target.result;
    }
    return out;
  },
  async get_page_text(a) {
    const tabId = await effectiveTabId(a.tabId);
    readScan(tabId);
    const r = await content(tabId, { type: "pageText", max_chars: a.max_chars });
    const out = text((r && r.result) || "Could not extract page text.");
    out.structuredContent = { page: await pageMeta(tabId) };
    return out;
  },
  async find(a) {
    const tabId = await effectiveTabId(a.tabId);
    await startFindVisual(tabId);
    let r;
    try {
      r = await content(tabId, { type: "find", query: a.query, present: true });
    } catch (error) {
      cancelFindVisual(tabId);
      throw error;
    }
    const data = (r && r.result) || { results: [] };
    const results = data.results || [];
    const more = !!data.more;
    finishFindVisual(tabId, results.length, more);
    let out;
    if (!results.length) {
      out = text(`No elements matching "${a.query}".`);
    } else {
      let s = `Found ${results.length} element(s), strongest matches first:\n` + results.map((e) => {
        const state = [e.visible ? "visible" : "hidden", e.enabled ? "enabled" : "disabled"];
        if (typeof e.checked === "boolean") state.push(e.checked ? "checked" : "not checked");
        if (typeof e.selected === "boolean") state.push(e.selected ? "selected" : "not selected");
        return `[${e.ref}] ${e.role} "${e.name}" at (${e.x}, ${e.y}); ${state.join(", ")}; actions: ${(e.mechanicalActions || []).join(", ") || "none"}`;
      }).join("\n");
      if (more) s += "\n(more than 20 matches; refine your query for the rest)";
      out = text(s);
    }
    out.structuredContent = { results, more, page: await pageMeta(tabId) };
    return out;
  },
  async form_input(a) {
    const tabId = await effectiveTabId(a.tabId);
    return withObservation(tabId, { action: "set_value", ref: a.ref, targetAssurance: "ref" }, async () => {
      const r = await content(tabId, { type: "setFormValue", ref: a.ref, value: a.value });
      // The engine is truthful: a content-script failure is a failure, never a masqueraded success.
      if (r && r.result && r.result.error) {
        const msg = r.result.error.endsWith(".") ? r.result.error.slice(0, -1) : r.result.error;
        throw hopError("page", msg);
      }
      return text(`Set ${a.ref} = ${JSON.stringify(a.value)}.`);
    });
  },
  async file_upload(a) {
    const tabId = await effectiveTabId(a.tabId);
    if (!a.files || a.files.length === 0) {
      if (a.paths && a.paths.length > 0) {
        throw hopError("binary", "file_upload no longer accepts host filesystem paths. The MCP controller must read the file and pass its contents via the `files` parameter.");
      }
      throw hopError("binary", "files parameter is required and must be a non-empty array");
    }
    return withObservation(tabId, { action: "file_upload", ref: a.ref, targetAssurance: "ref" }, async () => {
      const r = await content(tabId, { type: "setFiles", ref: a.ref, files: a.files });
      if (r && r.result && r.result.error) {
        const msg = r.result.error.endsWith(".") ? r.result.error.slice(0, -1) : r.result.error;
        throw hopError("page", msg);
      }
      return text(r.result.output);
    });
  },
  // upload_image (ADR-0050 Decision 4): place a previously captured screenshot -- the binary
  // resolves it from its per-session cache and passes the base64 `data`/`mimeType` here (never a
  // host path) -- into a file input (ref) or a drag-drop target (coordinate). Not an advertised
  // tool; dispatched by the binary's upload_image_handler. Mirrors file_upload.
  async upload_image_exec(a) {
    const tabId = await effectiveTabId(a.tabId);
    if (!a.data) {
      throw hopError("binary", "upload_image_exec requires base64 image data");
    }
    return withObservation(tabId, {
      action: "upload_image",
      ref: a.ref,
      targetAssurance: a.ref ? "ref" : "coordinate",
    }, async () => {
      const r = await content(tabId, {
        type: "setImage",
        ref: a.ref,
        coordinate: a.coordinate,
        data: a.data,
        filename: a.filename,
        mimeType: a.mimeType,
      });
      if (r && r.result && r.result.error) {
        const msg = r.result.error.endsWith(".") ? r.result.error.slice(0, -1) : r.result.error;
        throw hopError("page", msg);
      }
      return text(r.result.output);
    });
  },
  // gif_creator capture relay (ADR-0053 D2/D6): internal ops the binary's orchestrator dials.
  // NOT in the tool REGISTRY, so models cannot call them. The worker holds no recording state:
  // the seed frame and every kept screencast frame flow to the binary as gif_frame events.
  async gif_capture_start(a) {
    const tabId = await effectiveTabId(a.tabId);
    if (!a.recordingId || !Number.isSafeInteger(a.generation)) {
      throw hopError("binary", "gif_capture_start requires recording identity");
    }
    const cast = {
      recordingId: a.recordingId,
      generation: a.generation,
      nextSequence: 0,
      minIntervalMs: a.minIntervalMs || 200,
      lastSentTs: 0,
      leaseDeadline: Date.now() + boundedGifTimeout(a.leaseMs, 15000),
      hardDeadline: Date.now() + boundedGifTimeout(a.hardTimeoutMs, 120000),
      expiryTimer: null,
    };
    gifCast.set(tabId, cast);
    armGifExpiry(tabId, cast);
    let seeded = 0;
    let vpW = null;
    try {
      const shot = await screenshot(tabId);
      const sctx = screenshotCtx.get(tabId);
      vpW = (sctx && sctx.vpW) || null;
      sendGifFrame(tabId, cast, shot.base64, vpW, false);
      cast.lastSentTs = Date.now();
      seeded = 1;
    } catch (e) {
      /* the seed is best-effort; the screencast still starts */
    }
    try {
      if (gifCast.get(tabId) !== cast || Date.now() >= cast.leaseDeadline ||
          Date.now() >= cast.hardDeadline) {
        throw hopError("extension", "capture lease expired during start");
      }
      await ensureAttached(tabId);
      await enableDomain(tabId, "Page");
      // Change-driven capture (ADR-0052 D1): the compositor emits a frame only when the page
      // visually changes, downscaled at the source to the service-chosen cap.
      await cdp(tabId, "Page.startScreencast", {
        format: "jpeg",
        quality: a.quality || 70,
        maxWidth: a.maxSide || MAX_SIDE,
        maxHeight: a.maxSide || MAX_SIDE,
      });
      if (gifCast.get(tabId) !== cast) {
        chrome.debugger.sendCommand({ tabId }, "Page.stopScreencast", {}).catch(() => {});
        throw hopError("extension", "capture lease expired during start");
      }
      refreshActionBadge();
    } catch (e) {
      if (gifCast.get(tabId) === cast) {
        if (cast.expiryTimer) clearTimeout(cast.expiryTimer);
        gifCast.delete(tabId);
        refreshActionBadge();
      }
      throw e;
    }
    return text(JSON.stringify({ seeded, vpW }));
  },
  async gif_capture_stop(a) {
    const tabId = await effectiveTabId(a.tabId);
    const cast = gifCast.get(tabId);
    if (!cast || cast.recordingId !== a.recordingId || cast.generation !== a.generation) {
      throw hopError("binary", "capture generation is no longer active");
    }
    // Final-data-before-stop barrier (ADR-0073): post the final frame before this handler's tool
    // response. Native-port ordering makes the service receive the frame before the reply.
    try {
      const shot = await screenshot(tabId);
      const sctx = screenshotCtx.get(tabId);
      sendGifFrame(tabId, cast, shot.base64, (sctx && sctx.vpW) || null, true);
    } catch (e) {
      /* final capture is best-effort; the service can preserve an interrupted recording */
    }
    stopGifCast(tabId, cast, null);
    return text("Screencast stopped.");
  },
  // Rescale model-space points (read off the downscaled screenshot) to CSS viewport px against
  // the tab's live ScreenshotContext (ADR-0053 D4): the binary QUERIES this mechanism data where
  // Chrome produces it instead of mirroring it.
  async rescale_coords(a) {
    const tabId = await effectiveTabId(a.tabId);
    const points = (a.points || []).map((p) => rescaleCoord(tabId, p[0], p[1]));
    return text(JSON.stringify({ points }));
  },
  async wait_for(a) {
    // Defaults (ADR-0037 D1/D6): settle ON, state visible, timeout 10s, min 0.
    const state = a.state || "visible";
    const timeout_ms = a.timeout_ms === undefined ? 10000 : a.timeout_ms;
    const min_ms = a.min_ms === undefined ? 0 : a.min_ms;
    const settle = a.settle === undefined ? true : a.settle;
    const tabId = await effectiveTabId(a.tabId);
    // Corrective validation (ADR-0031): the wait shape is taught, not guessed.
    if (a.selector && a.text) {
      throw hopError("page", "provide at most one of selector or text, not both");
    }
    if (state === "settled" && (a.selector || a.text)) {
      throw hopError("page", 'state "settled" waits for the page to go quiet; do not also pass selector or text');
    }
    if (min_ms > timeout_ms) {
      throw hopError("page", `min_ms (${min_ms}) must not exceed timeout_ms (${timeout_ms})`);
    }
    if (timeout_ms > 30000) {
      throw hopError("page", `timeout_ms ${timeout_ms} exceeds the 30000ms cap`);
    }
    const spec = { selector: a.selector || null, text: a.text || null, state, timeout_ms, min_ms, settle };
    await startActionSignature(tabId, self.GhostlightActionSignature.KINDS.WAIT);
    let r;
    try {
      r = await content(tabId, { type: "waitFor", spec });
    } finally {
      finishActionSignature(tabId, self.GhostlightActionSignature.KINDS.WAIT);
    }
    const res = (r && r.result) || {};
    if (res.timeout) {
      // A bare settle wait that never quiets reports the sustained rate; a condition wait names
      // what WAS on the page (title + the closest matched ref, if any) so the model can adjust.
      if (a.selector || a.text) {
        throw hopError("page", `"${a.selector || a.text}" not visible within ${timeout_ms}ms. Page title: "${res.title}".`);
      }
      throw hopError("page", `did not settle within ${timeout_ms}ms (still changing at ~${res.rate} mutations/500ms)`);
    }
    const elapsed = res.elapsedMs;
    const peak = res.peakMutations;
    let s;
    if (a.selector || a.text) {
      s = `Condition met after ${elapsed}ms (settled; peak ${peak} mutations/window).`;
    } else {
      s = `Page settled after ${elapsed}ms (peak ${peak} mutations/window).`;
    }
    const out = text(s);
    const structured = { found: res.found, elapsed_ms: elapsed };
    if (res.ref) structured.ref = res.ref;
    if (settle) {
      structured.settled = res.settled;
      structured.peak_mutations = peak;
      structured.final_rate = res.finalRate;
    }
    structured.page = await pageMeta(tabId);
    out.structuredContent = structured;
    return out;
  },
  async javascript_tool(a) {
    const tabId = await effectiveTabId(a.tabId);
    await startActionSignature(tabId, self.GhostlightActionSignature.KINDS.JAVASCRIPT);
    try {
      let r = await cdp(tabId, "Runtime.evaluate", { expression: a.text, returnByValue: true, awaitPromise: true, replMode: true });
      if (r.exceptionDetails) {
        const ed = r.exceptionDetails.exception;
        const probe = (r.exceptionDetails.text || "") + ((ed && ed.description) || "");
        // A bare top-level "return" is only legal inside a function; retry once wrapped in an
        // async IIFE, which also preserves top-level await for the wrapped code.
        if (probe.includes("Illegal return statement")) {
          const wrapped = "(async () => {\n" + a.text + "\n})()";
          r = await cdp(tabId, "Runtime.evaluate", { expression: wrapped, returnByValue: true, awaitPromise: true });
        }
      }
      if (r.exceptionDetails) {
        // r.exceptionDetails.text is CDP's generic top-level label (almost always the bare string
        // "Uncaught"); the actual message lives on the exception object's own description.
        const ed = r.exceptionDetails.exception;
        const msg = (ed && ed.description) || r.exceptionDetails.text || "exception";
        const result = text(`Error: ${msg}`);
        result.structuredContent = { page: await pageMeta(tabId) };
        return result;
      }
      const v = r.result;
      let out = v.value !== undefined ? JSON.stringify(v.value) : (v.description || String(v.type));
      if (out.length > 50 * 1024) out = out.slice(0, 50 * 1024) + "\n[OUTPUT TRUNCATED: Exceeded 50KB limit]";
      const result = text(out);
      result.structuredContent = { page: await pageMeta(tabId) };
      return result;
    } finally {
      finishActionSignature(tabId, self.GhostlightActionSignature.KINDS.JAVASCRIPT);
    }
  },
  async read_console_messages(a) {
    const tabId = await effectiveTabId(a.tabId);
    await ensureAttached(tabId);
    // Only enable Runtime; the Console domain is the deprecated duplicate source (see onEvent).
    await enableDomain(tabId, "Runtime");
    const tab = await chrome.tabs.get(tabId);
    const host = hostOf(tab.url || "");
    tabHost.set(tabId, host);
    tabUrl.set(tabId, tab.url || "");
    const buf = bufferFor(consoleBuffer, tabId, host);
    const total = buf.items.length;
    let msgs = buf.items;
    if (a.onlyErrors) msgs = msgs.filter((m) => ["error", "exception"].includes(m.level));
    if (a.pattern) {
      try { const re = new RegExp(a.pattern, "i"); msgs = msgs.filter((m) => re.test(m.text) || re.test(m.level)); }
      catch { msgs = msgs.filter((m) => m.text.includes(a.pattern)); }
    }
    msgs = msgs.slice(-(a.limit || 100));
    if (a.clear) consoleBuffer.set(tabId, { host, items: [] });
    let out;
    if (msgs.length) {
      out = msgs.map((m) => `[${m.level}] ${m.text}`).join("\n");
    } else {
      const primary = total
        ? `${total} console message(s) recorded for this tab, but none matched your filter.`
        : "No console messages recorded for this tab.";
      out = `${primary}\nNote: console tracking begins when this tool is first used on a tab. Reload the page to capture messages emitted during page load.`;
    }
    if (consoleResetNotice) {
      out += "\nNote: console event buffer was reset by a browser service-worker restart; tracking resumed from that point.";
      consoleResetNotice = false;
    }
    const result = text(out);
    result.structuredContent = { page: await pageMeta(tabId) };
    return result;
  },
  async read_network_requests(a) {
    const tabId = await effectiveTabId(a.tabId);
    await ensureAttached(tabId);
    await enableDomain(tabId, "Network");
    const tab = await chrome.tabs.get(tabId);
    const host = hostOf(tab.url || "");
    tabHost.set(tabId, host);
    tabUrl.set(tabId, tab.url || "");
    const buf = bufferFor(networkBuffer, tabId, host);
    const total = buf.items.length;
    let reqs = buf.items;
    if (a.urlPattern) reqs = reqs.filter((r) => r.url.includes(a.urlPattern));
    reqs = reqs.slice(-(a.limit || 100));
    if (a.clear) networkBuffer.set(tabId, { host, items: [] });
    let out;
    if (reqs.length) {
      out = reqs.map((r) => `${r.method || "?"} ${r.url} ${r.status ? "-> " + r.status + (r.errorText ? " (" + r.errorText + ")" : "") : "(pending)"}`).join("\n");
    } else {
      const primary = total
        ? `${total} network request(s) recorded for this tab, but none matched your filter.`
        : "No network requests recorded for this tab.";
      out = `${primary}\nNote: network tracking begins when this tool is first used on a tab. Reload the page to capture requests made during page load, or interact with the page to trigger new requests.`;
    }
    if (networkResetNotice) {
      out += "\nNote: network event buffer was reset by a browser service-worker restart; tracking resumed from that point.";
      networkResetNotice = false;
    }
    const result = text(out);
    result.structuredContent = { page: await pageMeta(tabId) };
    return result;
  },
  async resize_window(a) {
    const tabId = await effectiveTabId(a.tabId);
    const tab = await chrome.tabs.get(tabId);
    await chrome.windows.update(tab.windowId, { width: a.width, height: a.height });
    // The viewport changed; drop any stale ScreenshotContext for this window's tabs so the next
    // screenshot re-establishes the coordinate mapping.
    for (const attachedId of attached.keys()) {
      try {
        const t = await chrome.tabs.get(attachedId);
        if (t.windowId === tab.windowId) screenshotCtx.delete(attachedId);
      } catch { /* tab gone */ }
    }
    return text(`Resized window to ${a.width}x${a.height}.`);
  },
  async update_plan(a) {
    const domains = (a.domains || []).join(", ");
    const approach = (a.approach || []).map((s) => `- ${s}`).join("\n");
    return text(`Plan (auto-approved by the v1.0 engine):\nDomains: ${domains}\n${approach}`);
  },
  async narrate(a) {
    const tabId = await effectiveTabId(a.tabId);
    if (typeof a.text !== "string" || a.text.trim().length === 0 || a.text.length > 240) {
      throw hopError("page", "text must be one non-empty sentence of at most 240 characters");
    }
    if (a.position !== undefined && !["auto", "top", "bottom"].includes(a.position)) {
      throw hopError("page", 'position must be one of "auto", "top", or "bottom"');
    }
    if (a.duration_ms !== undefined &&
        (!Number.isInteger(a.duration_ms) || a.duration_ms < 1000 || a.duration_ms > 30000)) {
      throw hopError("page", "duration_ms must be an integer from 1000 through 30000");
    }

    const durationMs = Number.isInteger(a.duration_ms)
      ? a.duration_ms
      : NARRATION_DEFAULT_DURATION_MS;
    const record = {
      text: a.text,
      position: ["auto", "top", "bottom"].includes(a.position) ? a.position : "auto",
      durationMs,
      deadline: Date.now() + durationMs,
    };
    const publication = renderNarration(tabId, record);
    const response = await publication.delivery;

    const shown = !!(response && response.shown === true);
    const reason = shown
      ? null
      : ((response && response.reason) || "the visual layer is unavailable on this page");
    const effectivePosition = (response && response.position) || record.position;
    const out = text(shown
      ? `Narration shown at ${effectivePosition} for ${record.durationMs}ms.`
      : `Narration not shown: ${reason}.`);
    out.structuredContent = {
      shown,
      position: effectivePosition,
      duration_ms: record.durationMs,
      replaced: publication.replaced,
    };
    if (reason) out.structuredContent.reason = reason;
    return out;
  },
  // ADR-0078 C3 internal mechanisms. They are not registry tools and cannot be called by a model;
  // the governed `act_on` local handler uses them after its one parent authorization.
  async resolve_actionable_internal(a) {
    const tabId = await effectiveTabId(a.tabId);
    const r = await content(tabId, { type: "resolveActionable", target: a.target || {} });
    return text(JSON.stringify((r && r.result) || { target: null, candidates: [] }));
  },
  async target_cue_internal(a) {
    const tabId = await effectiveTabId(a.tabId);
    await semanticTargetCue(tabId, a.x, a.y, a.action);
    return text("Target cue shown.");
  },
  // Internal read for form_fill (ADR-0036 D5, PINS.md SS12): NOT in the tool REGISTRY, so models
  // cannot call it -- only form_fill's handler dials it via browser.call. Returns the value-free
  // form identity (controls + submit candidates) as raw JSON, no prose rendering.
  async form_structure_internal(a) {
    const tabId = await effectiveTabId(a.tabId);
    const r = await content(tabId, { type: "formStructure" });
    const structure = (r && r.result) || { forms: [], formless: [] };
    structure.page = await pageMeta(tabId);
    return text(JSON.stringify(structure, null, 2));
  },
};

async function dispatch(item) {
  await ready; // never run a tool against un-rehydrated state
  const request = item.request;
  const tool = request.tool;
  const args = request.args || {};
  const key = request.clientKey || request.guid;
  const handler = handlers[tool];
  if (!handler) return fail(item.response, `Unknown tool: ${tool}`);
  try {
    // ADR-0066: `key` is the client's clientKey (or a legacy guid); the grouping handlers use it
    // to reuse the client's durable tab group. Every other handler ignores its second argument.
    reply(item.response, await handler(args, key, request.workspace));
  } catch (e) {
    if (e instanceof TabAccessError) return reply(item.response, text(e.message));
    // Hop-tagged errors (cdp/page) pass through as-is; untagged errors keep the tool-name prefix.
    if (e && e.hop) fail(item.response, e);
    else fail(item.response, `${tool} failed: ${(e && e.message) || e}`);
  }
}

// Startup recovery for the kill switch (g11): if a kill was in force (possibly interrupted by
// a service-worker restart mid-kill), finish it -- set the hot-path flag and re-run the detach
// sweep -- and do NOT connect. Recovery is explicit; only RECONNECT_SESSION calls connect()
// again. Otherwise, normal startup: connect as always.
async function init() {
  await ready;
  const s = await chrome.storage.session.get("session_killed");
  if (s.session_killed) {
    sessionKilled = true;
    await sweepDetachAll();
    return;
  }
  connect();
}

const ready = rehydrate();
init();
