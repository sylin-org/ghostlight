"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");

test("Firefox extension emits valid BrowserFrame::Hello with ghostlight/gecko platform", async () => {
  const sentMessages = [];
  const portListeners = {
    message: [],
    disconnect: []
  };

  const mockPort = {
    postMessage(msg) {
      sentMessages.push(msg);
    },
    onMessage: {
      addListener(fn) { portListeners.message.push(fn); }
    },
    onDisconnect: {
      addListener(fn) { portListeners.disconnect.push(fn); }
    }
  };

  const mockStorage = {
    data: {},
    async get(key) { return { [key]: this.data[key] }; },
    async set(items) { Object.assign(this.data, items); }
  };

  globalThis.browser = {
    storage: { local: mockStorage },
    runtime: {
      connectNative(name) {
        assert.equal(name, "org.sylin.ghostlight");
        return mockPort;
      },
      getManifest() {
        return { version: "1.3.9" };
      }
    },
    windows: {
      async getCurrent() {
        return { id: 1, focused: true };
      },
      onFocusChanged: {
        addListener() {}
      }
    }
  };

  // Require background script under the mock
  const bg = require("../background.js");
  await bg.connectNative("test");

  assert.equal(sentMessages.length, 1);
  const hello = sentMessages[0];

  assert.equal(hello.kind, "hello");
  assert.equal(hello.major, 2);
  assert.equal(hello.adapter_version, "1.3.9");
  assert.equal(hello.browser_name, "Firefox");
  assert.equal(hello.platform, "ghostlight/gecko");
  assert.equal(hello.attended, true);
  assert.match(hello.browser_id, /^browser_firefox_[0-9a-f]{16}$/);
  assert.match(hello.adapter_epoch, /^adapter_[0-9a-f]{16}$/);

  // Validate advertised capabilities
  const capNames = hello.capabilities.map((c) => c.name);
  assert.ok(capNames.includes("tabs"));
  assert.ok(capNames.includes("atomic_tab_open"));
  assert.ok(capNames.includes("navigation"));
  assert.ok(capNames.includes("window_geometry"));
  assert.ok(capNames.includes("capture"));
  assert.ok(capNames.includes("script"));
});
