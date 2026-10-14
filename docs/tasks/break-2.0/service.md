# Break 2.0: service lane ledger

Lane: service. It owns `crates/orchestrator/src/**` (except the page assets under
`chromium/assets/` and `chromium/level4/assets/`) and this ledger. Ground rules are in
`BOOTSTRAP.md` when the connectors lane has landed it. The target is
`docs/adr/0197-contained-seams-and-the-2-0-break.md`; the wire is
`docs/design/adapter-protocol-4.md`.

BASE: `f28bcbd7` (dev when the lane started). Read pre-break sources with
`git show f28bcbd7:<path>`.

Gate: `cargo fmt -p ghostlight --check`, `cargo clippy -p ghostlight --all-targets -- -D
warnings`, `cargo test -p ghostlight`, with `CARGO_TARGET_DIR=.target-assessment`. No `--locked`.
Never stage `Cargo.lock` or a `Cargo.toml`.

## RESUME HERE

Next slice: `service-6`. `call_inner` runs protocol-4 connections through `level4::run` and the
Link. The browser-native rows (2, 4-11, 27-28, 42-43, 45, 47-52) are translated; every other
command, `InDocuments`, `ReadDiagnostics`, `StartRecording` and a recording export into a page
answer `CapabilityVersion { capability, 1, 0 }` until services 6-9 translate them. Nothing sets the
page assets yet (switch-2 wires them), so a running service still refuses protocol 4.

## Landed

| Slice | What | Commit |
|---|---|---|
| service-1-p4-wire | `chromium/level4/{mod.rs, wire.rs}`: protocol 4 frames, primitives, arguments, outcomes, `Failure`, `CallFailure`, and tolerant mirrors of the physical vocabulary. 23 inline tests from the spec's JSON examples. | see `git log` |
| service-2-p4-attach | `Adapter::select` reads the first frame as JSON (`protocol` field for level 4, `major` for level 3); `Level::Four`; `level4/connection.rs` (hello rules, reader, `Receipts`); `level4/fake.rs` FakeExtension; `adapter_incompatible` error frame with its sentence in `language/adapter.rs`; `BrowserSummary.protocol` and the workbench row; `set_protocol4_assets`. Level-3 byte goldens in `contract_tests/level3_bytes.rs`, captured before the change. | see `git log` |
| service-3-p4-world-scope | `level4/world.rs` (one typed constructor per world kind with its scope and effect class, `AssetErrorCode` with `is_pre_effect`, envelope decoding, typed answer values), `locator.rs` (`{frame}:{loader}:{local}` codec), `tree.rs` (`FrameTree` from an inventory or from the admitted documents, depth-first, placeholders, owner-box chains), `scope.rs` (`Scope`, `PlannedCall`: admission, guards, settle budget, observation). Ported from `documents.test.js`; all 32 payloads equal `tests/golden/protocol4/world`. | see `git log` |
| service-4-p4-merge-keys-geometry | `level4/{merge,keys,geometry}.rs`: per-document merges, key and click steps, owner offsets, routing reducer, view tolerance and capture budgets. | 6a89f7cf, 65ce2a19 |
| service-5-p4-link-native | `level4/link.rs` (`Link`, `Spec` per primitive, `RecordingIndex`, `Line`; cancel grace, deadline, effect ledger, failure mapping), `level4::run`, `translate/{tabs,records,present}.rs`; `OpenTab.focus_window` (skipped on the level-3 wire) set when the workspace holds no tab; `Dialog.message` rendered in the dialog status sentence; protocol-4 notice tab choice in `BrowserPresentation`. | see `git log` |

## Notes

- A hello that names an unsupported protocol is told the newest protocol of the field it used:
  `major` hellos hear 3, `protocol` hellos hear 4. A published 1.3.x extension therefore still
  reads a major it can act on, and the relay contract suite stays unchanged.
- `Connection` holds a `Channel`: level 3 keeps its pending map and capability table, level 4 its
  `Receipts`. Only level 3 lets a heartbeat timeout fail pending calls.
- The workspace MSRV rose to 1.89 in the connectors lane, which turned on clippy's
  `manual_is_multiple_of`; `language/outcome.rs` was fixed in its own commit.

- Every module on the path from `lib.rs` is `pub`, so the new items are not dead code before
  they are wired.
- Mirrors of types the extension sends (`DiagnosticEntry`, `RecordingSummary`, `Delivery`) convert
  into the physical types with `TryFrom`, because an `Unknown` arm has no physical counterpart.
  Mirrors of types only the service sends convert with `From`.
- The wire `RuntimeControlState` has no attention state. The physical `Attention` maps to
  `Held` until switch-11 removes it.
- Optional arguments follow the spec's Rust sketch literally: only the fields the spec marks
  `skip_serializing_if` are left out when absent; the others serialize as `null`. switch-2 binds
  both sides to the shared fixtures and reconciles any difference.
- `FrameTree` puts unseen placeholders (`unseen.{n}`) under the top, after its real children. A
  frame is supported when it is reachable and its URL is http or https, as the level-3 inventory
  decided.
- World coordinates are written as whole numbers when they are whole, so a built message equals
  the golden fixture value for value. `world::Rect` is `{left, top, width, height}` (the asset's
  `getBoundingClientRect` shape); `wire::Rect` is the capture clip.
- A recording a workspace names must be one it started on the current connection
  (`RecordingIndex`). A named id the index does not hold answers `RecordingNotFound` with nothing
  sent, because the extension no longer knows workspaces. `StartRecording` (service-6+) learns ids
  through `translate::records::recorded`.
- The Link checks cancellation and the deadline, takes the writer, checks them again, and only
  then asks runtime control to admit an effectful primitive, so admission happens at the
  transmission boundary as it does at level 3.
- A protocol-4 notice for a workspace with several tabs lists all tabs once (`BrowserCommand::ListTabs`
  carries no ids) and picks the active one among the workspace's.
- `Scope::doc_of` decodes a locator and does not check the tree: `Scope::call` refuses a document
  that is not admitted, so a replaced document fails there, before anything is sent.
