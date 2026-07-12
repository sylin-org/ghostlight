#!/usr/bin/env node
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Thin launcher: fetch the version-matched ghostlight role executables from the GitHub release on
// first run, cache them under ~/.ghostlight/bin/<version>/, then exec the right one for the caller.
// ADR-0046 + ADR-0051 Phase 3: a bare `npx ghostlight` (what an MCP client launches) execs
// `ghostlight-relay --role agent` (the MCP pass-through); a CLI subcommand (install/doctor/...)
// execs ghostlight. Zero dependencies.
//
// SUPPLY-CHAIN INTEGRITY: this is a download-and-execute-native-binary launcher, so it verifies
// what it runs. Every downloaded binary is checked (sha256) against a checksums.json that travels
// INSIDE this npm package -- an immutable, independently hosted manifest -- BEFORE it is made
// executable or run. A mismatch (tampered release asset, corrupted transfer, hijacked redirect)
// aborts. Downloads and their redirects are also constrained to GitHub hosts. See ADR/Socket note.
//
// IMPORTANT: stdout belongs to the MCP stdio protocol when a client spawns this; every message this
// launcher prints goes to stderr.

"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const https = require("https");
const crypto = require("crypto");
const { spawnSync } = require("child_process");

const VERSION = require("../package.json").version;
const REPO = "sylin-org/ghostlight";

// The two executables (ADR-0046 + ADR-0051 Phase 3): ghostlight = CLI + service; ghostlight-relay =
// the single thin pass-through carrying both roles. Both are cached in ONE dir, so `ghostlight
// install` resolves the relay as a sibling.
const BINS = ["ghostlight", "ghostlight-relay"];

// When the caller names one of these `ghostlight` CLI subcommands, exec `ghostlight`; otherwise
// this is an MCP launch (bare, or with only flags like --instance), so exec the agent adapter.
const CLI_SUBCOMMANDS = new Set([
  "install",
  "uninstall",
  "doctor",
  "status",
  "config",
  "policy",
  "service",
]);

function targetTriple() {
  const { platform, arch } = process;
  if (platform === "win32" && arch === "x64") return "x86_64-pc-windows-msvc";
  if (platform === "darwin" && arch === "arm64") return "aarch64-apple-darwin";
  if (platform === "darwin" && arch === "x64") return "x86_64-apple-darwin";
  if (platform === "linux" && arch === "x64") return "x86_64-unknown-linux-gnu";
  return null;
}

// Redirect/download allowlist: GitHub release downloads start at github.com and 302 to the
// object store on *.githubusercontent.com. Anything else is refused so a hijacked Location header
// cannot redirect the fetch off-platform.
function isAllowedHost(host) {
  return host === "github.com" || host.endsWith(".githubusercontent.com");
}

function sha256File(file) {
  return crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
}

// The pinned-integrity manifest bundled in this package (written at release time). Its absence or a
// version skew is fatal: we will not run a binary we cannot verify.
function loadChecksums() {
  const file = path.join(__dirname, "..", "checksums.json");
  let manifest;
  try {
    manifest = JSON.parse(fs.readFileSync(file, "utf8"));
  } catch (e) {
    throw new Error(
      `integrity manifest (checksums.json) missing or unreadable (${e.message}). ` +
        `Reinstall a published version (npx ghostlight@latest) or build from source.`
    );
  }
  if (manifest.version !== VERSION) {
    throw new Error(
      `integrity manifest is for ${manifest.version}, but this launcher is ${VERSION}; refusing to proceed.`
    );
  }
  return manifest;
}

