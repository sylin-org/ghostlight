// A throwaway Ghostlight home for the process journeys, and a plain relay protocol 4 client.
//
// A journey never touches the real home. It makes a fresh directory under the operating system's
// temp directory, copies the two executables under test into its `bin`, and points every process
// it starts at that home with GHOSTLIGHT_TEST_HOME. The service the journey talks to is the one
// that home runs; the endpoint is computed here the way `ghostlight_relay::endpoint` computes it,
// from the user and the home path. Every closed word comes from tests/golden/relay4/vocabulary.json,
// the same file the Rust goldens bind.
import { execFileSync, spawn, spawnSync } from "node:child_process";
import { copyFileSync, cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, readlinkSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import net from "node:net";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";

/** The root of the repository. */
export const repository = resolve(import.meta.dirname, "..", "..");

/** The relay 4 vocabulary the Rust goldens bind. */
export const vocabulary = JSON.parse(readFileSync(join(repository, "tests", "golden", "relay4", "vocabulary.json"), "utf8"));

/** The executable suffix of this platform. */
export const suffix = process.platform === "win32" ? ".exe" : "";

/** The file name of the edge. */
export const EDGE_NAME = `ghostlight${suffix}`;

/** The file name of the service image. */
export const SERVICE_NAME = `ghostlight-service${suffix}`;

/** The home's `bin` directory name; the relay starts a service only from here. */
export const BIN_DIRECTORY = "bin";

/** The quit latch under the home: it is written when a person quits and read by browser starts. */
export const QUIT_LATCH = ["state", "quit"];

/** The diagnostics directory under the home; the twin of DIAGNOSTICS_DIRECTORY_NAME in crates/relay/src/home.rs. */
export const DIAGNOSTICS_DIRECTORY = "diagnostics";

/**
 * The environment variable that names a directory to keep a journey's failure evidence in. The
 * hardening suite sets it to a folder under its uploaded output; `JourneyHome.cleanup` writes
 * each started program's command line, exit and captured output there, and copies the home's
 * diagnostics, before it removes the home.
 */
export const EVIDENCE_VARIABLE = "GHOSTLIGHT_JOURNEY_EVIDENCE";

/** The file in a home's evidence folder that lists the programs the journey ran. */
export const EVIDENCE_PROGRAMS_FILE = "programs.json";

/** How much of a program's output the journey keeps, in characters. */
const OUTPUT_TAIL = 6000;

/** The signal that ends a process with no chance to say goodbye. Windows ends it the same way. */
const HARD_KILL = "SIGKILL";

/** Time a journey gives a service to come up from nothing. */
export const SERVICE_START_MS = 30_000;

/** A closed word list as an object that refuses a word it does not hold. */
export function closed(words) {
  const table = Object.fromEntries(words.map((word) => [word, word]));
  return new Proxy(Object.freeze(table), {
    get(target, name) {
      if (typeof name === "string" && !(name in target) && name !== "then") {
        throw new Error(`"${name}" is not in the relay vocabulary: ${words.join(", ")}`);
      }
      return target[name];
    }
  });
}

/** The roles of an opening. */
export const role = closed(vocabulary.roles);
/** The reasons a starter gives a service. */
export const startReason = closed(vocabulary.start_reasons);
/** The reasons of a goodbye. */
export const goodbyeReason = closed(vocabulary.goodbye_reasons);
/** The reasons of a quit. */
export const quitReason = closed(vocabulary.quit_reasons);
/** The programs an edge can be. */
export const program = closed(vocabulary.edge_programs);
/** The control operations. */
export const controlOp = closed(vocabulary.control_ops);
/** The refusal codes. */
export const refusalCode = closed(vocabulary.refusal_codes);
/** The failure templates a reply carries. */
export const templateKind = closed(vocabulary.failure_template_kinds);
/** The kinds of an MCP stream frame. */
export const mcpKind = closed(vocabulary.mcp_frame_kinds);

/** The version of the build under test, read from the workspace manifest. */
export function buildVersion() {
  const manifest = readFileSync(join(repository, "Cargo.toml"), "utf8");
  const version = manifest.match(/\[workspace\.package\][\s\S]*?^version = "([^"]+)"/m)?.[1];
  if (!version) throw new Error("Could not read the workspace version from Cargo.toml");
  return version;
}

