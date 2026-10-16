// The installed Ghostlight a live journey drives.
//
// A live journey runs `ghostlight mcp` from the Ghostlight home's `bin` directory: an edge that runs
// from anywhere else is a carrier, and `mcp` through a repo-built one would carrier-install that
// build as a release. So the journey names the installed edge, and records the images it ran
// against so the evidence says which build was active.
import { createHash } from "node:crypto";
import { existsSync, readFileSync, realpathSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { BIN_DIRECTORY, EDGE_NAME, SERVICE_NAME, suffix } from "./home.mjs";

/** The shell's installed image. It is optional: the product runs without a window. */
const DESKTOP_NAME = `ghostlight-desktop${suffix}`;

/** Compare paths as the edge's own route check does: with links resolved, case-blind on Windows. */
function samePlace(left, right) {
  const real = (path) => {
    try { return realpathSync(path); } catch { return resolve(path); }
  };
  const [a, b] = [real(left), real(right)];
  return process.platform === "win32" ? a.toLowerCase() === b.toLowerCase() : a === b;
}

/** The `bin` directory of the Ghostlight home the edge and the service resolve. */
export function homeBinDirectory() {
  const home = process.env.GHOSTLIGHT_TEST_HOME || join(homedir(), ".ghostlight");
  return join(home, BIN_DIRECTORY);
}

/**
 * The installed `bin` directory a journey drives: `GHOSTLIGHT_BIN_DIR` when set, otherwise the home's
 * own. It must be the home's `bin`, because that is the only place an edge serves from.
 */
export function installedBinDirectory() {
  const home = homeBinDirectory();
  const named = process.env.GHOSTLIGHT_BIN_DIR;
  if (named && !samePlace(named, home)) {
    throw Object.assign(new Error(
      `GHOSTLIGHT_BIN_DIR ${named} is not the Ghostlight home's bin directory ${home}; an edge outside it is a carrier and would install itself as a release`),
    { code: "ERR_NOT_INSTALLED_BIN" });
  }
  return resolve(named || home);
}

/** The installed edge in `bin`; the journey runs `<edge> mcp`. */
export function installedEdge(bin) {
  const path = join(bin, EDGE_NAME);
  if (!existsSync(path)) throw Object.assign(new Error(`The installed Ghostlight edge is missing: ${path}`), { code: "ENOENT" });
  return path;
}

/**
 * The installed images with their SHA-256: the edge and the service, plus the shell when it is
 * installed. The edge and the service must be there.
 */
export function installedImages(bin) {
  const images = {};
  for (const [label, name, required] of [["ghostlight", EDGE_NAME, true], ["ghostlight-service", SERVICE_NAME, true],
    ["ghostlight-desktop", DESKTOP_NAME, false]]) {
    const path = join(bin, name);
    if (!existsSync(path)) {
      if (required) throw Object.assign(new Error(`The installed Ghostlight image is missing: ${path}`), { code: "ENOENT" });
      continue;
    }
    images[label] = { path, sha256: createHash("sha256").update(readFileSync(path)).digest("hex") };
  }
  return images;
}
