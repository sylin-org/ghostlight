// Exercise opaque native relay reconnection against a repeatedly closing synthetic service.
// No browser or orchestrator starts; runtime discovery is isolated to this temporary fixture.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { existsSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { performance } from "node:perf_hooks";

const repository = resolve(import.meta.dirname, "..");
const binDirectory = resolve(process.env.GHOSTLIGHT_BIN_DIR || join(repository, ".target-ghostlight-1.0", "debug"));
const executable = join(binDirectory, `ghostlight-browser-connector${process.platform === "win32" ? ".exe" : ""}`);
assert.ok(existsSync(executable), `Build the connector under test first: ${executable}`);
const fixture = mkdtempSync(join(tmpdir(), "ghostlight-relay-reconnect-"));
assert.equal(dirname(resolve(fixture)), resolve(tmpdir()));
const runtime = join(fixture, "runtime.json");
const token = "runtime_reconnect_fixture";
const adapterHello = Buffer.from('{"kind":"hello","opaque":"cached adapter identity"}');
const ended = { kind: "control_state", state: "ended" };
const refusal = { kind: "error", correlation: null, code: "adapter_update_required", message: "Synthetic compatibility refusal.", effect_unknown: false };
const connectionTimes = [];
const forwarded = [];
const hellos = [];
const sockets = new Set();
let child;
let stderr = "";
let failure;

function frame(payload) {
  const bytes = Buffer.isBuffer(payload) ? payload : Buffer.from(JSON.stringify(payload));
  const header = Buffer.alloc(4);
  header.writeUInt32LE(bytes.length);
  return Buffer.concat([header, bytes]);
}

function decodeFrames(stream, receive) {
  let buffered = Buffer.alloc(0);
  stream.on("data", chunk => {
    try {
      buffered = Buffer.concat([buffered, chunk]);
      while (buffered.length >= 4) {
        const length = buffered.readUInt32LE(0);
        if (buffered.length < length + 4) break;
        const bytes = buffered.subarray(4, length + 4);
        buffered = buffered.subarray(length + 4);
        receive(bytes);
      }
    } catch (error) {
      failure ??= error;
      child?.stdin.end();
    }
  });
}

const server = createServer(socket => {
  connectionTimes.push(performance.now());
  sockets.add(socket);
  socket.on("close", () => sockets.delete(socket));
  socket.on("error", error => { failure ??= error; });
  let stage = 0;
  decodeFrames(socket, payload => {
    if (stage++ === 0) {
      assert.deepEqual(JSON.parse(payload.toString()), { kind: "hello", major: 1, token });
      socket.write(frame({ kind: "accepted", major: 1 }));
      return;
    }
    assert.deepEqual(payload, adapterHello, "every reconnect must replay the exact opaque opening frame");
    hellos.push(payload.toString());
    socket.end(Buffer.concat([frame(ended), frame(refusal)]));
  });
});

try {
  await new Promise((resolveReady, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolveReady);
  });
  writeFileSync(runtime, JSON.stringify({
    service_port: server.address().port,
    browser_port: server.address().port,
    token,
    service_bridge_major: 1,
    browser_relay_major: 1,
    service_version: "fixture"
  }));
  const startedAt = performance.now();
  child = spawn(executable, [], {
    env: { ...process.env, GHOSTLIGHT_RUNTIME_FILE: runtime, GHOSTLIGHT_DIAGNOSTICS_DIR: join(fixture, "diagnostics") },
    stdio: ["pipe", "pipe", "pipe"], windowsHide: true
  });
  child.stderr.on("data", chunk => { stderr += chunk.toString(); });
  decodeFrames(child.stdout, payload => {
    forwarded.push(JSON.parse(payload.toString()));
    if (forwarded.length === 8) child.stdin.end();
  });
  const exited = new Promise((resolveExit, reject) => {
    child.once("error", reject);
    child.once("exit", (code, signal) => resolveExit({ code, signal }));
  });
  child.stdin.write(frame(adapterHello));
  let timeout;
  const outcome = await Promise.race([
    exited,
    new Promise((_, reject) => { timeout = setTimeout(() => reject(new Error("Connector reconnect fixture timed out.")), 10_000); })
  ]).finally(() => clearTimeout(timeout));
  if (failure) throw failure;
  assert.equal(outcome.code, 0, stderr);
  assert.equal(hellos.length, 4, "closing browser native input must end the reconnect episode");
  assert.equal(connectionTimes.length, 4);
  assert.deepEqual(forwarded, Array.from({ length: 4 }, () => [ended, refusal]).flat(), "pre-admission control and error frames remain opaque and visible");
  const gaps = connectionTimes.slice(1).map((time, index) => time - connectionTimes[index]);
  for (const gap of gaps) assert.ok(gap >= 450, `Established service losses must be spaced by the existing 500ms retry interval; observed ${gap.toFixed(1)}ms.`);
  console.log(JSON.stringify({
    result: "passed", executable, exchanges: hellos.length,
    initial_connection_ms: Math.round(connectionTimes[0] - startedAt),
    reconnect_gaps_ms: gaps.map(value => Math.round(value)), opaque_frames_forwarded: forwarded.length
  }));
} finally {
  child?.stdin.end();
  if (child && child.exitCode === null) child.kill();
  for (const socket of sockets) socket.destroy();
  await new Promise(resolveClosed => server.close(resolveClosed));
  rmSync(fixture, { recursive: true, force: true });
}
