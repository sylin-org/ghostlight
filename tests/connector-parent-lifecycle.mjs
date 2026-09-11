// Proves both thin connectors leave with their spawning client even when stdin remains open.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { accessSync, constants } from "node:fs";
import { join, resolve } from "node:path";
import { createInterface } from "node:readline";

const root = resolve(import.meta.dirname, "..");
const executable = name => join(
  process.env.GHOSTLIGHT_BIN_DIR || join(root, ".target-ghostlight-1.0", "debug"),
  `${name}${process.platform === "win32" ? ".exe" : ""}`
);

const delay = milliseconds => new Promise(resolveDelay => setTimeout(resolveDelay, milliseconds));
const alive = processId => {
  try {
    process.kill(processId, 0);
    return true;
  } catch (error) {
    if (error.code === "EPERM") return true;
    if (error.code === "ESRCH") return false;
    throw error;
  }
};

if (process.argv[2] === "--launcher") {
  const child = spawn(process.argv[3], [], {
    windowsHide: true,
    stdio: [0, "ignore", "ignore"]
  });
  process.stdout.write(`${child.pid}\n`, () => setTimeout(() => process.exit(0), 250));
} else {
  for (const name of ["ghostlight-mcp-connector", "ghostlight-browser-connector"]) {
    const path = executable(name);
    accessSync(path, constants.X_OK);

    const attended = spawn(path, [], {
      windowsHide: true,
      stdio: ["pipe", "ignore", "ignore"]
    });
    await delay(1_500);
    assert.equal(attended.exitCode, null, `${name} survives inactivity while its parent lives`);
    attended.stdin.end();
    const attendedCode = await new Promise((resolveClose, reject) => {
      attended.once("close", resolveClose);
      attended.once("error", reject);
    });
    assert.equal(attendedCode, 0, `${name} exits cleanly on stdin EOF`);

    const keeper = spawn(process.execPath, ["-e", "setInterval(() => {}, 1000)"], {
      windowsHide: true,
      stdio: ["ignore", "pipe", "ignore"]
    });
    const launcher = spawn(process.execPath, [import.meta.filename, "--launcher", path], {
      windowsHide: true,
      stdio: [keeper.stdout, "pipe", "inherit"]
    });
    const lines = createInterface({ input: launcher.stdout });
    const processId = Number(await new Promise((resolveLine, reject) => {
      lines.once("line", resolveLine);
      launcher.once("error", reject);
    }));
    assert.ok(Number.isInteger(processId) && processId > 0, `${name} reported a process id`);
    await new Promise((resolveClose, reject) => {
      launcher.once("close", code => code === 0
        ? resolveClose()
        : reject(new Error(`${name} launcher exited ${code}`)));
      launcher.once("error", reject);
    });
    assert.equal(keeper.exitCode, null, `${name} input writer remains alive after its parent exits`);

    const deadline = Date.now() + 5_000;
    while (alive(processId) && Date.now() < deadline) await delay(100);
    try {
      assert.equal(alive(processId), false, `${name} exits after its exact parent dies`);
    } finally {
      if (keeper.exitCode === null) {
        const keeperExited = new Promise(resolveExit => keeper.once("exit", resolveExit));
        keeper.kill();
        await keeperExited;
      }
      if (alive(processId)) process.kill(processId, "SIGKILL");
    }
  }
  console.log("connector parent lifecycle ok: open stdin cannot preserve an orphan");
}
