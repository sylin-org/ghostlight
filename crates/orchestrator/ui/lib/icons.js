// Ghostlight workbench -- the agents' images.
//
// The service has one image address per agent and decides what it answers there: the installed
// app's own icon when it has one, else the agent's monogram. It answers the window and a watcher,
// and says only that watching is off to everyone else. An image drawn by address carries no key,
// so this page asks for each address the way it asks for everything else, and then draws what came
// back. The view leaves `data-icon` on an image where an agent's image belongs; this module gives
// that image its source.
//
// It holds no policy and no markup. It asks for an address once, remembers the answer by the
// address (which carries the image's version), and fills every image that names it, again after
// each redraw. An image that could not be had keeps the neutral tile it waits on; nothing is said.
(function installGhostlightIcons(root, factory) {
  const api = factory();
  root.GhostlightIcons = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightIconsApi() {
  "use strict";

  const { ICON_ATTRIBUTE } = globalThis.GhostlightWords;
  /** The selector of an image that waits for its source. */
  const WAITING = `img[${ICON_ATTRIBUTE}]:not([src])`;

  /**
   * Images for the images below `scope`.
   *
   * `load(address)` answers a source an image can draw, or throws. A failure leaves the image on
   * its tile and is not asked again, so a missing image costs nothing and never loops.
   */
  function create({ scope, load }) {
    /** The source of each address that was loaded, or null for one that could not be. */
    const sources = new Map();
    const asked = new Set();

    /** Give every waiting image whose source is known that source, and ask for the ones that are not. */
    function settle() {
      for (const image of scope.querySelectorAll(WAITING)) {
        const address = image.getAttribute(ICON_ATTRIBUTE);
        if (sources.has(address)) {
          const source = sources.get(address);
          if (source !== null) image.setAttribute("src", source);
        } else {
          ask(address);
        }
      }
    }

    async function ask(address) {
      if (asked.has(address)) return;
      asked.add(address);
      try {
        sources.set(address, await load(address));
      } catch {
        sources.set(address, null);
      }
      settle();
    }

    return Object.freeze({ settle });
  }

  return Object.freeze({ create });
});
