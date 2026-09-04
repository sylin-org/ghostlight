// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- frame-scoped locator identity and cross-frame result merging.
//
// The extension observes every frame (ADR-0138), but each frame instance owns a private
// element registry keyed by plain `locator_N`. This module is the one place that binds a
// locator to the frame that mints it and the one place that folds per-frame results back
// into the single document a model sees. The scoped form never leaves the extension: the
// bridge and orchestrator treat locators as opaque strings behind TargetHandle.
(function installGhostlightFrames(root, factory) {
  const api = factory();
  root.GhostlightFrames = api;
  if (typeof module !== "undefined" && module.exports) {
    module.exports = api;
  }
})(globalThis, function createGhostlightFrames() {
  "use strict";

  const TOP_FRAME_ID = 0;
  const SCOPED_LOCATOR = /^(0|[1-9][0-9]*):(locator_.+)$/;

  function scopedLocator(frameId, local) {
    return `${frameId}:${local}`;
  }

  // Returns the owning frame id, or null when the handle predates frame scoping or is not
  // a locator at all. Callers treat null as the top frame only where a legacy handle is
  // still legal; every fresh mint is scoped.
  function frameOf(handle) {
    const match = SCOPED_LOCATOR.exec(String(handle ?? ""));
    return match ? Number(match[1]) : null;
  }

  function localOf(handle) {
    const match = SCOPED_LOCATOR.exec(String(handle ?? ""));
    return match ? match[2] : String(handle ?? "");
  }

  // Stamps every observed target in a fulfilled per-frame result with its minting frame.
  function scopeTargets(frameId, targets) {
    return (targets ?? []).map((target) => ({ ...target, locator: scopedLocator(frameId, target.locator) }));
  }

  // Merges per-frame target lists in stable frame order, top frame first, under one total
  // ceiling. Frame keys arrive as strings from object enumeration; sort numerically.
  function mergeTargets(perFrame, maximum) {
    const merged = [];
    const frameIds = Object.keys(perFrame).map(Number).sort((left, right) => left - right);
    for (const frameId of frameIds) {
      for (const target of perFrame[frameId]) {
        if (merged.length >= maximum) return merged;
        merged.push(target);
      }
    }
    return merged;
  }

  // Folds frame-local visible text into one bounded page result. The caller may stop
  // collecting once the ceiling is full; `skipped` preserves the truthful truncation bit.
  function mergeTextSections(perFrame, maximum, skipped = false) {
    let text = "";
    let truncated = Boolean(skipped);
    const frameIds = Object.keys(perFrame).map(Number).sort((left, right) => left - right);
    for (const frameId of frameIds) {
      const section = perFrame[frameId] ?? {};
      const value = String(section.text ?? "").trim();
      truncated = truncated || Boolean(section.truncated);
      if (!value) continue;
      const separator = text ? "\n\n" : "";
      const remaining = maximum - text.length;
      if (separator.length >= remaining) {
        truncated = true;
        break;
      }
      const piece = `${separator}${value}`;
      if (piece.length > remaining) {
        text += piece.slice(0, Math.max(0, remaining));
        truncated = true;
        break;
      }
      text += piece;
    }
    return { text, truncated };
  }

  // Runs one negotiated document read through an injected frame reader. Article mode is
  // explicitly top-first; when no useful article exists, the same globally bounded visible
  // read used by the default path becomes its fallback.
  async function readDocument(frameIds, mode, maximum, readFrame) {
    const orderedFrameIds = Array.from(new Set([TOP_FRAME_ID, ...frameIds])).sort((left, right) => left - right);
    let knownTop = null;
    if (mode === "article") {
      try {
        knownTop = await readFrame(TOP_FRAME_ID, "article", maximum);
      } catch (_error) { /* the visible fallback reports whatever document remains available */ }
      if (knownTop?.article_found) {
        const { article_found: _articleFound, ...article } = knownTop;
        return article;
      }
    }

    const perFrame = {};
    let top = knownTop;
    let skipped = false;
    for (let index = 0; index < orderedFrameIds.length; index += 1) {
      const current = mergeTextSections(perFrame, maximum);
      const separatorChars = current.text ? 2 : 0;
      const available = Math.max(1, maximum - current.text.length - separatorChars);
      const frameId = orderedFrameIds[index];
      try {
        const result = await readFrame(frameId, "visible", available);
        perFrame[frameId] = result;
        if (frameId === TOP_FRAME_ID) top = result;
        if (mergeTextSections(perFrame, maximum).truncated) {
          skipped = index + 1 < orderedFrameIds.length;
          break;
        }
      } catch (_error) { /* a navigating or absent child frame contributes no text */ }
    }
    return {
      ...mergeTextSections(perFrame, maximum, skipped),
      title: String(top?.title ?? ""),
      url: String(top?.url ?? "")
    };
  }

  // Folds frame-local composed trees into one bounded page tree. The top document stays
  // the root when available; each embedded document becomes one child in stable frame
  // order. The caller supplies the remaining node budget to each frame, so the 400-node
  // ceiling is page-wide rather than multiplied by the number of embeds.
  async function inspectDocument(frameIds, maxDepth, maximum, readFrame) {
    const orderedFrameIds = Array.from(new Set([TOP_FRAME_ID, ...frameIds])).sort((left, right) => left - right);
    let tree = null;
    let nodes = 0;
    let truncated = false;
    for (let index = 0; index < orderedFrameIds.length; index += 1) {
      if (nodes >= maximum) {
        truncated = true;
        break;
      }
      const frameId = orderedFrameIds[index];
      if (tree && maxDepth <= 1) {
        truncated = true;
        break;
      }
      try {
        const localDepth = tree ? Math.max(1, maxDepth - 1) : maxDepth;
        const result = await readFrame(frameId, localDepth, maximum - nodes);
        if (!result?.tree) continue;
        if (!tree) tree = result.tree;
        else tree.children = [...(tree.children ?? []), result.tree];
        nodes += Number(result.nodes ?? 0);
        truncated = truncated || Boolean(result.truncated);
        if (truncated && index + 1 < orderedFrameIds.length) break;
      } catch (_error) { /* a navigating or absent frame contributes no subtree */ }
    }
    return { tree, nodes, truncated };
  }

  // Groups locator-bearing request fields by owning frame, preserving first-appearance
  // order of the frames and, inside a group, the caller's field order. Throws on an
  // unscoped handle rather than silently routing it to the top frame.
  function groupLocators(handles) {
    const groups = new Map();
    for (const handle of handles) {
      const frameId = frameOf(handle);
      if (frameId === null) throw new Error("target handle is not frame-scoped");
      if (!groups.has(frameId)) groups.set(frameId, []);
      groups.get(frameId).push({ handle, local: localOf(handle) });
    }
    return groups;
  }

  // Decides whether a parent document's embed element owns a child frame. The child's
  // current URL and the embed's absolute src usually agree; redirects and vendor
  // rewrites can differ in query or fragment, so identity is origin plus path. An empty
  // src matches nothing: a src-less embed is a scripting hole, not a navigated frame we
  // can vouch for.
  function embedMatches(embedSrc, frameUrl) {
    if (!embedSrc || !frameUrl) return false;
    try {
      const embed = new URL(embedSrc);
      const frame = new URL(frameUrl);
      return embed.origin === frame.origin && embed.pathname === frame.pathname;
    } catch (_error) {
      return false;
    }
  }

  function childFrameForEmbed(navigationFrames, parentFrameId, embedSrc) {
    const children = (navigationFrames ?? []).filter((entry) =>
      entry.parentFrameId === parentFrameId && embedMatches(embedSrc, entry.url)
    );
    if (children.length === 0) throw new Error("no child frame matches the embed at the page point");
    if (children.length > 1) throw new Error("several child frames match the embed at the page point; refusing to guess");
    return children[0];
  }

  return Object.freeze({
    TOP_FRAME_ID,
    scopedLocator,
    frameOf,
    localOf,
    scopeTargets,
    mergeTargets,
    mergeTextSections,
    readDocument,
    inspectDocument,
    groupLocators,
    embedMatches,
    childFrameForEmbed
  });
});
