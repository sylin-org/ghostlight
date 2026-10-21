// Test budget guard (ADR-0204 Decisions 3 and 4).
//
// The owner set the mandate on 2026-10-21: meaningful tests, not many, with at most 500 Rust tests
// in the workspace. A test is a `#[test]` attribute in any `.rs` file under crates/, inline modules
// and `tests/` directories alike; a test gated to one platform counts on every platform. A new test
// replaces a weaker one, or the owner raises the ceiling in an amendment to ADR-0204.

import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");

/** The most Rust tests the workspace may hold (ADR-0204 Decision 3). */
const RUST_TEST_CEILING = 500;

/** The text that marks one Rust test, as the tests-1 batch counted them. */
const TEST_ATTRIBUTE = /#\[test\]/g;

/** Every `.rs` file under `directory`, skipping build output. */
function rustFiles(directory) {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      if (entry.name !== "target") files.push(...rustFiles(path));
    } else if (entry.name.endsWith(".rs")) {
      files.push(path);
    }
  }
  return files;
}

test("the workspace holds at most 500 Rust tests", () => {
  let count = 0;
  for (const file of rustFiles(join(ROOT, "crates"))) {
    count += (readFileSync(file, "utf8").match(TEST_ATTRIBUTE) ?? []).length;
  }
  assert.ok(count > 0, "no Rust tests found under crates/");
  assert.ok(
    count <= RUST_TEST_CEILING,
    `${count} Rust tests, over the ceiling of ${RUST_TEST_CEILING}: let a new test replace a weaker one (ADR-0204)`,
  );
});
