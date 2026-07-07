#!/usr/bin/env node
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Thin launcher: fetch the version-matched ghostlight binary from the GitHub release on first
// run, cache it under ~/.ghostlight/bin/<version>/, then exec it with the caller's arguments.
// Zero dependencies. IMPORTANT: stdout belongs to the MCP stdio protocol when a client spawns
// this; every message this launcher prints goes to stderr.

"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const https = require("https");
const { spawnSync } = require("child_process");

const VERSION = require("../package.json").version;
const REPO = "sylin-org/ghostlight";

function targetTriple() {
  const { platform, arch } = process;
  if (platform === "win32" && arch === "x64") return "x86_64-pc-windows-msvc";
  if (platform === "darwin" && arch === "arm64") return "aarch64-apple-darwin";
  if (platform === "darwin" && arch === "x64") return "x86_64-apple-darwin";
  if (platform === "linux" && arch === "x64") return "x86_64-unknown-linux-gnu";
  return null;
}

function download(url, dest, redirectsLeft) {
  return new Promise((resolve, reject) => {
    if (redirectsLeft <= 0) return reject(new Error("too many redirects"));
    https
      .get(url, { headers: { "User-Agent": `ghostlight-npm/${VERSION}` } }, (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          res.resume();
          return resolve(download(res.headers.location, dest, redirectsLeft - 1));
        }
        if (res.statusCode !== 200) {
          res.resume();
          return reject(new Error(`download failed: HTTP ${res.statusCode} for ${url}`));
        }
        const tmp = `${dest}.download-${process.pid}`;
        const out = fs.createWriteStream(tmp, { mode: 0o755 });
        res.pipe(out);
        out.on("finish", () => {
          out.close(() => {
            try {
              fs.renameSync(tmp, dest);
              resolve();
            } catch (e) {
              // A concurrent launcher won the rename race; theirs is identical.
              if (fs.existsSync(dest)) {
                fs.rmSync(tmp, { force: true });
                resolve();
              } else {
                reject(e);
              }
            }
          });
        });
        out.on("error", (e) => {
          fs.rmSync(tmp, { force: true });
          reject(e);
        });
      })
      .on("error", reject);
  });
}

async function ensureBinary() {
  const triple = targetTriple();
  if (!triple) {
    process.stderr.write(
      `ghostlight: no prebuilt binary for ${process.platform}/${process.arch}.\n` +
        `Build from source (cargo install --git https://github.com/${REPO}) or see\n` +
        `https://sylin-org.github.io/ghostlight/install.html for options.\n`
    );
    process.exit(1);
  }
  const exe = process.platform === "win32" ? ".exe" : "";
  const dir = path.join(os.homedir(), ".ghostlight", "bin", `v${VERSION}`);
  const bin = path.join(dir, `ghostlight${exe}`);
  if (fs.existsSync(bin)) return bin;

  fs.mkdirSync(dir, { recursive: true });
  const asset = `ghostlight-${triple}${exe}`;
  const url = `https://github.com/${REPO}/releases/download/v${VERSION}/${asset}`;
  process.stderr.write(`ghostlight: first run, fetching v${VERSION} for ${triple}...\n`);
  await download(url, bin, 5);
  if (process.platform !== "win32") fs.chmodSync(bin, 0o755);
  process.stderr.write(
    `ghostlight: ready. Tip: run "npx ghostlight install" once to connect the browser\n` +
      `extension and register your MCP clients (idempotent; --dry-run to preview).\n`
  );
  return bin;
}

ensureBinary()
  .then((bin) => {
    const result = spawnSync(bin, process.argv.slice(2), { stdio: "inherit" });
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
