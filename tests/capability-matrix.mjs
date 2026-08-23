// Checked behavioral-parity matrix for the capability-restoration batch.
//
// Reads the LEDGER's behavioral restoration matrix and fails when any published
// 0.8 browser job is not COMPLETE with recorded evidence or an explicit
// SUPERSEDED disposition. Run via scripts/check-repository-integrity.ps1.

import { readFileSync } from "node:fs";
import assert from "node:assert/strict";

const ledger = readFileSync(
  new URL("../docs/tasks/capability-restoration/LEDGER.md", import.meta.url),
  "utf8"
);

const matrixStart = ledger.indexOf("## Behavioral restoration matrix");
assert.notEqual(matrixStart, -1, "the LEDGER carries a behavioral restoration matrix");
const matrixEnd = ledger.indexOf("\n## ", matrixStart + 4);
const matrix = ledger
  .slice(matrixStart, matrixEnd === -1 ? undefined : matrixEnd)
  .split("\n")
  .filter((line) => line.startsWith("| ") && !line.includes(" --- "))
  .map((line) => line.split("|").map((cell) => cell.trim()).slice(1, -1));

const header = matrix[0];
const stateColumn = header.indexOf("State");
const evidenceColumn = header.indexOf("Evidence");
assert.ok(stateColumn > 0 && evidenceColumn > 0, "matrix header keeps State and Evidence columns");

let complete = 0;
let superseded = 0;
for (const row of matrix.slice(1)) {
  const [behavior, , task, state, evidence] = row;
  assert.ok(behavior.length > 0, "every matrix row names a behavior");
  assert.ok(
    state === "COMPLETE" || state === "SUPERSEDED",
    `row "${behavior}" is ${state}; the batch is not closed`
  );
  if (state === "COMPLETE") {
    complete += 1;
    assert.notEqual(evidence, "--", `COMPLETE row "${behavior}" has no recorded evidence`);
  } else {
    superseded += 1;
    assert.match(evidence, /ADR-\d+/, `SUPERSEDED row "${behavior}" cites no decision`);
  }
  if (task !== "--") {
    assert.match(task, /^R\d$/, `row "${behavior}" names an unknown task "${task}"`);
  }
}
assert.ok(complete >= 15, `expected the full restored surface, found ${complete} COMPLETE rows`);
assert.ok(superseded >= 3, `expected the explicit supersessions, found ${superseded}`);

console.log(`capability matrix ok: ${complete} COMPLETE rows, ${superseded} SUPERSEDED rows, all evidenced`);
