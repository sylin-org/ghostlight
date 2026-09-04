"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const frames = require("../lib/frames.js");

test("scoped locators round-trip through frameOf and localOf", () => {
  const handle = frames.scopedLocator(3, "locator_12");
  assert.equal(handle, "3:locator_12");
  assert.equal(frames.frameOf(handle), 3);
  assert.equal(frames.localOf(handle), "locator_12");
});

test("the top frame is frame zero, not an unscoped locator", () => {
  const handle = frames.scopedLocator(frames.TOP_FRAME_ID, "locator_1");
  assert.equal(handle, "0:locator_1");
  assert.equal(frames.frameOf(handle), 0);
});

test("unscoped or foreign handles report no owning frame", () => {
  assert.equal(frames.frameOf("locator_4"), null);
  assert.equal(frames.frameOf(""), null);
  assert.equal(frames.frameOf(null), null);
  assert.equal(frames.frameOf(undefined), null);
});

test("localOf passes unscoped handles through unchanged", () => {
  assert.equal(frames.localOf("locator_9"), "locator_9");
});

test("scopeTargets stamps every observed target with its minting frame", () => {
  const stamped = frames.scopeTargets(2, [
    { locator: "locator_1", role: "button", name: "Save" },
    { locator: "locator_2", role: "textbox", name: "" }
  ]);
  assert.deepEqual(stamped.map((target) => target.locator), ["2:locator_1", "2:locator_2"]);
  assert.equal(stamped[0].role, "button");
  assert.deepEqual(frames.scopeTargets(0, undefined), []);
});

test("mergeTargets orders by frame id with the top document first under one ceiling", () => {
  const merged = frames.mergeTargets({
    "5": [{ locator: frames.scopedLocator(5, "locator_a") }],
    "0": [{ locator: frames.scopedLocator(0, "locator_top") }, { locator: frames.scopedLocator(0, "locator_top2") }],
    "12": [{ locator: frames.scopedLocator(12, "locator_deep") }]
  }, 10);
  assert.deepEqual(merged.map((target) => target.locator), ["0:locator_top", "0:locator_top2", "5:locator_a", "12:locator_deep"]);
});

test("mergeTargets caps the total across frames, top frame winning its share first", () => {
  const merged = frames.mergeTargets({
    "1": [{ locator: "1:locator_b" }, { locator: "1:locator_c" }, { locator: "1:locator_d" }],
    "0": [{ locator: "0:locator_a" }]
  }, 3);
  assert.deepEqual(merged.map((target) => target.locator), ["0:locator_a", "1:locator_b", "1:locator_c"]);
  assert.deepEqual(frames.mergeTargets({}, 3), []);
});

test("mergeTextSections keeps stable frame order under one page-wide ceiling", () => {
  assert.deepEqual(frames.mergeTextSections({
    "7": { text: "third frame", truncated: false },
    "0": { text: "top frame", truncated: false },
    "2": { text: "second frame", truncated: false }
  }, 26), {
    text: "top frame\n\nsecond frame\n\nt",
    truncated: true
  });
});

test("mergeTextSections preserves local and skipped-frame truncation", () => {
  assert.deepEqual(
    frames.mergeTextSections({ "0": { text: "complete", truncated: true } }, 20),
    { text: "complete", truncated: true }
  );
  assert.deepEqual(
    frames.mergeTextSections({ "0": { text: "complete", truncated: false } }, 20, true),
    { text: "complete", truncated: true }
  );
  assert.deepEqual(
    frames.mergeTextSections({
      "0": { text: "12345", truncated: false },
      "1": { text: "", truncated: false }
    }, 5),
    { text: "12345", truncated: false }
  );
  assert.deepEqual(
    frames.mergeTextSections({
      "0": { text: "12345", truncated: false },
      "1": { text: "x", truncated: false }
    }, 5),
    { text: "12345", truncated: true }
  );
});

test("readDocument reads visible frames in stable order under one request budget", async () => {
  const calls = [];
  const source = { 0: "1234567890", 2: "abcdef", 7: "never reached" };
  const result = await frames.readDocument([7, 2, 0], "visible", 15, async (frameId, mode, maximum) => {
    calls.push({ frameId, mode, maximum });
    const text = source[frameId];
    return {
      text: text.slice(0, maximum),
      truncated: text.length > maximum,
      title: frameId === 0 ? "Top" : "Child",
      url: frameId === 0 ? "https://top.test/" : "https://child.test/"
    };
  });

  assert.deepEqual(calls, [
    { frameId: 0, mode: "visible", maximum: 15 },
    { frameId: 2, mode: "visible", maximum: 3 }
  ]);
  assert.deepEqual(result, {
    text: "1234567890\n\nabc",
    truncated: true,
    title: "Top",
    url: "https://top.test/"
  });
});

test("readDocument returns an explicit article without probing child frames", async () => {
  const calls = [];
  const result = await frames.readDocument([0, 4], "article", 500, async (frameId, mode, maximum) => {
    calls.push({ frameId, mode, maximum });
    return {
      text: "Useful article prose",
      truncated: false,
      article_found: true,
      title: "Article",
      url: "https://article.test/"
    };
  });

  assert.deepEqual(calls, [{ frameId: 0, mode: "article", maximum: 500 }]);
  assert.deepEqual(result, {
    text: "Useful article prose",
    truncated: false,
    title: "Article",
    url: "https://article.test/"
  });
});

