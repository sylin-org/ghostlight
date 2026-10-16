#!/usr/bin/env node
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- recapture tests/fixtures/sylin-iframe.json from the public Sylin iframe demo.
//
// It fetches the two public HTML pages and every CSS, JavaScript and image file they reference
// (stylesheets, icons, scripts, images, and url() targets inside fetched CSS), and writes the
// fixture the frame journey serves: page bodies and assets with source, content type and SHA-256.
// Unicode is JSON-escaped and binary bodies are base64, so the file stays ASCII. It fetches public
// pages only and starts no Ghostlight program. Usage: node scripts/capture-sylin-iframe.mjs [out]
// (default tests/fixtures/sylin-iframe.json). Review the changed bytes, then repeat both browser
// lanes of the frame journey before recording evidence.
import { createHash } from "node:crypto";
import { writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const ORIGIN = "https://sylin.org";
const PAGES = [`${ORIGIN}/ghostlight/demo/iframe/`, `${ORIGIN}/ghostlight/demo/iframe/form/`];
const TIMEOUT_MS = 20000;
const out = process.argv[2] ?? join(dirname(fileURLToPath(import.meta.url)), "..", "tests/fixtures/sylin-iframe.json");
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

async function get(url) {
  const response = await fetch(url, { signal: AbortSignal.timeout(TIMEOUT_MS) });
  if (response.status !== 200) throw new Error(`${url} answered ${response.status}`);
  return { response, bytes: Buffer.from(await response.arrayBuffer()) };
}

// The same-origin resources an HTML document loads: stylesheets, icons, scripts and images.
function referencedBy(html, base) {
  const found = [];
  const tag = /<(link|script|img)\b[^>]*>/gi;
  for (const [markup, name] of html.matchAll(tag)) {
    const attribute = name.toLowerCase() === "link" ? "href" : "src";
    const value = markup.match(new RegExp(`\\s${attribute}="([^"]+)"`, "i"))?.[1];
    if (!value) continue;
    if (name.toLowerCase() === "link" && !/\brel="(?:stylesheet|icon)"/i.test(markup)) continue;
    found.push(new URL(value, base));
  }
  return found;
}

// The same-origin url() targets of a stylesheet, other than inline data.
function urlsOfCss(css, base) {
  return [...css.matchAll(/url\(\s*["']?([^"')]+)["']?\s*\)/gi)]
    .filter(([, value]) => !value.startsWith("data:"))
    .map(([, value]) => new URL(value, base));
}

const pages = [];
const queue = [];
for (const url of PAGES) {
  const { bytes } = await get(url);
  const body = bytes.toString("utf8");
  pages.push({ url, sha256: sha256(body), body });
  queue.push(...referencedBy(body, url));
}
const assets = [];
const seen = new Set();
while (queue.length > 0) {
  const url = queue.shift();
  if (url.origin !== ORIGIN) throw new Error(`${url} is not on ${ORIGIN}`);
  const path = url.pathname + url.search;
  if (seen.has(path)) continue;
  seen.add(path);
  const { response, bytes } = await get(url);
  const contentType = response.headers.get("content-type");
  assets.push({ path, content_type: contentType, sha256: sha256(bytes), data_base64: bytes.toString("base64") });
  if (/^text\/css/i.test(contentType ?? "")) queue.push(...urlsOfCss(bytes.toString("utf8"), url));
}

const text = JSON.stringify({ captured_on: new Date().toISOString().slice(0, 10), pages, assets }, null, 2)
  .replace(/[\u0080-\uffff]/g, (unit) => `\\u${unit.charCodeAt(0).toString(16).padStart(4, "0")}`);
writeFileSync(out, `${text}\n`);
console.log(`${out}: ${pages.length} pages, ${assets.length} assets`);
for (const asset of assets) console.log(`  ${asset.path} ${asset.content_type} ${asset.sha256}`);
