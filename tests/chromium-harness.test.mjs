// Fault injection for the test-owned browser lifecycle, including the Windows failures observed
// during the complete hardening suite. No real Chrome, profile, or user process is touched.
import assert from "node:assert/strict";
import test from "node:test";
import { join, resolve } from "node:path";
import { readDevToolsPort, removeBrowserScratch, waitForChromiumExit } from "./lib/chromium.mjs";

const running = () => ({ exitCode: null, signalCode: null });
const busy = code => Object.assign(new Error(code), { code });

test("DevTools publication tolerates known sharing errors and a partially written file", async () => {
  const answers = [busy("ENOENT"), busy("EBUSY"), busy("EACCES"), busy("EPERM"), "", "12345\n", "12345\n/devtools/browser/test-id\n"];
  const endpoint = await readDevToolsPort("unused", running(), { read: async () => {
    const answer = answers.shift(); if (answer instanceof Error) throw answer; return answer;
  } });
  assert.deepEqual(endpoint, ["12345", "/devtools/browser/test-id"]); assert.equal(answers.length, 0);
});

test("DevTools rejects unexpected filesystem errors and a dead launcher without retry", async () => {
  const unexpected = busy("EIO"); let attempts = 0;
  await assert.rejects(readDevToolsPort("unused", running(), { read: async () => { attempts++; throw unexpected; } }), error => error === unexpected);
  assert.equal(attempts, 1);
  await assert.rejects(readDevToolsPort("unused", { exitCode: 1, signalCode: null }), /Chromium exited/);
});

test("DevTools never accepts incomplete or invalid endpoints after its timeout", async () => {
  for (const body of ["12345\n", "0\n/devtools/browser/x", "65536\n/devtools/browser/x", "12345\nhttp://elsewhere", "12345\n/devtools/browser/x\nextra"]) {
    await assert.rejects(readDevToolsPort("unused", running(), { timeoutMs: 0, read: async () => body }), /Timed out reading Chromium/);
  }
});

test("profile removal rejects paths outside its exact owned prefix before invoking deletion", async () => {
  const parent = resolve(".tmp"); let attempts = 0;
  const remove = async () => { attempts++; };
  for (const [target, prefix] of [[parent, "frame-browser-"], [join(parent, "other"), "frame-browser-"],
    [join(parent, "frame-browser-test", "nested"), "frame-browser-"], [join(parent, "frame-browser-test"), ""]]) {
    await assert.rejects(removeBrowserScratch(target, parent, prefix, { remove }));
  }
  assert.equal(attempts, 0);
});

test("profile removal retries transient locks but surfaces persistent or unexpected failure", async () => {
  const parent = resolve(".tmp"), target = join(parent, "frame-browser-test");
  const codes = ["EPERM", "EBUSY", "EACCES", "ENOTEMPTY"];
  await removeBrowserScratch(target, parent, "frame-browser-", { remove: async (path, options) => {
    assert.equal(path, target); assert.equal(options.maxRetries, 0);
    if (codes.length) throw busy(codes.shift());
  } });
  assert.equal(codes.length, 0);
  await assert.rejects(removeBrowserScratch(target, parent, "frame-browser-", { timeoutMs: 0, remove: async () => { throw busy("EPERM"); } }), /Timed out removing owned/);
  const unexpected = busy("EIO");
  await assert.rejects(removeBrowserScratch(target, parent, "frame-browser-", { remove: async () => { throw unexpected; } }), error => error === unexpected);
});

test("graceful browser waiting is bounded and never terminates a process itself", async () => {
  const child = running(); child.kill = () => assert.fail("wait must not kill a process");
  assert.equal(await waitForChromiumExit(child, 0), false);
  child.exitCode = 0; assert.equal(await waitForChromiumExit(child), true);
});
