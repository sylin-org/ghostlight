"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");

test("Firefox extension executes commands and handles screenshots and window geometry", async () => {
  let updatedBounds = null;

  globalThis.browser = {
    tabs: {
      async get(tabId) {
        return { id: tabId, url: "https://example.com" };
      },
      async captureVisibleTab(windowId, options) {
        assert.equal(options.format, "jpeg");
        return "data:image/jpeg;base64,/9j/4AAQSkZJRg==";
      }
    },
    windows: {
      async getCurrent() {
        return { id: 1, left: 100, top: 100, width: 1200, height: 800, focused: true };
      },
      async get(windowId) {
        return { id: windowId, left: 150, top: 150, width: 1400, height: 900 };
      },
      async update(windowId, updateInfo) {
        updatedBounds = { windowId, ...updateInfo };
        return { id: windowId, ...updateInfo };
      },
      onFocusChanged: { addListener() {} }
    }
  };

  const bg = require("../background.js");

  // Screenshot test
  const screenshotResult = await bg.executeCommand({ command: "capture_screenshot", tab_id: 1 });
  assert.equal(screenshotResult.outcome, "screenshot");
  assert.equal(screenshotResult.mime_type, "image/jpeg");
  assert.equal(screenshotResult.data, "/9j/4AAQSkZJRg==");

  // Window geometry test
  const geoResult = await bg.executeCommand({
    command: "window_geometry",
    window_id: 1,
    bounds: { width: 1400, height: 900 }
  });
  assert.equal(geoResult.outcome, "window_resized");
  assert.equal(geoResult.width, 1400);
  assert.deepEqual(updatedBounds, { windowId: 1, width: 1400, height: 900 });

  // Describe documents test
  const descResult = await bg.executeCommand({ command: "describe_documents", tab_id: 1 });
  assert.equal(descResult.outcome, "documents");
  assert.equal(descResult.tab_id, 1);
  assert.equal(descResult.inventory.documents.length, 1);

  // In documents test
  const inDocResult = await bg.executeCommand({
    command: "in_documents",
    scope: { allowed: ["doc_1"] },
    primitive: { command: "screenshot", tab_id: 1 }
  });
  assert.equal(inDocResult.outcome, "in_documents");
  assert.equal(inDocResult.result.outcome, "screenshot");
  assert.deepEqual(inDocResult.observation.visited, ["doc_1"]);

  // Unsupported command test
  await assert.rejects(
    async () => {
      await bg.executeCommand({ command: "unsupported_primitive" });
    },
    /unsupported command/
  );
});
