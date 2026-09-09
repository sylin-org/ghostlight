"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");
const vm = require("node:vm");

// Exercise the shipped worker's sequencing. Content tests cover browser-edit preparation;
// the installed browser journey proves trusted events and retained controlled-editor values.
function fixture() {
  const source = readFileSync(join(__dirname, "../service-worker.js"), "utf8");
  const calls = [];
  const sandbox = {
    navigationWatchers: new Map(), cancelled: new Set(),
    documents: {
      verify: async () => { calls.push("documents"); },
      verifyInput: async () => { calls.push("input-admission"); }
    },
    content: async (tab, message) => { calls.push({ tab, message }); return { subject: { name: "Draft" } }; },
    firstFrameAnswer: async (_tab, message) => { calls.push(message.kind); },
    ensureDebugger: async () => { calls.push("attach"); },
    detachDebugger: async () => { calls.push("detach"); },
    sendDebugger: async () => { calls.push("insert"); },
    chrome: { tabs: { get: async id => ({ id }) } },
    physicalTab: tab => tab
  };
  vm.createContext(sandbox);
  for (const [start, end] of [["async function typeText(", "async function dispatchDrag("],
    ["async function typeFocused(", "async function inspectDialog("]]) {
    const first = source.indexOf(start), last = source.indexOf(end, first);
    assert.ok(first >= 0 && last > first);
    vm.runInContext(source.slice(first, last), sandbox);
  }
  return { calls, sandbox };
}

const command = { tab_id: 7, locator: "0:doc_top-1:locator_3", text: "Replacement", clear_first: true };

test("targeted typing sends one document-bound edit after identity admission", async () => {
  const { calls, sandbox } = fixture();
  const result = await sandbox.typeText("typing", command);
  assert.equal(result.outcome, "typed");
  assert.equal(result.subject.name, "Draft");
  assert.equal(result.character_count, 11);
  assert.equal(calls.length, 2);
  assert.equal(calls[0], "documents");
  assert.equal(calls[1].tab, 7);
  assert.equal(calls[1].message.kind, "type_text");
  assert.equal(calls[1].message.locator, command.locator);
  assert.equal(calls[1].message.clear_first, true);
  assert.equal(sandbox.navigationWatchers.size, 0);
});

test("a stale document refuses targeted typing before any editing message", async () => {
  const { calls, sandbox } = fixture();
  sandbox.documents.verify = async () => { throw Object.assign(new Error("changed"), { effectUnknown: false }); };
  await assert.rejects(sandbox.typeText("typing", command), { effectUnknown: false });
  assert.deepEqual(calls, []);
  assert.equal(sandbox.navigationWatchers.size, 0);
});

test("focused typing refuses absent desktop focus before clearing an existing draft", async () => {
  const { calls, sandbox } = fixture();
  sandbox.documents.verifyInput = async () => { throw Object.assign(new Error("no focus"), { effectUnknown: false }); };
  await assert.rejects(sandbox.typeFocused("typing", command), { effectUnknown: false });
  assert.deepEqual(calls, ["attach", "detach"]);
  assert.equal(sandbox.navigationWatchers.size, 0);
});

test("a lost reply after document-bound typing preserves effect uncertainty", async () => {
  const { sandbox } = fixture();
  sandbox.content = async () => { throw new Error("reply lost after edit"); };
  await assert.rejects(sandbox.typeText("typing", command), { effectUnknown: true });
  assert.equal(sandbox.navigationWatchers.size, 0);
});
