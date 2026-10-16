// Proves the edge leaves with its spawning client in both relay roles, even when stdin stays open:
// `ghostlight mcp` and `ghostlight chrome-extension://<id>/`. Each runs from a test home.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { alive, createHome, delay, until } from "./lib/home.mjs";
import { EXTENSION_ORIGIN } from "./lib/peers.mjs";

const ROLES = [
  { name: "the MCP relay", arguments: ["mcp"] },
  { name: "the browser relay", arguments: [EXTENSION_ORIGIN] }
];
const INACTIVITY_MS = 1_500;
const ORPHAN_LIMIT_MS = 5_000;

if (process.argv[2] === "--launcher") {
  // Start the edge with this process's standard input, report its id, and leave.
  const child = spawn(process.argv[3], JSON.parse(process.argv[4]), {
    windowsHide: true,
    stdio: [0, "ignore", "ignore"]
  });
  process.stdout.write(`${child.pid}\n`, () => setTimeout(() => process.exit(0), 250));
} else {
  const home = createHome({ label: "parent" });
  try {
    for (const { name, arguments: edgeArguments } of ROLES) {
      const attended = home.start(home.edge, edgeArguments, { stdio: ["pipe", "ignore", "ignore"] });
      await delay(INACTIVITY_MS);
      assert.equal(attended.exitCode, null, `${name} survives inactivity while its parent lives`);
      attended.stdin.end();
      const { code } = await attended.exited;
      assert.equal(code, 0, `${name} exits cleanly on stdin end`);

      const keeper = home.start(process.execPath, ["-e", "setInterval(() => {}, 1000)"], { stdio: ["ignore", "pipe", "ignore"] });
      const launcher = home.start(process.execPath, [import.meta.filename, "--launcher", home.edge, JSON.stringify(edgeArguments)], {
        stdio: [keeper.stdout, "pipe", "inherit"]
      });
      const lines = createInterface({ input: launcher.stdout });
      const processId = Number(await new Promise((done, fail) => {
        lines.once("line", done);
        launcher.once("error", fail);
      }));
      assert.ok(Number.isInteger(processId) && processId > 0, `${name} reported a process id`);
      const launched = await launcher.exited;
      assert.equal(launched.code, 0, `${name} launcher exits`);
      assert.equal(keeper.exitCode, null, `${name} input writer outlives its parent`);
      try {
        await until(() => !alive(processId), `${name} exits after its exact parent dies`, ORPHAN_LIMIT_MS);
      } finally {
        if (keeper.exitCode === null) {
          keeper.kill();
          await keeper.exited;
        }
        if (alive(processId)) process.kill(processId, "SIGKILL");
      }
    }
    console.log("edge parent lifecycle ok: open stdin cannot preserve an orphan");
  } finally {
    await home.cleanup();
  }
}