/** The directory the executables under test are built into. */
export function binaryDirectory() {
  return resolve(process.env.GHOSTLIGHT_BIN_DIR || join(repository, ".target-ghostlight-1.0", "debug"));
}

/** The value of `promise`, or a failure after `ms` milliseconds. */
export function within(promise, ms, label) {
  let timer;
  const limit = new Promise((_, fail) => { timer = setTimeout(() => fail(new Error(`Timed out: ${label}`)), ms); });
  return Promise.race([promise, limit]).finally(() => clearTimeout(timer));
}

/** Resolve after `ms` milliseconds. */
export const delay = (ms) => new Promise((done) => setTimeout(done, ms));

/** Wait until `test` returns something truthy and return it, or throw after `timeout` ms. */
export async function until(test, label, timeout = 8000) {
  const end = Date.now() + timeout;
  for (;;) {
    const value = await test();
    if (value) return value;
    if (Date.now() >= end) throw new Error(`Timed out: ${label}`);
    await delay(25);
  }
}

// ---- the endpoint ----

/** The FNV-1a 32-bit hash of `bytes`. */
export function fnv1a32(bytes) {
  const { fnv1a32_offset_basis: basis, fnv1a32_prime: prime } = vocabulary.endpoint.windows;
  let hash = basis;
  for (const byte of bytes) hash = Math.imul(hash ^ byte, prime) >>> 0;
  return hash;
}

/** The home hash of a pipe name: the FNV-1a hash of the path with ASCII letters lowercased. */
export function homeHash(home) {
  const lowered = home.replace(/[A-Z]/g, (letter) => letter.toLowerCase());
  return fnv1a32(Buffer.from(lowered, "utf8")).toString(16).padStart(8, "0");
}

/** The Windows pipe name for a user and a home. */
export function pipeName(userSid, home) {
  return `${vocabulary.endpoint.windows.pipe_prefix}${userSid}-${homeHash(home)}`;
}

/** The Unix socket path for a home. */
export function socketPath(home) {
  return join(home, vocabulary.files.run_directory, vocabulary.files.relay_socket);
}

/** The user's security identifier, as `whoami` prints it. Windows only. */
export function userSid() {
  const printed = execFileSync("whoami", ["/user", "/fo", "csv", "/nh"], { encoding: "utf8", windowsHide: true });
  const sid = printed.match(/"(S-1-[0-9-]+)"/)?.[1];
  if (!sid) throw new Error(`Could not read the user SID from: ${printed}`);
  return sid;
}

/** The endpoint of a home: the pipe name on Windows, the socket path elsewhere. */
export function endpointOf(home) {
  return process.platform === "win32" ? pipeName(userSid(), home) : socketPath(home);
}

// ---- frames ----

/** The wire bytes of one frame: a `u32` little-endian length, then the JSON. */
export function encodeFrame(value) {
  const payload = Buffer.from(JSON.stringify(value), "utf8");
  if (payload.length > vocabulary.frame.max_bytes) throw new Error("A frame may not exceed the relay limit");
  const header = Buffer.alloc(4);
  header.writeUInt32LE(payload.length);
  return Buffer.concat([header, payload]);
}

/** Split `buffer` into the whole frames it holds and the bytes left over. */
export function decodeFrames(buffer) {
  const frames = [];
  let rest = buffer;
  while (rest.length >= 4) {
    const length = rest.readUInt32LE(0);
    if (length > vocabulary.frame.max_bytes) throw new Error(`A frame declares ${length} bytes`);
    if (rest.length < 4 + length) break;
    frames.push(JSON.parse(rest.subarray(4, 4 + length).toString("utf8")));
    rest = rest.subarray(4 + length);
  }
  return { frames, rest };
}

/** The goodbye reason in a browser stream frame, or null when the frame is not a goodbye. */
export function goodbyeIn(frame) {
  return typeof frame?.goodbye === "string" ? frame.goodbye : null;
}