test("readDocument falls back from an empty article to composed full-page text", async () => {
  const calls = [];
  const result = await frames.readDocument([0, 3], "article", 100, async (frameId, mode, maximum) => {
    calls.push({ frameId, mode, maximum });
    if (mode === "article") {
      return { text: "", truncated: false, article_found: false, title: "Shell", url: "https://shell.test/" };
    }
    if (frameId === 0) throw new Error("top frame navigated between probes");
    return { text: "Embedded application", truncated: false, title: "Embed", url: "https://embed.test/" };
  });

  assert.deepEqual(calls.map(({ frameId, mode }) => ({ frameId, mode })), [
    { frameId: 0, mode: "article" },
    { frameId: 0, mode: "visible" },
    { frameId: 3, mode: "visible" }
  ]);
  assert.deepEqual(result, {
    text: "Embedded application",
    truncated: false,
    title: "Shell",
    url: "https://shell.test/"
  });
});

test("inspectDocument merges frame trees in stable order under one node budget", async () => {
  const calls = [];
  const result = await frames.inspectDocument([8, 0, 3], 5, 5, async (frameId, maxDepth, maxNodes) => {
    calls.push({ frameId, maxDepth, maxNodes });
    const nodes = frameId === 0 ? 2 : Math.min(2, maxNodes);
    return {
      tree: { kind: "container", label: `frame ${frameId}`, children: frameId === 0 ? [{ kind: "heading", label: "top", children: [] }] : [] },
      nodes,
      truncated: nodes >= maxNodes
    };
  });

  assert.deepEqual(calls, [
    { frameId: 0, maxDepth: 5, maxNodes: 5 },
    { frameId: 3, maxDepth: 4, maxNodes: 3 },
    { frameId: 8, maxDepth: 4, maxNodes: 1 }
  ]);
  assert.deepEqual(result.tree.children.map((node) => node.label), ["top", "frame 3", "frame 8"]);
  assert.equal(result.nodes, 5);
  assert.equal(result.truncated, true);
});

test("inspectDocument keeps a depth-one tree bounded to the top document", async () => {
  const calls = [];
  const result = await frames.inspectDocument([0, 2], 1, 400, async (frameId, maxDepth, maxNodes) => {
    calls.push({ frameId, maxDepth, maxNodes });
    return { tree: { kind: "container", label: "top", children: [] }, nodes: 1, truncated: false };
  });

  assert.deepEqual(calls, [{ frameId: 0, maxDepth: 1, maxNodes: 400 }]);
  assert.equal(result.truncated, true);
});

test("groupLocators preserves first-appearance frame order and in-group field order", () => {
  const groups = frames.groupLocators([
    "7:locator_s3",
    "0:locator_f1",
    "7:locator_s1",
    "2:locator_i1",
    "0:locator_f2"
  ]);
  assert.deepEqual(Array.from(groups.keys()), [7, 0, 2]);
  assert.deepEqual(
    Array.from(groups.entries()).map(([frameId, entries]) => [frameId, entries.map((entry) => entry.handle)]),
    [[7, ["7:locator_s3", "7:locator_s1"]], [0, ["0:locator_f1", "0:locator_f2"]], [2, ["2:locator_i1"]]]
  );
  const first = groups.get(7)[0];
  assert.equal(first.local, "locator_s3");
});

test("groupLocators refuses an unscoped handle instead of silently routing it to the top frame", () => {
  assert.throws(() => frames.groupLocators(["0:locator_ok", "locator_legacy"]), /not frame-scoped/);
});

test("embed identity is origin plus path, tolerant of query and fragment drift", () => {
  assert.equal(frames.embedMatches("https://js-eu1.hsforms.net/embed/form", "https://js-eu1.hsforms.net/embed/form?__hstc=1"), true);
  assert.equal(frames.embedMatches("https://sylin.org/ghostlight/demo/iframe/form/", "https://sylin.org/ghostlight/demo/iframe/form/"), true);
  assert.equal(frames.embedMatches("https://sylin.org/other/form/", "https://sylin.org/ghostlight/demo/iframe/form/"), false);
  assert.equal(frames.embedMatches("https://evil.test/ghostlight/demo/iframe/form/", "https://sylin.org/ghostlight/demo/iframe/form/"), false);
  assert.equal(frames.embedMatches("", "https://sylin.org/x/"), false);
  assert.equal(frames.embedMatches("not a url", "https://sylin.org/x/"), false);
});

test("point routing selects one matching child frame and refuses ambiguity", () => {
  const navigationFrames = [
    { frameId: 0, parentFrameId: -1, url: "https://top.test/" },
    { frameId: 4, parentFrameId: 0, url: "https://child.test/form/?session=1" },
    { frameId: 8, parentFrameId: 4, url: "https://deep.test/widget/" }
  ];
  assert.equal(
    frames.childFrameForEmbed(navigationFrames, 0, "https://child.test/form/").frameId,
    4
  );
  assert.throws(
    () => frames.childFrameForEmbed(navigationFrames, 0, "https://missing.test/"),
    /no child frame matches/
  );
  assert.throws(
    () => frames.childFrameForEmbed([
      ...navigationFrames,
      { frameId: 9, parentFrameId: 0, url: "https://child.test/form/?session=2" }
    ], 0, "https://child.test/form/"),
    /several child frames match/
  );
});
