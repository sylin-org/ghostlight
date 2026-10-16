// Connection attribution through the real service on relay protocol 4, in a test home. Browser
// receipts are synthetic. Each receipt names the connection that started it and its channel, and
// nothing a client claims about itself reaches a durable record.
import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createHome, startReason, until } from "./lib/home.mjs";
import { McpEdge, RelayMcp, SyntheticBrowser, World, structured } from "./lib/peers.mjs";

const CANARIES = ["PRIVATE_C1_CLAIM_A", "PRIVATE_C1_CLAIM_B", "PRIVATE_C1_CLAIM_C", "PRIVATE_C1_SESSION", "PRIVATE_C1_PATH", "PRIVATE_C1_MCP_CLAIM"];
const [CLAIM_A, CLAIM_B, CLAIM_C, SESSION_KEY, PATH_CLAIM, MCP_CLAIM] = CANARIES;
const MARKER = { kind: "declared", key: SESSION_KEY };
const EVIDENCE_KEYS = ["channel", "connection_id", "observed_at_ms"];

const home = createHome({
  label: "provenance",
  policy: {
    schema: 4,
    name: "C1 fixture",
    version: "1",
    grants: [{ id: "sylin", hosts: { allow: ["sylin.org"] }, allowed: ["read", "action", "write", "execute"] }],
    config: [{ key: "browser.startup", value: "manual", level: "mandatory" }]
  }
});
const peers = [];
let browser;

function group(result) {
  return home.auditRecords().filter((record) => record.invocation === result.invocation);
}

function provenance(result) {
  const receipt = group(result).find((record) => !record.step);
  assert.ok(receipt, `Saved receipt for ${result.invocation}`);
  return receipt.provenance;
}

function checkEvidence(evidence, channel) {
  assert.ok(evidence, "New receipts carry connection provenance");
  assert.match(evidence.connection_id, /^connection_/);
  assert.ok(Number.isSafeInteger(evidence.observed_at_ms) && evidence.observed_at_ms > 0);
  assert.equal(evidence.channel, channel);
  assert.deepEqual(Object.keys(evidence).sort(), EVIDENCE_KEYS);
}

function checkGroup(result, evidence, expectedCount) {
  const receipts = group(result);
  assert.equal(receipts.length, expectedCount);
  for (const receipt of receipts) {
    assert.deepEqual(receipt.provenance, evidence);
    assert.equal(receipt.channel, evidence.channel, "the channel agrees with the connection evidence");
    assert.equal("peer_image" in receipt, false, "receipts name no peer executable");
  }
  return receipts;
}

function diagnosticText(path) {
  if (!existsSync(path)) return "";
  return readdirSync(path, { withFileTypes: true }).map((entry) => entry.isDirectory()
    ? diagnosticText(join(path, entry.name))
    : entry.isFile() ? readFileSync(join(path, entry.name), "utf8") : "").join("\n");
}

async function client(label, channel) {
  const peer = await RelayMcp.open(home, { channel, session: MARKER });
  peers.push(peer);
  await peer.initialize(label);
  return peer;
}