/** One relay connection: send frames, wait for the frames that come back. */
export class RelayStream {
  constructor(socket) {
    this.socket = socket;
    this.buffer = Buffer.alloc(0);
    this.frames = [];
    this.waiters = [];
    this.closed = false;
    this.failure = null;
    this.ended = new Promise((done) => socket.once("close", done));
    socket.on("data", (chunk) => {
      try {
        const { frames, rest } = decodeFrames(Buffer.concat([this.buffer, chunk]));
        this.buffer = rest;
        for (const frame of frames) this.frames.push(frame);
      } catch (error) {
        this.failure = error;
      }
      this.settle();
    });
    socket.on("error", (error) => { this.failure = error; });
    socket.on("close", () => {
      this.closed = true;
      this.settle();
    });
  }

  /** Send one frame. */
  send(value) {
    this.socket.write(encodeFrame(value));
  }

  /** Send bytes exactly as given. */
  sendRaw(bytes) {
    this.socket.write(bytes);
  }

  settle() {
    for (const waiter of [...this.waiters]) {
      const found = this.frames.findIndex(waiter.test);
      if (found >= 0) {
        this.waiters.splice(this.waiters.indexOf(waiter), 1);
        waiter.done(this.frames.splice(found, 1)[0]);
      } else if (this.closed || this.failure) {
        this.waiters.splice(this.waiters.indexOf(waiter), 1);
        waiter.fail(new Error(`The relay stream ended while waiting for ${waiter.label}`));
      }
    }
  }

  /** The next frame `test` accepts, in order; frames it skips stay queued. */
  next(test = () => true, timeoutMs = 5000, label = "a frame") {
    return new Promise((done, fail) => {
      const waiter = { test, label, done: (frame) => { clearTimeout(timer); done(frame); }, fail: (error) => { clearTimeout(timer); fail(error); } };
      const timer = setTimeout(() => {
        this.waiters.splice(this.waiters.indexOf(waiter), 1);
        fail(new Error(`Timed out waiting for ${label}`));
      }, timeoutMs);
      this.waiters.push(waiter);
      this.settle();
    });
  }

  /** The frames received and not yet taken. */
  pending() {
    return [...this.frames];
  }

  /** Close the connection. */
  close() {
    this.socket.destroy();
  }
}

const NO_SERVICE_CODES = new Set(["ENOENT", "ECONNREFUSED"]);
const BUSY_CODES = new Set(["EBUSY", "EAGAIN"]);

function dial(endpoint) {
  return new Promise((done, fail) => {
    const socket = net.connect(endpoint);
    socket.once("connect", () => { socket.removeListener("error", fail); done(socket); });
    socket.once("error", fail);
  });
}

/**
 * Connect to the service of `home`. With `patience` 0 a missing service fails at once; with more,
 * a missing service is retried until the time is up.
 */
export async function connectRelay(home, patience = 0) {
  const endpoint = endpointOf(home);
  const end = Date.now() + patience;
  for (;;) {
    try {
      return new RelayStream(await dial(endpoint));
    } catch (error) {
      const retry = BUSY_CODES.has(error.code) || (patience > 0 && NO_SERVICE_CODES.has(error.code));
      if (!retry || Date.now() >= end) throw error;
      await delay(25);
    }
  }
}

/** An opening for `kind` from a program of the service image, with `extra` fields. */
export function opening(kind, extra = {}) {
  return { relay: vocabulary.relay, role: kind, edge: { program: program.service, revision: vocabulary.relay, build: buildVersion() }, ...extra };
}

/** Open a relay stream with `open` and return the stream and the service's reply. */
export async function openRelay(home, open, patience = 0) {
  const stream = await connectRelay(home, patience);
  stream.send(open);
  const reply = await stream.next((frame) => "accepted" in frame, vocabulary.opening_deadline_ms, "the reply to an opening");
  return { stream, reply };
}

/**
 * One control operation. The answer is `null` when no service listens, otherwise
 * `{ reply, value }` with `value` the service's result when the opening was accepted.
 */
