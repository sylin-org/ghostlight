// Edge guards (ADR-0197 D25, ADR-0199 D21).
//
// Four facts about the edge crate, the relay crate it shares with the service, the service and
// the shell:
//   1. the crate names in the normal dependency tree of the edge on each supported
//      target (tests/golden/cargo-tree/<crate>.<target>.txt), with no network crate among
//      them (ADR-0028, never phone home);
//   2. everything in crates/relay and crates/edge except test files equals the last published
//      tag at the same relay revision, so a published revision never changes underneath users;
//   3. the service links no UI toolkit and no async network stack (ADR-0199 D1 and D21): the
//      window, the tray and the notices belong to ghostlight-desktop;
//   4. the shell links no network crate except `http`, whose request and response types its web
//      view library still brings (ADR-0199 D21).
//
// Run `GHOSTLIGHT_GUARD_UPDATE=1 node --test tests/guards/connectors.test.mjs`
// to rewrite the dependency-tree goldens after a real dependency add or remove, then commit
// them with a message saying why. Versions are not pinned, so a plain `cargo update` needs no refresh.
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

const EDGES = ["ghostlight"];
const TARGETS = ["x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu", "x86_64-unknown-linux-musl"];

/**
 * Paths locked to the published relay revision: everything in the two crates except test files,
 * which are every file named tests.rs and every file under a directory named tests. A pathspec
 * `*` also matches `/`, so each exclude reaches any depth.
 */
const LOCKED = ["crates/relay", "crates/edge", ":(exclude)crates/*/tests/**", ":(exclude)crates/*/tests.rs"];

/** Crates the edge must never link: any network stack would break the never-phone-home promise (ADR-0028). */
const NETWORK_CRATES = /^(hyper|reqwest|ureq|h2|http|rustls|native-tls|openssl|curl|isahc|tokio|socket2|mio)$/;

/**
 * The one network crate the shell may link: `wry` brings `http` for its request and response
 * types, and the shell registers no custom scheme or IPC handler, so it does no I/O and opens no
 * socket (ADR-0199 Decision 21).
 */
const SHELL_NETWORK_EXCEPTION = "http";

/** The shell package, whose tree holds no network crate but the exception above (ADR-0199 D21). */
const SHELL = "ghostlight-desktop";

/** Cargo stderr that means the registry was unreachable, so the tree cannot be resolved. */
const OFFLINE_FAILURE = /failed to download|failed to get|unable to update registry|couldn't resolve host|network failure/i;

/** True in CI, where an unresolvable tree fails instead of skipping. */
const IN_CI = Boolean(process.env.CI);

/** The relay revision declared by `RELAY` in crates/relay/src/lib.rs. */
export function relayRevision() {
  const source = readFileSync(join(ROOT, "crates", "relay", "src", "lib.rs"), "utf8");
  const match = /pub const RELAY: u16 = (\d+);/.exec(source);
  assert.ok(match, "crates/relay/src/lib.rs no longer declares `pub const RELAY: u16`");
  return Number(match[1]);
}

/** Reduces `cargo tree` output to the sorted, deduplicated crate names, without versions or markers. */
export function normalizeTree(text) {
  const names = new Set();
  for (const raw of text.split(/\r?\n/)) {
    const name = raw.trim().split(/\s+/)[0];
    if (name) names.add(name);
  }
  return [...names].sort().join("\n") + "\n";
}

/** The skip message for a `cargoTree` result of the `{ offline }` shape. */
export function offlineSkipMessage(crate, target, result) {
  return `cargo cannot resolve ${crate} for ${target}: ${result.offline.split("\n")[0]}`;
}

/** The service package, whose tree holds no UI toolkit (ADR-0199 D21). */
const SERVICE = "ghostlight-service";

/** Crates that draw a window, a tray, a web view or a notice. The service never links one; the shell does. */
const UI_TOOLKIT_CRATES =
  /^(tauri.*|tao|wry|gtk.*|gdk.*|webkit2gtk.*|javascriptcore.*|soup3.*|libappindicator.*|tray-icon|muda|webview2-com.*)$/;

/** Crates the service never links: no async runtime, no HTTP stack (ADR-0199 Decision 21). */
const LEFT_THE_SERVICE_CRATES = /^(tokio|reqwest|hyper|socket2)$/;

/** Runs `cargo tree`; returns `{ offline }` when the registry is unreachable outside CI and throws on any other failure. */
function cargoTree(crate, target, edges = "normal") {
  try {
    const out = execFileSync(
      "cargo",
      ["tree", "-p", crate, "-e", edges, "--target", target, "--prefix", "none", "--locked"],
      { cwd: ROOT, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] },
    );
    return normalizeTree(out);
  } catch (error) {
    const stderr = String(error.stderr || error.message).trim();
    if (!IN_CI && OFFLINE_FAILURE.test(stderr)) return { offline: stderr };
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

for (const crate of EDGES) {
  for (const target of TARGETS) {
    test(`dependency tree of ${crate} on ${target} is frozen`, (t) => {
      const file = join(TREE_DIR, `${crate}.${target}.txt`);
      const tree = cargoTree(crate, target);
      if (typeof tree !== "string") {
        t.skip(offlineSkipMessage(crate, target, tree));
        return;
      }
      const network = tree.split("\n").filter((name) => NETWORK_CRATES.test(name));
      assert.deepEqual(
        network,
        [],
        `${crate} on ${target} links a network crate (${network.join(", ")}): the edge never phones home (ADR-0028)`,
      );
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

for (const target of TARGETS) {
  test(`the service links no UI toolkit on ${target}`, (t) => {
    const tree = cargoTree(SERVICE, target, "normal,build");
    if (typeof tree !== "string") {
      t.skip(offlineSkipMessage(SERVICE, target, tree));
      return;
    }
    const names = tree.split(/\r?\n/);
    const toolkit = names.filter((name) => UI_TOOLKIT_CRATES.test(name));
    assert.deepEqual(
      toolkit,
      [],
      `${SERVICE} on ${target} links a UI toolkit (${toolkit.join(", ")}): the shell draws, the service serves (ADR-0199 D21)`,
    );
    const stack = names.filter((name) => LEFT_THE_SERVICE_CRATES.test(name));
    assert.deepEqual(
      stack,
      [],
      `${SERVICE} on ${target} links ${stack.join(", ")}, which left with the toolkit and must not come back`,
    );
  });
}

for (const target of TARGETS) {
  test(`the shell links no network crate except ${SHELL_NETWORK_EXCEPTION} on ${target}`, (t) => {
    const tree = cargoTree(SHELL, target);
    if (typeof tree !== "string") {
      t.skip(offlineSkipMessage(SHELL, target, tree));
      return;
    }
    const network = tree
      .split("\n")
      .filter((name) => NETWORK_CRATES.test(name) && name !== SHELL_NETWORK_EXCEPTION);
    assert.deepEqual(
      network,
      [],
      `${SHELL} on ${target} links a network crate (${network.join(", ")}): the shell never phones home (ADR-0028, ADR-0199 D21)`,
    );
  });
}

test("the offline skip message is built from the field cargoTree returns", () => {
  const message = offlineSkipMessage("crate-a", "target-b", { offline: "failed to download x\nmore" });
  assert.equal(message, "cargo cannot resolve crate-a for target-b: failed to download x");
});

test("edge source is unchanged since the last published tag at this relay revision", () => {
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
    `edge source changed since ${tag} at relay revision ${revision}: bump RELAY`,
  );
});
