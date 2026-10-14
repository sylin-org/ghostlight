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

Lane complete after `service-11`. Next is the switch. `intake::mcp::McpSession` is the
transport-free MCP engine; nothing feeds it yet. switch-1 connects it to relay-3 `mcp` streams,
sends the `HelloAccepted` session key to `bind_session`, hands out `ResumeState::encode` tokens, and
writes `failure_templates()` to `relay-templates.json` (replacing `TEMPLATES_RELAY` with the relay
crate's `RELAY`). It also wires the relay-3 control op `quit` to `ServiceHost::begin_quit(QUIT_GRACE)`
and makes connectors spawn `ghostlight serve`.

## Landed

| Slice | What | Commit |
|---|---|---|
| service-1-p4-wire | `chromium/level4/{mod.rs, wire.rs}`: protocol 4 frames, primitives, arguments, outcomes, `Failure`, `CallFailure`, and tolerant mirrors of the physical vocabulary. 23 inline tests from the spec's JSON examples. | see `git log` |
| service-2-p4-attach | `Adapter::select` reads the first frame as JSON (`protocol` field for level 4, `major` for level 3); `Level::Four`; `level4/connection.rs` (hello rules, reader, `Receipts`); `level4/fake.rs` FakeExtension; `adapter_incompatible` error frame with its sentence in `language/adapter.rs`; `BrowserSummary.protocol` and the workbench row; `set_protocol4_assets`. Level-3 byte goldens in `contract_tests/level3_bytes.rs`, captured before the change. | see `git log` |
| service-3-p4-world-scope | `level4/world.rs` (one typed constructor per world kind with its scope and effect class, `AssetErrorCode` with `is_pre_effect`, envelope decoding, typed answer values), `locator.rs` (`{frame}:{loader}:{local}` codec), `tree.rs` (`FrameTree` from an inventory or from the admitted documents, depth-first, placeholders, owner-box chains), `scope.rs` (`Scope`, `PlannedCall`: admission, guards, settle budget, observation). Ported from `documents.test.js`; all 32 payloads equal `tests/golden/protocol4/world`. | see `git log` |
| service-4-p4-merge-keys-geometry | `level4/{merge,keys,geometry}.rs`: per-document merges, key and click steps, owner offsets, routing reducer, view tolerance and capture budgets. | 6a89f7cf, 65ce2a19 |
| service-5-p4-link-native | `level4/link.rs` (`Link`, `Spec` per primitive, `RecordingIndex`, `Line`; cancel grace, deadline, effect ledger, failure mapping), `level4::run`, `translate/{tabs,records,present}.rs`; `OpenTab.focus_window` (skipped on the level-3 wire) set when the workspace holds no tab; `Dialog.message` rendered in the dialog status sentence; protocol-4 notice tab choice in `BrowserPresentation`. | see `git log` |
| service-6-p4-documents-reading | `translate/{read,observe}.rs` and `translate/mod.rs` (`Reach`, answer decoding, point routing); `run` admits `InDocuments`; `Scope::open` for work sent without a scope; diagnostics reads bound to `hex(sha256(sorted allowed ids))`; recording starts watch the scope's real documents; `world::Observed.readiness`. | see `git log` |
| service-7-p4-interaction | `translate/{pointer,keyboard,capture,script}.rs` and `translate::{settled, send_input, OnDialog}`: click and hover (prepare plus owner-box chain in one sequence, then input at the centre in top-viewport coordinates, settle 250 for a click, a dialog counts as success), scroll, text into a control, text into the focused control (first focused document in tree order, cleared there alone, precondition on the input), key presses (preconditioned, no text under Alt, Control or Meta), viewport, page, target and region captures (one `capture` primitive, best-effort settle, view check), upload (chunked over 768 KiB by the link) and script evaluation (an exception is an unknown effect). `keys::hover`, world answers for scroll, typing, focus and upload. 27 tests in `translate/tests/interaction.rs`. | see `git log` |
| service-8-p4-fill | `translate/forms.rs`: fields grouped by document in first-appearance order; an orphan submit refused before anything is sent; one guarded Sequence of `prepare_fill` per document before any edit; field kinds from the plans (`world::{FieldKind, FillPlan}`); consecutive local fields share one Sequence of `fill_local`; a text field is one `input` behind `focus_text_field` typing `keys::replace_text`; the fill's own budget checked before every field; a Parallel `verify_fill_retained` (limit `FILL_RETAINED_LIMIT_MS` = 2000, settle 25 without a submit); `submit_fill` with settle 250. 11 tests in `translate/tests/forms.rs`. `work/tests/protocol4.rs` runs the executor over a real `RelayBrowserPort` and the FakeExtension (now `pub(crate)` in test builds) for the five section 5 model-visible deltas, asserting status, summary, effect and repeat-safety. | see `git log` |
| service-9-p4-points-drag | `translate/points.rs`: `ActivatePoint` and `ActivatePointModified` (view first and validated before any effect; `scroll_point` on the top document; route; click steps, settle 250), `WheelAt` (ticks of 120, up is negative, settle 120, receipt at the viewport point), `HoverPoint`, `DragPoints` (two `viewport_point` calls in one sequence; a drag path of 12 strides), `DropImageAt` (always routed; `drop_files` at the local position in the deepest document). `translate::route_and_admit` places a point, admits its document as an input subject, and leaves a point that zero or several owners claim unresolved ("point has no current document subject"); a scope that allows every document skips the routing. `Activate` and `Hover` use it instead of refusing a restricted scope. `pointer::drag` (locators, spec 4.6 order). `records::export_to_page` (`attach_file` through `Scope::call`, with the scope guard). `route` returns a `Routed` (document, local point, subject). `untranslated` and the last `CapabilityVersion` fallback are gone. 21 tests in `translate/tests/points.rs`. | see `git log` |
| service-10-mcp-intake | `intake/{mod.rs, mcp.rs}`: `McpSession` (`new(channel)`, `resumed(ResumeState)`, `accept(line) -> Vec<Step>`, `render(ServiceResponse) -> Vec<Value>`, `resume_state(tabs, recording)`), ported from the BASE connector's `main.rs`, `mcp_2025_11_25.rs` and `mcp_2026_07_28.rs`. Batches reply as one array once their catalog requests are answered; tool results go out one by one. Tool results carry the envelope only in `structuredContent`; `tools/list` drops `outputSchema`. `ResumeState` travels as `rs1.` plus base64url JSON. `failure_templates()` for cut-off and expired requests; every sentence and JSON-RPC error message in `language/intake.rs`. `intake::server_profile()` now also feeds the bridge `HelloAccepted`. 26 tests. | see `git log` |
| service-11-serve-and-quit | `ghostlight serve` (`LaunchMode::Serve`, `cli::desktop::serve`, `desktop::Startup`): asks for the lease for 1 s and exits 0 quietly when another service holds it (a contended start no longer logs `process_failed`), never reveals a window, and exits 3 with a diagnostics event if discovery is not published within 30 s (`ServiceHost::expect_discovery_by`, cancelled by `publish_ready`). `service/lifetime.rs`: `Drain`/`InFlight`, `DiscoveryWatch`, `QUIT_GRACE`, `DISCOVERY_WINDOW`. `ServiceHost::begin_quit(grace)` refuses new invocations (`shutting_down`, sentence in `language/intake.rs`; `SessionQueue::admit` returns `Refused::ShuttingDown`), waits for in-flight ones up to the grace, then calls the quit signal the desktop installed (`app.exit(0)`). The tray Quit is unchanged. `serve` is in help but not in `SUBCOMMANDS`, like `native-host`; the shell completions are the packaging owner's to extend. 12 tests. | see `git log` |

## Notes

- A hello that names an unsupported protocol is told the newest protocol of the field it used:
  `major` hellos hear 3, `protocol` hellos hear 4. A published 1.3.x extension therefore still
  reads a major it can act on, and the relay contract suite stays unchanged.
- `Connection` holds a `Channel`: level 3 keeps its pending map and capability table, level 4 its
  `Receipts`. Only level 3 lets a heartbeat timeout fail pending calls.
- The MCP engine accepts requests once `initialize` is answered and announces catalog changes
  only after `notifications/initialized`. A request id reused while its call runs is refused
  with -32600, so a cancel always names one call. An expired request never reached the service,
  so its template reports no effect and a safe repeat; a cut-off call reports an unknown effect
  that is not safe to repeat.
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
- Work sent without a scope reads every addressable document of one `frames` inventory
  (`Scope::open`) and carries no guard. The executor wraps every content command in
  `InDocuments`, so this path costs one extra round trip only outside the executor.
- A scope that allows every document skips point routing, so a point input there names the subject
  the top document reports; over an embedded document that is the embed element. A file drop
  always routes, because it is delivered inside the document. `route_and_admit` lives beside `route`
  in `translate/mod.rs`, not in `geometry.rs`, because `geometry` is pure and routing sends calls.
- A first routing hop in page coordinates asks `hit_point` alone and probes owners afterwards at
  the viewport point it reports, because `owner_probe` reads viewport coordinates. Deeper hops
  ask both in one call.
- A describe inventory is `incomplete` when the extension says its frame snapshot was not
  complete; unseen frames are reported as unsupported `unseen.{n}` documents, which is what makes
  the authority refuse restricted unbounded work over them.
