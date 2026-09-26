"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const { createHash, webcrypto } = require("node:crypto");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");
const vm = require("node:vm");

function fixture() {
  const source = readFileSync(join(__dirname, "..", "service-worker.js"), "utf8");
  const sent = [];
  const sandbox = {
    crypto: webcrypto,
    TextEncoder,
    Uint8Array,
    Object,
    Number,
    debuggerLifecycle: {
      async installPageRuntime(script) {
        sent.push(script);
      }
    }
  };
  vm.createContext(sandbox);
  for (const name of ["sha256Text", "installPageRuntime"]) {
    const body = source.match(new RegExp(`async function ${name}\\([^]*?\\n}`));
    assert.ok(body, name);
    vm.runInContext(body[0], sandbox);
  }
  return { sandbox, sent };
}

test("page runtime installation verifies, injects, and acknowledges the exact bundle", async () => {
  const { sandbox, sent } = fixture();
  const script = "globalThis.__ghostlight_runtime_test__ = true;";
  const sha256 = createHash("sha256").update(script).digest("hex");
  const command = { revision: 1, sha256, script };

  const result = await sandbox.installPageRuntime(command);
  assert.equal(result.outcome, "page_runtime_installed");
  assert.equal(result.revision, 1);
  assert.equal(result.sha256, sha256);
  assert.deepEqual(sent, [script]);

  await sandbox.installPageRuntime(command);
  assert.equal(sent.length, 1, "the same immutable bundle is reused without reinjection");
});

test("page runtime installation refuses mismatched content before browser effects", async () => {
  const { sandbox, sent } = fixture();
  await assert.rejects(
    sandbox.installPageRuntime({ revision: 1, sha256: "0".repeat(64), script: "different" }),
    /checksum mismatch/
  );
  assert.equal(sent.length, 0);
});
