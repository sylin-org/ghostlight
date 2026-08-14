# M05: extract service-worker pure logic into extension/lib/ with node tests

## Goal

ADR-0026 Decision 6 (first layer): the service worker's pure algorithmic units
(screenshot geometry, key/input tables) move into standalone lib files that
load in the worker via importScripts and run under `node --test`, with pinned
unit tests. content.js, the manifest, and all behavior stay untouched.

## Authority

ADR-0026 Decision 6; 00-design.md "Extension lib extraction (m05)" (module
pattern and file placement pinned there). The narrowed scope (CLEAN units
only; content.js units deferred) is pinned in 00-design.md "Excluded from this
batch".

## Depends on

m02 (SPDX headers exist; your new files carry the engine header). m03 for the
final step only (ci.yml exists to append a job; if m03 was BLOCKED, do
everything except that step and record the sub-step as blocked-by-m03).
STOP preconditions: `rg -n "function rescaleCoord" extension/service-worker.js`
matches; `rg -n "importScripts" extension/` prints nothing; extension/lib/
does not exist. If any fails, STOP.

## Current behavior (verified 2026-07-03 at commit b1a7e9e; re-read every
cited site before editing -- line numbers WILL have shifted by one after m02's
header insertion)

extension/service-worker.js (1324 lines pre-m02):

- Line 315: `const PX_PER_TOKEN = 28, MAX_TOKENS = 1568, MAX_SIDE = 1568,
  MAX_SCREENSHOT_B64 = 1100000;`
- `targetDims(vpW, vpH)` lines 329-336 (token + longest-side budget; uses the
  three budget consts and Math only).
- `zoomScale(w, h)` lines 339-343 (min of side/token scales plus the 0.98
  shrink loop).
- `rescaleCoord(tabId, x, y)` lines 364-369: reads `screenshotCtx.get(tabId)`
  (module Map, line 36) then pure math over the ctx record
  `{ vpW, vpH, shotW, shotH, offX, offY, regionW, regionH }`:
  passthrough round when no ctx or no shotW/shotH; else
  `[round((offX||0) + (x*rw)/shotW), round((offY||0) + (y*rh)/shotH)]` with
  `rw = regionW || vpW`, `rh = regionH || vpH`.
  Call sites: zoomScreenshot (696-697), resolveCoords (789), left_click_drag
  (1099-1100).
- Key/input tables and functions: `KEY_MAP` (751-757), `BUTTON_BITS`
  (758-759), `modifierBits(str)` (762-771), `keyCode(key)` (891-899),
  `VK_NAMED` (900-905), `VK_PUNCT` (906-910), `CODE_PUNCT` (911-916),
  `vkCode(key)` (917-925), `SHIFT_BASE` (926-932), `charKeyInfo(ch)`
  (933-944). Consumers: pressKey (858-890), the type case (1002-1027),
  click (772-786), computer (line 961 uses modifierBits).
- The worker is a CLASSIC service worker (no "type": "module");
  importScripts is available and currently unused.
- All extension JS is pure ASCII (the ghost emoji is the escape
  `"\u{1F47B}"`, service-worker.js line 19).
