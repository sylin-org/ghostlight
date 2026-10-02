"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const vm = require("node:vm");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");

test("capture masks suppress embedded pixels without changing renderer visibility and expire cleanly", () => {
  const properties = new Map([["opacity", ["0.8", ""]], ["visibility", ["visible", ""]]]);
  const style = {
    getPropertyValue: name => properties.get(name)?.[0] ?? "",
    getPropertyPriority: name => properties.get(name)?.[1] ?? "",
    setProperty: (name, value, priority) => properties.set(name, [value, priority]),
    removeProperty: name => properties.delete(name)
  };
  const frame = { src: "https://excluded.test/", isConnected: true, style,
    getBoundingClientRect: () => ({ left: 20, top: 30, width: 100, height: 80 }) };
  let expiry;
  const sandbox = { URL, location: { href: "https://allowed.test/" }, scrollX: 0, scrollY: 0,
    captureMask: null, CAPTURE_MASK_TTL_MS: 10_000,
    queryAll: () => [frame], roots: () => [],
    MutationObserver: class { observe() {} disconnect() {} takeRecords() { return []; } },
    setTimeout: callback => { expiry = callback; return 1; }, clearTimeout() {},
    window: { getComputedStyle: element => ({ opacity: element.style.getPropertyValue("opacity"),
      visibility: element.style.getPropertyValue("visibility") }) },
    document: {
      documentElement: { append(element) { element.isConnected = true; } },
      createElement: () => ({ style: {}, isConnected: false, attachShadow: () => ({ append() {} }),
        remove() { this.isConnected = false; } })
    }
  };
  vm.createContext(sandbox);
  const source = readFileSync(join(__dirname, "../../crates/orchestrator/src/page_runtime/content.js"), "utf8");
  for (const name of ["clearCaptureMask", "installCaptureMask", "verifyCaptureMask"]) {
    vm.runInContext(source.match(new RegExp(`  function ${name}\\([^]*?\\n  }`))[0], sandbox);
  }
  assert.equal(sandbox.installCaptureMask({ urls: [frame.src], label: "Excluded" }).masked, 1);
  assert.equal(style.getPropertyValue("visibility"), "visible");
  assert.equal(style.getPropertyValue("opacity"), "0");
  assert.equal(style.getPropertyPriority("opacity"), "important");
  assert.equal(sandbox.verifyCaptureMask().valid, true);
  style.setProperty("opacity", "1", "important");
  assert.equal(sandbox.verifyCaptureMask().valid, false, "a visible embedded renderer cannot establish a safe capture");
  style.setProperty("opacity", "0", "important");
  expiry();
  assert.equal(sandbox.verifyCaptureMask().valid, false);
  assert.equal(style.getPropertyValue("opacity"), "0.8");
  assert.equal(style.getPropertyValue("visibility"), "visible");
  assert.equal(style.getPropertyValue("animation"), "");
});
