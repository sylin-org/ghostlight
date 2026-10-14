// A scratch branded-Chrome launcher driven over Chrome's DevTools pipe.
//
// Branded Chrome ignores --load-extension. It does accept Extensions.loadUnpacked when it is
// started with --remote-debugging-pipe and --enable-unsafe-extension-debugging, so the installed
// journey launches Chrome that way and loads the repository extension through the pipe. File
// descriptor 3 carries commands to Chrome and file descriptor 4 carries its responses and events.
// Every message is one JSON document terminated by a NUL byte.
//
// Nothing here touches an existing profile. The scratch user data directory is owned by the caller
// and removed with removeScratch.
import { EventEmitter } from "node:events";
import { spawn as spawnProcess } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync } from "node:fs";
import { rm } from "node:fs/promises";
import { homedir, tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { waitForChromiumExit } from "./chromium.mjs";

/** Prefix of every scratch directory this module creates under the temp directory. */
export const SCRATCH_PREFIX = "installed-";

/** The byte that ends each pipe message. */
export const NUL = 0;

/** Native messaging host name the installed Ghostlight registers. */
export const NATIVE_HOST_NAME = "org.sylin.ghostlight";

/** Default per-command timeout, in milliseconds. */
export const DEFAULT_COMMAND_TIMEOUT_MS = 30000;

/** Time Chrome gets to exit after Browser.close before the owned child is killed. */
export const EXIT_TIMEOUT_MS = 10000;

const REMOVE_RETRY_CODES = new Set(["ENOENT", "EBUSY", "EACCES", "EPERM", "ENOTEMPTY"]);
const REMOVE_BUDGET_MS = 15000;
const POLL_MS = 50;
const delay = (ms) => new Promise((done) => setTimeout(done, ms));

/** Create a fresh scratch directory (the Chrome user data directory) under `parent`. */
export function createScratch(parent = tmpdir()) {
  return mkdtempSync(join(parent, SCRATCH_PREFIX));
}

/**
 * Chrome arguments for a scratch profile driven over the pipe. `--load-extension` is never among
 * them: branded Chrome ignores it.
 */
export function chromeArguments(userDataDir, extra = []) {
  return [
    `--user-data-dir=${userDataDir}`,
    "--remote-debugging-pipe",
    "--enable-unsafe-extension-debugging",
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-background-networking",
    ...extra,
    "about:blank"
  ];
}

/**
 * One DevTools pipe connection. Commands get numeric ids and resolve with the matching response
 * result; protocol errors reject. Events (messages without an id) are emitted under their method
 * name and as `event` with `{method, params, sessionId}`.
 */
export class CdpPipe extends EventEmitter {
  /** `child` is a spawned Chrome whose stdio[3] is writable and stdio[4] readable. */
  constructor(child, { timeoutMs = DEFAULT_COMMAND_TIMEOUT_MS } = {}) {
    super();
    this.child = child;
    this.timeoutMs = timeoutMs;
    this.nextId = 0;
    this.pending = new Map();
    this.buffer = Buffer.alloc(0);
    this.closed = false;
    child.stdio[4].on("data", (chunk) => this.#receive(chunk));
    child.stdio[4].on("end", () => this.#fail(new Error("DevTools pipe ended")));
    child.stdio[4].on("error", (error) => this.#fail(error));
    child.stdio[3].on?.("error", (error) => this.#fail(error));
    child.once("exit", (code, signal) => this.#fail(new Error(`Chrome exited (${code ?? signal}) with commands pending`)));
  }

  /** Send one command and resolve with its result. */
  send(method, params = {}, sessionId, timeoutMs = this.timeoutMs) {
    if (this.closed) return Promise.reject(new Error(`DevTools pipe is closed: ${method}`));
    const id = ++this.nextId;
    return new Promise((done, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`Timed out after ${timeoutMs} ms: ${method}`));
      }, timeoutMs);
      this.pending.set(id, { method, done, reject, timer });
      const message = { id, method, params, ...(sessionId ? { sessionId } : {}) };
      try {
        this.child.stdio[3].write(JSON.stringify(message) + String.fromCharCode(NUL));
      } catch (error) {
        clearTimeout(timer);
        this.pending.delete(id);
        reject(error);
      }
    });
  }

  /** Load an unpacked extension and return its id. */
  async loadUnpacked(path) {
    const result = await this.send("Extensions.loadUnpacked", { path });
    if (typeof result?.id !== "string" || !result.id) throw new Error(`Extensions.loadUnpacked returned no id: ${JSON.stringify(result)}`);
    return result.id;
  }

  /**
   * Ask Chrome to close and wait for this exact child to exit. The owned child is killed only when
   * it outlives the budget; no other browser process is ever touched.
   */
  async close({ exitTimeoutMs = EXIT_TIMEOUT_MS, commandTimeoutMs = 5000 } = {}) {
    if (!this.closed) await this.send("Browser.close", {}, undefined, commandTimeoutMs).catch(() => {});
    if (await waitForChromiumExit(this.child, exitTimeoutMs)) return true;
    this.child.kill();
    return waitForChromiumExit(this.child, exitTimeoutMs);
  }

  #receive(chunk) {
    this.buffer = Buffer.concat([this.buffer, chunk]);
    for (let end = this.buffer.indexOf(NUL); end !== -1; end = this.buffer.indexOf(NUL)) {
      const text = this.buffer.subarray(0, end).toString("utf8");
      this.buffer = this.buffer.subarray(end + 1);
      if (text) this.#dispatch(text);
    }
  }

  #dispatch(text) {
    let message;
    try { message = JSON.parse(text); } catch { this.emit("malformed", text); return; }
    if (typeof message.id === "number") {
      const item = this.pending.get(message.id);
      if (!item) return;
      this.pending.delete(message.id);
      clearTimeout(item.timer);
      if (message.error) item.reject(new Error(`${item.method}: ${JSON.stringify(message.error)}`));
      else item.done(message.result ?? {});
    } else if (typeof message.method === "string") {
      this.emit(message.method, message.params ?? {}, message.sessionId);
      this.emit("event", { method: message.method, params: message.params ?? {}, sessionId: message.sessionId });
    }
  }

  #fail(error) {
    this.closed = true;
    for (const item of this.pending.values()) {
      clearTimeout(item.timer);
      item.reject(error);
    }
    this.pending.clear();
  }
}

