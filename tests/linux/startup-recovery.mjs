// Real Linux executable boundaries: a start with no desktop session, and contention.
//
// The service is a console program with no window of its own (ADR-0199 Decision 4), so a start
// that has no display, no session bus and no desktop variables still serves. Four MCP clients
// start at once in such an environment; all four must initialize and converge on one service.
// The lane runs in a fresh test home under the temp directory and never changes an installed
// registration or a browser profile.
import assert from "node:assert/strict";
import { readdirSync, readlinkSync } from "node:fs";
import { createHome, delay, samePath, startReason } from "../lib/home.mjs";
import { McpEdge } from "../lib/peers.mjs";

assert.equal(process.platform, "linux", "This process-observation lane requires Linux");

/** The variables that tell a program a desktop session exists. */
const DESKTOP_VARIABLES = ["DISPLAY", "WAYLAND_DISPLAY", "DBUS_SESSION_BUS_ADDRESS", "XAUTHORITY"];
const CALLERS = 4;

const home = createHome({ label: "startup" });
for (const name of DESKTOP_VARIABLES) delete home.environment[name];

/** The processes whose executable is this home's service image. */
function services() {
  return readdirSync("/proc").filter((name) => /^\d+$/.test(name)).filter((pid) => {
    try {
      return samePath(readlinkSync(`/proc/${pid}/exe`), home.service);
    } catch {
      return false;
    }
  });
}

try {
  const clients = Array.from({ length: CALLERS }, () => new McpEdge(home.start(home.edge, ["mcp"])));
  const started = Date.now();
  const initialized = await Promise.all(clients.map((client, index) => client.initialize(`startup recovery ${index}`)));
  assert.equal(initialized.length, CALLERS, "every caller initializes without a desktop session");
  for (const result of initialized) assert.equal(result.serverInfo.name, "ghostlight");
  const status = await home.serviceUp();
  assert.equal(status.started_by, startReason.agent);
  assert.equal(services().length, 1, "concurrent clients converge on one service");
  await delay(500);
  const later = await home.status();
  assert.equal(later.instance, status.instance, "the ready service survives startup");
  assert.equal(services().length, 1);
  for (const client of clients) client.child.stdin.end();
  console.log(JSON.stringify({ passed: true, callers: CALLERS, recovery_ms: Date.now() - started, service_pid: status.pid }, null, 2));
} finally {
  await home.cleanup();
}
