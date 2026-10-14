// Connector guards (ADR-0197 D25, connector half).
//
// Two frozen facts per connector crate and for the relay crate they share:
//   1. the normal dependency tree of each connector on each supported target
//      (tests/golden/cargo-tree/<crate>.<target>.txt);
//   2. a hash of the crate's tracked source (tests/golden/frozen-hashes.json).
//
// Run `GHOSTLIGHT_GUARD_UPDATE=1 node --test tests/guards/connectors.test.mjs`
// to rewrite the golden files, then commit them with a message saying why.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const TREE_DIR = join(ROOT, "tests", "golden", "cargo-tree");
const HASH_FILE = join(ROOT, "tests", "golden", "frozen-hashes.json");
const UPDATE = process.env.GHOSTLIGHT_GUARD_UPDATE === "1";

const CONNECTORS = ["ghostlight-mcp-connector", "ghostlight-browser-connector"];
const TARGETS = ["x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu"];

/** Frozen source directories, with the message shown when one changes. */
const FROZEN = {
  "crates/relay": "crates/relay changed: bump the relay revision, or update tests/golden/frozen-hashes.json in a commit that says why",
  "crates/mcp-connector": "crates/mcp-connector changed: bump the relay revision, or update tests/golden/frozen-hashes.json in a commit that says why",
  "crates/browser-connector": "crates/browser-connector changed: bump the relay revision, or update tests/golden/frozen-hashes.json in a commit that says why",
};

/** Matches a path-crate marker such as " (E:\repo\crates\relay)" at the end of a line. */
const PATH_MARKER = /^(\S+) v\S+ \((?!\*\)|proc-macro\))[^)]*\)$/;

/** Reduces `cargo tree` output to a sorted, deduplicated, machine-independent list. */
export function normalizeTree(text) {
  const lines = new Set();
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim().replace(/ \(\*\)$/, "");
    if (!line) continue;
    const path = PATH_MARKER.exec(line);
    lines.add(path ? path[1] : line);
  }
  return [...lines].sort().join("\n") + "\n";
}

/** Runs `cargo tree` offline; returns null when cargo cannot resolve it. */
function cargoTree(crate, target) {
  try {
    const out = execFileSync(
      "cargo",
      ["tree", "-p", crate, "-e", "normal", "--target", target, "--prefix", "none", "--locked", "--offline"],
      { cwd: ROOT, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] },
    );
    return normalizeTree(out);
  } catch (error) {
    return { error: String(error.stderr || error.message).trim() };
  }
}

/** Hash of the sorted tracked paths and their LF-normalized bytes under `dir`. */
export function hashTracked(dir) {
  const listing = execFileSync("git", ["ls-files", "-z", "--", dir], { cwd: ROOT, encoding: "utf8" });
  const paths = listing.split("\0").filter(Boolean).sort();
  const hash = createHash("sha256");
  for (const path of paths) {
    const bytes = readFileSync(join(ROOT, path)).toString("latin1").replace(/\r\n/g, "\n");
    hash.update(`${path}\0${Buffer.byteLength(bytes, "latin1")}\0`);
    hash.update(Buffer.from(bytes, "latin1"));
  }
  return hash.digest("hex");
}

for (const crate of CONNECTORS) {
  for (const target of TARGETS) {
    test(`dependency tree of ${crate} on ${target} is frozen`, (t) => {
      const file = join(TREE_DIR, `${crate}.${target}.txt`);
      const tree = cargoTree(crate, target);
      if (typeof tree !== "string") {
        t.skip(`cargo cannot resolve ${crate} for ${target} offline: ${tree.error.split("\n")[0]}`);
        return;
      }
      if (UPDATE) {
        mkdirSync(TREE_DIR, { recursive: true });
        writeFileSync(file, tree);
        return;
      }
      assert.ok(existsSync(file), `missing golden ${file}`);
      const expected = readFileSync(file, "utf8").replace(/\r\n/g, "\n");
      assert.equal(
        tree,
        expected,
        `${crate} dependency tree on ${target} changed: update tests/golden/cargo-tree in a commit that says why`,
      );
    });
  }
}

test("frozen sources match tests/golden/frozen-hashes.json", () => {
  const actual = Object.fromEntries(Object.keys(FROZEN).map((dir) => [dir, hashTracked(dir)]));
  if (UPDATE) {
    writeFileSync(HASH_FILE, JSON.stringify(actual, null, 2) + "\n");
    return;
  }
  const expected = JSON.parse(readFileSync(HASH_FILE, "utf8"));
  for (const [dir, message] of Object.entries(FROZEN)) {
    assert.equal(actual[dir], expected[dir], message);
  }
});
