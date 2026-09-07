// Isolated real-service H8 journey. Synthetic adapters and clients; never the installed runtime.
import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { createConnection } from "node:net";
import { readFileSync, writeFileSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { resolve, join, dirname } from "node:path";

const root = resolve(import.meta.dirname, "..");
const binaries = resolve(process.env.GHOSTLIGHT_BIN_DIR ?? join(root, ".target-ghostlight-1.0/debug"));
const scratch = join(root, ".tmp", `h8-${process.pid}`);
const runtime = join(binaries, `.h8-runtime-${process.pid}.json`);
assert.equal(dirname(scratch), join(root, ".tmp"));
assert.equal(dirname(runtime), binaries);
mkdirSync(scratch, { recursive: true });
const policy = join(scratch, "policy.json");
writeFileSync(policy, JSON.stringify({ schema: 3, name: "H8 fixture", version: "1", grants: [{ id: "all", hosts: { allow: ["*"] }, allowed: ["read", "action", "write", "execute"] }],
  config: [{ key: "browser.startup", value: "manual", level: "mandatory" }] }));
const sockets = [];
const delay = ms => new Promise(resolvePromise => setTimeout(resolvePromise, ms));
async function until(test, label, timeout = 8000) {
  const end = Date.now() + timeout;
  while (Date.now() < end) { if (test()) return; await delay(20); }
  throw new Error(`Timed out: ${label}`);
}
function socket(port) {
  const peer = createConnection({ host: "127.0.0.1", port });
  peer.on("error", () => {});
  sockets.push(peer); peer.resume();
  return peer;
}
class Peer {
  constructor(port, native = false) {
    this.socket = socket(port); this.native = native; this.buffer = Buffer.alloc(0); this.messages = []; this.nextId = 0;
    this.socket.on("data", bytes => {
      this.buffer = Buffer.concat([this.buffer, bytes]);
      for (;;) {
        const length = native ? (this.buffer.length >= 4 ? this.buffer.readUInt32LE() : -1) : this.buffer.indexOf(10);
        const header = native ? 4 : 0;
        const consumed = length + (native ? 4 : 1);
        if (length < 0 || this.buffer.length < consumed) return;
        const value = JSON.parse(this.buffer.subarray(header, length + header));
        this.buffer = this.buffer.subarray(consumed);
        this.messages.push(value); this.onMessage?.(value);
      }
    });
  }
  encode(value) {
    const bytes = Buffer.from(JSON.stringify(value));
    if (!this.native) return Buffer.concat([bytes, Buffer.from("\n")]);
    const header = Buffer.alloc(4); header.writeUInt32LE(bytes.length); return Buffer.concat([header, bytes]);
  }
  send(value) { this.socket.write(this.encode(value)); }
  async take(test, label, timeout) {
    await until(() => this.messages.some(test), label, timeout);
    return this.messages.splice(this.messages.findIndex(test), 1)[0];
  }
  invoke(tool, input = {}, id = `call-${++this.nextId}`, deadline_ms) {
    this.send({ kind: "invoke", id, tool, input, deadline_ms }); return id;
  }
  async result(id, timeout) { return (await this.take(value => value.kind === "result" && value.id === id, id, timeout)).result; }
}
let child;
let output = "";
try {
  writeFileSync(runtime, "{}"); // Existing inherited-access discovery must be replaced privately.
  child = spawn(join(binaries, process.platform === "win32" ? "ghostlight.exe" : "ghostlight"), [], {
    windowsHide: true, stdio: ["ignore", "ignore", "pipe"], env: { ...process.env,
      GHOSTLIGHT_RUNTIME_FILE: runtime, GHOSTLIGHT_POLICY_FILE: policy,
      GHOSTLIGHT_AUDIT_FILE: join(scratch, "audit.jsonl"), GHOSTLIGHT_DIAGNOSTICS_DIR: join(scratch, "diagnostics"),
      GHOSTLIGHT_NATIVE_HOST_DIR: join(scratch, "native-host") }
  });
  child.stderr.on("data", bytes => { output = (output + bytes).slice(-6000); });
  await until(() => { try { return JSON.parse(readFileSync(runtime)).service_port; } catch { return false; } }, "isolated service starts");
  const endpoint = JSON.parse(readFileSync(runtime));
  if (process.platform === "win32") {
    const script = `$ErrorActionPreference = 'Stop'; $a = [System.IO.File]::GetAccessControl('${runtime.replaceAll("'", "''")}'); [PSCustomObject]@{Protected=$a.AreAccessRulesProtected;Broad=@($a.Access | Where-Object { $_.IdentityReference.Translate([System.Security.Principal.SecurityIdentifier]).Value -in @('S-1-1-0','S-1-5-11','S-1-5-32-545') }).Count} | ConvertTo-Json -Compress`;
    const acl = JSON.parse(execFileSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", script], { windowsHide: true, encoding: "utf8" }));
    assert.deepEqual(acl, { Protected: true, Broad: 0 });
  }
  console.log("PASS private runtime publication");
  async function client(label) {
    const peer = new Peer(endpoint.service_port);
    peer.send({ kind: "hello", major: 2, token: endpoint.token, client_label: label });
    peer.session = (await peer.take(value => value.kind === "hello_accepted", "authenticated client")).session;
    return peer;
  }
  const a = await client("H8 A"), b = await client("H8 B");
  const browser = new Peer(endpoint.browser_port, true);
  const adapterHello = { kind: "hello", major: 2, adapter_version: "1.0.0", browser_id: "browser_h8",
    adapter_epoch: "adapter_h8", capabilities: ["document_scope", "tabs", "atomic_tab_open", "navigation", "semantic_document", "presentation", "adapter_liveness"]
      .map(name => ({ name, revision: { navigation: 2, semantic_document: 4 }[name] ?? 1 })) };
  // Both hellos in one write pin preservation of buffered bytes across transport authentication.
  browser.socket.write(Buffer.concat([browser.encode({ kind: "hello", major: 1, token: endpoint.token }), browser.encode(adapterHello)]));
  await browser.take(value => value.kind === "hello_accepted", "adapter negotiation");
  let nextTab = 0, hold = null, readDelay = 0;
  const tabs = new Map(), requests = [], held = [];
  browser.onMessage = frame => {
    if (frame.kind === "heartbeat") { browser.send({ kind: "heartbeat_ack", sequence: frame.sequence }); return; }
    if (frame.kind !== "request") return;
    const request = frame.request, scope = request.command.scope;
    const command = request.command.primitive ?? request.command;
    requests.push({ ...request, command });
    function reply(result) {
      if (scope) result = { outcome: "in_documents", result, observation: { visited: scope.allowed, unavailable: [], limited_by_size: false, masked_regions: 0 } };
      browser.send({ kind: "receipt", receipt: { correlation: request.correlation, result } });
    }
    if (command.command === "present") reply({ outcome: "presented", rendered: true });
    else if (command.command === "list_tabs") reply({ outcome: "tabs", tabs: [] });
    else if (command.command === "open_tab") {
      const tab = { tab_id: ++nextTab, title: "H8 page", url: command.url, readiness: "complete", active: true };
      tabs.set(tab.tab_id, tab); reply({ outcome: "tab_opened", tab, committed_urls: [tab.url] });
    } else if (command.command === "describe_documents") reply({ outcome: "documents", tab_id: command.tab_id, inventory: {
      documents: [{ id: "document-h8", url: tabs.get(command.tab_id).url, parent: null, supported: true }], subjects: [], unresolved: false, incomplete: false } });
    else if (command.command === "read_document" || command.command === "read_text") {
      const complete = () => reply({ outcome: "text", tab_id: command.tab_id, text: "H8 allowed content", title: "H8 page", url: tabs.get(command.tab_id).url, truncated: false });
      if (hold === request.workspace) held.push(request); else setTimeout(complete, readDelay);
    } else if (command.command === "cancel") reply({ outcome: "cancelled" });
    else if (command.command === "clear_diagnostics") reply({ outcome: "diagnostics_cleared" });
    else if (command.command === "close_tab") reply({ outcome: "tab_closed", tab_id: command.tab_id });
    else throw new Error(`Unexpected H8 primitive ${command.command}`);
  };
  for (const peer of [a, b]) {
    const result = await peer.result(peer.invoke("browser_navigate", { url: "https://sylin.org/" }));
    assert.equal(result.status, "succeeded", JSON.stringify(result));
  }
  // Small bounded abandoned-peer fixture, alongside two admitted clients and the adapter.
  const abandoned = Array.from({ length: 4 }, () => socket(endpoint.service_port));
  abandoned[1].write("{"); abandoned[2].write("{\"kind\":");
  const nativeAbandoned = socket(endpoint.browser_port); nativeAbandoned.write(Buffer.from([10]));
  const malformed = socket(endpoint.service_port); malformed.write("{broken\n");
  const oversized = socket(endpoint.service_port); oversized.write(Buffer.alloc(8 * 1024 * 1024 + 8192, 32));
  assert.equal((await b.result(b.invoke("browser_read"))).status, "succeeded");
  await until(() => [...abandoned, nativeAbandoned, malformed, oversized].every(peer => peer.destroyed), "abandoned and malformed peers retired", 7500);
  assert.equal((await a.result(a.invoke("policy_explain"))).status, "succeeded");
  console.log("PASS abandoned, malformed, oversized peers; healthy idle clients survive");

  hold = a.session;
  a.invoke("browser_read", {}, "original");
  await until(() => held.length === 1, "original dispatched");
  a.invoke("browser_read", {}, "original");
  await a.take(value => value.kind === "error" && value.code === "duplicate_request", "duplicate rejected");
  a.send({ kind: "cancel", id: "original" });
  const uncertain = await a.result("original", 2000);
  assert.equal(uncertain.effect, "unknown"); assert.equal(uncertain.repeat_safe, false);
  assert.equal(held.length, 1);
  console.log("PASS duplicate rejection preserves original cancellation; uncertain work is not replayed");

  hold = null; readDelay = 65;
  const burst = Array.from({ length: 12 }, (_, i) => a.invoke("browser_read", {}, `burst-${i}`));
  for (const id of burst) assert.equal((await a.result(id)).status, "succeeded");
  console.log("PASS ordinary 12-request burst completes without intervention");

  readDelay = 0; hold = a.session;
  a.invoke("browser_read", {}, "held");
  await until(() => held.length === 2, "capacity fixture dispatched");
  for (let i = 0; i < 31; i++) a.invoke("browser_read", {}, `queued-${i}`, i === 1 ? 100 : 8000);
  a.invoke("browser_read", {}, "overflow");
  const overflow = await a.result("overflow", 2000);
  assert.equal(overflow.facts.reason, "capacity"); assert.equal(overflow.effect, "none"); assert.equal(overflow.repeat_safe, true);
  assert.equal((await a.result(a.invoke("policy_explain"), 2000)).status, "succeeded");
  assert.equal((await b.result(b.invoke("browser_read"), 2000)).status, "succeeded");
  a.send({ kind: "cancel", id: "queued-0" });
  const cancelled = await a.result("queued-0", 2000);
  assert.equal(cancelled.status, "cancelled"); assert.equal(cancelled.effect, "none");
  const expired = await a.result("queued-1", 2000);
  assert.equal(expired.facts.reason, "deadline"); assert.equal(expired.effect, "none");
  const pauseAt = Date.now();
  browser.send({ kind: "event", event: { event: "runtime_control_requested", intent: "hold" } });
  await browser.take(value => value.kind === "control_state" && value.state === "held", "human pause", 1500);
  assert.ok(Date.now() - pauseAt < 1500);
  a.send({ kind: "cancel", id: "held" });
  assert.equal((await a.result("held", 2000)).effect, "unknown");
  for (let i = 2; i < 31; i++) assert.equal((await a.result(`queued-${i}`)).effect, "none");
  assert.equal(held.length, 2, "pause and cancellation prevent queued dispatch");
  hold = null;
  browser.send({ kind: "event", event: { event: "runtime_control_requested", intent: "resume" } });
  await browser.take(value => value.kind === "control_state" && value.state === "active", "human resume");
  assert.equal((await a.result(a.invoke("browser_read"))).status, "succeeded");
  console.log("PASS capacity isolation, independent controls, queued cancellation/expiry, Pause/Resume and recovery");

  const nonreader = await client("H8 stopped reading");
  nonreader.socket.pause();
  for (let i = 0; i < 256; i++) nonreader.send({ kind: "catalog" });
  await delay(3000);
  assert.equal((await b.result(b.invoke("browser_read"), 2000)).status, "succeeded");
  nonreader.socket.resume();
  await until(() => nonreader.socket.destroyed, "stalled response peer retired", 4000);
  const fresh = await client("H8 recovered connection");
  assert.equal((await fresh.result(fresh.invoke("policy_explain"))).status, "succeeded");
  console.log("PASS stalled response delivery retires only its connection; future work succeeds");
} catch (error) {
  process.stderr.write(output); throw error;
} finally {
  for (const peer of sockets) peer.destroy();
  if (child && child.exitCode === null) {
    const exited = new Promise(resolvePromise => child.once("exit", resolvePromise)); child.kill(); await exited;
  }
  rmSync(runtime, { force: true });
  rmSync(runtime.replace(/\.json$/, ".lock"), { force: true });
  rmSync(scratch, { recursive: true, force: true });
}
