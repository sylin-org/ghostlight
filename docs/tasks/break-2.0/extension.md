# Break 2.0: extension lane ledger

Lane: extension. It owns new files under `extension/`, `tests/golden/protocol4/**`, the level-4
page assets under `crates/orchestrator/src/browser/adapter/chromium/level4/assets/`,
`scripts/lint-js*.mjs`, and this ledger. Ground rules are in `BOOTSTRAP.md` when the connectors
lane has landed it. The target is `docs/adr/0197-contained-seams-and-the-2-0-break.md`; the wire
is `docs/design/adapter-protocol-4.md`.

BASE: `f28bcbd7` (dev when the lane started).

Gate: `npm test --prefix extension`, `node --check` on every new or changed `.js`, and from
extension-2 on `node --test "crates/orchestrator/src/browser/adapter/chromium/level4/assets/tests/*.test.js"`.
The lane builds no Rust.

## RESUME HERE

Next slice: the one after `extension-4-p4-sessions` in `lanes-break.json`. The level-4 page
assets are complete for this lane: `content.js` dispatches exactly the 32 vocabulary kinds, every
throw site is an `AssetError` with its code, and the cancel token ends `observe`,
`prepare_fill`, `verify_fill_retained` and the sensor waits. Any change to the six sources changes
the bundle hash, so update `tests/golden/protocol4/assets.sha256` deliberately in the same commit
(print it with `bundle.sha256(bundle.expand())` from `level4/assets/tests/bundle.js`).

## Landed

| Slice | What | Commit |
|---|---|---|
| extension-4-p4-sessions | `lib/sessions.js` (`GhostlightSessions`): attach leases, retained tabs, focus emulation, dialogs as `{type, message, sessionId}`, opt-in domains, `CDP_ALLOW` (33 methods) and `cdp()` with `browser_refused`, root and child session registry with registration identifiers, `forEachSession`, `rootSession`, `childSessions`, `onRelease` and `onSessionGone`; `tests/sessions.test.js` (41 tests). `lib/debugger.js` is untouched until the switch deletes it. | see `git log` |
| extension-3-p4-asset-content | Level-4 `content.js` behaviour: coded errors, locator epochs, cancel tokens, `verify_fill_retained`, `observe` `until_satisfied`, `hit_point`, `owner_box`, `owner_probe`, `focus_probe`, `require_focused`, `focus_text_field`, `attach_file`; `tests/content.test.js` (85 tests); new asset hash. | see `git log` |
| extension-2-p4-asset-shell | `level4/assets/` shell (world, page-shared, sensor, presentation-css, presentation, content), `tests/bundle.js`, `tests/fake-dom.js`, `tests/world.test.js` (18 tests), `tests/golden/protocol4/assets.sha256`. | see `git log` |
| extension-1-p4-contracts | `tests/golden/protocol4/` (vocabulary, 10 frames, 6 events, 40 requests, 80 receipts, 32 world messages, 12 tolerance cases), `extension/lib/protocol.js`, `extension/tests/golden-protocol4.test.js`. | see `git log` |

## Counts

- 17 primitives, 13 receipt failure codes, 10 call failure codes, 32 world message kinds,
  22 asset error codes (19 pre-effect).
- 22 tests in `golden-protocol4.test.js`, 41 in `sessions.test.js`.
- 19 tests in `level4/assets/tests/world.test.js`, 85 in `level4/assets/tests/content.test.js`.

## Fixture conventions

- Every wire fixture is a whole frame, so `requests/*` and `receipts/*` carry `kind`,
  `correlation`, and the primitive or outcome.
- Request files are `<primitive>[.<variant>].json`. Receipt files are `<primitive>.<case>.json`,
  where the case is `ok`, `ok_<name>` for another successful outcome, or the failure code. The
  one receipt for an unknown primitive is `unknown_primitive.unsupported.json`.
- An absent optional field is left out of the JSON, as the spec's receipt examples do. A field
  with a serde default that is not an `Option` (mouse `button`, `buttons`, `click_count`,
  `modifiers`, `delta_x`, `delta_y`, `force`) is always written. The service types must match
  this when switch-2 binds them to the fixtures.
- World messages are the exact messages in `world/<kind>.json`. A request that carries a world
  call must use that fixture message unchanged; the test enforces it. `scroll` shows all three
  optional fields at once so the fixture lists every field.
- `ok_*` recording cases cover each `RecordingFacts` result. The nested `delivery` object in an
  export outcome is the tagged `Delivery` value, `{"delivery": "attached", "tab_id": 12}`.
- Tolerance files are `{description, frame, expect}`. `expect` names one form:
  `decodes_as` (the frame holds everything in the named fixture plus extra fields, and decodes
  to that fixture), `ignored` (an unknown frame kind, event or intent that both sides drop),
  `maps_to` (`other` or `unknown`), or `answered` (the extension replies with the named
  receipt).
- The `error` frame is listed only under service-to-extension, as the spec's frame table has
  it, although the spec's Rust `Inbound` enum also decodes one.

## Notes

- `protocol.js` carries the vocabulary as code and never reads the JSON; the test compares the
  two. It adds `isKnown(list, name)` and upper-case aliases for the scalars the spec's
  pseudo-JS uses (`WORLD_NAME`, `NATIVE_HOST_NAME`, `MAX_UNSEEN`, and so on).
- `lib/protocol.js` is not yet loaded by any worker. `service-worker.js` still runs the
  protocol 3 path until the switch deletes it.
- `lib/sessions.js` is not yet loaded by any worker. `create(debuggerApi, {installer, onRelease,
  onSessionGone, protocolVersion})` requires the installer. A session is
  `{target, targetId, identifiers}` with a live `identifiers` list. The installer runs once per
  attachment for the root and once per child; the root also gets the iframe `Target.setAutoAttach`
  after its installer, which is what makes child sessions arrive. A child that ended before its
  setup finished does not fail the tab.
