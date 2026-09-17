"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");

test("Firefox extension registers preload script using browser.scripting", async () => {
  let registered = null;
  let unregistered = null;

  globalThis.browser = {
    scripting: {
      async unregisterContentScripts(filter) {
        unregistered = filter;
      },
      async registerContentScripts(scripts) {
        registered = scripts;
      }
    },
    windows: {
      onFocusChanged: { addListener() {} }
    }
  };

  const bg = require("../background.js");
  const testScript = "console.log('Ghostlight Glass Web Components injected');";

  const result = await bg.handleSetPreloadScript(testScript);
  assert.equal(result.outcome, "set_preload_script");
  assert.equal(globalThis.ghostlightPreloadScript, testScript);

  assert.deepEqual(unregistered, { ids: ["ghostlight-glass"] });
  assert.equal(registered.length, 1);
  assert.equal(registered[0].id, "ghostlight-glass");
  assert.equal(registered[0].runAt, "document_start");
  assert.equal(registered[0].allFrames, true);
  assert.deepEqual(registered[0].matches, ["http://*/*", "https://*/*"]);
  assert.deepEqual(registered[0].js, [{ code: testScript }]);
});
