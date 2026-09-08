// Lifecycle helpers for the disposable Chromium profiles owned by repository browser tests.
import assert from "node:assert/strict";
import { readFile, rm } from "node:fs/promises";
import { basename, dirname, join, resolve } from "node:path";

const READ_RETRY_CODES = new Set(["ENOENT", "EBUSY", "EACCES", "EPERM"]);
const REMOVE_RETRY_CODES = new Set([...READ_RETRY_CODES, "ENOTEMPTY"]);
const PROFILE_PREFIXES = new Set(["frame-browser-", "script-browser-", "history-browser-"]);
const POLL_MS = 50;
const delay = (ms) => new Promise(done => setTimeout(done, ms));
const exited = child => !child || child.startError || child.exitCode !== null || child.signalCode !== null;

// Chrome can publish this file before its final bytes or Windows sharing handle are available.
// Unexpected filesystem errors and a dead launcher fail immediately; incomplete data is bounded.
export async function readDevToolsPort(profile, child, { timeoutMs = 15000, read = readFile } = {}) {
  const path = join(profile, "DevToolsActivePort");
  const deadline = Date.now() + timeoutMs;
  let cause;
  do {
    if (child?.startError) throw child.startError;
    if (child && exited(child)) throw new Error(`Chromium exited before publishing its DevTools endpoint: ${child.exitCode ?? child.signalCode}`);
    try {
      const lines = (await read(path, "utf8")).trim().split(/\r?\n/);
      const port = Number(lines[0]);
      if (lines.length === 2 && /^\d+$/.test(lines[0]) && port > 0 && port <= 65535
          && /^\/devtools\/browser\/[A-Za-z0-9-]+$/.test(lines[1])) return lines;
      cause = new Error("DevTools endpoint file is incomplete or malformed");
    } catch (error) {
      if (!READ_RETRY_CODES.has(error.code)) throw error;
      cause = error;
    }
    if (Date.now() >= deadline) break;
    await delay(Math.min(POLL_MS, deadline - Date.now()));
  } while (true);
  throw new Error(`Timed out reading Chromium DevTools endpoint: ${path}`, { cause });
}

// Browser.close initiates shutdown. Give this exact spawned process time to finish before the
// caller's existing owned-child termination fallback; never enumerate or kill other browsers.
export async function waitForChromiumExit(child, timeoutMs = 5000) {
  const deadline = Date.now() + timeoutMs;
  while (!exited(child) && Date.now() < deadline) await delay(POLL_MS);
  return Boolean(exited(child));
}

// AV scanners and Chromium subprocesses can briefly retain profile files after launcher exit.
// Retry those locks for a fixed total budget, preserving both path guards and final failure.
export async function removeBrowserScratch(scratch, parent, prefix, { timeoutMs = 15000, remove = rm } = {}) {
  const target = resolve(scratch);
  assert.ok(PROFILE_PREFIXES.has(prefix), "Unknown browser scratch prefix");
  assert.equal(dirname(target), resolve(parent), "Browser scratch must be a direct child of its owner root");
  assert.ok(basename(target).startsWith(prefix) && basename(target).length > prefix.length, "Browser scratch must have its mkdtemp prefix");
  const deadline = Date.now() + timeoutMs;
  do {
    try { await remove(target, { recursive: true, force: true, maxRetries: 0 }); return; }
    catch (error) {
      if (!REMOVE_RETRY_CODES.has(error.code)) throw error;
      if (Date.now() >= deadline) throw new Error(`Timed out removing owned Chromium profile: ${target}`, { cause: error });
    }
    await delay(Math.min(POLL_MS, Math.max(0, deadline - Date.now())));
  } while (true);
}
