// Connector guards (ADR-0197 D25, connector half).
//
// Two facts per connector crate and for the relay crate they share:
//   1. the normal dependency tree of each connector on each supported target
//      (tests/golden/cargo-tree/<crate>.<target>.txt);
//   2. the connector and relay source equals the last published tag at the same
//      relay revision, so a published revision never changes underneath users.
//
// Run `GHOSTLIGHT_GUARD_UPDATE=1 node --test tests/guards/connectors.test.mjs`
// to rewrite the dependency-tree goldens, then commit them with a message saying why.
// A source change under a published revision is fixed by bumping RELAY and the
// crate versions, never by editing a recorded hash.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const TREE_DIR = join(ROOT, "tests", "golden", "cargo-tree");
const UPDATE = process.env.GHOSTLIGHT_GUARD_UPDATE === "1";

const CONNECTORS = ["ghostlight-mcp-connector", "ghostlight-browser-connector"];
const TARGETS = ["x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu"];

/** Source paths locked to the published relay revision; tests directories are excluded. */
const LOCKED = ["crates/relay", "crates/mcp-connector", "crates/browser-connector", ":(exclude)crates/*/tests/**"];

/** Cargo stderr that means the registry was unreachable, so the tree cannot be resolved offline. */
const OFFLINE_FAILURE = /failed to download|attempting to make an HTTP request, but --offline was specified/i;

/** The relay revision declared by `RELAY` in crates/relay/src/lib.rs. */
export function relayRevision() {
  const source = readFileSync(join(ROOT, "crates", "relay", "src", "lib.rs"), "utf8");
  const match = /pub const RELAY: u16 = (\d+);/.exec(source);
  assert.ok(match, "crates/relay/src/lib.rs no longer declares `pub const RELAY: u16`");
  return Number(match[1]);
}

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

/** The skip message for a `cargoTree` result of the `{ offline }` shape. */
export function offlineSkipMessage(crate, target, result) {
  return `cargo cannot resolve ${crate} for ${target} offline: ${result.offline.split("\n")[0]}`;
}

/** Runs `cargo tree` offline; returns `{ offline }` when the registry is unreachable and throws on any other failure. */
function cargoTree(crate, target) {
  try {
    const out = execFileSync(
      "cargo",
      ["tree", "-p", crate, "-e", "normal", "--target", target, "--prefix", "none", "--locked", "--offline"],
      { cwd: ROOT, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] },
    );
    return normalizeTree(out);
  } catch (error) {
    const stderr = String(error.stderr || error.message).trim();
    if (OFFLINE_FAILURE.test(stderr)) return { offline: stderr };
    throw new Error(`cargo tree failed for ${crate} on ${target}:
${stderr}`);
  }
}

/** Runs git in the repository root and returns trimmed stdout. */
function git(...args) {
  return execFileSync("git", args, { cwd: ROOT, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }).trim();
}

/** True when `git` exits zero for `args`. */
function gitOk(...args) {
  try {
    git(...args);
    return true;
  } catch {
    return false;
  }
}

/** The relay revision declared in the relay source at `tag`, or null when the tag has no relay crate. */
function revisionAt(tag) {
  if (!gitOk("cat-file", "-e", `${tag}:crates/relay/src/lib.rs`)) return null;
  const match = /pub const RELAY: u16 = (\d+);/.exec(git("show", `${tag}:crates/relay/src/lib.rs`));
  return match ? Number(match[1]) : null;
}

for (const crate of CONNECTORS) {
  for (const target of TARGETS) {
    test(`dependency tree of ${crate} on ${target} is frozen`, (t) => {
      const file = join(TREE_DIR, `${crate}.${target}.txt`);
      const tree = cargoTree(crate, target);
      if (typeof tree !== "string") {
        t.skip(offlineSkipMessage(crate, target, tree));
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

test("the offline skip message is built from the field cargoTree returns", () => {
  const message = offlineSkipMessage("crate-a", "target-b", { offline: "failed to download x\nmore" });
  assert.equal(message, "cargo cannot resolve crate-a for target-b offline: failed to download x");
});

test("connector source is unchanged since the last published tag at this relay revision", () => {
  const tags = git("tag", "--list", "v[0-9]*", "--sort=-v:refname").split(/\r?\n/).filter((tag) => tag && !tag.startsWith("archive/"));
  assert.ok(tags.length > 0, "no release tags: fetch with tags (fetch-depth: 0)");
  const revision = relayRevision();
  const tag = tags.find((candidate) => revisionAt(candidate) === revision);
  if (!tag) {
    console.log(`relay revision ${revision} is unpublished; nothing to compare`);
    return;
  }
  assert.ok(
    gitOk("diff", "--quiet", tag, "--", ...LOCKED),
    `connector source changed since ${tag} at relay revision ${revision}: bump RELAY`,
  );
});
