// Ghostlight -- the frame inventory and document guard of adapter protocol 4.
(function installGhostlightInventory(root, factory) {
  const node = typeof module !== "undefined" && module.exports;
  const api = factory(node
    ? { protocol: require("./protocol.js"), sessions: require("./sessions.js"), shared: require("./shared.js") }
    : { protocol: root.GhostlightProtocol, sessions: root.GhostlightSessions, shared: root.GhostlightShared });
  root.GhostlightInventory = api;
  if (node) module.exports = api;
})(globalThis, function createGhostlightInventoryApi(deps) {
  "use strict";

  const { protocol, sessions, shared } = deps;
  const { CDP } = sessions;

  // Every document of a tab, read from CDP across the root session and every child (out-of-process
  // iframe) session, keyed by frame id with its loader id. A session lists only the frames in its
  // own process, so frames Chrome would not attach are found through webNavigation and reported as
  // unseen urls. The guard is a mechanical currency check run right before the adapter acts; the
  // service decides what a guard holds.

  const TOP_FRAME_ID = 0;
  const ACTIVE_LIFECYCLE = "active";
  const { DOCUMENT_CHANGED, SESSION_LOST } = protocol.RECEIPT_FAILURE;
  const CHANGED_MESSAGE = "The page changed since it was read.";

  function walk(tree, visit) {
    if (!tree?.frame) return;
    visit(tree.frame);
    for (const child of tree.childFrames ?? []) walk(child, visit);
  }

  function findFrame(tree, frameId) {
    let found = null;
    walk(tree, (frame) => {
      if (!found && frame.id === frameId) found = frame;
    });
    return found;
  }

  function withoutFragment(url) {
    const text = String(url ?? "");
    const hash = text.indexOf("#");
    return hash < 0 ? text : text.slice(0, hash);
  }

  function origin(url) {
    try {
      return new URL(url).origin;
    } catch (_error) {
      return String(url ?? "");
    }
  }

  function frameFact(frame, parentId) {
    const fact = {
      frame_id: frame.id,
      loader_id: frame.loaderId,
      url: shared.bounded(`${frame.url ?? ""}${frame.urlFragment ?? ""}`, protocol.URL_CHARS),
      unreachable: Boolean(frame.unreachableUrl)
    };
    if (parentId) fact.parent_frame_id = parentId;
    return fact;
  }

  /**
   * Checks a DocumentGuard against a snapshot and throws `document_changed` when it fails. Every
   * listed document must still hold the same loader. In exact mode its url must be unchanged and the
   * inventory must be complete, list exactly the guarded documents, and have exactly `unseen` unseen
   * frames; otherwise only its origin must be unchanged. No guard passes.
   */
  function checkGuard(guard, snap) {
    if (!guard) return;
    for (const document of guard.documents ?? []) {
      const now = snap.owner.get(document.frame_id);
      if (!now || now.loaderId !== document.loader_id) throw protocol.failure(DOCUMENT_CHANGED, CHANGED_MESSAGE);
      const same = guard.exact ? now.fact.url === document.url : origin(now.fact.url) === origin(document.url);
      if (!same) throw protocol.failure(DOCUMENT_CHANGED, CHANGED_MESSAGE);
    }
    if (guard.exact && (!snap.complete || snap.owner.size !== (guard.documents ?? []).length || snap.unseen.length !== (guard.unseen ?? 0))) {
      throw protocol.failure(DOCUMENT_CHANGED, CHANGED_MESSAGE);
    }
  }

  /**
   * Creates the inventory over a GhostlightSessions instance and chrome.webNavigation.
   */
  function create({
    lifecycle,
    webNavigation
  } = {}) {
    if (!lifecycle?.cdp || !webNavigation?.getAllFrames) throw new TypeError("the inventory requires debugger sessions and webNavigation");
    const cdp = lifecycle.cdp;

    // No document is read while a dialog is open: a JavaScript dialog blocks the renderer, so
    // Page.getFrameTree would not answer until the dialog is handled.
    function rootOf(tabId) {
      lifecycle.requireNoDialog(tabId);
      const session = lifecycle.rootSession(tabId);
      if (!session) throw protocol.failure(SESSION_LOST, "The debugger is not attached to this tab.");
      return session;
    }

    /**
     * Reads every document of an attached tab; fails at once with `dialog_open` while a dialog is
     * open on it. Resolves `{ owner, complete, unseen, top }`: `owner`
     * maps a frame id to `{ session, loaderId, fact }` where `session` is the CDP target that owns
     * the frame; `complete` is false when a session could not be read; `unseen` lists the urls of
     * active child frames no session reported; `top` is the top document `{ frame_id, loader_id }`.
     */
    async function snapshot(tabId) {
      const rootSession = rootOf(tabId);
      const owner = new Map();
      let complete = true;
      let top = null;
      for (const session of [rootSession, ...lifecycle.childSessions(tabId)]) {
        let tree;
        try {
          tree = (await cdp(session.target, CDP.PAGE_GET_FRAME_TREE)).frameTree;
        } catch (_error) {
          complete = false;
          continue;
        }
        const child = Boolean(session.target.sessionId);
        walk(tree, (frame) => {
          const oopifRoot = child && frame.id === session.targetId;
          const known = owner.get(frame.id);
          if (known && !oopifRoot) return;
          const parentId = frame.parentId ?? known?.fact.parent_frame_id ?? null;
          owner.set(frame.id, { session: session.target, loaderId: frame.loaderId, fact: frameFact(frame, parentId) });
          if (!child && !frame.parentId) top = Object.freeze({ frame_id: frame.id, loader_id: frame.loaderId });
        });
      }
      const seen = new Map();
      for (const { fact } of owner.values()) {
        const url = withoutFragment(fact.url);
        seen.set(url, (seen.get(url) ?? 0) + 1);
      }
      let frames = [];
      try {
        frames = (await webNavigation.getAllFrames({ tabId })) ?? [];
      } catch (_error) {
        complete = false;
      }
      const unseen = [];
      for (const frame of frames) {
        if (frame.frameId === TOP_FRAME_ID || frame.documentLifecycle !== ACTIVE_LIFECYCLE) continue;
        const url = withoutFragment(frame.url);
        const count = seen.get(url) ?? 0;
        if (count > 0) {
          seen.set(url, count - 1);
          continue;
        }
        if (unseen.length < protocol.MAX_UNSEEN) unseen.push(shared.bounded(frame.url, protocol.URL_CHARS));
      }
      return { owner, complete, unseen, top };
    }

    /**
     * Reads only the top document from the root session, in the snapshot shape (incomplete, no
     * unseen frames). For the top world when no scoped document is named.
     */
    async function topSnapshot(tabId) {
      const rootSession = rootOf(tabId);
      const tree = (await cdp(rootSession.target, CDP.PAGE_GET_FRAME_TREE)).frameTree;
      const owner = new Map();
      let top = null;
      if (tree?.frame) {
        const frame = tree.frame;
        owner.set(frame.id, { session: rootSession.target, loaderId: frame.loaderId, fact: frameFact(frame, null) });
        top = Object.freeze({ frame_id: frame.id, loader_id: frame.loaderId });
      }
      return { owner, complete: false, unseen: [], top };
    }

    /** Resolves the current loader id of a frame in a session, or null when the frame is gone. */
    async function currentLoader(target, frameId) {
      lifecycle.requireNoDialog(target.tabId);
      const tree = (await cdp(target, CDP.PAGE_GET_FRAME_TREE)).frameTree;
      return findFrame(tree, frameId)?.loaderId ?? null;
    }

    /** The `frames` primitive: the tab's documents, completeness and unseen urls, inside a debugger lease. */
    async function framesPrimitive(args) {
      return lifecycle.withLease(args.tab_id, async () => {
        const snap = await snapshot(args.tab_id);
        return { frames: Array.from(snap.owner.values(), (entry) => entry.fact), complete: snap.complete, unseen: snap.unseen };
      });
    }

    return Object.freeze({ snapshot, topSnapshot, checkGuard, currentLoader, framesPrimitive });
  }

  return Object.freeze({ create, checkGuard });
});
