// Extension freeze guard (ADR-0200 D16).
//
// The packaged extension files are recorded, so no packaged file changes unnoticed. This test
// computes the packaged set the way scripts/package-extension.ps1 stages it and fails when a file
// is added, removed or changed against tests/golden/extension-2.0.0.files.sha256.
//
// An extension change regenerates the record in the same commit: package the extension with
// scripts/package-extension.ps1, then write the golden from the new zip with
// `node tests/guards/lib/extension-package.mjs <zip>`. Once a store submission is staged, changing
// a packaged file means resubmitting, which is the owner's call.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { GOLDEN_PATH, packagedHashes, parseGolden } from "./lib/extension-package.mjs";

const RULE =
  "An extension change regenerates tests/golden/extension-2.0.0.files.sha256 in the same commit; " +
  "once a store submission is staged, changing a packaged file means resubmitting, which is the " +
  "owner's call (ADR-0200 Decision 16).";

test("the packaged extension files equal the frozen 2.0.0 set", () => {
  const frozen = parseGolden(readFileSync(GOLDEN_PATH, "utf8"));
  const current = packagedHashes();
  const problems = [];
  for (const [path, hash] of current) {
    if (!frozen.hashes.has(path)) problems.push(`${path} was added. ${RULE}`);
    else if (frozen.hashes.get(path) !== hash) problems.push(`${path} changed. ${RULE}`);
  }
  for (const path of frozen.hashes.keys()) {
    if (!current.has(path)) problems.push(`${path} was removed. ${RULE}`);
  }
  assert.deepEqual(problems, []);
});

test("the golden records the zip it was taken from", () => {
  const frozen = parseGolden(readFileSync(GOLDEN_PATH, "utf8"));
  assert.match(frozen.zip ?? "", /^[0-9a-f]{64}$/, "the golden's last line must hold the zip's SHA-256");
});
