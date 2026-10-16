// The fixed home and bin on real executables, in a test home with an isolated user profile.
//   - only the copy in `<home>/bin` serves; a copy anywhere else refuses;
//   - a carrier copy (the downloaded package) installs itself into `bin` on first use and
//     afterwards only hands over;
//   - replacing the service image under a running MCP edge keeps the same edge process, the same
//     session and the same catalog.
// This proves executable custody and relay continuity, not an installed browser journey.
import assert from "node:assert/strict";
import { appendFileSync, existsSync, readFileSync, readdirSync } from "node:fs";
import { createHash } from "node:crypto";
import { join } from "node:path";
import { EDGE_NAME, SERVICE_NAME, binaryDirectory, copyImages, JourneyHome, startReason, until } from "./lib/home.mjs";
import { McpEdge } from "./lib/peers.mjs";

const digest = (path) => createHash("sha256").update(readFileSync(path)).digest("hex");
const home = new JourneyHome({ label: "fixed" });
const stage = join(home.area, "download");

try {
  copyImages(binaryDirectory(), stage);
  const carrierEdge = join(stage, EDGE_NAME);
  const carrierService = join(stage, SERVICE_NAME);
  assert.equal(existsSync(home.bin), false, "the home starts with nothing installed");

  // Only the installed service serves.
  const refused = home.run(carrierService, ["serve", "--reason", startReason.agent]);
  assert.notEqual(refused.status, 0, "a copy outside bin refuses to serve");
  assert.ok(refused.stderr.trim().length > 0, "the refusal says why");
  assert.equal(await home.status(), null, "a refused serve leaves no service");
  console.log("PASS serve from another path is refused");

  // A carrier installs itself on first use.
  const first = home.run(carrierEdge, ["call", "--catalog"], { timeout: 120_000 });
  assert.equal(first.status, 0, first.stderr || first.stdout);
  assert.ok(first.stdout.length > 0, "the catalog is printed");
  assert.equal(digest(home.edge), digest(carrierEdge), "the edge is installed in bin");
  assert.equal(digest(home.service), digest(carrierService), "the service is installed in bin");
  const installedEdge = digest(home.edge);
  const status = await home.serviceUp();
  assert.ok(status.pid > 0);
  const second = home.run(carrierEdge, ["call", "--catalog"], { timeout: 120_000 });
  assert.equal(second.status, 0, second.stderr || second.stdout);
  assert.equal(digest(home.edge), installedEdge, "a second use only hands over");
  assert.equal((await home.status()).instance, status.instance, "a second use does not replace the service");
  console.log("PASS a carrier copy installs into bin on first use and only hands over afterwards");

  // The same MCP edge and session survive a replaced service image.
  const mcp = new McpEdge(home.start(home.edge, ["mcp"]));
  await mcp.initialize("fixed installation");
  const catalogBefore = (await mcp.request("tools/list")).result.tools.map((tool) => tool.name);
  assert.ok(catalogBefore.length > 0, "the all-open policy advertises a catalog");
  const edgePid = mcp.child.pid;
  // PE and ELF allow trailing data: change only the service image, not the edge.
  appendFileSync(carrierService, Buffer.from("\nfixed-installation-service-update\n"));
  const replaced = home.run(carrierService, ["deployment", "install"], { timeout: 120_000 });
  assert.equal(replaced.status, 0, replaced.stderr || replaced.stdout);
  assert.equal(digest(home.service), digest(carrierService), "the service image is replaced in bin");
  await until(async () => {
    const now = await home.status();
    return now && now.instance !== status.instance;
  }, "a new service answers", 60_000);
  assert.equal(mcp.child.exitCode, null, "the MCP edge stays up");
  assert.equal(mcp.child.pid, edgePid);
  const catalogAfter = (await mcp.request("tools/list")).result.tools.map((tool) => tool.name);
  assert.deepEqual(catalogAfter, catalogBefore, "the same session serves the same catalog");
  const entries = readdirSync(home.bin).sort();
  assert.deepEqual(entries, [EDGE_NAME, SERVICE_NAME].sort(), "bin holds the two images and no leftovers");
  mcp.child.stdin.end();
  console.log("PASS replacing the service keeps the MCP edge, its session and the catalog; bin holds only the installed images");
  console.log("fixed installation journey ok");
} finally {
  await home.cleanup();
}