- No package.json, no JS tests anywhere. tests/ is Rust only.
- scripts/package-extension.ps1 zips everything under extension/ except
  native-messaging-host.json and README.md, so extension/lib/*.js ships in
  the store package automatically. Test files must therefore live OUTSIDE
  extension/ (tests/extension/).

## Required behavior

### 1. extension/lib/geometry.js (new)

Header comment: SPDX engine line, then a one-line role comment. Content:
the three budget consts (PX_PER_TOKEN, MAX_TOKENS, MAX_SIDE), then VERBATIM
moves of targetDims and zoomScale, then a new pure function:

    function rescaleCtxCoord(c, x, y) {
      if (!c || !c.shotW || !c.shotH) return [Math.round(x), Math.round(y)];
      const rw = c.regionW || c.vpW, rh = c.regionH || c.vpH;
      return [Math.round((c.offX || 0) + (x * rw) / c.shotW), Math.round((c.offY || 0) + (y * rh) / c.shotH)];
    }

(the body of today's rescaleCoord with the Map lookup lifted out). Footer:
the 00-design.md dual-export pattern with name `GhostlightGeometry`, exporting
PX_PER_TOKEN, MAX_TOKENS, MAX_SIDE, targetDims, zoomScale, rescaleCtxCoord.

### 2. extension/lib/keys.js (new)

Same shape, name `GhostlightKeys`. VERBATIM moves of: KEY_MAP, BUTTON_BITS,
modifierBits, keyCode, VK_NAMED, VK_PUNCT, CODE_PUNCT, vkCode, SHIFT_BASE,
charKeyInfo. Export all ten.

### 3. extension/service-worker.js edits

- First executable statement (after the opening comment block):
  `importScripts("lib/geometry.js", "lib/keys.js");`
- Replace the moved definitions with destructuring:
  `const { PX_PER_TOKEN, MAX_TOKENS, MAX_SIDE, targetDims, zoomScale, rescaleCtxCoord } = self.GhostlightGeometry;`
  (keep `const MAX_SCREENSHOT_B64 = 1100000;` in the worker, split out of the
  old combined const), and
  `const { KEY_MAP, BUTTON_BITS, modifierBits, keyCode, VK_NAMED, VK_PUNCT, CODE_PUNCT, vkCode, SHIFT_BASE, charKeyInfo } = self.GhostlightKeys;`
- rescaleCoord becomes a thin delegate at its current location:

      function rescaleCoord(tabId, x, y) {
        return rescaleCtxCoord(screenshotCtx.get(tabId), x, y);
      }

- Every call site stays byte-identical. The moved function bodies are DELETED
  from the worker (no duplicates).

### 4. tests/extension/geometry.test.js (new; node:test + node:assert)

Named tests with pinned assertions (computed at authoring; do not re-derive):

- `targetDims passes small viewports through`: targetDims(1280, 720) deep-equals
  { w: 1280, h: 720 }.
- `targetDims shrinks to the token budget`: targetDims(1920, 1080) deep-equals
  { w: 1466, h: 824 }.
- `targetDims clamps the longest side`: targetDims(4000, 100) deep-equals
  { w: 1568, h: 39 }.
- `targetDims never returns zero`: targetDims(1, 1) deep-equals { w: 1, h: 1 }.
- `zoomScale magnifies a small region within budget`: s = zoomScale(100, 100);
  assert 10.8 < s && s < 10.9, and
  Math.ceil(Math.round(100*s)/28) ** 2 <= 1568.
- `zoomScale shrinks a large region to the budget edge`: s = zoomScale(2000,
  1000); assert Math.round(2000*s) === 1568 and Math.round(1000*s) === 784.
- `rescaleCtxCoord passthrough without context`: rescaleCtxCoord(null, 10.4,
  20.6) deep-equals [10, 21].
- `rescaleCtxCoord maps screenshot px to viewport px`:
  rescaleCtxCoord({ vpW: 1280, vpH: 720, shotW: 1024, shotH: 576 }, 512, 288)
  deep-equals [640, 360].
- `rescaleCtxCoord adds zoom region offsets`:
  rescaleCtxCoord({ vpW: 1280, vpH: 720, shotW: 800, shotH: 600, offX: 100,
  offY: 50, regionW: 400, regionH: 300 }, 400, 300) deep-equals [300, 200].

### 5. tests/extension/keys.test.js (new)

Pinned from the data tables only (edge behavior of unquoted body paths is NOT
pinned; do not invent cases):

- `modifier bits match CDP values`: modifierBits("ctrl") === 2,
  modifierBits("alt") === 1, modifierBits("shift") === 8,
  modifierBits("meta") === 4, modifierBits("ctrl+shift") === 10.
- `named virtual key codes`: vkCode("Enter") === 13, vkCode("Tab") === 9.
- `punctuation maps`: vkCode(";") === 186, keyCode(";") === "Semicolon".
- `charKeyInfo maps newline to Enter`: charKeyInfo("\n").key === "Enter";
  same for "\r".
- `charKeyInfo rejects control and non-ASCII`: charKeyInfo("\u0001") === null
  and charKeyInfo("\u00e9") === null.

### 6. CI job (append to .github/workflows/ci.yml; skip-and-record if m03 BLOCKED)

    extension-unit:
      strategy:
        fail-fast: false
        matrix:
          os: [ubuntu-latest, windows-latest]
      runs-on: ${{ matrix.os }}
      steps:
        - uses: actions/checkout@v4
        - uses: actions/setup-node@v4
          with:
            node-version: "22"
        - run: node --test tests/extension/

(Indented as a sibling of the existing jobs.)

## Constraints

Function bodies move verbatim; the ONLY semantic change in the worker is the
Map-lookup lift in rescaleCoord (behavior identical by construction). No
manifest edit, no content.js edit, no chrome.* in lib files, no package.json.
lib files and test files carry the engine SPDX header. ASCII only.

## Tests

`node --test tests/extension/` passes (14 tests). rg checks:
`rg -c "function targetDims" extension/service-worker.js` prints nothing
(exit 1); `rg -c "function targetDims" extension/lib/geometry.js` prints `1`;
`rg -c "importScripts" extension/service-worker.js` prints `1`;
`rg -c "GhostlightKeys" extension/lib/keys.js` >= 2.

## Verification

`node --test tests/extension/` green; `cargo test` unchanged (no Rust edits);
`rg -n "[^\x00-\x7F]" extension/lib/ tests/extension/` empty; ASCII diff scan;
OPTIONAL local sanity: reload the unpacked extension in Chrome and run
scripts/live-demo.ps1 (record outcome if run; not required for acceptance).
Ledger entry; commit.

Commit subject: `refactor(extension): extract geometry and key tables to lib/ with node tests`

## Out of scope

content.js and its DUCKABLE units (a11y measure/emit, innerInput, find:
deferred per 00-design.md); agent-visual-indicator.js; popup.js; manifest.json;
package-extension.ps1; any behavior change; ES modules ("type": "module").