export async function controlOnce(home, control, patience = 0) {
  let opened;
  try {
    opened = await openRelay(home, opening(role.control, { control }), patience);
  } catch (error) {
    if (NO_SERVICE_CODES.has(error.code) || /ended while waiting/.test(error.message)) return null;
    throw error;
  }
  const { stream, reply } = opened;
  try {
    if (!reply.accepted) return { reply, value: undefined };
    const result = await stream.next((frame) => frame.kind === vocabulary.control_result_kind, 5000, "the control result");
    return { reply, value: result.value };
  } finally {
    stream.close();
  }
}

// ---- processes ----

/** Whether the process `pid` exists. */
export function alive(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    if (error.code === "EPERM") return true;
    if (error.code === "ESRCH") return false;
    throw error;
  }
}

/** The executable image of the process `pid`, or null when it is gone or cannot be read. */
export function imagePath(pid) {
  try {
    if (process.platform === "win32") {
      const printed = execFileSync("powershell.exe",
        ["-NoProfile", "-NonInteractive", "-Command", `(Get-Process -Id ${Number(pid)} -ErrorAction SilentlyContinue).Path`],
        { encoding: "utf8", windowsHide: true }).trim();
      return printed || null;
    }
    return readlinkSync(`/proc/${Number(pid)}/exe`);
  } catch {
    return null;
  }
}

/**
 * The ids of the running processes whose executable is one of `images`. A start relaunches
 * itself (ADR-0199 amendment of 2026-10-21), so the process that serves is not the one a journey
 * started; this finds it by its image.
 */
export function processesOf(images) {
  const wanted = [...images];
  if (process.platform === "win32") {
    const names = [...new Set(wanted.map((image) => basename(image).replace(/\.exe$/i, "")))];
    const command = "Get-Process -Name " + names.map((name) => "'" + name + "'").join(",") +
      " -ErrorAction SilentlyContinue | ForEach-Object { '{0}|{1}' -f $_.Id, $_.Path }";
    const printed = execFileSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", command],
      { encoding: "utf8", windowsHide: true });
    return printed.split(/\r?\n/).filter(Boolean).map((line) => line.split("|"))
      .filter(([, path]) => path && wanted.some((image) => samePath(image, path)))
      .map(([pid]) => Number(pid));
  }
  return readdirSync("/proc").filter((entry) => /^\d+$/.test(entry)).filter((pid) => {
    try {
      const image = readlinkSync(`/proc/${pid}/exe`);
      return wanted.some((own) => samePath(own, image));
    } catch {
      return false;
    }
  }).map(Number);
}

/** Whether two paths name one file. */
export function samePath(left, right) {
  const key = (path) => {
    let real = path;
    try { real = realpathSync.native(path); } catch { /* a path that is gone compares as written */ }
    return process.platform === "win32" ? real.toLowerCase() : real;
  };
  return key(left) === key(right);
}

/** Resolve once the process `pid` is gone, or throw after `timeout` ms. */
export async function waitGone(pid, label, timeout = 10_000) {
  await until(() => !alive(pid), label, timeout);
}

/** The policy that permits every ordinary action, with `deny` hosts refused. */
export function openPolicy(deny = []) {
  return {
    schema: 4,
    name: "Process journey",
    version: "1",
    grants: [{ id: "ordinary-web", hosts: { allow: ["*"], deny }, allowed: ["read", "action", "write", "execute"] }],
    config: [{ key: "browser.startup", value: "manual", level: "mandatory" }]
  };
}

/** Copy the edge and the service image from `from` into `directory`. */
export function copyImages(from, directory) {
  mkdirSync(directory, { recursive: true });
  for (const name of [EDGE_NAME, SERVICE_NAME]) {
    const source = join(from, name);
    if (!existsSync(source)) throw new Error(`Missing ${source}; build the workspace first.`);
    copyFileSync(source, join(directory, name));
  }
}

/**
 * A fresh Ghostlight home with the two executables in its `bin`, a policy, an audit file path and
 * a user profile fixture, all under one scratch area. Everything a journey starts runs with this
 * environment. `cleanup` stops what the journey started and removes the area.
 */
