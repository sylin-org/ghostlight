// Ghostlight -- the frame inventory and document guard of adapter protocol 4.
(function installGhostlightInventory(root, factory) {
  const node = typeof module !== "undefined" && module.exports;
  const api = factory(node
    ? { protocol: require("./protocol.js"), sessions: require("./sessions.js"), shared: require("./shared.js"), args: require("./arguments.js") }
    : { protocol: root.GhostlightProtocol, sessions: root.GhostlightSessions, shared: root.GhostlightShared, args: root.GhostlightArguments });
  root.GhostlightInventory = api;
  if (node) module.exports = api;
})(globalThis, function createGhostlightInventoryApi(deps) {
  "use strict";

  const { protocol, sessions, shared, args: check } = deps;
  const { CDP } = sessions;

  // Every document of a tab, read from CDP across the root session and every child (out-of-process
  // iframe) session, keyed by frame id with its loader id. A session lists only the frames in its
  // own process, so frames Chrome would not attach are found through webNavigation and reported as
  // unseen urls. The guard is a mechanical currency check run right before the adapter acts; the
  // service decides what a guard holds.
  //
  // The inventory reports what the sessions say and does not shape it: a frame whose parent is not
  // listed is still listed, because the service owns the tree. Whether a session is alive is the
  // session registry's fact, so a failed read is judged by asking the registry, never by reading
  // the error.

  const TOP_FRAME_ID = 0;
  const ACTIVE_LIFECYCLE = "active";
  const { DOCUMENT_CHANGED, SESSION_LOST } = protocol.RECEIPT_FAILURE;
  const CHANGED_MESSAGE = "The page changed since it was read.";
  const NO_TOP_MESSAGE = "The tab has no top document.";
  const NOT_ATTACHED_MESSAGE = "The debugger is not attached to this tab.";

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
      if (!session) throw protocol.failure(SESSION_LOST, NOT_ATTACHED_MESSAGE);
      return session;
    }

    /**
     * Reads every document of an attached tab; fails at once with `dialog_open` while a dialog is
     * open on it. Resolves `{ owner, complete, unseen, top }`: `owner`
     * maps a frame id to `{ session, loaderId, fact }` where `session` is the CDP target that owns
     * the frame; `unseen` lists the urls of active child frames no session reported; `top` is the
     * top document `{ frame_id, loader_id }`. `complete` says every attached session answered and
     * the browser's own frame list was read: it is false when a child session that is still
     * attached could not be read, or when that list could not be had. A child session that has
     * ended holds no document, so it adds nothing and leaves `complete` alone. The root must be
     * read: when it cannot be, the snapshot rejects with that error. When the whole attachment ends
     * under a child's read there is nothing left to list, and the snapshot rejects `session_lost`.
     */
    async function snapshot(tabId) {
      const rootSession = rootOf(tabId);
      const owner = new Map();
      let complete = true;
      let top = null;
      const attached = (session) => lifecycle.childSessions(tabId).includes(session);
      const rootTree = (await cdp(rootSession.target, CDP.PAGE_GET_FRAME_TREE)).frameTree;
      if (!rootTree?.frame) throw protocol.failure(DOCUMENT_CHANGED, NO_TOP_MESSAGE);
      for (const session of [rootSession, ...lifecycle.childSessions(tabId)]) {
        const child = Boolean(session.target.sessionId);
        let tree = rootTree;
        if (child) {
          // A session that ended with an earlier one of this pass is not asked.
          if (!attached(session)) continue;
          try {
            tree = (await cdp(session.target, CDP.PAGE_GET_FRAME_TREE)).frameTree;
          } catch (_error) {
            // No child is registered once the tab's attachment has ended. That is not a whole
            // inventory, it is none.
            if (!lifecycle.rootSession(tabId)) throw protocol.failure(SESSION_LOST, NOT_ATTACHED_MESSAGE);
            if (attached(session)) complete = false;
            continue;
          }
        }
        walk(tree, (frame) => {
          const oopifRoot = child && frame.id === session.targetId;
          const known = owner.get(frame.id);
          if (known && !oopifRoot) return;
          const parentId = frame.parentId ?? known?.fact.parent_frame_id ?? null;
          owner.set(frame.id, { session: session.target, loaderId: frame.loaderId, fact: frameFact(frame, parentId) });
          if (!child && !frame.parentId) top = Object.freeze({ frame_id: frame.id, loader_id: frame.loaderId });
        });
      }
      // The browser's list is compared without its top frame, so the top document is not counted
      // here either: its url would otherwise stand in for an unread frame that shows the same url.
      const seen = new Map();
      for (const [frameId, { fact }] of owner) {
        if (frameId === top?.frame_id) continue;
        const url = withoutFragment(fact.url);
        seen.set(url, (seen.get(url) ?? 0) + 1);
      }
      let frames = [];
      try {
        const listed = await webNavigation.getAllFrames({ tabId });
        // Chrome answers with no list for a tab it cannot describe: the check did not run.
        if (Array.isArray(listed)) frames = listed;
        else complete = false;
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

    // The `frames` primitive's spec: a tab, and its frame inventory.
    const framesSpec = check.handler({
      addresses: true,
      validate(request) {
        check.requireArgs(request);
        check.requireTabId(request.tab_id);
      },
      run: framesPrimitive
    });

    return Object.freeze({ snapshot, topSnapshot, checkGuard, currentLoader, framesPrimitive, framesSpec });
  }

  return Object.freeze({ create, checkGuard });
});
