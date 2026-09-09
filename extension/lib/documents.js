// Browser document identity and execution constraints. No policy or model-facing decisions.
(function install(root, factory) {
  const api = factory();
  root.GhostlightDocuments = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createDocumentApi() {
  "use strict";
  const DOCUMENT_LIMIT = 256;
  const METADATA_KINDS = new Set(["document_route", "frame_boxes", "capture_mask", "capture_mask_check", "capture_mask_clear", "presentation_visibility", "scroll_offset", "viewport_point"]);
  const changed = () => Object.assign(new Error("document scope changed before access"), { code: "document_scope_changed", effectUnknown: false });

  function inventory(raw) {
    if (!Array.isArray(raw) || !raw.length || raw.length > DOCUMENT_LIMIT) throw changed();
    const ordered = raw.slice().sort((a, b) => a.frameId - b.frameId);
    const documents = ordered.map((frame) => {
      if (typeof frame.documentId !== "string" || !frame.documentId || frame.documentId.length > 160) throw changed();
      const parent = frame.parentFrameId === -1 ? null : ordered.find((item) => item.frameId === frame.parentFrameId)?.documentId;
      if (parent === undefined) throw changed();
      return { id: frame.documentId, url: String(frame.url ?? ""), parent,
        supported: /^https?:/i.test(frame.url ?? "") && frame.documentLifecycle !== "prerender" && !frame.errorOccurred };
    });
    if (new Set(documents.map((document) => document.id)).size !== documents.length
      || documents.filter((document) => document.parent === null).length !== 1) throw changed();
    return documents;
  }

  function same(left, right) {
    return JSON.stringify(left) === JSON.stringify(right);
  }

  function create({ getFrames, sendDocument, frames }) {
    const active = new Map();

    async function current(tabId) {
      const raw = await getFrames(tabId);
      return { raw, documents: inventory(raw) };
    }

    async function describe(command) {
      const { raw, documents } = await current(command.tab_id);
      const subjects = new Set();
      let unresolved = false;
      for (const locator of command.locators) {
        const frame = raw.find((item) => item.frameId === frames.frameOf(locator));
        const documentId = frames.documentOf(locator);
        if (!frame || !documentId || frame.documentId !== documentId) unresolved = true;
        else subjects.add(documentId);
      }
      for (const point of command.points) {
        let frame = raw.find((item) => item.frameId === 0);
        let x = point.x;
        let y = point.y;
        const visited = new Set();
        try {
          while (frame && !visited.has(frame.documentId)) {
            visited.add(frame.documentId);
            const answer = await sendDocument(command.tab_id, frame.documentId, { kind: "document_route", page_x: x, page_y: y, viewport: command.viewport || frame.frameId !== 0 });
            if (!answer.embed) { subjects.add(frame.documentId); break; }
            const child = frames.childFrameForEmbed(raw, frame.frameId, answer.embed.src);
            x = answer.x - answer.embed.left;
            y = answer.y - answer.embed.top;
            frame = child;
          }
          if (!frame) unresolved = true;
        } catch (_) { unresolved = true; }
      }
      if (command.focused) {
        let found = false;
        for (const frame of raw) {
          try {
            const answer = await sendDocument(command.tab_id, frame.documentId, { kind: "document_route", focused: true });
            if (answer.focused) { subjects.add(frame.documentId); found = true; }
          } catch (_) { /* unsupported focus cannot establish an exact subject */ }
        }
        unresolved ||= !found;
      }
      return { documents, subjects: Array.from(subjects), unresolved, incomplete: false };
    }

    async function run(tabId, scope, operation) {
      if (active.has(tabId)) throw changed();
      const snapshot = await current(tabId);
      if (!same(snapshot.documents, scope.documents) || scope.allowed.length > DOCUMENT_LIMIT
        || scope.allowed.some((id) => !snapshot.documents.some((document) => document.id === id && document.supported))) throw changed();
      const context = { ...snapshot, scope, visited: new Set(), unavailable: new Set(), masked: 0, dispatched: false, limited: false };
      active.set(tabId, context);
      try {
        const result = await operation();
        return { outcome: "in_documents", result, observation: {
          visited: Array.from(context.visited), unavailable: Array.from(context.unavailable),
          limited_by_size: Boolean(result.truncated || context.limited), masked_regions: context.masked
        } };
      } catch (error) {
        if (error.code === "document_scope_changed") error.effectUnknown = context.dispatched;
        throw error;
      } finally { active.delete(tabId); }
    }

    async function route(tabId, frameId, message, fallback) {
      const context = active.get(tabId);
      if (!context) return fallback();
      const frame = context.raw.find((item) => item.frameId === frameId);
      if (!frame) throw changed();
      const metadata = METADATA_KINDS.has(message.kind);
      if (!metadata && !context.scope.allowed.includes(frame.documentId)) throw changed();
      try {
        if (["activate", "fill", "focus", "clear", "clear_focused", "type_text", "scroll", "scroll_point", "hover", "drop_files"].includes(message.kind)) context.dispatched = true;
        const result = await sendDocument(tabId, frame.documentId, message);
        if (result?.error) throw new Error(result.error);
        if (!metadata) context.visited.add(frame.documentId);
        if (!metadata && result?.truncated) context.limited = true;
        return result;
      } catch (error) {
        if (!metadata) context.unavailable.add(frame.documentId);
        throw error;
      }
    }

    function frameIds(tabId) {
      const context = active.get(tabId);
      return context ? context.raw.filter((frame) => context.scope.allowed.includes(frame.documentId)).map((frame) => frame.frameId) : null;
    }

    function locator(tabId, frameId, local) {
      const context = active.get(tabId);
      return frames.scopedLocator(frameId, local, context?.raw.find((frame) => frame.frameId === frameId)?.documentId);
    }

    async function verify(tabId) {
      const context = active.get(tabId);
      if (context && !same((await current(tabId)).documents, context.documents)) throw changed();
    }

    async function verifyInput(tabId, method, params) {
      const context = active.get(tabId);
      if (!context) return;
      await verify(tabId);
      if (method.startsWith("Input.")) {
        const point = Number.isFinite(params?.x) && Number.isFinite(params?.y);
        const subject = await describe({ tab_id: tabId, locators: [],
          points: point ? [{ x: params.x, y: params.y }] : [], focused: !point, viewport: true });
        if (subject.unresolved || subject.subjects.some((id) => !context.scope.allowed.includes(id))) throw changed();
      }
    }

    async function input(tabId, method, params) {
      await verifyInput(tabId, method, params);
      const context = active.get(tabId);
      if (context) context.dispatched = true;
    }

    function limit(tabId) { const context = active.get(tabId); if (context) context.limited = true; }
    return { describe, run, route, frameIds, locator, verify, verifyInput, input, limit, context: (tabId) => active.get(tabId), current };
  }
  return { create, inventory, same, changed, DOCUMENT_LIMIT };
});
