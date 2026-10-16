// The set of files the extension store package holds, and the hash of each (ADR-0200 D16).
//
// This reads the same lists scripts/package-extension.ps1 stages and computes each file's
// SHA-256 the way the packager writes it: text files with LF line endings, the manifest without
// its development `key` and in the packager's serialization. The freeze guard compares the result
// with tests/golden/extension-2.0.0.files.sha256.
//
// Run `node tests/guards/lib/extension-package.mjs <zip>` to print the golden for a packaged zip:
// one `<sha256>  <path>` line per packaged path, then the zip's own hash.

import { createHash } from "node:crypto";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
const PACKAGER = join(ROOT, "scripts", "package-extension.ps1");

/** Extensions the packager rewrites with LF line endings. */
const TEXT_EXTENSIONS = new Set([".css", ".html", ".js", ".json", ".md", ".txt"]);

/** Directories the packager copies whole. */
const WHOLE_DIRECTORIES = ["lib", "vendor"];

/** The packaged license files and the repository files they come from. */
const LICENSES = [
  ["licenses/Apache-2.0.txt", "LICENSE"],
  ["licenses/MIT.txt", "docs/licenses/MIT.txt"],
];

/** The golden's last line names the zip, which is recorded and not compared. */
export const ZIP_NAME = "extension-2.0.0.zip";

/** The golden file the freeze guard compares against. */
export const GOLDEN_PATH = join(ROOT, "tests", "golden", "extension-2.0.0.files.sha256");

/** Reads one `$name = @( "a", "b" )` list from the packager script. */
function packagerList(name) {
  const script = readFileSync(PACKAGER, "utf8");
  const match = new RegExp(String.raw`[$]${name}\s*=\s*@\(([^)]*)\)`).exec(script);
  if (!match) throw new Error(`scripts/package-extension.ps1 no longer declares $${name}`);
  const items = [...match[1].matchAll(/"([^"]+)"/g)].map((item) => item[1]);
  if (items.length === 0) throw new Error(`scripts/package-extension.ps1 declares an empty $${name}`);
  return items;
}

function walk(directory) {
  const found = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) found.push(...walk(path));
    else found.push(path);
  }
  return found;
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function packagedBytes(path, source) {
  const bytes = readFileSync(source);
  const extension = path.includes(".") ? path.slice(path.lastIndexOf(".")).toLowerCase() : "";
  if (!TEXT_EXTENSIONS.has(extension)) return bytes;
  return Buffer.from(bytes.toString("latin1").replace(/\r\n/g, "\n"), "latin1");
}

/** The manifest as the packager writes it: no `key`, CRLF lines, one trailing LF. */
function packagedManifest(source) {
  const manifest = JSON.parse(readFileSync(source, "utf8"));
  delete manifest.key;
  return Buffer.from(JSON.stringify(manifest, null, 2).replace(/\n/g, "\r\n") + "\n", "utf8");
}

/** Maps every packaged path to the SHA-256 of its packaged bytes. */
export function packagedHashes() {
  const extension = join(ROOT, "extension");
  const paths = new Map();
  for (const file of packagerList("rootFiles")) paths.set(file, join(extension, file));
  for (const file of packagerList("iconFiles")) paths.set(`icons/${file}`, join(extension, "icons", file));
  for (const directory of WHOLE_DIRECTORIES) {
    for (const file of walk(join(extension, directory))) {
      paths.set(relative(extension, file).split("\\").join("/"), file);
    }
  }
  for (const [path, from] of LICENSES) paths.set(path, join(ROOT, from));
  const hashes = new Map();
  for (const path of [...paths.keys()].sort()) {
    const source = paths.get(path);
    hashes.set(path, sha256(path === "manifest.json" ? packagedManifest(source) : packagedBytes(path, source)));
  }
  return hashes;
}

/** Renders the golden: sorted file lines, then the zip's hash. */
export function renderGolden(hashes, zipHash) {
  const lines = [...hashes].map(([path, hash]) => `${hash}  ${path}`);
  lines.push(`${zipHash}  ${ZIP_NAME}`);
  return lines.join("\n") + "\n";
}

/** Parses the golden into a path-to-hash map and the recorded zip hash. */
export function parseGolden(text) {
  const hashes = new Map();
  let zip = null;
  for (const line of text.split("\n")) {
    if (line === "") continue;
    const match = /^([0-9a-f]{64})  (\S+)$/.exec(line);
    if (!match) throw new Error(`malformed golden line: ${line}`);
    if (match[2] === ZIP_NAME) zip = match[1];
    else hashes.set(match[2], match[1]);
  }
  return { hashes, zip };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const zipPath = process.argv[2];
  if (!zipPath) throw new Error("usage: node tests/guards/lib/extension-package.mjs <zip>");
  process.stdout.write(renderGolden(packagedHashes(), sha256(readFileSync(zipPath))));
}
