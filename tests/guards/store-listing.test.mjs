// Store listing guard: the paste-ready Chrome Web Store permission justifications.
//
// The Chrome dashboard needs one justification per manifest permission and cuts each field at
// 1000 characters. On 2026-08-22 the `downloads` and `offscreen` permissions shipped in the
// manifest without a justification, and a hand-written section list let that pass. So the
// sections are read from docs/legal/PERMISSION_JUSTIFICATIONS.md itself: its bare-identifier
// headings must equal the manifest's permissions, and every section, the host-permission and
// page-context ones included, must carry a fenced text block that fits the dashboard's field.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const PERMISSION_COPY = join(ROOT, "docs", "legal", "PERMISSION_JUSTIFICATIONS.md");
const EXTENSION_MANIFEST = join(ROOT, "extension", "manifest.json");

/** The most characters the Chrome dashboard keeps in one justification field. */
const MAX_DASHBOARD_JUSTIFICATION_CHARS = 1_000;
const SECTION_PREFIX = "## ";
const MANIFEST_PERMISSION_KEYS = ["permissions", "optional_permissions"];
const PASTE_OPEN = "```text\n";
const PASTE_CLOSE = "\n```";

/** The document's sections, one `{ heading, body }` per `## ` heading, in order. */
function sections() {
  const document = readFileSync(PERMISSION_COPY, "utf8").replaceAll("\r\n", "\n");
  const found = [];
  for (const line of document.split("\n")) {
    if (line.startsWith(SECTION_PREFIX)) {
      found.push({ heading: line.slice(SECTION_PREFIX.length).trimEnd(), body: "" });
    } else if (found.length > 0) {
      found.at(-1).body += `${line}\n`;
    }
  }
  return found;
}

/** The trimmed text of a section's fenced paste block. */
function pasteBlock({ heading, body }) {
  const opened = body.indexOf(PASTE_OPEN);
  assert.notEqual(opened, -1, `section ${heading} must contain a fenced text block`);
  const rest = body.slice(opened + PASTE_OPEN.length);
  const closed = rest.indexOf(PASTE_CLOSE);
  assert.notEqual(closed, -1, `section ${heading} has an unclosed fenced text block`);
  return rest.slice(0, closed).trim();
}

/** Whether a heading names a manifest permission: a letter, then letters and digits only. */
const isBareIdentifier = (heading) => /^[A-Za-z][A-Za-z0-9]*$/.test(heading);

/** Every permission the manifest asks for, required and optional, in order. */
function manifestPermissions() {
  const manifest = JSON.parse(readFileSync(EXTENSION_MANIFEST, "utf8"));
  const permissions = [];
  for (const key of MANIFEST_PERMISSION_KEYS) {
    if (!(key in manifest)) continue;
    assert.ok(Array.isArray(manifest[key]), `manifest ${key} must be an array`);
    for (const name of manifest[key]) {
      assert.equal(typeof name, "string", `manifest ${key} must hold only strings`);
      permissions.push(name);
    }
  }
  return permissions;
}

test("every justification fits the Chrome dashboard's field", () => {
  const found = sections();
  assert.ok(found.length > 0, "no permission-justification sections");
  for (const section of found) {
    const copy = pasteBlock(section);
    assert.notEqual(copy, "", `section ${section.heading} has empty dashboard copy`);
    const characters = [...copy].length;
    assert.ok(
      characters <= MAX_DASHBOARD_JUSTIFICATION_CHARS,
      `section ${section.heading} is ${characters} characters; Chrome allows at most ${MAX_DASHBOARD_JUSTIFICATION_CHARS}`
    );
  }
});

test("every manifest permission has exactly one justification", () => {
  const permissions = manifestPermissions();
  const unique = new Set(permissions);
  assert.equal(unique.size, permissions.length, "the manifest lists a permission twice");
  const headings = sections().map(({ heading }) => heading).filter(isBareIdentifier);
  const documented = new Set(headings);
  assert.equal(documented.size, headings.length, "a permission has two justification sections");
  assert.deepEqual(
    [...documented].sort(),
    [...unique].sort(),
    "manifest permissions and Chrome Web Store justifications differ"
  );
});
