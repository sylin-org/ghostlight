// C1 reporting through a real isolated service. Browser receipts are synthetic Sylin fixtures.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createConnection } from "node:net";
import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { createInterface } from "node:readline";

const root = resolve(import.meta.dirname, "..");
const delay = ms => new Promise(done => setTimeout(done, ms));
async function until(test, label, timeout = 8000) {
  const end = Date.now() + timeout;
  while (Date.now() < end) { if (test()) return; await delay(20); }
  throw new Error(`Timed out: ${label}`);
}

class Peer {
  constructor(port, native = false, child = null) {
    this.messages = []; this.nextId = 0; this.native = native; this.child = child;
    if (child) {
      child.on("message", value => this.receive(value));
      return;
    }
    this.socket = createConnection({ host: "127.0.0.1", port });
    this.socket.on("error", () => {});
    this.buffer = Buffer.alloc(0);
    this.socket.on("data", bytes => {
      this.buffer = Buffer.concat([this.buffer, bytes]);
      for (;;) {
        const length = native ? (this.buffer.length >= 4 ? this.buffer.readUInt32LE() : -1) : this.buffer.indexOf(10);
        const header = native ? 4 : 0;
        const consumed = length + (native ? 4 : 1);
        if (length < 0 || this.buffer.length < consumed) return;
        const value = JSON.parse(this.buffer.subarray(header, length + header));
        this.buffer = this.buffer.subarray(consumed);
        this.receive(value);
      }
    });
  }
  receive(value) { this.messages.push(value); this.onMessage?.(value); }
  send(value) {
    if (this.child) { this.child.send(value); return; }
    const bytes = Buffer.from(JSON.stringify(value));
    if (!this.native) { this.socket.write(Buffer.concat([bytes, Buffer.from("\n")])); return; }
    const header = Buffer.alloc(4); header.writeUInt32LE(bytes.length);
    this.socket.write(Buffer.concat([header, bytes]));
  }
  async take(test, label, timeout) {
    await until(() => this.messages.some(test), label, timeout);
    return this.messages.splice(this.messages.findIndex(test), 1)[0];
  }
  invoke(tool, input = {}) {
    const id = `call-${++this.nextId}`;
    this.send({ kind: "invoke", id, tool, input }); return id;
  }
  async result(id) { return (await this.take(value => value.kind === "result" && value.id === id, id)).result; }
}

class McpPeer {
  constructor(child) {
    this.child = child; this.messages = []; this.nextId = 0;
    createInterface({ input: child.stdout }).on("line", line => this.messages.push(JSON.parse(line)));
  }
  async request(method, params) {
    const id = ++this.nextId;
    this.child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`);
    await until(() => this.messages.some(value => value.id === id), `MCP ${method}`);
    const response = this.messages.splice(this.messages.findIndex(value => value.id === id), 1)[0];
    assert.equal(response.error, undefined, JSON.stringify(response.error));
    return response.result;
  }
  notify(method) { this.child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", method })}\n`); }
}

if (process.argv[2] === "--peer") {
  // The copied executable gives the operating system a genuinely different direct peer image.
  const peer = new Peer(Number(process.argv[3]));
  peer.onMessage = value => process.send(value);
  process.on("message", value => peer.send(value));
  process.on("disconnect", () => { peer.socket.destroy(); process.exit(0); });
} else {
  await journey();
}