function download(url, tmpPath, redirectsLeft) {
  return new Promise((resolve, reject) => {
    let parsed;
    try {
      parsed = new URL(url);
    } catch {
      return reject(new Error(`bad download url: ${url}`));
    }
    if (!isAllowedHost(parsed.host)) {
      return reject(new Error(`refusing to download from untrusted host: ${parsed.host}`));
    }
    if (redirectsLeft <= 0) return reject(new Error("too many redirects"));
    https
      .get(url, { headers: { "User-Agent": `ghostlight-npm/${VERSION}` } }, (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          res.resume();
          // Resolve relative Location against the current URL; the recursive call re-checks the host.
          const next = new URL(res.headers.location, url).toString();
          return resolve(download(next, tmpPath, redirectsLeft - 1));
        }
        if (res.statusCode !== 200) {
          res.resume();
          return reject(new Error(`download failed: HTTP ${res.statusCode} for ${url}`));
        }
        const out = fs.createWriteStream(tmpPath, { mode: 0o755 });
        res.pipe(out);
        out.on("finish", () => out.close((err) => (err ? reject(err) : resolve())));
        out.on("error", (e) => {
          fs.rmSync(tmpPath, { force: true });
          reject(e);
        });
      })
      .on("error", reject);
  });
}

async function ensureBinaries() {
  const triple = targetTriple();
  if (!triple) {
    process.stderr.write(
      `ghostlight: no prebuilt binary for ${process.platform}/${process.arch}.\n` +
        `Build from source (cargo install --git https://github.com/${REPO}) or see\n` +
        `https://sylin.org/ghostlight/ for options.\n`
    );
    process.exit(1);
  }
  const checksums = loadChecksums();
  const exe = process.platform === "win32" ? ".exe" : "";
  const dir = path.join(os.homedir(), ".ghostlight", "bin", `v${VERSION}`);
  fs.mkdirSync(dir, { recursive: true });

  let announced = false;
  for (const b of BINS) {
    const bin = path.join(dir, `${b}${exe}`);
    if (fs.existsSync(bin)) continue; // already downloaded AND verified on a prior run
    if (!announced) {
      process.stderr.write(`ghostlight: first run, fetching v${VERSION} for ${triple}...\n`);
      announced = true;
    }
    const asset = `${b}-${triple}${exe}`;
    const expected = checksums.binaries && checksums.binaries[asset];
    if (!expected) {
      throw new Error(`no pinned checksum for ${asset}; refusing to run an unverified binary`);
    }
    const url = `https://github.com/${REPO}/releases/download/v${VERSION}/${asset}`;
    const tmp = `${bin}.download-${process.pid}`;
    await download(url, tmp, 5);

    const actual = sha256File(tmp);
    if (actual !== expected) {
      fs.rmSync(tmp, { force: true });
      throw new Error(
        `integrity check FAILED for ${asset}: expected ${expected}, got ${actual}. ` +
          `The downloaded binary does not match the pinned checksum; not running it.`
      );
    }
    if (process.platform !== "win32") fs.chmodSync(tmp, 0o755);
    try {
      fs.renameSync(tmp, bin);
    } catch (e) {
      // A concurrent launcher won the rename race; theirs passed the same verification.
      if (fs.existsSync(bin)) {
        fs.rmSync(tmp, { force: true });
      } else {
        throw e;
      }
    }
  }
  if (announced) {
    process.stderr.write(
      `ghostlight: ready. Tip: run "npx ghostlight install" once to connect the browser\n` +
        `extension and register your MCP clients (idempotent; --dry-run to preview).\n`
    );
  }
  return { dir, exe };
}

function main() {
  ensureBinaries()
    .then(({ dir, exe }) => {
      const args = process.argv.slice(2);
      // ADR-0046 + ADR-0051 Phase 3: a CLI subcommand runs the `ghostlight` binary; a bare/flags-only
      // invocation is an MCP launch and runs `ghostlight-relay --role agent` (the pass-through your
      // client relays through).
      const isCli = args.some((a) => CLI_SUBCOMMANDS.has(a));
      const binName = isCli ? "ghostlight" : "ghostlight-relay";
      const bin = path.join(dir, `${binName}${exe}`);
      const spawnArgs = isCli ? args : ["--role", "agent", ...args];
      const result = spawnSync(bin, spawnArgs, { stdio: "inherit" });
      if (result.error) {
        process.stderr.write(`ghostlight: failed to launch binary: ${result.error.message}\n`);
        process.exit(1);
      }
      process.exit(result.status === null ? 1 : result.status);
    })
    .catch((e) => {
      process.stderr.write(`ghostlight: ${e.message}\n`);
      process.exit(1);
    });
}

if (require.main === module) {
  main();
}

module.exports = { targetTriple, isAllowedHost, sha256File, loadChecksums };
