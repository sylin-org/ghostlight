// Extension freeze guard (ADR-0200 D16).
//
// The 2.0 extension is submitted early to the stores and stays the same package through the
// release, because a changed packaged file means withdrawing the submission and starting again.
// This test computes the packaged set the way scripts/package-extension.ps1 stages it and fails
// when a file is added, removed or changed against tests/golden/extension-2.0.0.files.sha256.
//
// A packaged file changes only with a recorded owner decision to withdraw and resubmit. The
// golden is then regenerated from the new zip with
// `node tests/guards/lib/extension-package.mjs <zip>`, in the same commit as the change.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { GOLDEN_PATH, packagedHashes, parseGolden } from "./lib/extension-package.mjs";

const RULE =
  "A packaged extension file changes only with a recorded owner decision to withdraw the store " +
  "submission and resubmit (ADR-0200 Decision 16).";

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
