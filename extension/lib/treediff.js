// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- read_page diff (ADR-0037 Decision 3): compute the added/removed/changed line sets
// between two rendered accessibility-tree snapshots, keyed by each line's first `ref_N` token (or
// by the whole line when a line carries no ref). Pure: no DOM, no chrome.* -- the caller (content.js)
// hands two arrays of rendered lines and receives three arrays back.
//
// Render order (PINS.md SS11): changed (`~ `), then removed (`- `), then added (`+ `); within
// changed/added the order is the NEW tree's order, within removed it is the OLD tree's order.
//
// IIFE-wrapped and exposed as a single namespace per lib/constants.js's pattern (idempotent under
// MV3 worker re-evaluation; loadable as a content-script global via the manifest and under node --test).
(function () {
// A line's identity key is its first `ref_<digits>` token; a line with no ref token is keyed by its
// whole text (so two identical keyless lines are "the same line", and any text change is a change).
function lineKey(line) {
  const m = line.match(/ref_\d+/);
  return m ? m[0] : line;
}

// diffLines(oldLines, newLines) -> { added, removed, changed }. A ref present in both old and new
// but with different text is "changed"; a ref only in new is "added"; a ref only in old is "removed".
// Keyless lines compare by whole-line identity (same key = same whole line, so they are either
// unchanged or, rarely, both removed and re-added -- the map math handles both). Each result array
// preserves its source order: added/changed follow the new tree, removed follows the old tree.
function diffLines(oldLines, newLines) {
  const oldByKey = new Map();
  const oldKeylessOrder = [];
  for (const line of oldLines) {
    const k = lineKey(line);
    if (k === line) {
      // Keyless line: index by whole text so duplicates are tracked by count.
      oldKeylessOrder.push(line);
    } else {
      oldByKey.set(k, line);
    }
  }
  const newByKey = new Map();
  const newKeylessOrder = [];
  for (const line of newLines) {
    const k = lineKey(line);
    if (k === line) {
      newKeylessOrder.push(line);
    } else {
      newByKey.set(k, line);
    }
  }

  const changed = [];
  const added = [];
  // Walk the new tree in order: a ref-line whose key is in old with different text is changed; one
  // whose key is absent from old is added. Keyless lines are compared as multisets below.
  const oldKeylessCounts = new Map();
  for (const line of oldKeylessOrder) oldKeylessCounts.set(line, (oldKeylessCounts.get(line) || 0) + 1);
  const newKeylessCounts = new Map();
  for (const line of newKeylessOrder) newKeylessCounts.set(line, (newKeylessCounts.get(line) || 0) + 1);

  for (const line of newLines) {
    const k = lineKey(line);
    if (k === line) continue; // keyless handled by multiset diff
    const oldLine = oldByKey.get(k);
    if (oldLine === undefined) {
      added.push(line);
    } else if (oldLine !== line) {
      changed.push(line);
    }
  }

  // Keyless multiset diff: a keyless line type appearing more times in new than old is added (in
  // new order); more times in old than new is removed (in old order). This is the rare case --
  // structural lines without refs -- and whole-line identity is the only honest comparison.
  const keylessAdded = [];
  const keylessRemoved = [];
  const seen = new Set();
  for (const line of newKeylessOrder) {
    if (seen.has(line)) continue;
    seen.add(line);
    const oldN = oldKeylessCounts.get(line) || 0;
    const newN = newKeylessCounts.get(line) || 0;
    for (let i = 0; i < newN - oldN; i++) keylessAdded.push(line);
  }
  seen.clear();
  for (const line of oldKeylessOrder) {
    if (seen.has(line)) continue;
    seen.add(line);
    const oldN = oldKeylessCounts.get(line) || 0;
    const newN = newKeylessCounts.get(line) || 0;
    for (let i = 0; i < oldN - newN; i++) keylessRemoved.push(line);
  }

  const removed = [];
  for (const line of oldLines) {
    const k = lineKey(line);
    if (k === line) continue; // keyless handled above
    if (!newByKey.has(k)) removed.push(line);
  }
  for (const line of keylessRemoved) removed.push(line);
  for (const line of keylessAdded) added.push(line);

  return { added, removed, changed };
}

const GhostlightTreeDiff = { diffLines };
if (typeof module !== "undefined" && module.exports) {
  module.exports = GhostlightTreeDiff;
} else {
  self.GhostlightTreeDiff = GhostlightTreeDiff;
}
})();
