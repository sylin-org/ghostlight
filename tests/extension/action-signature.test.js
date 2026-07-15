// SPDX-License-Identifier: Apache-2.0 OR MIT

const { test } = require("node:test");
const assert = require("node:assert/strict");
const signature = require("../../extension/lib/action-signature.js");

test("action signature messages are fixed and content-free", () => {
  const value = signature.message(signature.KINDS.JAVASCRIPT, signature.PHASES.START);
  assert.deepEqual(value, {
    type: "AGENT_ACTION_SIGNATURE",
    kind: "javascript",
    phase: "start",
  });
  assert.equal(signature.isMessage(value), true);
  assert.deepEqual(Object.keys(value).sort(), ["kind", "phase", "type"]);
});

test("action signature vocabulary rejects unknown kinds and phases", () => {
  assert.throws(() => signature.message("code", signature.PHASES.START), /unknown.*kind/);
  assert.throws(() => signature.message(signature.KINDS.WAIT, "progress"), /unknown.*phase/);
  assert.equal(signature.isMessage({ type: signature.TYPE, kind: "code", phase: "start" }), false);
  assert.equal(signature.isMessage({ type: signature.TYPE, kind: "wait", phase: "progress" }), false);
});
