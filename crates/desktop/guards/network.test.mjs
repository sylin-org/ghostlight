// Network-crate guard for the window process (ADR-0028, never phone home; ADR-0197 connector
// source lock amendment; ADR-0199 Decision 21, docs/adr/0199-one-service-thin-shells.md).
//
// ghostlight-desktop renders the service's loopback workbench and opens no network port, so no
// network client may be in its normal dependency tree on any supported target. One name is
// allowed, with its reason: `http`, which wry's own API types bring. The shell registers no custom
// scheme and no IPC handler, and it does no I/O and opens no socket. Each exception must still
// appear in the tree, so a stale exception fails and is deleted.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");

const CRATE = "ghostlight-desktop";
const TARGETS = ["x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu"];

/** Crates the window must never link: any network stack would break the never-phone-home promise (ADR-0028). */
const NETWORK_CRATES = /^(hyper|reqwest|ureq|h2|http|rustls|native-tls|openssl|curl|isahc|tokio|socket2|mio)$/;

/** Banned names that may stay in the tree, each with the reason it holds. */
const EXCEPTIONS = new Map([
  ["http", "request and response types that wry's own API brings; the shell registers no custom scheme and no IPC handler, does no I/O and opens no socket"],
]);

/** Cargo stderr that means the registry was unreachable, so the tree cannot be resolved. */
const OFFLINE_FAILURE = /failed to download|failed to get|unable to update registry|couldn't resolve host|network failure/i;

/** True in CI, where an unresolvable tree fails instead of skipping. */
const IN_CI = Boolean(process.env.CI);

/** Reduces `cargo tree` output to the sorted, deduplicated crate names, without versions or markers. */
export function crateNames(text) {
  const names = new Set();
  for (const raw of text.split(/\r?\n/)) {
    const name = raw.trim().split(/\s+/)[0];
    if (name) names.add(name);
  }
  return [...names].sort();
}

/** Runs `cargo tree`; returns `{ offline }` when the registry is unreachable outside CI and throws on any other failure. */
function cargoTree(target) {
  try {
    const out = execFileSync(
      "cargo",
      ["tree", "-p", CRATE, "-e", "normal", "--target", target, "--prefix", "none", "--locked"],
      { cwd: ROOT, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] },
    );
    return crateNames(out);
  } catch (error) {
    const stderr = String(error.stderr || error.message).trim();
    if (!IN_CI && OFFLINE_FAILURE.test(stderr)) return { offline: stderr };
    throw new Error(`cargo tree failed for ${CRATE} on ${target}:\n${stderr}`);
  }
}

for (const target of TARGETS) {
  test(`${CRATE} links no network client on ${target}`, (t) => {
    const names = cargoTree(target);
    if (!Array.isArray(names)) {
      t.skip(`cargo cannot resolve ${CRATE} for ${target}: ${names.offline.split("\n")[0]}`);
      return;
    }
    const banned = names.filter((name) => NETWORK_CRATES.test(name) && !EXCEPTIONS.has(name));
    assert.deepEqual(
      banned,
      [],
      `${CRATE} on ${target} links a network crate (${banned.join(", ")}): the window never phones home (ADR-0028)`,
    );
    for (const [name, reason] of EXCEPTIONS) {
      assert.ok(
        names.includes(name),
        `the exception for ${name} (${reason}) is stale: ${name} left the tree, so delete the exception`,
      );
    }
  });
}

test("every exception names a crate on the ban list", () => {
  for (const name of EXCEPTIONS.keys()) {
    assert.ok(NETWORK_CRATES.test(name), `${name} is not on the ban list, so it needs no exception`);
  }
});

test("the tree reduction keeps names only", () => {
  const text = "ghostlight-desktop v1.3.15 (E:\\x)\nhttp v1.5.0\nbytes v1.0.0\nhttp v1.5.0\n\n";
  assert.deepEqual(crateNames(text), ["bytes", "ghostlight-desktop", "http"]);
});
