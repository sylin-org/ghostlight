// The agents' icons, exercised for real.
//
// The view leaves an image with the address of an icon and no source; the icons module asks for
// the icon once and fills in every image that names it. These tests run the real modules against
// a small stand-in for the part of the document they touch. No page and no Ghostlight process is
// involved.
//
// Run with: node --test tests/workbench-icons.test.mjs
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { test } from "node:test";
import { join, resolve } from "node:path";

const require = createRequire(import.meta.url);
const ui = join(resolve(import.meta.dirname, ".."), "crates", "orchestrator", "ui", "lib");
const Words = require(join(ui, "words.js"));
const Icons = require(join(ui, "icons.js"));

const ADDRESS = "agent-icons/zed?v=0123456789ab";

/** An image as the module touches it. */
function image(address, source) {
  const attributes = new Map();
  if (address !== undefined) attributes.set(Words.ICON_ATTRIBUTE, address);
  if (source !== undefined) attributes.set("src", source);
  return {
    getAttribute: (name) => attributes.get(name) ?? null,
    setAttribute: (name, value) => attributes.set(name, value)
  };
}

/** A scope whose waiting images are the ones with an address and no source. */
function scopeOf(images) {
  return {
    querySelectorAll: (selector) => {
      assert.equal(selector, `img[${Words.ICON_ATTRIBUTE}]:not([src])`);
      return images.filter((one) => one.getAttribute(Words.ICON_ATTRIBUTE) !== null && one.getAttribute("src") === null);
    }
  };
}

const settled = () => new Promise((resolve) => setImmediate(resolve));

test("the view leaves an app's icon as an address and no source, and a tile as a source", () => {
  const zed = { symbol: { icon: ADDRESS, about: "Icon from the installed Zed app." } };
  assert.equal(
    Words.symbolMarkup(zed, "chip"),
    `<span class="symbol symbol-chip symbol-icon" aria-hidden="true"><img ${Words.ICON_ATTRIBUTE}="${ADDRESS}" alt="" width="14" height="14"></span>`
  );
  assert.equal(
    Words.symbolMarkup({ symbol: { src: "integrations/zed.svg" } }, "chip"),
    '<span class="symbol symbol-chip" aria-hidden="true"><img src="integrations/zed.svg" alt="" width="14" height="14"></span>'
  );
});

test("an icon is asked for once, however many images name it, and each gets its source", async () => {
  const images = [image(ADDRESS), image(ADDRESS), image("agent-icons/codex?v=0123456789ab")];
  const asked = [];
  const icons = Icons.create({
    scope: scopeOf(images),
    load: async (address) => {
      asked.push(address);
      return `data:image/png;base64,${address.length}`;
    },
    onFailure: () => assert.fail("nothing failed")
  });
  icons.settle();
  icons.settle();
  await settled();
  assert.deepEqual(asked, [ADDRESS, "agent-icons/codex?v=0123456789ab"]);
  assert.equal(images[0].getAttribute("src"), `data:image/png;base64,${ADDRESS.length}`);
  assert.equal(images[1].getAttribute("src"), images[0].getAttribute("src"));
  assert.ok(images[2].getAttribute("src").startsWith("data:image/png"));
});

test("an image drawn again is filled from what is known, without asking again", async () => {
  const images = [image(ADDRESS)];
  let asks = 0;
  const icons = Icons.create({
    scope: scopeOf(images),
    load: async () => {
      asks += 1;
      return "data:image/png;base64,AAAA";
    },
    onFailure: () => assert.fail("nothing failed")
  });
  icons.settle();
  await settled();
  images[0] = image(ADDRESS);
  icons.settle();
  assert.equal(images[0].getAttribute("src"), "data:image/png;base64,AAAA", "filled at once");
  assert.equal(asks, 1);
});

test("an image with a source of its own is left alone", async () => {
  const tile = image(ADDRESS, "integrations/zed.svg");
  const icons = Icons.create({
    scope: scopeOf([tile]),
    load: async () => assert.fail("a drawn image asks for nothing"),
    onFailure: () => assert.fail("nothing failed")
  });
  icons.settle();
  await settled();
  assert.equal(tile.getAttribute("src"), "integrations/zed.svg");
});

test("an icon that cannot be had is reported once and leaves the image empty", async () => {
  const images = [image(ADDRESS)];
  const reported = [];
  let asks = 0;
  const icons = Icons.create({
    scope: scopeOf(images),
    load: async () => {
      asks += 1;
      throw new Error("Watching is off.");
    },
    onFailure: (what, error) => reported.push(`${what}: ${error.message}`)
  });
  icons.settle();
  await settled();
  icons.settle();
  await settled();
  assert.equal(asks, 1);
  assert.deepEqual(reported, ["An agent's icon could not be loaded: Watching is off."]);
  assert.equal(images[0].getAttribute("src"), null);
});