/** Start Chrome on a scratch profile and attach a pipe to it. */
export function launchChrome({ chrome, userDataDir, extraArgs = [], env = process.env, spawn = spawnProcess }) {
  const child = spawn(chrome, chromeArguments(userDataDir, extraArgs), {
    env, windowsHide: false, stdio: ["ignore", "pipe", "pipe", "pipe", "pipe"]
  });
  child.startError = undefined;
  child.once("error", (error) => { child.startError = error; });
  child.stderr?.on("data", () => {});
  child.stdout?.on("data", () => {});
  return { child, pipe: new CdpPipe(child) };
}

/**
 * Where the installed user-level native host manifest lives on this platform, or null when the
 * platform registers it elsewhere (Windows uses a per-user registry key that Chrome reads for any
 * profile).
 */
export function nativeHostManifestSource({ platform = process.platform, home = homedir(), env = process.env, exists = existsSync } = {}) {
  const manifest = `${NATIVE_HOST_NAME}.json`;
  const candidates = [];
  if (platform === "linux") {
    candidates.push(join(home, ".ghostlight", "manifests", manifest),
      join(env.XDG_CONFIG_HOME || join(home, ".config"), "google-chrome", "NativeMessagingHosts", manifest));
  } else if (platform === "darwin") {
    candidates.push(join(home, ".ghostlight", "manifests", manifest),
      join(home, "Library", "Application Support", "Google", "Chrome", "NativeMessagingHosts", manifest));
  } else {
    return null;
  }
  const found = candidates.find((path) => exists(path));
  if (!found) throw new Error(`No user-level native host manifest found; looked in: ${candidates.join(", ")}`);
  return found;
}

/**
 * Chrome reads user-level manifests from the user data directory on Linux and macOS, so a scratch
 * profile needs its own copy. Returns the copy's path, or null where nothing needs copying.
 */
export function installScratchNativeHost(scratch, options = {}) {
  const source = nativeHostManifestSource(options);
  if (!source) return null;
  const directory = join(scratch, "NativeMessagingHosts");
  mkdirSync(directory, { recursive: true });
  const destination = join(directory, basename(source));
  copyFileSync(source, destination);
  return destination;
}

/**
 * Remove a scratch directory created by createScratch. The path must be a direct child of
 * `parent` carrying the scratch prefix; Windows file locks are retried for a fixed budget.
 */
export async function removeScratch(scratch, parent = tmpdir(), { budgetMs = REMOVE_BUDGET_MS, remove = rm } = {}) {
  const target = resolve(scratch);
  if (dirname(target) !== resolve(parent) || !basename(target).startsWith(SCRATCH_PREFIX) || basename(target).length <= SCRATCH_PREFIX.length) {
    throw new Error(`Refusing to remove a directory this module did not create: ${target}`);
  }
  const deadline = Date.now() + budgetMs;
  for (;;) {
    try { await remove(target, { recursive: true, force: true, maxRetries: 0 }); return; }
    catch (error) {
      if (!REMOVE_RETRY_CODES.has(error.code) || Date.now() >= deadline) throw error;
    }
    await delay(POLL_MS);
  }
}
