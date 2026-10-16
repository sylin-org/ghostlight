// Inject MCP replies at the installed driver's reporting boundary without starting any process.
import assert from "node:assert/strict";
import test from "node:test";
import { invoker } from "./lib/reporting.mjs";

function injected(reply) {
  const report = { invocations: [] };
  let saved = false;
  const call = invoker({ request: async () => reply, report, save: () => { saved = true; } });
  return { call, report, saved: () => saved };
}

test("protocol failure is saved and surfaced rather than masked by a reporter TypeError", async () => {
  const state = injected({ error: { code: -32000, message: "synthetic private detail" } });
  await assert.rejects(state.call("browser_tabs", {}), /Unexpected MCP protocol failure/);
  assert.equal(state.saved(), true);
  assert.equal(state.report.invocations[0].protocol_error_code, -32000);
  assert.equal(state.report.invocations[0].reason, null);
  assert.ok(!JSON.stringify(state.report).includes("synthetic private detail"));
});

test("missing result fails honestly and successful replies need no reason field", async () => {
  const missing = injected({ result: {} });
  await assert.rejects(missing.call("browser_tabs", {}), /browser_tabs did not succeed/);
  assert.equal(missing.saved(), true);
  const succeeded = injected({ result: { structuredContent: { status: "succeeded", effect: "none" } } });
  assert.equal((await succeeded.call("browser_tabs", {})).status, "succeeded");
  assert.equal(succeeded.report.invocations[0].reason, null);
});