export class JourneyHome {
  constructor({ policy = openPolicy(), label = "journey" } = {}) {
    this.area = mkdtempSync(join(realpathSync.native(tmpdir()), `ghostlight-${label}-`));
    this.path = join(this.area, "home");
    this.bin = join(this.path, BIN_DIRECTORY);
    this.edge = join(this.bin, EDGE_NAME);
    this.service = join(this.bin, SERVICE_NAME);
    this.policyFile = join(this.area, "policy.json");
    this.auditFile = join(this.area, "audit.jsonl");
    this.nativeHostDir = join(this.area, "native-host");
    const user = join(this.area, "user");
    mkdirSync(this.path, { recursive: true });
    mkdirSync(user, { recursive: true });
    writeFileSync(this.policyFile, JSON.stringify(policy));
    this.environment = {
      ...process.env,
      [vocabulary.test_home_variable]: this.path,
      GHOSTLIGHT_POLICY_FILE: this.policyFile,
      GHOSTLIGHT_AUDIT_FILE: this.auditFile,
      GHOSTLIGHT_NATIVE_HOST_DIR: this.nativeHostDir,
      GHOSTLIGHT_DIAGNOSTICS_DIR: join(this.path, DIAGNOSTICS_DIRECTORY),
      HOME: user,
      USERPROFILE: user,
      APPDATA: join(user, "roaming"),
      LOCALAPPDATA: join(user, "local"),
      XDG_CONFIG_HOME: join(user, "config"),
      XDG_STATE_HOME: join(user, "state"),
      CODEX_HOME: join(user, "codex")
    };
    delete this.environment.GHOSTLIGHT_RUNTIME_FILE;
    this.children = [];
    this.programs = [];
    this.serviceImages = new Set([this.service]);
  }

  /** The ids of the running processes of this home's service image. */
  serviceProcesses() {
    return processesOf(this.serviceImages);
  }

  /** Copy the built executables into the home's `bin`. */
  installImages(from = binaryDirectory()) {
    copyImages(from, this.bin);
    return this;
  }

  /** Where `relative` is under the home. */
  file(...relative) {
    return join(this.path, ...relative);
  }

  /** Start `executable` in this home. The journey stops it with `cleanup`. */
  start(executable, args = [], options = {}) {
    const child = spawn(executable, args, {
      env: { ...this.environment, ...options.env },
      stdio: options.stdio ?? ["pipe", "pipe", "pipe"],
      windowsHide: true,
      ...options.spawn
    });
    child.output = "";
    child.stderr?.on("data", (chunk) => { child.output = (child.output + chunk).slice(-OUTPUT_TAIL); });
    this.programs.push({ argv: [executable, ...args], child });
    child.exited = new Promise((done) => child.once("exit", (code, signal) => done({ code, signal })));
    child.on("error", (error) => { child.failure = error; });
    this.children.push(child);
    return child;
  }

  /** Run `executable` to the end in this home and return its result. */
  run(executable, args = [], options = {}) {
    const result = spawnSync(executable, args, {
      env: { ...this.environment, ...options.env },
      encoding: "utf8",
      windowsHide: true,
      timeout: options.timeout ?? 60_000,
      input: options.input
    });
    this.programs.push({
      argv: [executable, ...args],
      result: { code: result.status, signal: result.signal, error: result.error?.message, output: `${result.stdout ?? ""}${result.stderr ?? ""}`.slice(-OUTPUT_TAIL) }
    });
    return result;
  }

  /**
   * Keep what a failure needs from this home in the folder named by EVIDENCE_VARIABLE: the
   * programs the journey ran with their exit and output, and the home's diagnostics. Nothing is
   * written when the variable is unset. Journeys use no credentials, so nothing here is secret.
   */
  keepEvidence() {
    const root = process.env[EVIDENCE_VARIABLE];
    if (!root) return;
    const folder = join(root, basename(this.area));
    try {
      mkdirSync(folder, { recursive: true });
      const programs = this.programs.map(({ argv, child, result }) => ({
        argv,
        ...(child
          ? { code: child.exitCode, signal: child.signalCode, failure: child.failure?.message, running_at_cleanup: child.runningAtCleanup, output: child.output }
          : result)
      }));
      writeFileSync(join(folder, EVIDENCE_PROGRAMS_FILE), JSON.stringify(programs, null, 2));
      const diagnostics = join(this.path, DIAGNOSTICS_DIRECTORY);
      if (existsSync(diagnostics)) cpSync(diagnostics, join(folder, DIAGNOSTICS_DIRECTORY), { recursive: true });
    } catch (error) {
      process.stderr.write(`evidence of ${this.area}: ${error.message}\n`);
    }
  }

