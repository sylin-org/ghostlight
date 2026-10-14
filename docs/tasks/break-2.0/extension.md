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

Next slice: `extension-3-p4-asset-content`. The shell is in place: `level4/assets/` holds
`world.js`, `page-shared.js`, `sensor.js`, `presentation-css.js`, `presentation.js` and `content.js`.
Any change to them changes the bundle hash, so update `tests/golden/protocol4/assets.sha256`
deliberately in the same commit (print it with `bundle.sha256(bundle.expand())` from
`level4/assets/tests/bundle.js`).

Left for extension-3 in `content.js`: coded errors at the throw sites (only `unknown_kind` is
coded now), locator epochs, cancel checks inside `observe` and `prepareStableFill`, and the new
kinds. `fillValuesRetained` and `prepareBrowserText` are unused until `verify_fill_retained` and
`focus_text_field` land. The old `clear` kind is gone because the vocabulary has no such kind.

## Landed

| Slice | What | Commit |
|---|---|---|
| extension-2-p4-asset-shell | `level4/assets/` shell (world, page-shared, sensor, presentation-css, presentation, content), `tests/bundle.js`, `tests/fake-dom.js`, `tests/world.test.js` (18 tests), `tests/golden/protocol4/assets.sha256`. | see `git log` |
| extension-1-p4-contracts | `tests/golden/protocol4/` (vocabulary, 10 frames, 6 events, 40 requests, 80 receipts, 32 world messages, 12 tolerance cases), `extension/lib/protocol.js`, `extension/tests/golden-protocol4.test.js`. | see `git log` |

## Counts

- 17 primitives, 13 receipt failure codes, 10 call failure codes, 32 world message kinds,
  22 asset error codes (19 pre-effect).
- 22 tests in `golden-protocol4.test.js`.
- 18 tests in `level4/assets/tests/world.test.js`.

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
