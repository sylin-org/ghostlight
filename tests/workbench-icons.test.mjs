// The agents' images, exercised for real.
//
// The view leaves an image with the agent's image address and no source, the same for every
// agent; the icons module asks for the address once and fills in every image that names it. These
// tests run the real modules against a small stand-in for the part of the document they touch. No
// page and no Ghostlight process is involved.
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

test("the view draws every agent's image the same way, an address and no source, and keeps an app's icon on its tile", () => {
  const appIcon = { symbol: { icon: ADDRESS, about: "Icon from the installed Zed app." } };
  const monogram = { symbol: { icon: "agent-icons/codex?v=fedcba987654" } };
  assert.equal(
    Words.symbolMarkup(appIcon, "chip"),
    `<span class="symbol symbol-chip ${Words.APP_ICON_CLASS}" aria-hidden="true"><img ${Words.ICON_ATTRIBUTE}="${ADDRESS}" alt="" width="14" height="14"></span>`,
    "the service says where an app's icon came from, so it stays on the neutral tile"
  );
  assert.equal(
    Words.symbolMarkup(monogram, "chip"),
    `<span class="symbol symbol-chip" aria-hidden="true"><img ${Words.ICON_ATTRIBUTE}="agent-icons/codex?v=fedcba987654" alt="" width="14" height="14"></span>`,
    "a monogram is a tile of its own"
  );
  assert.equal(Words.symbolAbout(appIcon), "Icon from the installed Zed app.");
  assert.equal(Words.symbolAbout(monogram), "", "a monogram says nothing about where it came from");
  assert.ok(!Words.symbolMarkup({ symbol: { src: "integrations/zed.svg" } }, "chip").includes("<img"),
    "the page draws no image by a tile address");
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
    }
  });
  icons.settle();
  await settled();
  images[0] = image(ADDRESS);
  icons.settle();
  assert.equal(images[0].getAttribute("src"), "data:image/png;base64,AAAA", "filled at once");
  assert.equal(asks, 1);
});

test("an image with a source of its own is left alone", async () => {
  const drawn = image(ADDRESS, "data:image/png;base64,AAAA");
  const icons = Icons.create({
    scope: scopeOf([drawn]),
    load: async () => assert.fail("a drawn image asks for nothing")
  });
  icons.settle();
  await settled();
  assert.equal(drawn.getAttribute("src"), "data:image/png;base64,AAAA");
});

test("an image that cannot be had keeps its tile, is not asked for again, and says nothing", async () => {
  const images = [image(ADDRESS)];
  let asks = 0;
  const icons = Icons.create({
    scope: scopeOf(images),
    load: async () => {
      asks += 1;
      throw new Error("Watching is off.");
    }
  });
  icons.settle();
  await settled();
  icons.settle();
  await settled();
  assert.equal(asks, 1);
  assert.equal(images[0].getAttribute("src"), null, "the image waits on its neutral tile");
});