try {
  writeFileSync(home.file("diagnostics.on"), "");
  home.start(home.service, ["serve", "--reason", startReason.agent], { stdio: ["ignore", "ignore", "pipe"] });
  await home.serviceUp();
  const a = await client(`${CLAIM_A}\nC:\\${PATH_CLAIM}\\secret.exe`, "mcp");
  browser = await SyntheticBrowser.viaRelay(home, new World(), "browser_c1");
  await browser.connect();

  const opened = await a.facts("browser_navigate", { url: "https://sylin.org/" });
  assert.equal(opened.status, "succeeded", JSON.stringify(opened));
  const evidenceA = provenance(opened);
  checkEvidence(evidenceA, "mcp");
  browser.holdNext("call");
  const inFlight = a.facts("browser_read");
  await until(() => browser.held.length === 1, "A's read is dispatched before B joins");
  const queuedA = a.facts("browser_read");

  const b = await client(CLAIM_B, "cli");
  const explainedB = await b.facts("policy_explain");
  assert.equal(explainedB.status, "succeeded");
  const evidenceB = provenance(explainedB);
  checkEvidence(evidenceB, "cli");
  assert.notEqual(evidenceB.connection_id, evidenceA.connection_id);
  browser.release();
  const completedA = await inFlight;
  assert.equal(completedA.status, "succeeded");
  checkGroup(completedA, evidenceA, 1);
  const completedQueuedA = await queuedA;
  assert.equal(completedQueuedA.status, "succeeded");
  checkGroup(completedQueuedA, evidenceA, 1);
  checkGroup(explainedB, evidenceB, 1);
  console.log("PASS two connections of one session keep their own identity and channel during in-flight and queued work");

  const invalidFlow = await a.facts("browser_flow", { steps: [
    { id: "read", tool: "browser_read", arguments: { max_chars: 500 } },
    { id: "invalid", tool: "browser_read", arguments: { max_chars: { flow_ref: { step: "read", pointer: "/facts/text" } } } },
    { id: "unreached", tool: "browser_read", arguments: {} }
  ] });
  assert.equal(invalidFlow.status, "failed");
  assert.equal(invalidFlow.facts.steps[1].status, "not_started");
  assert.equal(invalidFlow.facts.steps[2].status, "not_run");
  const flowReceipts = checkGroup(invalidFlow, evidenceA, 3);
  assert.equal(flowReceipts[0].step.position, 1);
  assert.equal(flowReceipts[1].step.preparation_failed, true);
  assert.equal(flowReceipts.at(-1).step, undefined);
  const invalid = await a.facts("browser_read", { max_chars: "invalid" });
  assert.equal(invalid.status, "failed");
  checkGroup(invalid, evidenceA, 1);
  const openedBeforeRefusal = browser.requested("open_tab").length;
  const refused = await a.facts("browser_navigate", { url: "https://excluded.example/", new_tab: true });
  assert.equal(refused.status, "blocked");
  assert.equal(refused.effect, "none");
  checkGroup(refused, evidenceA, 1);
  assert.equal(browser.requested("open_tab").length, openedBeforeRefusal);
  console.log("PASS direct, composed child, preparation-failure, parent and refusal receipts keep the initiating connection");

  b.close();
  await b.stream.ended;
  const c = await client(CLAIM_C, "cli");
  const explainedC = await c.facts("policy_explain");
  assert.equal(explainedC.status, "succeeded");
  const evidenceC = provenance(explainedC);
  checkEvidence(evidenceC, "cli");
  assert.notEqual(evidenceC.connection_id, evidenceB.connection_id, "a reconnect observes fresh evidence");
  assert.notEqual(evidenceC.connection_id, evidenceA.connection_id);
  assert.ok(evidenceC.observed_at_ms >= evidenceB.observed_at_ms);
  checkGroup(explainedC, evidenceC, 1);
  checkGroup(completedA, evidenceA, 1);
  checkGroup(await a.facts("policy_explain"), evidenceA, 1);

  const mcp = new McpEdge(home.start(home.edge, ["mcp"]));
  await mcp.initialize(MCP_CLAIM);
  const explainedMcp = structured(await mcp.call("policy_explain"));
  assert.equal(explainedMcp.status, "succeeded");
  const evidenceMcp = provenance(explainedMcp);
  checkEvidence(evidenceMcp, "mcp");
  assert.notEqual(evidenceMcp.connection_id, evidenceA.connection_id);
  checkGroup(explainedMcp, evidenceMcp, 1);
  mcp.child.stdin.end();

  const saved = readFileSync(home.auditFile, "utf8");
  const diagnostic = diagnosticText(home.file("diagnostics"));
  assert.ok(diagnostic.length > 0, "diagnostics are on for the confidentiality check");
  assert.match(diagnostic, /"component"\s*:\s*"edge-mcp"/, "the edge wrote its own diagnostics");
  for (const canary of CANARIES) {
    assert.equal(saved.includes(canary), false, `${canary} is absent from the audit`);
    assert.equal(diagnostic.includes(canary), false, `${canary} is absent from diagnostics`);
  }
  assert.equal(saved.includes(JSON.stringify(process.execPath).slice(1, -1)), false, "full executable paths stay out of the audit");
  console.log("PASS a reconnect refreshes evidence; the real MCP edge records an mcp connection; claims, session keys and paths stay out of durable records");
  console.log(`provenance journey ok: ${home.auditRecords().length} receipts, per-connection identity and channel only`);
} finally {
  for (const peer of peers) peer.close();
  browser?.close();
  await home.cleanup();
}
