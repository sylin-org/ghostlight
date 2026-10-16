// Unit tests for the journey helper's relay arithmetic. Nothing here starts a process or a service.
import assert from "node:assert/strict";
import test from "node:test";
import { join } from "node:path";
import { decodeFrames, encodeFrame, fnv1a32, goodbyeIn, homeHash, pipeName, role, socketPath, vocabulary } from "./home.mjs";

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
