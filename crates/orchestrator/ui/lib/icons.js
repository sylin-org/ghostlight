// Ghostlight workbench -- the agents' own icons.
//
// The service serves an installed app's icon to the window and to a watcher, and says only that
// watching is off to everyone else. An image drawn by address carries no key, so this page asks
// for each icon the way it asks for everything else, and then draws what came back. The view
// leaves `data-icon` on an image where an icon belongs; this module gives that image its source.
//
// It holds no policy and no markup. It learns of an icon once, remembers the answer by the icon's
// address (which carries its version), and fills every image that names it, again after each
// redraw.
(function installGhostlightIcons(root, factory) {
  const api = factory();
  root.GhostlightIcons = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightIconsApi() {
  "use strict";

  const { ICON_ATTRIBUTE } = globalThis.GhostlightWords;
  /** The selector of an image that waits for an icon. */
  const WAITING = `img[${ICON_ATTRIBUTE}]:not([src])`;
  /** What the page says when an icon could not be had. */
  const NOT_LOADED = "An agent's icon could not be loaded";

  /**
   * Icons for the images below `scope`.
   *
   * `load(address)` answers a source an image can draw, or throws. A failure is reported once per
   * address and leaves the image empty, so a missing icon costs one notice, never a retry loop.
   */
  function create({ scope, load, onFailure }) {
    /** The source of each address that was loaded, or null for one that could not be. */
    const sources = new Map();
    const asked = new Set();

    /** Give every waiting image whose icon is known its source, and ask for the ones that are not. */
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
      } catch (error) {
        sources.set(address, null);
        onFailure(NOT_LOADED, error);
      }
      settle();
    }

    return Object.freeze({ settle });
  }

  return Object.freeze({ create });
});
