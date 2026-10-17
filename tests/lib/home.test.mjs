// Unit tests for the journey helper's relay arithmetic. No Ghostlight program or service starts.
import assert from "node:assert/strict";
import test from "node:test";
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { frameIdOf, loaderIdOf } from "./peers.mjs";
import {
  DIAGNOSTICS_DIRECTORY, EVIDENCE_PROGRAMS_FILE, EVIDENCE_VARIABLE, JourneyHome, decodeFrames, encodeFrame, fnv1a32, goodbyeIn, homeHash, pipeName, role, socketPath, vocabulary
} from "./home.mjs";

const example = vocabulary.endpoint.windows.example;

test("the hash of the worked example matches the relay vocabulary", () => {
  assert.equal(fnv1a32(Buffer.from(example.lowercased_home, "utf8")), example.fnv1a32);
  assert.equal(homeHash(example.home), example.home_hash);
});

test("the pipe name of the worked example matches the relay vocabulary", () => {
  assert.equal(pipeName(example.user_sid, example.home), example.name);
});

test("only ASCII letters are lowercased in the home hash", () => {
  assert.equal(homeHash("C:\\Users\\\u00c4da\\.ghostlight"), homeHash("c:\\users\\\u00c4da\\.ghostlight"));
  assert.notEqual(homeHash("C:\\Users\\\u00c4da\\.ghostlight"), homeHash("C:\\Users\\\u00e4da\\.ghostlight"));
});

test("the socket path is the relay socket under the run directory", () => {
  const home = join("/", "home", "ada", vocabulary.files.home_directory);
  assert.equal(socketPath(home), join(home, vocabulary.files.run_directory, vocabulary.files.relay_socket));
});

test("a frame round trips and a partial frame waits for the rest", () => {
  const first = encodeFrame({ kind: "message", line: "{}" });
  const second = encodeFrame({ goodbye: "quit" });
  const bytes = Buffer.concat([first, second]);
  const cut = first.length + 2;
  const early = decodeFrames(bytes.subarray(0, cut));
  assert.deepEqual(early.frames, [{ kind: "message", line: "{}" }]);
  const late = decodeFrames(Buffer.concat([early.rest, bytes.subarray(cut)]));
  assert.deepEqual(late.frames, [{ goodbye: "quit" }]);
  assert.equal(late.rest.length, 0);
});

test("a frame that declares more than the limit is refused", () => {
  const header = Buffer.alloc(4);
  header.writeUInt32LE(vocabulary.frame.max_bytes + 1);
  assert.throws(() => decodeFrames(header), /declares/);
});

test("a goodbye is read only from a frame that is one", () => {
  assert.equal(goodbyeIn({ goodbye: "replace" }), "replace");
  assert.equal(goodbyeIn({ kind: "heartbeat" }), null);
});

test("a word outside the vocabulary is refused", () => {
  assert.equal(role.control, "control");
  assert.throws(() => role.bystander, /not in the relay vocabulary/);
});

test("the synthetic browser mints ids in the browser's alphabet", () => {
  // The service admits only ASCII letters, digits and '-' (is_browser_id in
  // crates/orchestrator/src/browser/adapter/chromium/protocol4/locator.rs).
  for (const id of [frameIdOf(41), loaderIdOf(7)]) assert.match(id, /^[A-Za-z0-9-]+$/);
});

test("cleanup keeps each program's exit and output as evidence, then removes the home", async () => {
  const evidence = mkdtempSync(join(tmpdir(), "ghostlight-evidence-test-"));
  const earlier = process.env[EVIDENCE_VARIABLE];
  process.env[EVIDENCE_VARIABLE] = evidence;
  const home = new JourneyHome({ label: "evidence-test" });
  try {
    const started = home.start(process.execPath, ["-e", "console.error('started program says boom'); process.exit(3)"]);
    await started.exited;
    home.run(process.execPath, ["-e", "console.log('ran program says hello')"]);
    mkdirSync(home.file(DIAGNOSTICS_DIRECTORY), { recursive: true });
    writeFileSync(home.file(DIAGNOSTICS_DIRECTORY, "service.log"), "diagnostic line");
    await home.cleanup();
    assert.equal(existsSync(home.area), false);
    const [folder] = readdirSync(evidence);
    const programs = JSON.parse(readFileSync(join(evidence, folder, EVIDENCE_PROGRAMS_FILE), "utf8"));
    assert.equal(programs.length, 2);
    assert.equal(programs[0].code, 3);
    assert.match(programs[0].output, /started program says boom/);
    assert.equal(programs[1].code, 0);
    assert.match(programs[1].output, /ran program says hello/);
    assert.equal(readFileSync(join(evidence, folder, DIAGNOSTICS_DIRECTORY, "service.log"), "utf8"), "diagnostic line");
  } finally {
    if (earlier === undefined) delete process.env[EVIDENCE_VARIABLE];
    else process.env[EVIDENCE_VARIABLE] = earlier;
    rmSync(home.area, { recursive: true, force: true });
    rmSync(evidence, { recursive: true, force: true });
  }
});