  /** The service's answer to one control operation, or null when no service listens. */
  async control(control, patience = 0) {
    return controlOnce(this.path, control, patience);
  }

  /** The service's `status` value, or null when no service listens. */
  async status(patience = 0) {
    const answer = await this.control({ op: controlOp.status }, patience);
    return answer?.reply.accepted ? answer.value : null;
  }

  /** Wait until a service answers `status`, and return what it says. */
  async serviceUp(timeout = SERVICE_START_MS) {
    return until(() => this.status(), "a service answers status", timeout);
  }

  /** Open a relay stream in this home. */
  async open(open, patience = 0) {
    return openRelay(this.path, open, patience);
  }

  /** Quit the service for `reason` and wait for its process to be gone. */
  async quit(reason = quitReason.quit) {
    const status = await this.status();
    if (!status) return null;
    const answer = await this.control({ op: controlOp.quit, reason });
    if (!answer?.reply.accepted) throw new Error(`The service refused to quit: ${JSON.stringify(answer?.reply)}`);
    await waitGone(status.pid, "the service exits after quit", 20_000);
    return status;
  }

  /**
   * Kill the running service as a crash would, only when its image is this home's own service.
   * The kill is hard (`SIGKILL`, and `TerminateProcess` on Windows): a gentler signal is the
   * session-end signal on Linux, which makes the service say goodbye to every stream, and a
   * crash says none.
   */
  async killService() {
    const status = await this.status();
    if (!status) throw new Error("No service is running to kill");
    const image = imagePath(status.pid);
    if (!image || ![...this.serviceImages].some((own) => samePath(own, image))) {
      throw new Error(`Refusing to kill process ${status.pid}: its image is ${image}, not this home's service`);
    }
    process.kill(status.pid, HARD_KILL);
    await waitGone(status.pid, "the killed service is gone");
    return status;
  }

  /** The audit lines written so far, parsed. */
  auditRecords() {
    if (!existsSync(this.auditFile)) return [];
    const text = readFileSync(this.auditFile, "utf8");
    return text.slice(0, text.lastIndexOf("\n") + 1).split("\n").filter(Boolean).map((line) => JSON.parse(line));
  }

  /** Stop what the journey started, then remove the scratch area. */
  async cleanup() {
    const problems = [];
    try {
      const status = await this.status();
      if (status) {
        await this.control({ op: controlOp.quit, reason: quitReason.quit });
        try {
          await waitGone(status.pid, "the service exits at cleanup", 10_000);
        } catch {
          const image = imagePath(status.pid);
          if (image && [...this.serviceImages].some((own) => samePath(own, image))) process.kill(status.pid, HARD_KILL);
          await waitGone(status.pid, "the service exits after a kill", 10_000);
        }
      }
    } catch (error) {
      problems.push(error.message);
    }
    for (const child of this.children) child.runningAtCleanup = child.exitCode === null && child.signalCode === null;
    for (const child of [...this.children].reverse()) {
      if (child.exitCode !== null || child.signalCode !== null) continue;
      child.kill();
      await Promise.race([child.exited, delay(5000)]);
    }
    this.keepEvidence();
    try {
      rmSync(this.area, { recursive: true, force: true, maxRetries: 20, retryDelay: 200 });
    } catch (error) {
      problems.push(error.message);
    }
    if (problems.length) process.stderr.write(`cleanup of ${this.area}: ${problems.join("; ")}\n`);
  }
}

/**
 * Make a home, install the built executables into it and return it. A home whose executables
 * cannot be installed is removed before the error reaches the journey, which never got it to clean.
 */
export function createHome(options = {}) {
  const home = new JourneyHome(options);
  try {
    return home.installImages(options.images);
  } catch (error) {
    rmSync(home.area, { recursive: true, force: true, maxRetries: 20, retryDelay: 200 });
    throw error;
  }
}
