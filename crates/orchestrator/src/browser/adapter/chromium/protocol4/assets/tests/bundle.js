"use strict";

// Test helper: builds the protocol-4 page bundle the way the service does.
//
// world.js holds one include marker. The service replaces the marker with the sources it names, in
// the order it names them, each followed by one newline. The result is `source`, and the
// lowercase hex SHA-256 of `source` is the asset id. Sources are read with LF line endings, so the
// id does not depend on the checkout's line-ending conversion. The extension then wraps the source as
// `function(__ghostlightAssetId) {\n<source>\n}` and runs it with the id as its argument.

const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");

const ASSETS = path.join(__dirname, "..");
const WORLD = "world.js";
const MARKER = /\/\*@include ([^*]*)\*\//g;

/** Read one asset source as text with LF line endings, as the service ships it. */
function read(name) {
  return fs.readFileSync(path.join(ASSETS, name), "utf8").replace(/\r\n/g, "\n");
}

/** The names the include marker lists, in order. */
function includes() {
  const markers = [...read(WORLD).matchAll(MARKER)];
  if (markers.length !== 1) throw new Error(`${WORLD} must hold exactly one include marker`);
  return markers[0][1].trim().split(/\s+/);
}

/** The expanded bundle source: the marker replaced by each listed file followed by a newline. */
function expand() {
  const body = includes().map((name) => `${read(name)}\n`).join("");
  return read(WORLD).replace(MARKER, () => body);
}

/** Lowercase hex SHA-256 of a string. */
function sha256(text) {
  return crypto.createHash("sha256").update(text).digest("hex");
}

/** The extension's fixed wrapper around the bundle source. */
function wrap(source) {
  return `function(__ghostlightAssetId) {\n${source}\n}`;
}

/** The script a registration or the lazy installer runs for one asset set. */
function registration(id, source) {
  return `(${wrap(source)})(${JSON.stringify(id)});`;
}

module.exports = { ASSETS, WORLD, includes, expand, sha256, wrap, registration, read };