async function journey() {
  const binaries = resolve(process.env.GHOSTLIGHT_BIN_DIR ?? join(root, ".target-ghostlight-1.0/debug"));
  const scratch = join(root, ".tmp", `c1-${process.pid}`);
  const runtime = join(binaries, `.c1-runtime-${process.pid}.json`);
  const audit = join(scratch, "audit.jsonl"), diagnostics = join(scratch, "diagnostics");
  const policy = join(scratch, "policy.json");
  const suffix = process.platform === "win32" ? ".exe" : "";
  const alternateExecutable = join(scratch, `c1-secondary-peer${suffix}`);
  assert.equal(dirname(scratch), join(root, ".tmp"));
  assert.equal(dirname(runtime), binaries);
  assert.equal(dirname(alternateExecutable), scratch);
  assert.ok(existsSync(join(binaries, `ghostlight${suffix}`)), "Build the workspace into GHOSTLIGHT_BIN_DIR first.");
  mkdirSync(scratch, { recursive: true });
  const privateCanaries = ["PRIVATE_C1_CLAIM_A", "PRIVATE_C1_CLAIM_B", "PRIVATE_C1_CLAIM_C", "PRIVATE_C1_SESSION", "PRIVATE_C1_PATH", "PRIVATE_C1_MCP_CLAIM"];
  const marker = { kind: "declared", key: privateCanaries[3] };
  const environment = { ...process.env,
    GHOSTLIGHT_RUNTIME_FILE: runtime, GHOSTLIGHT_POLICY_FILE: policy, GHOSTLIGHT_AUDIT_FILE: audit,
    GHOSTLIGHT_DIAGNOSTICS_DIR: diagnostics, GHOSTLIGHT_NATIVE_HOST_DIR: join(scratch, "native-host") };
  const peers = [], children = [];
  let output = "";
  function start(executable, args, options = {}) {
    const child = spawn(executable, args, { windowsHide: true, stdio: ["ignore", "ignore", "pipe"], ...options });
    children.push(child);
    child.stderr.on("data", bytes => { output = (output + bytes).slice(-6000); });
    return child;
  }
  function records() {
    if (!existsSync(audit)) return [];
    const text = readFileSync(audit, "utf8");
    return text.slice(0, text.lastIndexOf("\n") + 1).split("\n").filter(Boolean).map(JSON.parse);
  }
  function group(result) { return records().filter(record => record.invocation === result.invocation); }
  function provenance(result) {
    const receipt = group(result).find(record => !record.step);
    assert.ok(receipt, `Saved receipt for ${result.invocation}`);
    return receipt.provenance;
  }
  function checkEvidence(evidence, channel, image) {
    assert.ok(evidence, "New receipts carry connection provenance");
    assert.match(evidence.connection_id, /^connection_/);
    assert.ok(Number.isSafeInteger(evidence.observed_at_ms) && evidence.observed_at_ms > 0);
    assert.equal(evidence.channel, channel);
    assert.equal(evidence.signature, "not_checked");
    assert.deepEqual(evidence.peer, process.platform === "win32"
      ? { state: "observed", executable: image.toLowerCase() }
      : { state: "unsupported_platform" });
  }
  function checkGroup(result, evidence, expectedCount) {
    const receipts = group(result);
    assert.equal(receipts.length, expectedCount);
    for (const receipt of receipts) {
      assert.deepEqual(receipt.provenance, evidence);
      assert.equal(receipt.channel, evidence.channel, "legacy channel agrees with connection evidence");
      assert.equal(receipt.peer_image, evidence.peer.executable, "legacy image agrees with connection evidence");
    }
    return receipts;
  }
  function diagnosticText(path) {
    if (!existsSync(path)) return "";
    return readdirSync(path, { withFileTypes: true }).map(entry => entry.isDirectory()
      ? diagnosticText(join(path, entry.name))
      : entry.isFile() ? readFileSync(join(path, entry.name), "utf8") : "").join("\n");
  }
  try {
    writeFileSync(policy, JSON.stringify({ schema: 3, name: "C1 fixture", version: "1", grants: [
      { id: "sylin", hosts: { allow: ["sylin.org"] }, allowed: ["read", "action", "write", "execute"] }
    ], config: [{ key: "browser.startup", value: "manual", level: "mandatory" }] }));
    start(join(binaries, `ghostlight${suffix}`), [], { env: environment });
    await until(() => { try { return JSON.parse(readFileSync(runtime)).service_port; } catch { return false; } }, "isolated service starts");
    const endpoint = JSON.parse(readFileSync(runtime));
    async function client(label, channel = "mcp", alternate = false) {
      const child = alternate ? start(alternateExecutable, [import.meta.filename, "--peer", String(endpoint.service_port)],
        { stdio: ["ignore", "ignore", "pipe", "ipc"] }) : null;
      const peer = new Peer(endpoint.service_port, false, child); peers.push(peer);
      peer.send({ kind: "hello", major: 2, token: endpoint.token, client_label: label, channel, session: marker });
      peer.session = (await peer.take(value => value.kind === "hello_accepted", "authenticated client")).session;
      return peer;
    }
    const a = await client(`${privateCanaries[0]}\nC:\\${privateCanaries[4]}\\secret.exe`);
    const browser = new Peer(endpoint.browser_port, true); peers.push(browser);
    browser.send({ kind: "hello", major: 1, token: endpoint.token });
    browser.send({ kind: "hello", major: 2, adapter_version: "1.0.0", browser_id: "browser_c1", adapter_epoch: "adapter_c1",
      capabilities: ["document_scope", "tabs", "atomic_tab_open", "navigation", "semantic_document", "presentation", "adapter_liveness"]
        .map(name => ({ name, revision: { navigation: 2, semantic_document: 4 }[name] ?? 1 })) });
    await browser.take(value => value.kind === "hello_accepted", "adapter negotiation");
    let tab, holdNextRead = false, heldRead;
    const physicalCommands = [];
    browser.onMessage = frame => {
      if (frame.kind === "heartbeat") { browser.send({ kind: "heartbeat_ack", sequence: frame.sequence }); return; }
      if (frame.kind !== "request") return;
      const request = frame.request, scope = request.command.scope, command = request.command.primitive ?? request.command;
      physicalCommands.push(command.command);
      function reply(result) {
        if (scope) result = { outcome: "in_documents", result,
          observation: { visited: scope.allowed, unavailable: [], limited_by_size: false, masked_regions: 0 } };
        browser.send({ kind: "receipt", receipt: { correlation: request.correlation, result } });
      }
      if (command.command === "present") reply({ outcome: "presented", rendered: true });
      else if (command.command === "list_tabs") reply({ outcome: "tabs", tabs: tab ? [tab] : [] });
      else if (command.command === "open_tab") {
        tab = { tab_id: 1, title: "C1 page", url: command.url, readiness: "complete", active: true };
        reply({ outcome: "tab_opened", tab, committed_urls: [tab.url] });
      } else if (command.command === "describe_documents") reply({ outcome: "documents", tab_id: tab.tab_id, inventory: {
        documents: [{ id: "document-c1", url: tab.url, parent: null, supported: true }], subjects: [], unresolved: false, incomplete: false } });
      else if (command.command === "read_document" || command.command === "read_text") {
        const complete = () => reply({ outcome: "text", tab_id: tab.tab_id, text: "C1 permitted page content", title: tab.title, url: tab.url, truncated: false });
        if (holdNextRead) { holdNextRead = false; heldRead = complete; } else complete();
      } else if (command.command === "clear_diagnostics") reply({ outcome: "diagnostics_cleared" });
      else if (command.command === "close_tab") reply({ outcome: "tab_closed", tab_id: command.tab_id });
      else throw new Error(`Unexpected C1 primitive ${command.command}`);
    };

    const opened = await a.result(a.invoke("browser_navigate", { url: "https://sylin.org/" }));
    assert.equal(opened.status, "succeeded", JSON.stringify(opened));
    const evidenceA = provenance(opened);
    checkEvidence(evidenceA, "mcp", basename(process.execPath));
    holdNextRead = true;
    const inFlight = a.invoke("browser_read");
    await until(() => heldRead, "A read dispatched before B joins");
    const queuedA = a.invoke("browser_read");

    copyFileSync(process.execPath, alternateExecutable);
    const b = await client(privateCanaries[1], "cli", true);
    assert.equal(b.session, a.session, "two different peers share the declared workspace");
    const explainedB = await b.result(b.invoke("policy_explain"));
    assert.equal(explainedB.status, "succeeded");
    const evidenceB = provenance(explainedB);
    checkEvidence(evidenceB, "cli", basename(alternateExecutable));
    assert.notEqual(evidenceB.connection_id, evidenceA.connection_id);
    heldRead();
    const completedA = await a.result(inFlight);
    assert.equal(completedA.status, "succeeded");
    checkGroup(completedA, evidenceA, 1);
    const completedQueuedA = await a.result(queuedA);
    assert.equal(completedQueuedA.status, "succeeded");
    checkGroup(completedQueuedA, evidenceA, 1);
    checkGroup(explainedB, evidenceB, 1);
    console.log("PASS different same-session peers retain per-connection executable and channel attribution during in-flight and queued work");

    const invalidFlow = await a.result(a.invoke("browser_flow", { steps: [
      { id: "read", tool: "browser_read", arguments: { max_chars: 500 } },
      { id: "invalid", tool: "browser_read", arguments: { max_chars: { flow_ref: { step: "read", pointer: "/facts/text" } } } },
      { id: "unreached", tool: "browser_read", arguments: {} }
    ] }));
    assert.equal(invalidFlow.status, "failed");
    assert.equal(invalidFlow.facts.steps[1].status, "not_started");
    assert.equal(invalidFlow.facts.steps[2].status, "not_run");
    const flowReceipts = checkGroup(invalidFlow, evidenceA, 3);
    assert.equal(flowReceipts[0].step.position, 1);
    assert.equal(flowReceipts[1].step.preparation_failed, true);
    assert.equal(flowReceipts.at(-1).step, undefined);
    const invalid = await a.result(a.invoke("browser_read", { max_chars: "invalid" }));
    assert.equal(invalid.status, "failed");
    checkGroup(invalid, evidenceA, 1);
    const openedBeforeRefusal = physicalCommands.filter(command => command === "open_tab").length;
    const refused = await a.result(a.invoke("browser_navigate", { url: "https://excluded.example/", new_tab: true }));
    assert.equal(refused.status, "blocked");
    assert.equal(refused.effect, "none");
    checkGroup(refused, evidenceA, 1);
    assert.equal(physicalCommands.filter(command => command === "open_tab").length, openedBeforeRefusal);
    console.log("PASS direct, composed child, preparation-failure, parent and refusal receipts preserve the initiating connection");

    b.child.disconnect();
    await until(() => b.child.exitCode !== null || b.child.signalCode !== null, "B disconnected");
    const c = await client(privateCanaries[2], "cli", true);
    assert.equal(c.session, a.session);
    const explainedC = await c.result(c.invoke("policy_explain"));
    assert.equal(explainedC.status, "succeeded");
    const evidenceC = provenance(explainedC);
    checkEvidence(evidenceC, "cli", basename(alternateExecutable));
    assert.notEqual(evidenceC.connection_id, evidenceB.connection_id, "reconnect observes fresh connection evidence");
    assert.notEqual(evidenceC.connection_id, evidenceA.connection_id);
    assert.ok(evidenceC.observed_at_ms >= evidenceB.observed_at_ms);
    checkGroup(explainedC, evidenceC, 1);
    checkGroup(completedA, evidenceA, 1);
    const stillA = await a.result(a.invoke("policy_explain"));
    checkGroup(stillA, evidenceA, 1);
    const mcp = new McpPeer(start(join(binaries, `ghostlight-mcp-connector${suffix}`), [],
      { env: environment, stdio: ["pipe", "pipe", "pipe"] }));
    await mcp.request("initialize", { protocolVersion: "2025-11-25", capabilities: {}, clientInfo: { name: privateCanaries[5], version: "1" } });
    mcp.notify("notifications/initialized");
    const explainedMcp = (await mcp.request("tools/call", { name: "policy_explain", arguments: {} })).structuredContent;
    assert.equal(explainedMcp.status, "succeeded");
    const evidenceMcp = provenance(explainedMcp);
    checkEvidence(evidenceMcp, "mcp", `ghostlight-mcp-connector${suffix}`);
    assert.notEqual(evidenceMcp.connection_id, evidenceA.connection_id);
    checkGroup(explainedMcp, evidenceMcp, 1);
    const saved = readFileSync(audit, "utf8"), diagnostic = diagnosticText(diagnostics);
    assert.ok(diagnostic.length > 0, "diagnostics are enabled for the claim-confidentiality proof");
    assert.match(diagnostic, /"component"\s*:\s*"mcp-connector"/, "the connector wrote its own diagnostics");
    for (const canary of privateCanaries) {
      assert.equal(saved.includes(canary), false, `${canary} absent from audit`);
      assert.equal(diagnostic.includes(canary), false, `${canary} absent from diagnostics`);
    }
    assert.equal(saved.includes(JSON.stringify(alternateExecutable).slice(1, -1)), false, "full executable paths stay out of audit");
    console.log("PASS reconnect refreshes evidence; surviving peers retain theirs; raw application claims, session keys and paths stay out of durable records");
    console.log("PASS actual MCP initialization records the connector executable; its application claim stays out of connector and service diagnostics");
    console.log(`provenance journey ok: ${records().length} real service receipts, ${process.platform === "win32" ? "three OS-observed executable images" : "explicit unsupported-platform evidence"}, no signature trust inferred`);
  } catch (error) {
    process.stderr.write(output); throw error;
  } finally {
    for (const peer of peers) {
      peer.socket?.destroy();
      if (peer.child?.connected) peer.child.disconnect();
    }
    for (const child of [...children].reverse()) {
      if (child.exitCode !== null || child.signalCode !== null) continue;
      const exited = new Promise(done => child.once("exit", done)); child.kill(); await exited;
    }
    rmSync(runtime, { force: true });
    rmSync(runtime.replace(/\.json$/, ".lock"), { force: true });
    rmSync(scratch, { recursive: true, force: true });
  }
}
