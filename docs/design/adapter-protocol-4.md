# Ghostlight adapter protocol 4: consolidated implementation spec

> **Owner amendment (2026-10-13, ADR-0197 Decisions 5 and 24): the break is strict.** The 2.0
> service does not keep the protocol-3 path. Protocol-4 pieces are built beside today's path so
> every commit leaves the installed graph working, and the switch commit deletes the old path on
> both sides: the service's level 3 (its MAIN-world bundle, capability table, InstallPageRuntime,
> Acknowledge, DescribeDocuments and InDocuments) as well as the extension's legacy files. Where
> this document says level 3 stays "for one release", read "until the switch commit".


Status: implementation spec for the owner. Written read-only against dev at `95a7e646`.

Sources, in order of authority:
1. The owner decisions given with this task (numbered D1 to D7 below).
2. `arch/SYNTHESIS.md` section 2 (S3, S4, S5, S6) and section 8 (M4c).
3. `arch/CRITIQUE.md` corrections.
4. `design/FINAL.md`, used as the base wherever 1 to 3 do not override it.
5. The current code: `extension/`, `crates/orchestrator/src/browser/adapter/`, `crates/orchestrator/src/browser/`,
   `crates/orchestrator/src/browser/adapter/chromium/assets/`, `crates/orchestrator/src/work/`.

Line references such as `service-worker.js:1472` point at dev `95a7e646`.

---

## 0. What this spec settles

### 0.1 Owner decisions applied

- **D1. Protocol-4-only extension.** The new extension speaks only protocol 4. It has no legacy dispatch.
  The service keeps today's protocol-3 path ("level 3") unchanged for one release, so published 1.3.x
  extensions keep working. Level 3 is deleted later, as its own decision.
- **D2. Assets arrive in `hello_accepted`.** The frame is `{service_epoch, assets{id, source}, control_state}`.
  There is no `InstallAssets` primitive and no not-ready phase.
  - The extension installs the assets into the isolated world `ghostlight` on every session, the root and
    every out-of-process child.
  - It removes superseded registrations.
  - It calls through `objectId` handles of the world's global, never through numeric context ids.
- **D3. Deleted at protocol 4:**
  - the capability table and its revisions;
  - `Acknowledge` and the extension's operation ledger;
  - the per-request probe and quarantine;
  - the `workspace` field on requests;
  - `DescribeDocuments` and `InDocuments` as wire commands. Document scope is enforced in the service,
    with a `DocumentGuard` riding on primitives.
  - correlated presentation;
  - events for tabs Ghostlight does not own or address.
- **D4. The primitive set** is the one in section 1.5. An unknown primitive is answered
  `{failure:"unsupported", dispatched:false}`. New primitives get new names, and existing ones never
  change meaning.
- **D5. Receipts carry facts only:** `{dispatched, committed_urls, tab?}`. Failure codes are closed. The
  service classifies effects.
- **D6. Evolution rule.** Both sides ignore unknown fields, frame kinds and events. Nothing is removed or
  renamed within protocol 4. Golden fixtures in `tests/golden/protocol4/` are shared by `cargo test` and
  `npm test`.
- **D7. The browser connector is a frozen relay.** It holds the first frame and pumps both ways without
  parsing. When no service is there it writes `backend_unavailable` with `service: "absent"` and never spawns
  while waiting: after the service is lost, or when none has accepted five seconds after the connector's own
  start. It exits when a service accepts, so the extension reconnects with a fresh hello. The extension reads
  an absent service as not connected and stops waiting for a hello answer; a bare `backend_unavailable`, which
  a 1.x connector writes, does not end that wait.

### 0.2 Where this spec departs from FINAL.md

| FINAL.md | This spec | Why |
|---|---|---|
| Dual-level extension, hello `major:3, levels:[3,4]` | Protocol-4-only extension, hello `protocol:4` | D1 |
| `InstallAssets` primitive, `ready` only on echo | Assets inside `hello_accepted`; connection ready at once | D2 |
| Protocol-4 types in `crates/bridge`, many `deny_unknown_fields` | Types in the orchestrator's browser module, all tolerant | Bridge dissolves; D6 |
| Tagged `PrimitiveResult` and `EffectFacts` object | Flat receipt: `ok` or `failure`, plus `dispatched`, `committed_urls`, `tab` | D5 |
| `Notice` and `Cancel` as correlated primitives | One-way frames `notice` and `cancel` | D3 (no correlated presentation), S4 |
| `Acknowledge`, ledger and probe kept | Deleted at level 4 | D3 |
| `form_diagnostics` local fact and `drain()` | Removed; the form-reset trace is deleted | OD14, task |
| `OpenTab{url, group_title, ...}` | `open_tab{url?, group_key, group_title, adopt_tab_id?, focus_window, ready_timeout_ms}` | D4, CRITIQUE (group identity) |
| Recording ops with optional ids, ambiguity outcomes | Ids required on the wire; a workspace holds one recording and the service supplies its id | OD13, D3 (no workspace field), ADR-0197 D21 |
| No child-tab story | `list_tabs` reports `opener_tab_id`; adoption only by explicit `adopt_tab_id` | D4, CRITIQUE (OD18 contradiction) |
| Dialog inspect returns type only | Status returns a bounded message | D4, LU-5 |
| No accessibility primitive | `accessibility_snapshot` | D4, OD18 |
| `evaluate_main` with no deadline enforcement | Deadline enforced with `Runtime.terminateExecution` | D4, EXT-9 |

### 0.3 Kept from FINAL.md

- `objectId` handles of the world global.
- Unseen out-of-process frames reported by the inventory.
- The recording watch carries the document set.
- The focus precondition runs right before key input.
- The `clear_focused` frame is chosen first.
- Capture hiding inside the extension's `finally`.
- Exact embeds through `DOM.getFrameOwner`.
- Race mode.
- Event-driven drag interception.
- Coded asset errors, locator epochs and cancel tokens.
- Service-only effect classification.

---

## 1. Wire

All frames use today's framing: 4-byte little-endian length, then UTF-8 JSON, carried by native messaging and
the relay. A service-to-extension frame must stay under 768 KiB (Chromium caps host-to-extension messages at
1 MiB). Larger requests use `command_chunk`, unchanged from level 3. Every frame is an object with a `kind`.

### 1.1 Frame kinds

| Direction | `kind` | Fields | Notes |
|---|---|---|---|
| ext to svc | `hello` | `protocol:4, browser_id, browser_name?, platform, engine_version?, adapter_version, attended` | Always the first frame on a port |
| svc to ext | `hello_accepted` | `service_epoch, service_version?, control_state, assets{id, source}` | `service_version` is display-only (popup) |
| svc to ext | `request` | `correlation, primitive{name, args}` | One receipt answers it |
| svc to ext | `command_chunk` | as level 3 | Reassembled by `chunks.js` into a `request` |
| ext to svc | `receipt` | `correlation, ok? or failure?, message?, dispatched, committed_urls, tab?` | Section 1.3 |
| svc to ext | `cancel` | `correlation` | One-way. The interrupted primitive answers with `failure:"cancelled"` |
| svc to ext | `notice` | `tab_id, frame?, signal, defer_until_visible` | One-way presentation |
| svc to ext | `control_state` | `state` | Runtime control |
| svc to ext | `heartbeat` | `sequence` | Periodic liveness only (20 s), never per request |
| ext to svc | `heartbeat_ack` | `sequence` | |
| ext to svc | `event` | `event{event, ...}` | Only for controlled tabs (section 2.2) |
| svc to ext | `error` | `code, message` | Uncorrelated, for example `adapter_incompatible` |
| relay to ext | `backend_unavailable` | `service?` | Written by the connector, not the service. `service: "absent"` says no service is there at all |

Events (extension to service):

| `event` | Fields |
|---|---|
| `document_committed` | `tab_id, url` (top frame only) |
| `readiness_changed` | `tab_id, readiness` |
| `dialog_changed` | `tab_id, present, dialog_type` |
| `tab_closed` | `tab_id` |
| `attended` | none (browser-level) |
| `runtime_control_requested` | `intent` |

Removed at protocol 4:
- the `correlation` field on `document_committed` (commits now ride in receipts);
- `child_tab_opened` (replaced by `opener_tab_id` in `list_tabs`);
- `diagnostics_toggle_requested` (OD14: `hello_accepted` carries no diagnostics state, so the popup row stays
  hidden).

### 1.2 Handshake

```
extension                        relay                       service
connectNative  ---------------->  hold first frame
hello{protocol:4,...} ---------->  open relay stream  ----->  read first frame as serde_json::Value
                                                             protocol == 4: level-4 attach
                                                             major == 3:    level-3 attach (today's path)
                                                             else: error{adapter_incompatible}, close
                <---------------------------------------  hello_accepted{service_epoch, assets, control_state}
connection is ready at once; no install round trip
```

**Service side (`browser/adapter/mod.rs::attach`).**
1. Read the first frame as a `Value`.
2. If `protocol` is 4, decode it as `level4::wire::Hello`.
   - Validate it with today's rules: the `browser_` id grammar (`browser/mod.rs:475-483`) and the browser
     name bound.
   - `engine_version` is informational. The manifest's `minimum_chrome_version: 125` enforces the floor.
3. If `major` is 3, decode the same `Value` as `crate::browser::adapter::BrowserFrame` and continue in
   today's `attach_reader` body unchanged.
4. Anything else gets `{kind:"error", code:"adapter_incompatible", message}` and the socket closes. Today the
   socket closes silently.
5. At level 4, after validation:
   - write `hello_accepted`, remembering the control state it carried;
   - register the connection as `ready` and publish `adapter_attached`;
   - read the control state again and, if it changed since `hello_accepted`, write `control_state` with
     the new state while holding the state lock;
   - start its reader thread (`read_level4`) and its heartbeat thread.

   `hello_accepted` goes first so no request or `control_state` frame can precede it. A hold, resume or
   end published between that write and the registration misses the new connection, so the second read
   settles it. A publish stores the state before it snapshots the registered connections, so whichever
   side runs second sees the other.

   There is no `InstallPageRuntime` and no capability gate.

**Extension side.**
- The hello is sent first on every port. This is the R1 reland: `attended` and `engine_version` are computed
  before `connectNative`, so no other frame can precede the hello (`7dd12972`, `07b544a5`).
- On `hello_accepted` the extension:
  1. checks `sha256(assets.source) == assets.id`;
  2. stores the assets and re-registers them on every live session (section 2.3);
  3. if `service_epoch` changed, settles the service boundary state: interrupts recordings, clears
     diagnostics, cancels in-flight primitives, clears chunk transfers;
  4. applies `control_state`.
- A second `hello_accepted` on the same port is legal and idempotent. Today's connector replays the cached
  hello to a restarted service, so the extension must accept a new epoch and new assets on an open port.
  Under D7 the port recycles instead. Both connectors therefore work.
- With no `hello_accepted` within 10 s of the hello, the extension shows "Update Ghostlight on this computer."
  This is the case of a 1.3.x service, which closes the socket on a protocol-4 hello.

`hello` example:

```json
{"kind":"hello","protocol":4,"browser_id":"browser_3f2a9c","browser_name":"Chrome",
 "platform":"ghostlight/chromium","engine_version":"131.0.6778.86","adapter_version":"2.0.0","attended":true}
```

`hello_accepted` example (fixture source shortened):

```json
{"kind":"hello_accepted","service_epoch":"service_7b1e","service_version":"2.0.0","control_state":"active",
 "assets":{"id":"<lowercase hex sha256 of source>","source":"\"use strict\";\nconst ID = __ghostlightAssetId; ..."}}
```

### 1.3 Requests and receipts

```json
{"kind":"request","correlation":"p4_5d0c...","primitive":{"name":"navigate",
  "args":{"tab_id":12,"to":{"to":"url","url":"https://example.com/"},"discard_beforeunload":false,"ready_timeout_ms":8000}}}

{"kind":"receipt","correlation":"p4_5d0c...","ok":{"tab":{"tab_id":12,"title":"Example","url":"https://example.com/",
  "active":true,"readiness":"complete"}},"dispatched":true,"committed_urls":["https://example.com/"],
  "tab":{"tab_id":12,"title":"Example","url":"https://example.com/","active":true,"readiness":"complete"}}

{"kind":"receipt","correlation":"p4_77aa...","failure":"tab_missing","message":"No tab with id: 99.",
  "dispatched":false,"committed_urls":[]}

{"kind":"receipt","correlation":"p4_9e01...","failure":"unsupported","dispatched":false,"committed_urls":[]}
```

Rules:
- Every `request` gets exactly one `receipt`. Correlations are minted per primitive and never resent, so no
  ledger or `Acknowledge` is needed.
- `ok` is the primitive's outcome object (section 1.5). `failure` is a closed code (section 1.6). A receipt
  has exactly one of the two. A receipt with both or neither is `BrowserError::Protocol`.
- `dispatched` is true when the extension issued anything that can change browser or page state for this
  primitive:
  - a tab or window mutation;
  - any input event;
  - a world call that started;
  - an `evaluate_main` send;
  - a dialog `handle`;
  - a recording export into a page.
- `committed_urls` lists the top-frame commits seen on the primitive's tab, from just before dispatch to the
  end of `settle_ms`.
- `tab` is the tab read after the primitive, when the primitive names a tab and `dispatched` is true.
- A rejected chunk transfer is answered with a receipt for its correlation: `failure:"invalid_request"`,
  `dispatched:false`.

### 1.4 Rust types

Location: `crates/orchestrator/src/browser/adapter/chromium/level4/wire.rs`. If ADR-0196 step 2 (M2) has not
created `adapter/`, commit C2 creates the module shells. Level-3 types stay where they are until M4a moves
them.

No type here uses `deny_unknown_fields`. Each tolerant mirror converts into the `BrowserOutcome` types with
`From` impls. The browser connector never sees these types (D7).

```rust
//! Adapter protocol 4: the frozen browser primitive vocabulary. Mechanism only; no product semantics.
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROTOCOL: u16 = 4;
pub const WORLD_NAME: &str = "ghostlight";
pub const DIRECT_FRAME_BYTES: usize = 768 * 1024;
pub const MAX_WORLD_CALLS: usize = 256;          // = DOCUMENT_LIMIT, so a route never splits
pub const MAX_INPUT_STEPS: usize = 4096;
pub const MAX_UNSEEN: usize = 64;

// ---- frames, extension to service ----
#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Inbound {
    Hello(Hello),
    Receipt(Receipt),
    Event { event: Event },
    HeartbeatAck { sequence: u32 },
    Error { #[serde(default)] code: String, #[serde(default)] message: String },
    #[serde(other)] Unknown,
}
#[derive(Deserialize, Serialize)]
pub struct Hello {
    pub protocol: u16,
    pub browser_id: String,
    #[serde(default)] pub browser_name: Option<String>,
    #[serde(default)] pub platform: Option<String>,        // "ghostlight/chromium"
    #[serde(default)] pub engine_version: Option<String>,  // [0-9.]{1,32}, informational
    pub adapter_version: String,
    #[serde(default)] pub attended: bool,
}
#[derive(Deserialize, Serialize)]
pub struct Receipt {
    pub correlation: String,
    #[serde(default)] pub ok: Option<Value>,              // decoded as P::Outcome by the Link
    #[serde(default)] pub failure: Option<Failure>,
    #[serde(default)] pub message: Option<String>,
    #[serde(default)] pub dispatched: bool,
    #[serde(default)] pub committed_urls: Vec<String>,
    #[serde(default)] pub tab: Option<Tab>,
}
#[derive(Deserialize, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    DocumentCommitted { tab_id: u64, url: String },
    ReadinessChanged { tab_id: u64, readiness: Readiness },
    DialogChanged { tab_id: u64, present: bool, #[serde(default)] dialog_type: String },
    TabClosed { tab_id: u64 },
    Attended,
    RuntimeControlRequested { intent: String },          // parsed into RuntimeControlIntent; unknown ignored
    #[serde(other)] Unknown,
}

// ---- frames, service to extension ----
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Outbound {
    HelloAccepted { service_epoch: String, service_version: String,
                    control_state: RuntimeControlState, assets: Assets },
    Request { correlation: String, primitive: Primitive },
    CommandChunk { transfer_id: String, correlation: String, index: u16, count: u16,
                   total_bytes: u32, sha256: String, data: String },
    Cancel { correlation: String },
    Notice(Notice),
    ControlState { state: RuntimeControlState },
    Heartbeat { sequence: u32 },
    Error { code: String, message: String },
}
#[derive(Serialize, Deserialize)]
pub struct Assets { pub id: String, pub source: String }   // id = lowercase hex sha256(source)

#[derive(Serialize, Deserialize)]
#[serde(tag = "name", content = "args", rename_all = "snake_case")]
pub enum Primitive {
    ListTabs(ListTabs), OpenTab(OpenTab), Navigate(Navigate), FocusTab(TabRef), CloseTab(CloseTab),
    SetZoom(SetZoom), ResizeWindow(ResizeWindow), Frames(TabRef), Call(Call), EvaluateMain(EvaluateMain),
    View(TabRef), Input(Input), Capture(Capture), Dialog(Dialog), Diagnostics(DiagnosticsOp),
    Recording(RecordingOp), AccessibilitySnapshot(AccessibilitySnapshot),
}

// ---- document identity ----
#[derive(Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub struct FrameRef { pub frame_id: String, pub loader_id: String }   // one exact document
#[derive(Clone, Serialize, Deserialize)]
pub struct GuardDoc { pub frame_id: String, pub loader_id: String, pub url: String }
/// Mechanical currency check run by the extension right before it acts.
/// exact: the current inventory equals `documents` (same set, same urls), is complete, and has
///        exactly `unseen` unseen frames. Otherwise each listed document is current with the same origin.
#[derive(Clone, Serialize, Deserialize)]
pub struct DocumentGuard { pub documents: Vec<GuardDoc>, pub exact: bool, #[serde(default)] pub unseen: u16 }

// ---- arguments ----
#[derive(Serialize, Deserialize)] pub struct TabRef { pub tab_id: u64 }
#[derive(Serialize, Deserialize)]
pub struct ListTabs { #[serde(default, skip_serializing_if = "Option::is_none")] pub tab_ids: Option<Vec<u64>> }
#[derive(Serialize, Deserialize)]
pub struct OpenTab {
    #[serde(default, skip_serializing_if = "Option::is_none")] pub url: Option<String>, // required unless adopting
    pub group_key: String, pub group_title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub adopt_tab_id: Option<u64>,
    pub focus_window: bool, pub ready_timeout_ms: u64,
}
#[derive(Serialize, Deserialize)]
pub struct Navigate { pub tab_id: u64, pub to: NavigationTarget,
                      #[serde(default)] pub discard_beforeunload: bool, pub ready_timeout_ms: u64 }
#[derive(Serialize, Deserialize)]
#[serde(tag = "to", rename_all = "snake_case")]
pub enum NavigationTarget { Url { url: String }, Back, Forward, Reload { #[serde(default)] bypass_cache: bool } }
#[derive(Serialize, Deserialize)] pub struct CloseTab { pub tab_id: u64, #[serde(default)] pub released: bool }
#[derive(Serialize, Deserialize)] pub struct SetZoom { pub tab_id: u64, pub zoom: f64 }
#[derive(Serialize, Deserialize)] pub struct ResizeWindow { pub tab_id: u64, pub width: u32, pub height: u32 }

#[derive(Serialize, Deserialize)]
pub struct WorldCall {
    /// Absent: the tab's current top document, without a loader check (unscoped top reads only).
    #[serde(default, skip_serializing_if = "Option::is_none")] pub frame: Option<FrameRef>,
    /// The asset receives the owner element of this child frame (inside `frame`) as `this`.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub owner_of: Option<String>,
    /// Opaque to the extension: `{kind, ...}` built only by world.rs.
    pub message: Value,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallMode {
    Sequence,   // in order; after the first answer that is not ok:true, the rest are skipped
    Parallel,   // concurrently; complete when all answer
    Race,       // concurrently; complete at the first ok:true; cancel the rest by token; still-pending ones are skipped
}
#[derive(Serialize, Deserialize)]
pub struct Call { pub tab_id: u64, pub mode: CallMode, pub calls: Vec<WorldCall>,
                  #[serde(default)] pub guard: Option<DocumentGuard>,
                  #[serde(default)] pub settle_ms: u64, pub timeout_ms: u64 }
#[derive(Serialize, Deserialize)]
pub struct EvaluateMain { pub tab_id: u64, pub script: String, pub max_result_chars: usize,
                          #[serde(default)] pub guard: Option<DocumentGuard>,
                          #[serde(default)] pub settle_ms: u64, pub timeout_ms: u64 }
#[derive(Serialize, Deserialize)]
pub struct Input { pub tab_id: u64, #[serde(default)] pub precondition: Option<WorldCall>,
                   pub steps: Vec<InputStep>, #[serde(default)] pub guard: Option<DocumentGuard>,
                   #[serde(default)] pub settle_ms: u64, pub timeout_ms: u64 }
#[derive(Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum InputStep {
    Mouse { action: MouseAction, x: f64, y: f64, #[serde(default)] button: MouseButton,
            #[serde(default)] buttons: u8, #[serde(default)] click_count: u8, #[serde(default)] modifiers: u8,
            #[serde(default)] delta_x: f64, #[serde(default)] delta_y: f64, #[serde(default)] force: f64 },
    Key { action: KeyAction, key: String, #[serde(default)] code: Option<String>,
          #[serde(default)] key_code: Option<u16>, #[serde(default)] text: Option<String>,
          #[serde(default)] unmodified_text: Option<String>, #[serde(default)] modifiers: u8 },
    InsertText { text: String },
    /// Press at path[0], move through path, release at the last point; an intercepted HTML5 drag
    /// is replayed as dragEnter, dragOver, drop at the last point.
    Drag { path: Vec<Point>, #[serde(default)] modifiers: u8 },
    Pause { ms: u16 },                                    // <= 1000
}
#[derive(Serialize, Deserialize)] #[serde(rename_all = "snake_case")] pub enum MouseAction { Moved, Pressed, Released, Wheel }
#[derive(Default, Serialize, Deserialize)] #[serde(rename_all = "snake_case")]
pub enum MouseButton { #[default] None, Left, Middle, Right }
#[derive(Serialize, Deserialize)] #[serde(rename_all = "snake_case")] pub enum KeyAction { Down, Up, RawDown, Char }
#[derive(Clone, Copy, Serialize, Deserialize)] pub struct Point { pub x: f64, pub y: f64 }

#[derive(Serialize, Deserialize)]
#[serde(tag = "clip", rename_all = "snake_case")]
pub enum CaptureClip {
    Viewport, FullPage,
    /// Top-viewport CSS px; the extension adds cssVisualViewport.pageX/Y from the same metrics read and clamps at 0.
    ViewportRect { left: f64, top: f64, width: f64, height: f64 },
    PageRect { x: f64, y: f64, width: f64, height: f64 },
}
#[derive(Serialize, Deserialize)]
pub struct CaptureBudget { pub max_side: u32, pub max_pixels: u32, pub min_scale: f64, pub magnify: bool,
                           pub qualities: Vec<u8>, pub max_base64_chars: usize }
#[derive(Serialize, Deserialize)] pub struct Capture { pub tab_id: u64, pub clip: CaptureClip, pub budget: CaptureBudget }
#[derive(Serialize, Deserialize)] pub struct Dialog { pub tab_id: u64, pub op: DialogOp }
#[derive(Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum DialogOp { Status, Handle { accept: bool, #[serde(default)] text: Option<String> } }
#[derive(Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum DiagnosticsOp {
    Read { tab_id: u64, source: DiagnosticSource, detail: DiagnosticDetail,
           #[serde(default)] match_text: Option<String>, #[serde(default)] after: Option<String>, limit: u16,
           /// Opaque key of the admitted document set; a different key resets the tab's ring.
           #[serde(default)] binding: Option<String>, #[serde(default)] guard: Option<DocumentGuard> },
    Clear { tab_ids: Vec<u64> },
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum RecordingOp {
    /// watch: stop with document_boundary when the CDP inventory no longer equals this set.
    Start { tab_id: u64, #[serde(default)] watch: Option<Vec<GuardDoc>> },
    Status { recording_id: String }, Stop { recording_id: String },
    Export { recording_id: String, destination: RecordingTarget, max_output_bytes: usize },
    Discard { recording_id: String },
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "destination", rename_all = "snake_case")]
pub enum RecordingTarget {
    Download { file_name: String }, Client,
    /// The extension runs `call` with the encoded GIF as the call's attachment.
    Page { tab_id: u64, call: WorldCall, #[serde(default)] guard: Option<DocumentGuard> },
}
#[derive(Serialize, Deserialize)]
pub struct AccessibilitySnapshot { pub tab_id: u64, pub frame: FrameRef, pub max_nodes: u32,
                                   #[serde(default)] pub max_depth: Option<u32>,
                                   #[serde(default)] pub query: Option<AxQuery>,
                                   #[serde(default)] pub guard: Option<DocumentGuard> }
#[derive(Serialize, Deserialize)]
pub struct AxQuery { #[serde(default)] pub role: Option<String>, #[serde(default)] pub name: Option<String> }

#[derive(Serialize, Deserialize)]
pub struct Notice { pub tab_id: u64, #[serde(default)] pub frame: Option<FrameRef>,
                    pub signal: NoticeSignal, pub defer_until_visible: bool }
/// Same fields as bridge PresentationSignal minus tab_id; `locator` is frame-local.
#[derive(Serialize, Deserialize)]
pub struct NoticeSignal { pub invocation: String, pub signal: PresentationKind, pub activity: PresentationActivity,
                          pub phase: String, #[serde(default)] pub label: Option<String>, #[serde(default)] pub detail: Option<String>,
                          #[serde(default)] pub locator: Option<String>, #[serde(default)] pub click: Option<ClickShape> }

// ---- outcomes (the `ok` object of each primitive) ----
#[derive(Clone, Serialize, Deserialize)]
pub struct Tab { pub tab_id: u64, #[serde(default)] pub title: String, #[serde(default)] pub url: String,
                 #[serde(default)] pub active: bool, pub readiness: Readiness,
                 #[serde(default)] pub opener_tab_id: Option<u64> }
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Readiness { Loading, Complete, #[serde(other)] Unknown }
#[derive(Deserialize)] pub struct Tabs { pub tabs: Vec<Tab> }
#[derive(Deserialize)] pub struct TabOpened { pub tab: Tab, #[serde(default)] pub adopted: bool }
#[derive(Deserialize)] pub struct Navigated { pub tab: Tab }
#[derive(Deserialize)] pub struct TabFocused { pub tab_id: u64, pub active: bool, pub window_focused: bool }
#[derive(Deserialize)] pub struct TabClosed { pub tab_id: u64 }
#[derive(Deserialize)] pub struct Zoomed { pub tab_id: u64, pub zoom: f64 }
#[derive(Deserialize)] pub struct WindowResized { pub tab_id: u64, pub width: u32, pub height: u32, pub affected_tab_ids: Vec<u64> }
#[derive(Deserialize)]
pub struct FrameFact { pub frame_id: String, pub loader_id: String,
                       #[serde(default)] pub parent_frame_id: Option<String>, pub url: String,
                       #[serde(default)] pub unreachable: bool }
#[derive(Deserialize)] pub struct FrameInventory { pub frames: Vec<FrameFact>, pub complete: bool,
                                                   #[serde(default)] pub unseen: Vec<String> }
#[derive(Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CallAnswer {
    /// The world answered `{ok:true,value}` or `{ok:false,error:{code,message}}`, unparsed by the extension.
    Returned { envelope: Value },
    Failed { code: CallFailure, #[serde(default)] message: String, started: bool },
    Skipped,
    #[serde(other)] Unknown,                              // treated as Failed{other, started:true}
}
#[derive(Deserialize)] pub struct Called { pub answers: Vec<CallAnswer> }
#[derive(Deserialize)]
pub struct Evaluated { #[serde(default)] pub value: String, #[serde(default)] pub truncated: bool,
                       /// The page threw; the bounded exception description.
                       #[serde(default)] pub exception: Option<String> }
#[derive(Clone, Copy, Deserialize, Serialize)]
pub struct ViewFacts { pub visual_page_x: f64, pub visual_page_y: f64, pub visual_css_width: f64,
                       pub visual_css_height: f64, pub content_css_width: f64, pub content_css_height: f64,
                       pub device_scale: f64, pub zoom: f64 }
#[derive(Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum InputStop { Dialog { #[serde(default)] dialog_type: String }, Cancelled, #[serde(other)] Other }
#[derive(Deserialize)]
pub struct InputDone { #[serde(default)] pub precondition: Option<CallAnswer>, pub completed_steps: u32,
                       #[serde(default)] pub stopped: Option<InputStop>, #[serde(default)] pub native_drag: bool }
#[derive(Deserialize)]
pub struct Captured { pub mime_type: String, pub data: String, pub width: u32, pub height: u32,
                      pub output_scale: f64, pub clip: Rect, pub view: ViewFacts }
#[derive(Clone, Copy, Deserialize, Serialize)] pub struct Rect { pub x: f64, pub y: f64, pub width: f64, pub height: f64 }
#[derive(Deserialize)]
pub struct DialogFacts { pub present: bool, #[serde(default)] pub dialog_type: String,
                         #[serde(default)] pub message: Option<String>, // bounded 500, status only
                         #[serde(default)] pub url: Option<String>,     // the frame that raised it, bounded to URL_CHARS
                         #[serde(default)] pub handled: bool, #[serde(default)] pub accepted: bool }
#[derive(Deserialize)]
pub struct DiagnosticsFacts { #[serde(default)] pub entries: Vec<DiagnosticEntry>, #[serde(default)] pub cursor: Option<String>,
                              #[serde(default)] pub truncated: bool, #[serde(default)] pub evicted: bool,
                              #[serde(default)] pub capture_started: bool, #[serde(default)] pub omitted_count: usize,
                              #[serde(default)] pub cleared_count: usize }
#[derive(Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum RecordingFacts {
    Started { summary: RecordingSummary, existing: bool }, Status { summary: RecordingSummary },
    Stopped { summary: RecordingSummary, changed: bool },
    Exported { summary: RecordingSummary, encoded: Encoded, delivery: Delivery },
    ExportFailed { reason: String }, Discarded { recording_id: String, released_bytes: usize }, NotFound,
}
#[derive(Deserialize)] pub struct AxSnapshot { pub nodes: Vec<AxNode>, pub truncated: bool }
#[derive(Deserialize)]
pub struct AxNode { pub node_id: String, #[serde(default)] pub parent_id: Option<String>, pub role: String,
                    #[serde(default)] pub name: String, #[serde(default)] pub value: Option<String>,
                    #[serde(default)] pub properties: Vec<(String, String)> }
// DiagnosticEntry, RecordingSummary, Encoded, Delivery, PresentationKind, PresentationActivity, ClickShape,
// RuntimeControlState, DiagnosticSource, DiagnosticDetail: field-for-field tolerant mirrors of the
// bridge types, with From impls into crate::browser::adapter types.
```

### 1.5 Primitive catalogue

Class decides how the service treats a failure after dispatch (section 5). "Addresses" means the extension's
prelude marks the tab controlled and retains its debugger (section 2.2).

| `name` | Arguments | `ok` outcome | Class | Addresses | Extension file |
|---|---|---|---|---|---|
| `list_tabs` | `tab_ids?` | `Tabs` | Read | no | tabs.js |
| `open_tab` | `url?, group_key, group_title, adopt_tab_id?, focus_window, ready_timeout_ms` | `TabOpened` | Effect | the opened tab | tabs.js |
| `navigate` | `tab_id, to, discard_beforeunload, ready_timeout_ms` | `Navigated` | Effect | yes | tabs.js |
| `focus_tab` | `tab_id` | `TabFocused` | Effect | yes | tabs.js |
| `close_tab` | `tab_id, released` | `TabClosed` | Effect | no | tabs.js |
| `set_zoom` | `tab_id, zoom` | `Zoomed` | Effect | yes | tabs.js |
| `resize_window` | `tab_id, width, height` | `WindowResized` | Effect | yes | tabs.js |
| `frames` | `tab_id` | `FrameInventory` | Read | yes | inventory.js |
| `call` | `tab_id, mode, calls, guard?, settle_ms, timeout_ms` | `Called` | per call (world.rs) | yes | world.js |
| `evaluate_main` | `tab_id, script, max_result_chars, guard?, settle_ms, timeout_ms` | `Evaluated` | Effect | yes | primitives.js |
| `view` | `tab_id` | `ViewFacts` | Read | yes | capture.js |
| `input` | `tab_id, precondition?, steps, guard?, settle_ms, timeout_ms` | `InputDone` | Effect | yes | input.js |
| `capture` | `tab_id, clip, budget` | `Captured` | Read | yes | capture.js |
| `dialog` | `tab_id, op{status or handle}` | `DialogFacts` | status Read, handle Effect | yes | primitives.js |
| `diagnostics` | `op{read or clear}` | `DiagnosticsFacts` | Read | read: yes | buffers.js |
| `recording` | `op{start, status, stop, export, discard}` | `RecordingFacts` | export to page Effect, else Read | start, export page | buffers.js |
| `accessibility_snapshot` | `tab_id, frame, max_nodes, max_depth?, query?, guard?` | `AxSnapshot` | Read | yes | primitives.js |
| (frame) `cancel` | `correlation` | none | n/a | no | primitives.js |
| (frame) `notice` | `tab_id, frame?, signal, defer_until_visible` | none | n/a | no (never attaches) | notice.js |

Argument bounds checked by the extension (a violation is `invalid_request`, `dispatched:false`):
- `calls`: 1 to 256.
- `steps`: 1 to 4096.
- `pause`: at most 1000 ms.
- `resize_window`: width 320 to 7680, height 240 to 4320.
- `diagnostics` `limit`: 1 to 200.
- `max_nodes`: at most 5000.
- strings bounded as today: urls 8192, titles 500.

### 1.6 Failure codes and call answers

Receipt `failure` (closed; Rust `Failure`, unknown values decode as `Other`):

| Code | Meaning |
|---|---|
| `unsupported` | Unknown primitive name. Always `dispatched:false` |
| `invalid_request` | Arguments failed validation, or a chunk transfer was rejected |
| `tab_missing` | The named tab does not exist |
| `debugger_unavailable` | Attach refused (restricted page, another debugger, infobar dismissed) |
| `document_changed` | The guard failed, or a named frame is gone or has a new loader |
| `dialog_open` | A JavaScript dialog blocks the page |
| `timeout` | The primitive's own `timeout_ms` or `ready_timeout_ms` elapsed |
| `cancelled` | A `cancel` frame interrupted it |
| `local_interlock` | The preserve-tabs veto |
| `capture_too_large` | No quality in the budget fits `max_base64_chars` |
| `browser_refused` | Chrome rejected the call, or the CDP allowlist refused a method |
| `session_lost` | The debugger session detached mid-primitive |
| `other` | Anything else, with `message` |

`CallAnswer::Failed.code` (closed; Rust `CallFailure`): `document_changed`, `frame_missing`, `owner_missing`,
`dialog_open`, `timeout`, `cancelled`, `session_lost`, `page_exception`, `unserializable`, `other`.

The asset envelope codes are a separate closed set owned by the service (section 3.3). They can grow without a
protocol change, because the extension never parses envelopes.

### 1.7 Evolution and golden fixtures

- **Readers are tolerant.**
  - Rust: no `deny_unknown_fields`; `#[serde(other)]` on every closed enum read from the extension.
  - JS: validators read only known fields; the router ignores unknown `kind`s; the event sink ignores unknown
    events.
- **Within protocol 4, nothing is removed or renamed.** An addition is either:
  - an optional field that the old side ignores, or
  - a new primitive name that an old extension answers `unsupported`. The service renders that as
    `CapabilityVersion { capability: <name>, required: 1, advertised: 0 }`, which the language layer already
    words as "update the Ghostlight extension".
- **Fixtures** live in `tests/golden/protocol4/`:

```
vocabulary.json        closed names: frame kinds, primitive names, failure codes, call failures,
                       events, input steps, clips, modes (drift source for Rust and protocol.js)
frames/                hello, hello_accepted (tiny source and its sha256), heartbeat, heartbeat_ack,
                       control_state, cancel, notice, error, backend_unavailable, command_chunk
events/                one per event
requests/<name>[.<variant>].json         one per primitive and op variant
receipts/<name>.<case>.json              ok and failure cases per primitive, incl. unsupported
tolerance/             frames with extra fields, unknown kinds, unknown events, unknown codes
```

- **Rust** (`level4/golden.rs`):
  - requests built from typed values serialize to exactly the fixture `Value`;
  - every receipt decodes into the typed outcome expected for its primitive;
  - every frame round-trips;
  - tolerance fixtures decode with extras ignored and unknowns mapped to `Unknown` or `Other`;
  - the enum names equal `vocabulary.json`.
- **JS** (`extension/tests/golden-protocol4.test.js`):
  - `lib/protocol.js` equals `vocabulary.json` without the page-message lists (`world_message_kinds`, `asset_error_codes`, `pre_effect_codes`), which belong to the service and its page assets, not the extension;
  - every request fixture passes `primitives.validate`;
  - each primitive's handler test uses its receipt fixtures as the expected output (C13);
  - the router ignores the tolerance frames.

### 1.8 The relay

Protocol 4 needs nothing from the browser connector.

Under D7 the connector, rebuilt on the `relay` crate at M4a:
1. holds the first frame and drops later Chrome frames until it has a service connection;
2. spawns with backoff until connected, then forwards the held frame;
3. pumps frames both ways without parsing them;
4. when no service is there (lost, or none accepted within five seconds of its start), writes
   `{kind:"backend_unavailable", service:"absent"}`, reads and drops Chrome frames, and never spawns again;
5. exits 0 when a service accepts an opening again.

The extension's existing 3 s reconnect then opens a fresh port and sends a fresh hello.

Today's connector replays the cached hello instead. The protocol-4 extension also works with it, because a
repeated `hello_accepted` on one port is idempotent (section 1.2). This spec therefore does not depend on M4a.

---

## 2. The extension after the change

### 2.1 Files

The extension's JavaScript shrinks from 7,119 lines (worker, libs and UI scripts) to about 5,300. The
largest remaining parts are the diagnostics ring (720 lines), recording and chunks, which stay byte-identical.

| File | Job | Approx. lines | Change |
|---|---|---|---|
| `worker.js` (manifest `background.service_worker`) | Bootstrap, native port, hello-first, frame router, controlled-tab set, chrome event listeners, ambient triggers, runtime control, UI messages | 520 | new; replaces `service-worker.js` (2,527) |
| `lib/protocol.js` | Closed vocabulary: frame kinds, primitive names, failure codes, limits, `WORLD_NAME` | 70 | new |
| `lib/debugger.js` | Attach leases, focus emulation, dialogs (with message and session), domains, child-session registry, registration identifiers, pluggable installer, release hook, CDP allowlist `cdp()` | 440 | extended (345 today) |
| `lib/world.js` | Fixed shims, asset registration, lazy install, world handles, `callOne`, Sequence/Parallel/Race, cancel tokens, ambient, present, dispose, DPR | 300 | new |
| `lib/inventory.js` | CDP frame snapshot across root and child sessions, unseen frames, `checkGuard`, `currentLoader` | 140 | new; replaces `frames.js` and `documents.js` (432) |
| `lib/input.js` | Input batch: precondition, dialog race, mouse, key, insertText, pause, drag with interception | 210 | new |
| `lib/tabs.js` | `list_tabs`, `open_tab` (group by key, title fallback, duplicate merge), `navigate`, `focus_tab`, `close_tab` (preserve veto), `set_zoom`, `resize_window`, ready wait | 290 | new; replaces `topology.js` (272) |
| `lib/capture.js` | `view` and `capture`: metrics, clip, budget scale, quality ladder, image dimensions, hide in `finally` | 170 | new; absorbs `screenshot.js` (67) |
| `lib/notice.js` | `notice` delivery, quiet completion, deferral via `presentation-queue.js`, flush triggers, activity list | 130 | new |
| `lib/buffers.js` | Diagnostics read and clear (binding, guard), recording ops (watch, seed and final frames, encoder, downloads, page export) | 300 | new; moved out of the worker |
| `lib/primitives.js` | Handler table, argument validation, prelude, effect watch, receipt building, failure mapping, cancel, `evaluate_main`, `dialog`, `accessibility_snapshot` | 300 | new |
| `lib/script-evaluator.js` + `vendor/acorn.js` | REPL wrap selection (unchanged) plus a deadline and `Runtime.terminateExecution` | 135 | additive |
| `lib/presentation-queue.js` | Deferral queue (16 entries, 10 min TTL); used with a constant namespace | 82 | unchanged |
| `lib/shared.js` | `bounded`, readiness, `browserName`, `linkState` (gains `service_outdated`), heartbeat ack | 110 | trimmed (key and drag helpers move to Rust) |
| `lib/state.js`, `lib/chunks.js`, `lib/connection-log.js`, `lib/diagnostics.js`, `lib/recording.js`, `lib/gif.js` | as today | 1,668 | unchanged |
| `popup.*`, `options.*`, `offscreen.*`, `setup.html` | UI; popup diagnostics row and options form-diagnostics export removed | ~560 | small edits |

Deleted in the final commit:
- `service-worker.js`;
- `lib/engine.js` (ledger);
- `lib/documents.js`, `lib/frames.js`, `lib/topology.js`, `lib/form-diagnostics.js`, `lib/screenshot.js`;
- `lib/presentation.js` and `lib/presentation-css.js`. These are page code and now live only in the service's
  level-3 and level-4 asset sets.
- the `scripting` permission. Its only callers are `service-worker.js:9,1313`.
- the tests of the deleted files.

`host_permissions` stay until a live check proves that attach, `webNavigation` and capture work without them.
That check is not part of this plan.

**The extension never:**
- decides whether to touch a tab;
- picks a tab to adopt;
- builds or edits a world message `kind`;
- merges frames;
- sequences a fill;
- classifies an effect.

### 2.2 Worker (`worker.js`)

```js
importScripts("lib/protocol.js", "lib/shared.js", "lib/state.js", "lib/connection-log.js", "lib/chunks.js",
  "lib/debugger.js", "lib/inventory.js", "lib/world.js", "lib/input.js", "lib/tabs.js", "lib/capture.js",
  "lib/presentation-queue.js", "lib/notice.js", "lib/diagnostics.js", "lib/recording.js", "lib/buffers.js",
  "vendor/acorn.js", "lib/script-evaluator.js", "lib/primitives.js");

const HELLO_TIMEOUT_MS = 10_000;

async function establishNativeConnection() {                 // single-flight via connectNative(), as today
  if (!browserId) await initializeLocalState();              // browser id, preferences, controlled set, groups
  if (nativePort) return;
  const attended = await holdsFocusedWindow();               // R1: everything async happens before connect
  const engine = await engineVersion();                      // userAgentData fullVersionList, else UA regex; [0-9.]{1,32}
  if (nativePort) return;
  const port = chrome.runtime.connectNative(protocol.NATIVE_HOST_NAME);
  nativePort = port;
  port.onMessage.addListener((frame) => onNativeMessage(frame, port).catch(recordError));
  port.onDisconnect.addListener(() => onPortClosed(port));   // cancels in-flight primitives, ambient "disconnected", retry 3 s
  send({ kind: "hello", protocol: 4, browser_id: browserId, browser_name: shared.browserName(navigator.userAgentData?.brands),
         platform: "ghostlight/chromium", engine_version: engine, adapter_version: manifest.version, attended });
  helloTimer = setTimeout(() => setConnection({ compatible: false, last_error: OUTDATED_SERVICE }), HELLO_TIMEOUT_MS);
}

async function onNativeMessage(frame, port) {
  if (port !== nativePort) return;
  switch (frame?.kind) {
    case "heartbeat": return send(shared.heartbeatAcknowledgement(frame));
    case "hello_accepted": return accept(frame);
    case "command_chunk": return commandChunks.accept(frame, (request) => onNativeMessage(request, port),
      (correlation, reason) => primitives.reject(correlation, "invalid_request", reason));
    case "request": return primitives.run(frame.correlation, frame.primitive);   // exactly one receipt
    case "cancel": return primitives.cancel(frame.correlation);
    case "notice": return notice.deliver(frame);
    case "control_state": return applyRuntimeState(stateApi.controlState(frame.state));
    case "backend_unavailable": return serviceLost("The local Ghostlight service is unavailable.", frame.service === "absent");
    case "error": return setConnection({ last_error: shared.bounded(frame.message, 500) });
    default: return;                                         // D6: unknown kinds are ignored
  }
}

async function accept(frame) {
  clearTimeout(helloTimer);
  if (!(await world.setAssets(frame.assets))) {              // sha256(source) !== id
    setConnection({ connected: false, compatible: false, last_error: "The service sent page code that failed its checksum." });
    return nativePort?.disconnect();
  }
  if (frame.service_epoch !== serviceEpoch) { serviceEpoch = frame.service_epoch; await settleServiceBoundaryState(); }
  setConnection({ connected: true, compatible: true, service_version: frame.service_version ?? null, last_error: null });
  await applyRuntimeState(stateApi.controlState(frame.control_state));   // focus emulation, held/ended effects, ambient
}
```

**Controlled-tab set.** `controlled: Map<tabId, "managed" | "released">` is persisted in `storage.session`.
It replaces `topology.js`'s workspace mirror (`topology.js:16`).
- A tab becomes `managed` in the prelude of every addressing primitive (section 1.5): retain the debugger, then
  send AMBIENT with `managed:true`. This restores the cache from explicit service work only (MEMORY: "Restore
  the adapter's opaque cache from explicit service work").
- `close_tab{released:true}` refused by the veto marks the tab `released`: unretain the debugger, then AMBIENT
  with `managed:false`.
- A `released` tab is observed only until its next top-level commit. That one commit is reported, so the
  service can void reuse (ADR-0196 D9). Then the tab leaves the set.
- `tabs.onRemoved` drops the tab.
- On worker start, every `managed` tab is re-retained, gets AMBIENT, and has its notices flushed (as
  `service-worker.js:201-208`).

**Events.**
- `document_committed`, `readiness_changed`, `dialog_changed` and `tab_closed` are sent only for tabs in
  `controlled`. Opener relationships, group labels and passive browsing never add a tab (MEMORY "Ghostlight
  manages only its own actions").
- `attended` and `runtime_control_requested` are browser-level, as today.

**Chrome listeners:**

| Listener | Actions |
|---|---|
| `webNavigation.onCommitted` | Feed effect watches (frame 0); cancel drag interception (frame 0); interrupt a watched recording (any frame); controlled-tab event |
| `webNavigation.onBeforeNavigate` | Diagnostics ring boundary (today `:383-392`) |
| `tabs.onUpdated(status)` | Readiness event; on `complete`, AMBIENT and notice flush for managed tabs |
| `tabs.onActivated`, `windows.onFocusChanged` | Notice flush; `attended` event |
| `tabs.onRemoved` | Cleanup and `tab_closed` |
| `debugger.onEvent` | Dialog open and close, recording each dialog's `message` and `sessionId`; `Input.dragIntercepted`; `Page.screencastFrame`; Runtime and Network diagnostics; the `beforeunload` auto-accept for `navigate{discard_beforeunload}`. Target attach and detach are handled inside `debugger.js` |
| `debugger.onDetach` | Drop handles, interrupt recordings, forget diagnostics |

**AMBIENT** (section 2.4) is sent to the top world of every managed, attached tab on:
- tab complete;
- a control-state change;
- recording start or stop;
- a preference change;
- native disconnect (`runtime:"disconnected"`).

It replaces `syncPresentationState` and `broadcastRuntimeState` (`service-worker.js:1428-1439`).

### 2.3 Debugger sessions, registrations and the CDP allowlist (`lib/debugger.js`)

```js
const CDP_ALLOW = new Set([
  "Page.enable", "Page.getFrameTree", "Page.addScriptToEvaluateOnNewDocument", "Page.removeScriptToEvaluateOnNewDocument",
  "Page.createIsolatedWorld", "Page.getLayoutMetrics", "Page.captureScreenshot", "Page.handleJavaScriptDialog",
  "Page.startScreencast", "Page.stopScreencast", "Page.screencastFrameAck",
  "Runtime.evaluate", "Runtime.callFunctionOn", "Runtime.releaseObject", "Runtime.releaseObjectGroup",
  "Runtime.runIfWaitingForDebugger", "Runtime.terminateExecution", "Runtime.enable", "Runtime.disable",
  "Network.enable", "Network.disable", "Target.setAutoAttach", "DOM.getFrameOwner", "DOM.resolveNode",
  "Input.dispatchMouseEvent", "Input.dispatchKeyEvent", "Input.insertText", "Input.dispatchDragEvent",
  "Input.setInterceptDrags", "Input.cancelDragging", "Emulation.setFocusEmulationEnabled",
  "Accessibility.getFullAXTree", "Accessibility.queryAXTree"]);

function cdp(target, method, params) {                      // target = {tabId} or {tabId, sessionId}
  if (!CDP_ALLOW.has(method)) return Promise.reject(failure("browser_refused", `CDP method ${method} is not allowed`));
  return debuggerApi.sendCommand(target, method, params);
}

function create(debuggerApi, { installer = null, onRelease = null, protocolVersion = "1.3" } = {}) {
  // state per tab as today, plus: root: { identifiers: [] }, children: Map<sessionId, { targetId, identifiers: [] }>
  // ensureAttached: attach; cdp(root, "Page.enable"); await installer?.(rootSession(tabId)); syncFocus  (as :173-189)
  // Runtime.enable is never sent here; only enableDomain (opt-in diagnostics) sends it.
}

debuggerApi.onEvent.addListener(async (source, method, params) => {
  if (method === "Target.attachedToTarget" && params.targetInfo?.type === "iframe") {
    const child = { tabId: source.tabId, sessionId: params.sessionId };
    const state = tabs.get(source.tabId);
    if (!state?.attached) return;
    state.children.set(params.sessionId, { targetId: params.targetInfo.targetId, identifiers: [] });   // 1. record
    track(state, (async () => {
      try {
        await cdp(child, "Page.enable");                                                                // 2.
        await installer?.(childSession(state, params.sessionId));                                       // 3. register world
        await cdp(child, "Target.setAutoAttach", iframeAutoAttach);                                     // 4. not recursive
      } finally {
        if (params.waitingForDebugger) await cdp(child, "Runtime.runIfWaitingForDebugger");             // 5. last
      }
    })());
  }
  if (method === "Target.detachedFromTarget") {
    tabs.get(source.tabId)?.children.delete(params.sessionId);
    onSessionGone?.(source.tabId, params.sessionId);                                                    // world drops handles
  }
});

// Voluntary detach (unretain, release with no leases, ended, detachAll): before chrome.debugger.detach,
// await onRelease?.(tabId) (best effort): DISPOSE in cached worlds, Runtime.releaseObjectGroup{objectGroup:"ghostlight"}
// per session, Page.removeScriptToEvaluateOnNewDocument for stored identifiers.
// openDialog(tabId, { type, message, sessionId }); currentDialog(tabId) returns all three.
```

The default `installer` is today's MAIN-world `installRuntimeInSession` (`debugger.js:96-103`). It keeps the
legacy worker byte-for-byte behaved until the final commit, which deletes the default and
`installPageRuntime`.

### 2.4 World handles and calls (`lib/world.js`, extension side)

```js
const WORLD = protocol.WORLD_NAME;                         // "ghostlight"
// Fixed strings, compiled by the inspector and called on an objectId: no eval, no page CSP, process-safe.
const SHIMS = Object.freeze({
  CALL: `async function(id, message, local, token, attachment, owned) {
    const rt = globalThis.__ghostlightRuntime;
    if (!rt || rt.id !== id) return { ok: false, error: { code: "runtime_missing", message: "Ghostlight page code is not installed in this document." } };
    return await rt.call(message, { local, token, attachment, owner: owned ? this : null }); }`,
  CANCEL:  `function(token) { return globalThis.__ghostlightRuntime?.cancel(token) ?? false; }`,
  AMBIENT: `function(id, local) { const rt = globalThis.__ghostlightRuntime; return rt?.id === id ? rt.ambient(local) : false; }`,
  PRESENT: `function(id, signal, local) { const rt = globalThis.__ghostlightRuntime; return rt?.id === id ? rt.present(signal, local) : false; }`,
  DISPOSE: `function() { globalThis.__ghostlightRuntime?.dispose(); return true; }`,
  DPR:     `function() { return globalThis.devicePixelRatio; }`,
  DOC:     `function() { return document; }`,
});
const WRAP = (source) => `function(__ghostlightAssetId) {\n${source}\n}`;
const registration = (a) => `(${WRAP(a.source)})(${JSON.stringify(a.id)});`;
// local = { preferences: { effects, captions }, managed, recording, runtime, hidden } -- extension-owned facts only.

let assets = null;                                          // { id, source }
const handles = new Map();                                  // `${tabId}|${frameId}|${loaderId}` -> { session, global }
const inflight = new Map();                                 // correlation -> Set<{ session, global, token }>

async function setAssets(next) {
  if (typeof next?.source !== "string" || !/^[0-9a-f]{64}$/.test(next?.id ?? "")) return false;
  if (await sha256Hex(next.source) !== next.id) return false;
  if (assets?.id === next.id) return true;
  assets = next; handles.clear();
  await debuggerLifecycle.forEachSession(register);         // every attached root and child session
  return true;
}

async function register(session) {                         // the debugger.js installer hook
  if (!assets) return;
  for (const identifier of session.identifiers.splice(0))  // superseded registrations
    await cdp(session.target, "Page.removeScriptToEvaluateOnNewDocument", { identifier }).catch(() => {});
  const { identifier } = await cdp(session.target, "Page.addScriptToEvaluateOnNewDocument",
    { source: registration(assets), worldName: WORLD, runImmediately: true });
  session.identifiers.push(identifier);
  // Documents that runImmediately missed are covered lazily on runtime_missing (callOne).
}

// Numeric context ids may be reused by another renderer process; objectIds carry their isolate and
// fail closed in a foreign one. Numeric ids are used only between creation and the loader re-check.
async function worldHandle(tabId, ref, snap) {
  const target = ref ?? snap.top;                            // absent frame: the current top document
  const key = `${tabId}|${target.frame_id}|${target.loader_id}`;
  const known = handles.get(key); if (known) return known;  // a dead document fails the call; it never aliases
  const owner = snap.owner.get(target.frame_id);
  if (!owner) throw callFailure("frame_missing", false);
  if (owner.loaderId !== target.loader_id) throw callFailure("document_changed", false);
  const { executionContextId } = await cdp(owner.session, "Page.createIsolatedWorld", { frameId: target.frame_id, worldName: WORLD });
  const { result } = await cdp(owner.session, "Runtime.evaluate", { expression: "globalThis", contextId: executionContextId, objectGroup: WORLD });
  if (await inventory.currentLoader(owner.session, target.frame_id) !== target.loader_id) throw callFailure("document_changed", false);
  const handle = { session: owner.session, global: result.objectId, ref: target };
  handles.set(key, handle); return handle;
}

async function ownerObject(tabId, ref, childFrameId, snap) { // exact embed; no URL matching
  const h = await worldHandle(tabId, ref, snap);
  const { executionContextId } = await cdp(h.session, "Page.createIsolatedWorld", { frameId: h.ref.frame_id, worldName: WORLD });
  const owner = await cdp(h.session, "DOM.getFrameOwner", { frameId: childFrameId }).catch(() => { throw callFailure("owner_missing", false); });
  const { object } = await cdp(h.session, "DOM.resolveNode", { backendNodeId: owner.backendNodeId, executionContextId, objectGroup: WORLD });
  if (await inventory.currentLoader(h.session, h.ref.frame_id) !== h.ref.loader_id) throw callFailure("document_changed", false);
  return { h, objectId: object.objectId };
}

async function callOne(tabId, call, ctx) {                  // ctx: { correlation, token, deadline, snap, attachment }
  if (debuggerLifecycle.currentDialog(tabId)) return failed("dialog_open", false);
  let reinstalled = false;
  for (;;) {
    let h, target;
    try {
      if (call.owner_of) ({ h, objectId: target } = await ownerObject(tabId, call.frame, call.owner_of, ctx.snap));
      else { h = await worldHandle(tabId, call.frame, ctx.snap); target = h.global; }
    } catch (error) { return asCallFailure(error); }
    track(ctx.correlation, { session: h.session, global: h.global, token: ctx.token });
    try {
      const reply = await raceDialogAndDeadline(tabId, ctx.deadline,
        cdp(h.session, "Runtime.callFunctionOn", { objectId: target, functionDeclaration: SHIMS.CALL,
          awaitPromise: true, returnByValue: true,
          arguments: [assets.id, call.message, localFacts(tabId), ctx.token, ctx.attachment, Boolean(call.owner_of)].map((value) => ({ value })) }),
        () => cdp(h.session, "Runtime.callFunctionOn", { objectId: h.global, functionDeclaration: SHIMS.CANCEL, arguments: [{ value: ctx.token }] }).catch(() => {}));
      if (reply.exceptionDetails) return failed("page_exception", true, describe(reply.exceptionDetails));
      if (reply.result?.value?.error?.code === "runtime_missing" && !reinstalled) {   // nothing ran in the page
        reinstalled = true;
        await cdp(h.session, "Runtime.callFunctionOn", { objectId: h.global, functionDeclaration: WRAP(assets.source),
          arguments: [{ value: assets.id }] });
        continue;
      }
      return { status: "returned", envelope: reply.result?.value ?? null };
    } catch (error) { return mapCallError(error, tabId, call); }   // table below; drops the handle on context loss
    finally {
      untrack(ctx.correlation, ctx.token);
      if (call.owner_of) cdp(h.session, "Runtime.releaseObject", { objectId: target }).catch(() => {});
    }
  }
}
```

`raceDialogAndDeadline` races three things: the CDP promise, the tab's dialog watch (`service-worker.js:2212-2233`
moved here), and the deadline. On the deadline it runs the cancel thunk and answers `timeout` with
`started:true`. On a dialog it answers `dialog_open` with `started:true`.

`mapCallError` is a closed mapping on CDP error text:

| CDP error | `CallFailure` | `started` |
|---|---|---|
| "Cannot find context with specified id", "Could not find object with given id", "No frame with given id found" | `document_changed` | false; handle dropped |
| "Execution context was destroyed", "Inspected target navigated or closed" | `document_changed` | true |
| debugger detached | `session_lost` | true |
| "Object couldn't be returned by value" | `unserializable` | true |
| anything else after send | `other` | true |

**Modes** (`callPrimitive`, used by the `call` primitive):

```js
async function callPrimitive(args, fx, correlation) {
  await ensureDebugger(args.tab_id);
  try {
    const snap = await inventory.snapshot(args.tab_id);
    inventory.checkGuard(args.guard, snap);                  // document_changed, dispatched:false
    fx.watch(args.tab_id);
    const deadline = Date.now() + args.timeout_ms;
    const ctx = (i) => ({ correlation, token: `${correlation}:${i}`, deadline, snap, attachment: null });
    let answers;
    if (args.mode === "sequence") {
      answers = [];
      for (const [i, c] of args.calls.entries())
        answers.push(answers.some((a) => !okAnswer(a)) || primitives.isCancelled(correlation) ? SKIPPED : await callOne(args.tab_id, c, ctx(i)));
    } else if (args.mode === "parallel") {
      answers = await Promise.all(args.calls.map((c, i) => callOne(args.tab_id, c, ctx(i))));
    } else {                                                  // race
      answers = await race(args.tab_id, args.calls, ctx);    // first ok:true wins; CANCEL to pending tokens; pending = skipped
    }
    fx.dispatched = answers.some(started);                   // returned, or failed with started:true
    if (primitives.isCancelled(correlation)) throw failure("cancelled");   // receipt keeps the true facts
    if (args.settle_ms) await sleep(args.settle_ms);
    return { answers };
  } finally { pruneHandles(args.tab_id); await releaseDebugger(args.tab_id); }
}
```

**Ambient, present, dispose, DPR, document:**
- `ambientAll(tabId, patch)` calls AMBIENT on the top world and on every cached handle of the tab, best
  effort.
- `present(tabId, ref, signal)` calls PRESENT on `ref` or the top world.
  - It never attaches: if the tab is not attached it returns false.
  - It never lazily installs: a world without a runtime returns false.
- `disposeTab(tabId)` calls DISPOSE on every cached handle, then releases the object group.
- `devicePixelRatio(tabId)` calls DPR on the top world. It is read in the isolated world, so the page cannot
  override it. Today it is read in the main world (`service-worker.js:2114`).
- `documentObject(tabId, ref)` calls DOC with `returnByValue:false` and is used by `accessibility_snapshot`.
- `localFacts(tabId)` returns:
  - `preferences: { effects, captions }`;
  - `managed`: the tab is `managed` in `controlled`;
  - `recording`: `recording.activeForTab(tabId)`;
  - `runtime`: the control state, or `"disconnected"`;
  - `hidden`: the per-tab capture-hold count is above zero. Concurrent primitives therefore never unhide a
    capture in progress.

### 2.5 Frame inventory and guard (`lib/inventory.js`)

```js
async function snapshot(tabId) {
  const sessions = [debuggerLifecycle.rootSession(tabId), ...debuggerLifecycle.childSessions(tabId)];
  const owner = new Map(); let complete = true; let top = null;
  for (const session of sessions) {
    let tree; try { tree = (await cdp(session.target, "Page.getFrameTree")).frameTree; } catch { complete = false; continue; }
    walk(tree, ({ frame }) => {                              // pre-order
      const oopifRoot = Boolean(session.target.sessionId) && frame.id === session.targetId;
      if (owner.has(frame.id) && !oopifRoot) return;         // an OOPIF root belongs to its own session
      const fact = { frame_id: frame.id, loader_id: frame.loaderId, parent_frame_id: frame.parentId ?? null,
                     url: shared.bounded(frame.url + (frame.urlFragment ?? ""), 8192), unreachable: Boolean(frame.unreachableUrl) };
      owner.set(frame.id, { session: session.target, loaderId: frame.loaderId, fact });
      if (!frame.parentId) top = { frame_id: frame.id, loader_id: frame.loaderId };
    });
  }
  // A session lists only frames in its own process; an OOPIF Chrome would not attach is otherwise invisible.
  const seen = urlMultiset([...owner.values()].map((f) => withoutFragment(f.fact.url)));
  const unseen = ((await chrome.webNavigation.getAllFrames({ tabId })) ?? [])
    .filter((f) => f.frameId !== 0 && f.documentLifecycle === "active" && !takeOne(seen, withoutFragment(f.url)))
    .slice(0, protocol.MAX_UNSEEN).map((f) => shared.bounded(f.url, 8192));
  return { owner, complete, unseen, top };
}

function checkGuard(guard, snap) {                          // identity, url and origin only
  if (!guard) return;
  for (const d of guard.documents) {
    const now = snap.owner.get(d.frame_id);
    if (!now || now.loaderId !== d.loader_id) throw failure("document_changed");
    if (guard.exact ? now.fact.url !== d.url : origin(now.fact.url) !== origin(d.url)) throw failure("document_changed");
  }
  if (guard.exact && (!snap.complete || snap.owner.size !== guard.documents.length || snap.unseen.length !== guard.unseen))
    throw failure("document_changed");
}
const currentLoader = async (target, frameId) => findFrame((await cdp(target, "Page.getFrameTree")).frameTree, frameId)?.loaderId ?? null;
```

The guard mirrors today's `validateCurrent` (`documents.js:44-57`) exactly:
- the strict mode compares the whole tree including URLs;
- the subject mode compares identity and origin of the required documents.

A loader id adds what webNavigation could not: a replaced document always fails, even at the same URL.

The `frames` primitive returns `{frames: [...owner.values()].map(f => f.fact), complete, unseen}`.

Prerender and back/forward-cache pages are separate targets or absent from the tree. Auto-attach filters to
`iframe`, so they drop out with no lifecycle filter. This supersedes ADR-0189 (ADR-0196 Appendix A).

### 2.6 Dispatcher, effect facts and cancel (`lib/primitives.js`)

```js
const HANDLERS = Object.freeze({                           // name -> { validate, run, addresses }
  list_tabs: tabs.listTabs, open_tab: tabs.openTab, navigate: tabs.navigate, focus_tab: tabs.focusTab,
  close_tab: tabs.closeTab, set_zoom: tabs.setZoom, resize_window: tabs.resizeWindow,
  frames: inventory.framesPrimitive, call: world.callPrimitive, evaluate_main: evaluateMain, view: capture.view,
  input: input.run, capture: capture.capture, dialog, diagnostics: buffers.diagnostics, recording: buffers.recording,
  accessibility_snapshot: accessibilitySnapshot });
const cancelled = new Map();                                // correlation -> expiry (60 s), so a cancel that beats reassembly still counts

async function run(correlation, primitive) {
  const spec = HANDLERS[primitive?.name];
  if (!spec) return reply(correlation, { failure: "unsupported", dispatched: false, committed_urls: [] });
  const args = primitive.args ?? {};
  const fx = effects.open();                                // per-primitive watch: { dispatched, commits, tabId }
  try {
    spec.validate(args);                                    // invalid_request
    if (isCancelled(correlation)) throw failure("cancelled");
    if (spec.addresses(args)) await controlled.manage(spec.tabOf(args));   // prelude: retain debugger, AMBIENT managed
    const ok = await spec.run(args, fx, correlation);
    return reply(correlation, { ok, ...(await fx.facts()) });
  } catch (error) {
    return reply(correlation, { failure: failureCode(error), message: shared.bounded(error?.message ?? error, 500), ...(await fx.facts()) });
  } finally { fx.close(); cancelled.delete(correlation); }
}

function cancel(correlation) {
  cancelled.set(correlation, Date.now() + 60_000);          // checked by sequence, input and ready-wait loops
  for (const entry of world.inflight(correlation)) world.cancelToken(entry);   // CANCEL shim, best effort
}
```

Effect watches are per primitive. Each one is an entry in a per-tab `Set`, not the single
`navigationWatchers` slot (`service-worker.js:74`). Concurrent primitives on one tab therefore never clobber
each other.
- `fx.watch(tabId)` opens it.
- `webNavigation.onCommitted` with frame 0 pushes the url into every open watch of that tab.
- `fx.facts()` returns `{ dispatched, committed_urls, tab? }`. `tab` is read with `chrome.tabs.get` when
  `dispatched` is true and a tab was watched.

`failureCode(error)` is a closed mapping:

| Error | Code |
|---|---|
| `error.code` that is a protocol code | that code |
| "No tab with id" | `tab_missing` |
| attach refusal | `debugger_unavailable` |
| detach | `session_lost` |
| a CDP error | `browser_refused` |
| anything else | `other` |

On port disconnect every in-flight primitive is cancelled, and its receipt is dropped.

### 2.7 Primitive pseudo-JS

Every primitive that sends CDP holds a debugger lease: `ensureDebugger` (retain if controlled, then acquire)
at the start and `releaseDebugger` in `finally`, as today (`service-worker.js:2018-2027`). `root(tabId)` is
`{tabId}`.

**Tabs (`lib/tabs.js`)**

```js
async function listTabs(args) {                             // read; no prelude; no custody
  const named = args.tab_ids ? new Set(args.tab_ids) : null;
  const openers = new Set([...controlled.managedIds(), ...(named ?? [])]);
  return { tabs: (await chrome.tabs.query({}))
    .filter((t) => !named || named.has(t.id) || named.has(t.openerTabId))
    .map((t) => tabFact(t, openers.has(t.openerTabId) ? t.openerTabId : undefined)) };
}
// tabFact: { tab_id, title: bounded 500, url: url || pendingUrl || "about:blank", active, readiness, opener_tab_id? }

async function openTab(args, fx) {
  if (args.adopt_tab_id !== undefined) {                    // only a tab the service chose (ADR-0196 D9)
    const tab = await chrome.tabs.get(args.adopt_tab_id);    // tab_missing, dispatched:false
    fx.watch(tab.id); fx.dispatched = true;
    await groups.place(tab.id, args.group_key, args.group_title);   // tabs.move into the group window, tabs.group, tabGroups.update
    await chrome.tabs.update(tab.id, args.url ? { url: args.url, active: true } : { active: true });
    if (args.focus_window) await chrome.windows.update(tab.windowId, { focused: true });
    await controlled.manage(tab.id);
    return { tab: tabFact(await waitForReady(tab.id, args.ready_timeout_ms, fx)), adopted: true };
  }
  const tab = await groups.open(args.url, args.group_key, args.group_title, args.focus_window,
    (created) => { fx.watch(created.id); fx.dispatched = true; });
  await controlled.manage(tab.id);
  return { tab: tabFact(await waitForReady(tab.id, args.ready_timeout_ms, fx)), adopted: false };
}

// groups: Map<group_key, { groupId, title }> in storage.session, serialized like topology.js:21-25.
async function canonical(key, title) {
  // 1. stored id whose current title equals `title`; 2. else the lowest-id group with exact `title`
  //    (repair after extension reload); 3. merge same-title duplicates into it (topology.js:83-98).
}
async function open(url, key, title, focus, onCreated) {
  const group = await canonical(key, title);
  const windowId = group?.windowId ?? await anyOwnGroupWindow();   // a window holding any group in the map
  const tab = windowId !== undefined
    ? await chrome.tabs.create({ url, active: true, windowId })
    : (await chrome.windows.create({ url, focused: focus, type: "normal" })).tabs[0];
  onCreated(tab);
  const groupId = await chrome.tabs.group(group ? { groupId: group.id, tabIds: [tab.id] } : { tabIds: [tab.id] });
  await chrome.tabGroups.update(groupId, { title, color: "blue", collapsed: false });
  remember(key, groupId, title);
  if (focus) await chrome.windows.update(tab.windowId, { focused: true });
  return tab;
}

async function navigate(args, fx) {
  await chrome.tabs.get(args.tab_id);                        // tab_missing before any effect
  fx.watch(args.tab_id);
  if (args.discard_beforeunload) { await ensureDebugger(args.tab_id); beforeUnload.add(args.tab_id); }
  try {
    fx.dispatched = true;
    if (args.to.to === "url") await chrome.tabs.update(args.tab_id, { url: args.to.url, active: true });
    if (args.to.to === "back") { await chrome.tabs.goBack(args.tab_id); await sleep(75); }
    if (args.to.to === "forward") { await chrome.tabs.goForward(args.tab_id); await sleep(75); }
    if (args.to.to === "reload") { await chrome.tabs.reload(args.tab_id, { bypassCache: Boolean(args.to.bypass_cache) }); await sleep(75); }
    return { tab: tabFact(await waitForReady(args.tab_id, args.ready_timeout_ms, fx)) };
  } finally { if (args.discard_beforeunload) { beforeUnload.delete(args.tab_id); await releaseDebugger(args.tab_id); } }
}
// worker debugger.onEvent: Page.javascriptDialogOpening type "beforeunload" on a tab in beforeUnload
//   -> cdp(dialogSession, "Page.handleJavaScriptDialog", { accept: true })
// waitForReady: poll chrome.tabs.get every 100 ms until "complete", or not loading with a url, or the timeout;
//   a cancelled correlation throws failure("cancelled") (dispatched stays true).

async function focusTab(args, fx) {
  await chrome.tabs.get(args.tab_id);                        // tab_missing before any effect
  fx.watch(args.tab_id); fx.dispatched = true;
  const tab = await chrome.tabs.update(args.tab_id, { active: true });
  const win = await chrome.windows.update(tab.windowId, { focused: true });
  return { tab_id: args.tab_id, active: Boolean(tab.active), window_focused: Boolean(win.focused) };
}

async function closeTab(args, fx) {
  if (await preserveTabsEnabled()) {                        // the person's local veto (service-worker.js:773-789)
    if (args.released) await controlled.release(args.tab_id);
    throw failure("local_interlock", "Ghostlight is preserving controlled tabs by local browser choice.");
  }
  fx.dispatched = true; await chrome.tabs.remove(args.tab_id);
  return { tab_id: args.tab_id };
}

async function setZoom(args, fx) {
  await chrome.tabs.get(args.tab_id);                        // tab_missing before any effect
  fx.watch(args.tab_id); fx.dispatched = true;
  await chrome.tabs.setZoom(args.tab_id, args.zoom);
  return { tab_id: args.tab_id, zoom: await chrome.tabs.getZoom(args.tab_id) };
}
async function resizeWindow(args, fx) {                     // bounds validated; body as service-worker.js:1005-1033
  const tab = await chrome.tabs.get(args.tab_id); fx.dispatched = true;
  const resized = await chrome.windows.update(tab.windowId, { width: args.width, height: args.height });
  const affected = sortedUnique([...(await chrome.tabs.query({ windowId: tab.windowId })).map((t) => t.id), args.tab_id]);
  return { tab_id: args.tab_id, width: resized?.width ?? args.width, height: resized?.height ?? args.height, affected_tab_ids: affected };
}
```

**`frames`** returns `inventory.snapshot` facts (section 2.5), inside a debugger lease.

**`call`**: section 2.4.

**`evaluate_main` (`lib/primitives.js`)**

```js
async function evaluateMain(args, fx) {
  const expression = scriptEvaluator.prepare(args.script, args.max_result_chars);   // acorn; invalid -> invalid_request, dispatched:false
  await ensureDebugger(args.tab_id);
  try {
    inventory.checkGuard(args.guard, await inventory.snapshot(args.tab_id));
    fx.watch(args.tab_id); fx.dispatched = true;
    const reply = await scriptEvaluator.run(
      () => cdp(root(args.tab_id), "Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true, userGesture: true, replMode: true }),
      args.timeout_ms,
      () => cdp(root(args.tab_id), "Runtime.terminateExecution").catch(() => {}));  // on deadline: terminate, then failure("timeout")
    if (reply.exceptionDetails) return { exception: shared.bounded(describe(reply.exceptionDetails), args.max_result_chars) };
    const serialized = JSON.stringify(reply.result?.value ?? null);
    const value = serialized.slice(0, args.max_result_chars);
    await sleep(args.settle_ms);
    return { value, truncated: serialized.length > value.length };
  } finally { await releaseDebugger(args.tab_id); }
}
```

There is no `contextId`, so this is the main world of the top frame. `terminateExecution` stops a running
script. It cannot stop an awaited promise, so the receipt is `timeout` with `dispatched:true` either way.

**`view` and `capture` (`lib/capture.js`)**

```js
async function readView(tabId) {
  const m = await cdp(root(tabId), "Page.getLayoutMetrics");
  const v = m.cssVisualViewport, c = m.cssContentSize;
  return { metrics: v, view: { visual_page_x: v.pageX ?? 0, visual_page_y: v.pageY ?? 0, visual_css_width: v.clientWidth,
           visual_css_height: v.clientHeight, content_css_width: c.width, content_css_height: c.height,
           device_scale: Number(await world.devicePixelRatio(tabId)) || 1, zoom: await chrome.tabs.getZoom(tabId) },
           content: c };
}
async function view(args) { await ensureDebugger(args.tab_id); try { return (await readView(args.tab_id)).view; } finally { await releaseDebugger(args.tab_id); } }

async function capture(args) {
  await ensureDebugger(args.tab_id);
  holdHidden(args.tab_id);                                  // localFacts(tab).hidden is true while any hold exists
  await world.ambientAll(args.tab_id);                       // hide Ghostlight's own presentation in every known world
  try {
    const { metrics: v, view: facts, content: c } = await readView(args.tab_id);
    const clip = clipFor(args.clip, v, c);                  // viewport: pageX/Y + client size; full_page: 0,0 + content size;
                                                            // viewport_rect: max(0, pageX+left), max(0, pageY+top); page_rect: as given
    const scale = outputScale(clip.width, clip.height, args.budget);   // screenshot.js:24-34 with the budget's numbers
    let shot = null;
    for (const quality of args.budget.qualities) {
      shot = await cdp(root(args.tab_id), "Page.captureScreenshot",
        { format: "jpeg", quality, clip: { ...clip, scale }, captureBeyondViewport: true, fromSurface: true });
      if (shot.data.length <= args.budget.max_base64_chars) break;
      shot = null;
    }
    if (!shot) throw failure("capture_too_large");
    const dims = await imageDimensions(shot.data, clip, scale);         // service-worker.js:2149-2169
    return { mime_type: "image/jpeg", data: shot.data, width: dims.width, height: dims.height, output_scale: dims.outputScale,
             clip: { x: clip.x, y: clip.y, width: clip.width, height: clip.height }, view: facts };
  } finally {
    releaseHidden(args.tab_id); await world.ambientAll(args.tab_id).catch(() => {});   // a dead service cannot leave it hidden
    await releaseDebugger(args.tab_id);
  }
}
```

**`input` (`lib/input.js`)**

```js
async function run(args, fx, correlation) {
  await ensureDebugger(args.tab_id);
  try {
    let snap = await inventory.snapshot(args.tab_id);
    inventory.checkGuard(args.guard, snap);
    fx.watch(args.tab_id);
    const deadline = Date.now() + args.timeout_ms;
    let precondition;
    if (args.precondition) {                                 // e.g. focus_text_field, require_focused, verify_focused_text, focus
      precondition = await world.callOne(args.tab_id, args.precondition, { correlation, token: `${correlation}:p`, deadline, snap, attachment: null });
      fx.dispatched = started(precondition);
      if (!okAnswer(precondition)) return { precondition, completed_steps: 0, native_drag: false };   // no input sent
      snap = await inventory.snapshot(args.tab_id);          // right before the first event
      inventory.checkGuard(args.guard, snap);
    }
    let completed = 0, nativeDrag = false;
    const dialog = watchForDialog(args.tab_id);
    try {
      for (const step of args.steps) {
        if (primitives.isCancelled(correlation)) return { precondition, completed_steps: completed, stopped: { reason: "cancelled" }, native_drag: nativeDrag };
        if (Date.now() >= deadline) throw failure("timeout");
        fx.dispatched = true;
        const sent = step.step === "drag" ? drag(args.tab_id, step, dialog).then((r) => { nativeDrag ||= r.native; })
                                          : sendStep(args.tab_id, step);
        const opened = await Promise.race([sent.then(() => null), dialog.opened]);
        if (opened) return { precondition, completed_steps: completed, stopped: { reason: "dialog", dialog_type: opened }, native_drag: nativeDrag };
        completed += 1;
      }
    } finally { dialog.forget(); }
    await sleep(args.settle_ms);
    return { precondition, completed_steps: completed, native_drag: nativeDrag };
  } finally { await releaseDebugger(args.tab_id); }
}

function sendStep(tabId, s) {
  if (s.step === "mouse") return cdp(root(tabId), "Input.dispatchMouseEvent", { type: MOUSE[s.action], x: s.x, y: s.y,
    button: s.button ?? "none", buttons: s.buttons ?? 0, clickCount: s.click_count ?? 0, modifiers: s.modifiers ?? 0,
    deltaX: s.delta_x ?? 0, deltaY: s.delta_y ?? 0, force: s.force ?? 0 });
    // MOUSE = { moved: "mouseMoved", pressed: "mousePressed", released: "mouseReleased", wheel: "mouseWheel" }
  if (s.step === "key") return cdp(root(tabId), "Input.dispatchKeyEvent", { type: KEY[s.action], key: s.key, code: s.code,
    windowsVirtualKeyCode: s.key_code, nativeVirtualKeyCode: s.key_code, text: s.text, unmodifiedText: s.unmodified_text,
    modifiers: s.modifiers ?? 0 });
    // KEY = { down: "keyDown", up: "keyUp", raw_down: "rawKeyDown", char: "char" }
  if (s.step === "insert_text") return cdp(root(tabId), "Input.insertText", { text: s.text });
  if (s.step === "pause") return sleep(Math.min(1000, s.ms));
}

async function drag(tabId, s, dialog) {                     // event-driven; replaces service-worker.js:1842-1934
  const [start, ...rest] = s.path, end = s.path.at(-1);
  const interception = beginDragInterception(tabId);        // resolved by Input.dragIntercepted in the worker
  let intercepting = false, pressed = false, released = false;
  try {
    try { await cdp(root(tabId), "Input.setInterceptDrags", { enabled: true }); intercepting = true; } catch { cancelDragInterception(tabId); }
    await mouse(tabId, "mouseMoved", start, { modifiers: s.modifiers });
    await mouse(tabId, "mousePressed", start, { button: "left", clickCount: 1, modifiers: s.modifiers }); pressed = true;
    for (const point of rest) {
      await mouse(tabId, "mouseMoved", point, { button: "left", buttons: 1, force: 1, modifiers: s.modifiers });
      const data = intercepting ? await interception.within(10) : null;     // wait up to 10 ms per held move
      if (data) return await replay(tabId, data, end, s);
    }
    const late = intercepting ? await interception.within(100) : null;     // grace for late interception data
    if (late) return await replay(tabId, late, end, s);
    await mouse(tabId, "mouseReleased", end, { button: "left", clickCount: 1, modifiers: s.modifiers }); released = true;
    return { native: false };                                // pointer-library drag
  } finally {
    cancelDragInterception(tabId);                           // data arriving after release triggers Input.cancelDragging in the worker
    if (intercepting) await cdp(root(tabId), "Input.setInterceptDrags", { enabled: false }).catch(() => {});
    if (pressed && !released) {
      await mouse(tabId, "mouseReleased", end, { button: "left", clickCount: 1 }).catch(() => {});
      await cdp(root(tabId), "Input.cancelDragging").catch(() => {});
    }
  }
  async function replay(tabId, data, end, s) {
    await cdp(root(tabId), "Input.setInterceptDrags", { enabled: false }); intercepting = false;
    for (const type of ["dragEnter", "dragOver", "drop"])
      await cdp(root(tabId), "Input.dispatchDragEvent", { type, x: end.x, y: end.y, data, modifiers: s.modifiers ?? 0 });
    await mouse(tabId, "mouseReleased", end, { button: "left", clickCount: 1 }); released = true;
    return { native: true };
  }
}
```

**`dialog` (`lib/primitives.js`)**

```js
async function dialog(args, fx) {
  if (args.op.op === "status") {
    let d = debuggerLifecycle.currentDialog(args.tab_id);
    if (!d) { await ensureDebugger(args.tab_id); try { await sleep(50); d = debuggerLifecycle.currentDialog(args.tab_id); } finally { await releaseDebugger(args.tab_id); } }
    return { present: Boolean(d), dialog_type: d?.type ?? "unknown", message: d ? shared.bounded(d.message, 500) : undefined,
             ...(d?.url ? { url: shared.bounded(d.url, URL_CHARS) } : {}) };
  }
  await ensureDebugger(args.tab_id);
  const d = debuggerLifecycle.currentDialog(args.tab_id);
  try {
    fx.dispatched = true;
    await cdp(d?.session ?? root(args.tab_id), "Page.handleJavaScriptDialog", { accept: args.op.accept, promptText: args.op.text });
    await debuggerLifecycle.closeDialog(args.tab_id);
    return { present: true, dialog_type: d?.type ?? "unknown", handled: true, accepted: args.op.accept,
             ...(d?.url ? { url: shared.bounded(d.url, URL_CHARS) } : {}) };
  } catch (error) {
    if (!/No dialog is showing/.test(String(error?.message))) throw error;   // other, dispatched:true
    fx.dispatched = false; await debuggerLifecycle.closeDialog(args.tab_id);
    return { present: false, dialog_type: "unknown", handled: false, accepted: false };
  } finally { await releaseDebugger(args.tab_id); }
}
```

The dialog is handled on the session that raised it, so out-of-process frame dialogs work too. Today the
dialog is always handled on the root session.

`url` is the `url` of `Page.javascriptDialogOpening`: the frame that raised the dialog. The worker records it
with the dialog, and it is omitted when the browser did not say.

A JavaScript dialog blocks its renderer, so a primitive that needs the renderer must not wait on it. Every
read of the frame tree (`inventory.snapshot`, `topSnapshot`, `currentLoader`, and so the `frames`, `call`,
`input`, `diagnostics`, `recording` and `accessibility_snapshot` primitives) refuses at once with
`dialog_open` while a dialog is recorded for the tab, as `capture`, `evaluate_main` and `input` already
did. `dialog` itself is answered from the extension's record and never reads the renderer.

The service therefore authorizes a dialog by the one document that raised it: it sends `dialog{status}`
alone (no `frames`, no document inventory), authorizes `url` for the invocation's capabilities, and only then
returns the message or sends `dialog{handle}`. A denied document is document access; a dialog with no usable
`url` fails closed as `DocumentUnavailable`. `HandleDialog` looks first and sends no `handle` when no dialog
is showing. A `dialog_open` receipt on any other primitive becomes `BrowserError::DialogOpen`, which the
executor says as the dialog refusal.

**`diagnostics` and `recording` (`lib/buffers.js`)**

```js
async function diagnosticsRead(op) {                        // body of service-worker.js:559-600
  if (op.binding !== undefined && bindings.get(op.tab_id) !== op.binding) {   // replaces diagnosticDocuments
    if (bindings.has(op.tab_id)) { diagnostics.forget(op.tab_id); await disableCapture([op.tab_id]); }
    bindings.set(op.tab_id, op.binding);
  }
  const started = diagnostics.enable(op.tab_id);
  let sourceStarted = false;
  if (op.source !== "network") sourceStarted = await debuggerLifecycle.enableDomain(op.tab_id, "Runtime") || sourceStarted;
  if (op.source !== "console") sourceStarted = await debuggerLifecycle.enableDomain(op.tab_id, "Network") || sourceStarted;
  inventory.checkGuard(op.guard, await inventory.snapshot(op.tab_id));
  const result = diagnostics.read(op.tab_id, op);
  return { ...result, capture_started: result.capture_started || started || sourceStarted };
}
async function diagnosticsClear(op) {
  for (const id of op.tab_ids) bindings.delete(id);
  const cleared = diagnostics.forgetMany(op.tab_ids); await disableCapture(op.tab_ids);
  return { cleared_count: cleared };
}

const NS = "protocol4";                                     // recording.js namespace; the service owns workspaces
async function recordingStart(op) {                         // service-worker.js:1074-1103 with watch
  const tab = await chrome.tabs.get(op.tab_id);
  const started = recording.start(NS, op.tab_id, tab.url);
  if (started.existing) return { result: "started", summary: started.existing, existing: true };
  if (op.watch) watches.set(op.tab_id, op.watch);
  const state = recording.activeForTab(op.tab_id);
  try {
    await captureFrame(state, "seed").catch(() => false);   // hide via a capture hold, as capture()
    await debuggerLifecycle.enableDomain(op.tab_id, "Page");
    await cdp(root(op.tab_id), "Page.startScreencast", { format: "jpeg", quality: Rec.JPEG_QUALITY,
      maxWidth: Rec.MAX_WIDTH, maxHeight: Rec.MAX_HEIGHT, everyNthFrame: 1 });
    await world.ambientAll(op.tab_id);                       // recording flag
    return { result: "started", summary: recording.status(NS, state.id).summary, existing: false };
  } catch (error) { recording.discard(NS, state.id); cdp(root(op.tab_id), "Page.stopScreencast").catch(() => {}); throw error; }
}
// On every Page.screencastFrame: Page.screencastFrameAck; then, if watches.has(tab), take inventory.snapshot and
// compare its { frame_id, loader_id, url } set (complete, no unseen) with the watch; any difference interrupts
// with document_boundary (today recordingScopeCurrent, service-worker.js:529-541). Note every snapshot url.
// Status/stop/discard: recording.status/beginStop/finishStop/discard(NS, op.recording_id) as service-worker.js:1105-1140.
// Export: askEncoder as service-worker.js:1237-1266; destination
//   download -> chrome.downloads.download(object_url) and wait for completion;
//   client   -> { delivery: "returned", mime_type, data };
//   page     -> checkGuard, then world.callOne(dest.tab_id, dest.call, { ..., attachment: { media_type: "image/gif", data, size } }),
//               with fx.dispatched = started(answer), and { delivery: "attached", tab_id } when the envelope is ok.
// A failed encode or delivery is { result: "export_failed", reason } (as today).
```

**`accessibility_snapshot` (`lib/primitives.js`)**

```js
async function accessibilitySnapshot(args) {
  await ensureDebugger(args.tab_id);
  try {
    const snap = await inventory.snapshot(args.tab_id); inventory.checkGuard(args.guard, snap);
    const owner = snap.owner.get(args.frame.frame_id);
    if (!owner || owner.loaderId !== args.frame.loader_id) throw failure("document_changed");
    const raw = args.query
      ? await cdp(owner.session, "Accessibility.queryAXTree", { objectId: await world.documentObject(args.tab_id, args.frame, snap),
                                                               role: args.query.role, accessibleName: args.query.name })
      : await cdp(owner.session, "Accessibility.getFullAXTree", { frameId: args.frame.frame_id, depth: args.max_depth });
    if (await inventory.currentLoader(owner.session, args.frame.frame_id) !== args.frame.loader_id) throw failure("document_changed");
    const nodes = raw.nodes.filter((n) => !n.ignored);
    const max = Math.min(args.max_nodes, 5000);
    return { nodes: nodes.slice(0, max).map(axFact), truncated: nodes.length > max };
    // axFact: { node_id, parent_id, role: bounded 100, name: bounded 500, value: bounded 500,
    //           properties: only checked, disabled, expanded, focused, required, selected, level }
  } finally { await releaseDebugger(args.tab_id); }
}
```

Nothing calls `Accessibility.enable`. The primitive exists so that a future tool needs no store release
(OD18). No service command uses it yet.

**`notice` (`lib/notice.js`)**

```js
async function deliver({ tab_id, frame, signal, defer_until_visible }) {
  activity.update(signal, await groupTitleOf(tab_id));      // popup activity label = the tab's group title
  if (defer_until_visible && !(await tabIsVisible(tab_id))) {
    // The notice waits for its page, but the invocation is over: retire its decoration now (service-worker.js:946-955).
    await world.present(tab_id, frame, { ...signal, signal: "completion", activity: "quiet", phase: "", detail: null, locator: null, click: null });
    return defer(tab_id, { frame, signal });                 // presentationQueue.defer(NS, tab_id, signal) + frame; badge "!"
  }
  const rendered = await world.present(tab_id, frame, signal);   // false when not attached; never attaches
  if (rendered) forget(tab_id); else if (defer_until_visible) defer(tab_id, { frame, signal });
}
async function flush(tabId) {                               // on tabs.onActivated, windows.onFocusChanged, tab complete
  const pending = queued(tabId);
  if (!pending || !(await tabIsVisible(tabId))) return;
  const current = pending.frame && await inventory.isCurrent(tabId, pending.frame);
  const signal = current ? pending.signal : { ...pending.signal, locator: null };   // a stale frame falls back to the top
  if (await world.present(tabId, current ? pending.frame : null, signal)) forget(tabId);
}
```

**`cancel`**: section 2.6.

---

## 3. The isolated-world asset entry

### 3.1 Identity and wrapping

- **The asset set.** The service builds one level-4 asset set: `{id, source}`.
  - `source` is `assets/world.js`, with the other level-4 sources inlined at its include marker, in this
    order: `page-shared.js` (`bounded`, `isCredentialMetadata`, `activityLabel`, `presentationLabel`),
    `sensor.js`, `presentation-css.js`, `presentation.js`, `content.js`.
  - `id` is the lowercase hex SHA-256 of `source`. One value is both integrity and identity.
- **The wrapper is fixed extension glue**, `WRAP(source) = function(__ghostlightAssetId) {\n<source>\n}`:
  - the registration runs `(WRAP)("<id>");`;
  - the lazy installer calls `WRAP` through `Runtime.callFunctionOn` on the world global, with `[id]`.

  The asset reads its own id from the parameter, so `id` can be the hash of `source` with no self-reference.
- **The level-3 bundle is frozen.**
  - Its six sources are byte-pinned copies under `level3/assets/`.
  - Its SHA-256 is pinned by a test.
  - It is never built from the evolving level-4 files, or from `extension/lib`.

### 3.2 `assets/world.js`

```js
"use strict";
const ID = __ghostlightAssetId;
const previous = globalThis.__ghostlightRuntime;            // isolated-world global, invisible to the page
if (previous?.id === ID) return;
try { previous?.dispose(); } catch (_) {}
const IS_TOP = window.self === window.top;
const EPOCH = Array.from(crypto.getRandomValues(new Uint8Array(4)), (b) => b.toString(16).padStart(2, "0")).join("");
const disposers = [];
const tokens = new Map();
const world = Object.freeze({ IS_TOP, EPOCH, onDispose: (fn) => disposers.push(fn) });
class AssetError extends Error { constructor(code, message, detail) { super(message); this.code = code; if (detail !== undefined) this.detail = detail; } }
const CODES = new Set([/* AssetErrorCode values; drift-tested against world.rs */]);
/*@include page-shared.js sensor.js presentation-css.js presentation.js content.js*/
// content.js defines, as closure bindings (never globals): dispatch(message, ctx), applyAmbient(local),
// renderPresentation(signal, preferences); and registers its disposers with world.onDispose.
const failure = (e) => {
  const error = { code: CODES.has(e?.code) ? e.code : "page_exception", message: String(e?.message ?? e).slice(0, 500) };
  if (e instanceof AssetError && e.detail !== undefined) error.detail = e.detail;
  return { ok: false, error };
};
globalThis.__ghostlightRuntime = Object.freeze({
  id: ID,
  async call(message, { local, token, attachment, owner }) {
    if (typeof message?.kind !== "string") return failure(new AssetError("invalid_request", "world call requires a kind"));
    const signal = { cancelled: false };
    if (token) tokens.set(token, signal);
    try {
      if (local) applyAmbient(local);
      return { ok: true, value: (await dispatch(message, { signal, attachment, owner })) ?? null };
    } catch (e) { return failure(e); }                      // the envelope never rejects
    finally { if (token) tokens.delete(token); }
  },
  cancel(token) { const s = tokens.get(token); if (s) s.cancelled = true; return Boolean(s); },
  ambient(local) { applyAmbient(local); return true; },
  present(signal, local) { return renderPresentation(signal, local?.preferences ?? {}); },
  dispose() {
    for (const s of tokens.values()) s.cancelled = true;
    tokens.clear();
    for (const fn of disposers.splice(0).reverse()) { try { fn(); } catch (_) {} }
    if (globalThis.__ghostlightRuntime?.id === ID) delete globalThis.__ghostlightRuntime;
  },
});
```

`applyAmbient(local)` calls `setHidden(local.hidden)` in every frame. In the top frame only, it also calls
`setManaged(local.managed)`, `setRecording(local.recording)` and `setRuntimeState(local.runtime)`
(`presentation.js:488-513`).

`present` keeps today's rule that a frame other than the top renders only target-anchored signals
(`content.js:979`).

### 3.3 Envelope

```
{ ok: true,  value: <JSON> }
{ ok: false, error: { code: <AssetErrorCode>, message: <bounded 500>, detail?: <object> } }
```

`detail` is optional and says what the code and message do not. The extension relays an envelope
without reading it, so a detail needs no extension release. Only a fill carries one: a refused fill
and a fill the page partly undid (ADR-0198 Decisions 8, 10 and 11).

**Fill problems.** `prepareFill` checks every field before it throws, so one failed call reports
all the fields that cannot take their value and nothing is edited. Its detail is
`{problems:[{field, problem, options?, options_total?, containing?, expected?, limits?, group?, type?, route?}]}`.
`field` is the index into the `fields` the call carried. `problem` is a word from a closed
vocabulary: `option_missing`, `option_ambiguous`, `option_disabled`, `multiple_values`,
`not_boolean`, `format`, `wrong_tool`, `reverted`, `submit_not_owned`. The Rust enum
`FieldProblemKind` in `language/field_problem.rs` owns the words, and a drift test checks that the
words `content.js` names are among them. `options` lists what would succeed (25 entries of 60
characters, with `options_total` when cut, a group as `{group, options}`, a disabled option as
`{text, disabled:true}`); for a select too long to list, `containing` holds the enabled options
whose text contains the refused value. A `format` problem carries `type` (the input type),
`expected` (the shape the type takes) and `limits` (the declared `min`, `max` and `step`); the page
assigns the value to a detached input of that type with those bounds and refuses a value that does
not read back as given or that falls outside the range. A range compares numerically, since it
writes a number back in its own form, and always reports the `min`, `max` and `step` in force (0,
100 and 1 when it declares none), because it clamps and snaps whatever it declares. A `wrong_tool`
problem always carries `route`, a word from the closed list `file`, `button` and `widget`: a file
input, a `<button>` element or a submit, reset or image input, and a non-form element with the
role `combobox` or `listbox`. The page finds the route from the element, never from the role the
control reads as, and finds it before the control is checked for visibility. The service refuses
to decode a `wrong_tool` problem without a known route. A credential-class control lists none of
these. The page
never echoes the value the caller gave. The serialized detail stays within 4 KB: options are
dropped first, then `containing`. A problem that loses its options this way gains `options_total`
with the number it held, so the service never reads a dropped list as an empty select.

**Fill echo and reverted fields.** `verify_fill_retained` compares each field with what was given and
answers `{retained:true, fields:[{field, now?, tolerated?, matched?, picked?, group?}]}` once every field has held its value for
`stable_ms` (ADR-0198 Decision 11). `fields` lists only the entries worth reading back: `now` is the
page's value (`outwardValue`, bounded to 80 characters; a checkbox or radio reads `checked` or
`unchecked`; a select reads its chosen option text, the empty placeholder included unless the fill just picked it, which reads empty) for a select, a
radio, a checkbox and the `date`, `time`, `datetime-local`, `month`, `week`, `number`, `range` and
`color` inputs, and for a text field whose value differs from the given one only in formatting, which
also sets `tolerated:true`. A choice that matched only after normalizing sets `matched:"normalized"`.
A radio picked through the label of a sibling keeps its own reading in `now` (it is no longer the checked
radio) and adds `picked`, the sibling's label, and `group`, the group name when it has one. The page
checks that the sibling can be acted on, scrolls to it, focuses it and sends it `input` and `change`.
Formatting is white space, `(`, `)`, `-` and `/`, compared case-folded on
both sides; `.` and `,` are significant. A credential-class control is never listed. When `limit_ms`
passes with changed fields, the call fails with `not_retained` and the detail
`{problems:[{field, problem:"reverted", now?}]}`, one entry for each changed field. `now` is left out
for a credential-class control and for a field whose option the page disabled. A `not_retained` with no
detail means no field could be named, and any other failure of the call is a real failure. The service
reports the first as a partly applied fill and the others as an unknown effect, and does not rewrite
one into the other.

A field `value` is a string, a boolean (a checkbox or radio takes it as such) or, for a multiple select,
an array of strings. The page resolves it with one routine (`resolveChoice`) for the check, the edit and
the retained check. A problem for an entry of an array carries `value_index`, the entry's position.

The code stays `option_missing` when every problem is an option problem and `not_fillable`
otherwise, and the message keeps the prefix of its code (`select option not found`, `target is not
fillable`), so a service that predates the detail still reads the failure. A lone problem may add its
own words in parentheses after the prefix, and several problems add the count (`target is not fillable
(2 fields refused)`). A service that does not know a
problem word treats the whole detail as absent. Only `prepareFill` and `verify_fill_retained`
attach a detail: the single-field paths (`fill_local`, `focus_text_field`) run after an edit may
have started and throw the code and message alone.

`AssetErrorCode` is closed and owned by `world.rs`:
- `stale_locator`, `not_visible`, `disabled`, `read_only`, `not_editable`, `not_fillable`, `option_missing`;
- `credential_authorization_required`, `submit_outside_form`, `not_file_input`, `no_focused_control`,
  `focus_lost`, `outside_viewport`, `no_subject`;
- `unknown_kind`, `invalid_request`, `runtime_missing` (produced by the extension's CALL shim);
- `cancelled`, `unsatisfied`, `not_retained`, `payload_mismatch`, `page_exception`.

The **pre-effect codes** are every code above except `cancelled`, `not_retained` and `page_exception`. An
effect kind throws a pre-effect code only before its first mutation. The C3 asset tests enforce that
invariant.

### 3.4 Level-4 `content.js`

The level-4 `content.js` is a copy of today's `content.js` with these edits. The level-3 copy is frozen.

- **Dispatcher.** `window.__ghostlight_dispatch__` (`:760`) becomes the closure `dispatch(message, ctx)`.
  The page cannot see, call or replace it.
- **Coded errors.** Each throw site keeps today's message text, which reaches the model, and gains a code.
  - `resolve`: `stale_locator`.
  - `requireActionable` (`:636-658`): `not_visible`, `disabled`, `read_only`.
  - `credentialAuthorizationError`: `credential_authorization_required`.
  - `validateFillElement` and `prepareFill`: `not_fillable`, `option_missing`, with the optional
    detail above.
  - `validateTextEditElement`: `not_editable`.
  - `requireUploadTarget`: `not_file_input`, `disabled`.
  - `prepareFill`: `submit_outside_form`.
  - `describe_focused` and `verify_focused_text`: `no_focused_control`.
  - `verifyBrowserTextFocus`: `focus_lost`.
  - `viewport_point`: `outside_viewport`.
  - `hit_point`: `no_subject`.
  - `decodeFile`: `payload_mismatch`.
  - unknown kind: `unknown_kind`.
- **Locator epochs.** Locators are `locator_${EPOCH}_${n}`. `resolve` rejects a foreign epoch as
  `stale_locator`, so a rebuilt world never aliases an old handle to a new element.
- **Cancel tokens.** Every page loop checks `ctx.signal.cancelled` and throws `cancelled`:
  - `observe` (`:708-757`);
  - `prepareStableFill` (`:581-603`);
  - `verify_fill_retained`;
  - `sensor.settle` and `settleVisual`, through a new `options.cancelled` callback.
- **Tail clicks.** `submit_fill` runs `setTimeout(() => submitElement.click(), 0)` after returning
  `{submitted:true}`, so a dialog cannot block `callFunctionOn`. Today this is `:802`, which runs after
  `sendResponse`.
- **Form diagnostics removed.**
  - The `chrome.runtime` emit and state request (`:29-44`) go.
  - The `formDiagnostics.run(...)` wrappers go; their functions are called directly.
  - The `form_diagnostics_state` branch (`:761-766`) goes.
  - There is no `drain()` and no `form_diagnostics` local fact.
- **Dispose hooks:**
  - remove the presentation host (`presentation.js:125-164`);
  - stop sensor timers, observers and rAF loops;
  - clear `locators`.
- **No browser APIs.** A test forbids `chrome.`, `browser.`, `fetch(`, `XMLHttpRequest`, `WebSocket`,
  `sendMessage` and `__ghostlight_dispatch__` in every level-4 source.

### 3.5 World message kinds at level 4

Every kind is built only by a typed constructor in `world.rs`.
- **Scope class.** *Content* needs the document in `allowed`. *Metadata* needs it in the admitted tree.
- **Effect class.** *Read*, *Prepare* (positions things but is not the action), or *Effect*.
- **Settle.** *Settle* kinds receive the scope's remaining preparation budget as `settle_ms` and
  `visual_settle`. `visual_settle` is the agent's intent and is false only when the agent turned
  settling off; `settle_ms` is what is left of the budget. A spent budget is zero with
  `visual_settle` still true (ADR-0198 Decision 35).

| Kind | Scope | Effect | Settle | Notes |
|---|---|---|---|---|
| `read_text{locator?, mode?, max_chars}` | Content | Read | yes | today |
| `inspect{inspect_kind, max_items}` | Content | Read | yes | today |
| `inspect_tree{locator?, max_depth, max_nodes}` | Content | Read | yes | today |
| `find{text, find_kind, max_results}` | Content | Read | yes | today |
| `query_semantic{name, role?, exact, form_scope}` | Content | Read | yes | today |
| `describe{locators}` | Content | Read | | today |
| `describe_focused` | Content | Read | | today |
| `verify_focused_text{allow_credentials}` | Content | Read | | today |
| `box{locator}` | Content | Read | | today |
| `prepare_fill{fields, allow_credentials, submit_locator?}` | Content | Read | yes | carries `settle_ms` and `visual_settle` like the other settling calls: false skips the stability wait, a spent budget with true rechecks values once for 50 ms, otherwise it waits for stable values up to `settle_ms`; cancellable |
| `verify_fill_retained{fields, allow_credentials, stable_ms:250, poll_ms:25, limit_ms}` | Content | Read | | **new**; ok `{retained, fields:[{field, now?, tolerated?, matched?, picked?, group?}]}` when stable, else `not_retained` with a `reverted` detail; replaces the worker loop `service-worker.js:1700-1728` |
| `observe{condition, value?, locator?, timeout_ms, until_satisfied}` | Content | Read | | changed: with `until_satisfied` it throws `unsatisfied` at its timeout (race mode) |
| `activate{locator}` | Content | Prepare | | today: scroll into view, rect and subject |
| `hover{locator}` | Content | Prepare | | today |
| `focus{locator}` | Content | Prepare | | today |
| `scroll_point{x, y}` | Content | Prepare | | today |
| `viewport_point{x, y}`, `scroll_offset` | Metadata | Read | | today |
| `hit_point{x, y, page}` | Metadata | Read | | **new**: `{x, y, embed, subject}`; replaces `document_route` point mode and `point_context` |
| `owner_box` (owner = the iframe element) | Metadata | Read | | **new**: content-box origin and size (the `frame_boxes` arithmetic, `content.js:911-922`) |
| `owner_probe{x, y}` (owner) | Metadata | Read | | **new**: `{hit: deepestElementFromPoint(x, y) === owner, left, top}` |
| `focus_probe` | Metadata | Read | | **new**: `document_route{focused}` |
| `require_focused` | Metadata | Read | | **new**: throws `focus_lost` unless this document has focus on a non-frame element |
| `fill_local{field, allow_credentials}` | Content | Effect | | today |
| `focus_text_field{field, allow_credentials}` | Content | Effect | yes | **new**: `prepare_text_fill` plus `verify_text_fill_focus` |
| `submit_fill{fields, submit_locator, allow_credentials}` | Content | Effect | | changed: tail click via `setTimeout` |
| `type_text{locator, text, clear_first, allow_credentials}` | Content | Effect | | today |
| `clear_focused{allow_credentials}` | Content | Effect | | today |
| `scroll{locator?, direction?, amount?}` | Content | Effect | | today |
| `upload_files{locator, files}` | Content | Effect | | today |
| `drop_files{x, y, files}` | Content | Effect | | today |
| `attach_file{locator, file_name}` | Content | Effect | | **new**: the file bytes come from `ctx.attachment` (recording export) |

**Observed targets (ADR-0198 decision 7).** `inspect`, `find`, `query_semantic`, `describe` and
`describe_focused` answer `{targets, truncated}`. Each target always has `locator`, `role`, `name`,
`state` and `credential_class`. A control adds these keys only when they apply:

| Key | Holds |
|---|---|
| `value` | What a text field, textarea, native select, rich editor, range or number holds: at most 80 characters, with `value_chars` when longer. A multiple select gives an array of at most 25 chosen texts. Empty when nothing is chosen or the empty placeholder option is. |
| `options`, `options_total` | A native select only: at most 25 entries of 60 characters in document order, with `options_total` when cut. An entry is the bare text, `{text, value}` when the text is not unique, `{text, disabled:true}` when the option or its optgroup is disabled, or `{group, options}` for an optgroup. The placeholder is the first option when its value is empty; it is left out. Any other option with an empty value is a real choice. |
| `type` | `date`, `time`, `datetime-local`, `month`, `week`, `number`, `range`, `color`, `file`, `email`, `url` or `tel`. |
| `limits` | Only the declared attributes among `min`, `max`, `step`, `minlength`, `maxlength`, `pattern` and `accept`, as strings, and `multiple:true`. |
| `hint` | A placeholder that differs from the name, else the `aria-describedby` text, else the select's placeholder option text. At most 120 characters. |
| `group` | For a radio or checkbox, the fieldset legend or the nearest `radiogroup` or `group` name. At most 80 characters. |
| `error` | Present when `:user-invalid` matches or `aria-invalid` is `true`: the `aria-errormessage` text, else a Ghostlight phrase from the validity flags, else `is invalid`. The browser's own message is read only for a custom error. At most 120 characters. |

Any target, control or not, also carries these structure keys when they apply (ADR-0198 decision 19):

| Key | Holds |
|---|---|
| `area` | The landmark the target sits in: `header`, `nav`, `aside`, `footer`, `dialog` or `search`, absent for page content. The page finds it by walking the composed tree to the nearest landmark element or ARIA landmark role. A `header` or `footer` element counts only when no `article`, `aside`, `main`, `nav`, `section` or dialog (the `dialog` element, or role `dialog` or `alertdialog`) contains it; inside a dialog it reports `dialog`. The service drops any other word. |
| `form` | The 1-based ordinal of the form that owns the target, counted over the forms of its own document in composed order. The owner is the browser's `form` property, which honors the `form` attribute, else the nearest form around the target in the composed tree, which reaches a control in a shadow root. A form is its own owner. The service renumbers ordinals across documents so one number names one form in a merged answer. |

The `inspect`, `find` and `query_semantic` answers also carry `forms`, the number of forms the
document holds, whatever the answer kept. The service offsets each document's ordinals by the
`forms` of the documents before it, so a form's number is page-wide and does not change with what
an answer kept.

Matches from shadow roots come out in composed tree order, where the host sits, before any cap
cuts the list.

**Cutting by tier (ADR-0198 decision 20).** When `inspect` or `find` has more targets than its
cap, the page cuts by tier and keeps document order. The tiers, cut first to last: `hidden` (not
composed-visible), `chrome` (area `header`, `nav` or `footer`), `aside`, and `content` (everything
else, including `dialog` and `search`). Within a tier the earlier targets stay. The answer adds
`left_out`, an object of counts keyed by those four words, holding only keys with a count, and
`truncated` is true exactly when the cap cut something. `inspect` also drops, before the cut,
every actionable target that sits under a composed `[inert]` ancestor or outside every open
`dialog:modal`, counts them as `behind_dialog`, and does not set `truncated` for them. The answer
carries `modal_open`, true when the document has an open modal dialog. Structure targets are not
dropped. The service reads `left_out` against the closed key list, drops other words, and when the
documents together hold more than the cap it cuts again by the same tiers, derived from each
target's `hidden` state word and `area`, and adds its own counts. `truncated` and the service's
own cut make the read's coverage report its size limit; `behind_dialog` does not.

State words gain `required`, `readonly` and `invalid`. A credential-class control carries `value`
(`[hidden]` when set, empty when empty) and none of the other keys. The service bounds every key
again and drops what it does not know. `read_text` with a `locator` that names a native select
ends its text with a line `Options: ` and every option in order, bounded only by `max_chars`.

Removed:
- `fill`, `prepare_text_fill`, `verify_text_fill_focus`, `verify_fill_values`;
- `frame_boxes`, `point_context`, `document_route`;
- `drag_observation_arm`, `drag_observation_status`, `drag_observation_finish`;
- `capture_mask*`, `form_diagnostics_state`;
- `presentation_visibility`, `managed_scope`, `recording_state`, `runtime_state` and `present`. These are now
  the AMBIENT and PRESENT shims.

---

## 4. Service-side translation

### 4.1 Layout

```
crates/orchestrator/src/browser/
  mod.rs                    port, RelayBrowserPort; Connection gains `level: Level`; BrowserSummary gains `protocol`
  adapter/
    mod.rs                  attach(): first-frame peek, Level::{Three, Four}
    chromium/
      level3/               today's path (moved by M2 when it lands); assets/ = frozen bundle (C1)
      level4/
        mod.rs     ~260     run(command, &mut Link) -> Result<BrowserOutcome, BrowserError>; exhaustive match
        wire.rs    ~650     section 1.4
        connection.rs ~300  level-4 attach, reader thread, heartbeat, pending map, chunked writes
        link.rs    ~350     one command over one connection: send, cancel frame, grace, deadline, effect ledger, failure mapping
        assets.rs  ~80      assets() -> &'static Assets { id, source }
        world.rs   ~420     typed constructors; AssetErrorCode; ScopeClass; EffectClass; envelope decoding
        tree.rs    ~250     FrameTree from FrameInventory or DocumentScope; "{frame}.{loader}" ids; DFS; ancestors; chain()
        locator.rs ~80      "{frame_id}:{loader_id}:{local}" codec
        scope.rs   ~320     Scope: admit, call, guard, input subjects, settle budget, observation
        merge.rs   ~200     mergeTargets, mergeTextSections, inspect-tree append and trim (ports of frames.js)
        keys.rs    ~220     keyDescriptor, modifierMask, press and replace-text steps, click steps, drag path (ports of shared.js)
        geometry.rs ~300    owner offsets, route reducer, view tolerance (1 px, 0.02, 0.001), capture budgets, viewport mapping
        translate/{tabs,read,observe,pointer,keyboard,forms,capture,records,present}.rs  ~1,500
        golden.rs           #[cfg(test)] shared fixtures
        fake.rs    ~550     #[cfg(test)] FakeExtension
        assets/             world.js page-shared.js sensor.js presentation-css.js presentation.js content.js
```

The following do not change: `work/`, `RequestScope::from`, `dispatch_documents`, `bind_command`,
`FakeBrowser`. `BrowserCommand` stays the internal work vocabulary (S6: `type Work = BrowserCommand` until
level 3 retires).

Three internal additions are needed. Each is `#[serde(skip)]`, so the level-3 wire stays byte-identical, and
C6 adds a byte golden for it:
- `BrowserCommand::OpenTab.adopt_tab_id: Option<u64>`, filled by `work/` from the released-tab registry of
  ADR-0196 D9. If step 1 has no registry yet, it is `None`.
- `BrowserCommand::OpenTab.focus_window: bool`, filled by `work/` when the workspace has no tab.
- `BrowserOutcome::Dialog.message: Option<String>`, with `#[serde(default, skip_serializing_if)]`. Level-3
  receipts decode with `None`.

### 4.2 `call_inner` and the Link

```rust
// browser/mod.rs::call_inner, after the existing cancel, deadline and liveness checks:
match connection.level {
    Level::Three => { /* today's body, :741-853, unchanged: capability gate, probe, await_receipt */ }
    Level::Four => {
        let mut link = Link::new(&connection.level4, browser, workspace, dispatch);
        level4::run(command, &mut link)
    }
}

// level4/mod.rs
pub fn run(command: BrowserCommand, l: &mut Link) -> Result<BrowserOutcome, BrowserError> {
    match command {
        BrowserCommand::InDocuments { scope, primitive } => {
            if matches!(*primitive, BrowserCommand::InDocuments { .. } | BrowserCommand::DescribeDocuments { .. }) {
                return Err(BrowserError::Protocol("nested document execution is invalid".into()));
            }
            let mut s = Scope::admit(&scope)?;
            let result = translate(*primitive, l, Some(&mut s))?;
            Ok(BrowserOutcome::InDocuments { observation: s.observation(&result), result: Box::new(result) })
        }
        BrowserCommand::InstallPageRuntime { .. } => Err(BrowserError::Protocol("page runtime installs are level 3 only".into())),
        other => translate(other, l, None),
    }
}
```

Link state:
- `effect_started: bool`;
- `committed: Vec<String>`: the union of receipt `committed_urls`, in order, de-duplicated;
- `last_tab: Option<Tab>`;
- the `BrowserDispatch` (deadline, cancelled flag, `admit`);
- the per-connection `RecordingIndex` (workspace to its one recording id), learned from `RecordingFacts::Started`.

`Link::tab(tab_id)` returns `last_tab` when its id matches. Otherwise it sends one
`list_tabs{tab_ids:[tab_id]}`.

```rust
const CANCEL_GRACE: Duration = Duration::from_millis(500);
const RECEIPT_MARGIN: Duration = Duration::from_millis(250);   // primitive timeouts end before the service deadline

impl Link<'_> {
    pub fn send<P: Spec>(&mut self, p: P) -> Result<P::Outcome, BrowserError> {
        let effectful = p.carries_effect();                     // Spec::CLASS, or any Effect-class world call in a Call
        self.before(effectful)?;                                 // cancel; deadline; admit() when effectful (Pause between fill fields)
        let correlation = format!("p4_{}", Uuid::new_v4().simple());
        let rx = self.conn.register(&correlation);
        if self.conn.write_request(&correlation, p.into_primitive()).is_err() {   // direct <= 768 KiB, else command_chunk (8 MiB)
            self.conn.forget(&correlation);
            return Err(self.lost(effectful));
        }
        loop {
            if self.cancel_requested() {
                self.conn.write(Outbound::Cancel { correlation: correlation.clone() });
                let until = (Instant::now() + CANCEL_GRACE).min(self.deadline());
                return match rx.recv_until(until) {
                    Ok(receipt) => { let facts = self.absorb(&receipt, effectful); Err(self.cancelled(facts)) }
                    Err(_) => { self.conn.forget(&correlation); Err(self.cancelled(effectful)) }
                };
            }
            if Instant::now() >= self.deadline() {
                self.conn.write(Outbound::Cancel { correlation: correlation.clone() });
                self.conn.forget(&correlation);
                return Err(self.expired(effectful));
            }
            match rx.recv_timeout(Duration::from_millis(20).min(self.remaining())) {
                Ok(Ok(receipt)) => return self.finish::<P>(receipt, effectful),
                Ok(Err(error)) => return Err(error),             // socket closed: DisconnectedAfterDispatch
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Err(self.lost(effectful)),
            }
        }
    }
    fn finish<P: Spec>(&mut self, r: Receipt, effectful: bool) -> Result<P::Outcome, BrowserError> {
        let dispatched_effect = self.absorb(&r, effectful);     // unions commits and tab; sets effect_started
        match (r.ok, r.failure) {
            (Some(ok), None) => serde_json::from_value(ok).map_err(|e| BrowserError::Protocol(e.to_string())),
            (None, Some(code)) => Err(self.failure(code, r.message.unwrap_or_default(), dispatched_effect)),
            _ => Err(BrowserError::Protocol("receipt must carry exactly one of ok and failure".into())),
        }
    }
}
```

Section 5 gives the mapping tables.

**Level-4 connection.**
- The reader thread decodes `Inbound`:
  - a receipt goes to its pending sender, and an unknown correlation is ignored;
  - an event is converted into a `BrowserEvent` and given to the sink; `Attended` updates attention as today;
  - a heartbeat ack updates liveness;
  - `Unknown` frames are ignored;
  - a receipt that fails typed decoding is delivered as `Err(Protocol)` to its pending call.
- Only a closed socket fails other pending calls (S6). There is no per-request probe and no quarantine.
- The heartbeat thread is today's, with its 20 s interval and 45 s timeout. Staleness hides the browser from
  `browsers()` but does not fail pending calls.
- Control state is published by `broadcast_control`. The frame is `Outbound::ControlState` at level 4 and
  today's `BrowserFrame::ControlState` at level 3.

### 4.3 Translation of all 52 `BrowserCommand` variants

Abbreviations:
- **T** is the top document. **F** is the locator's document. **A** is the allowed content documents in DFS
  order.
- **G**: the scope guard rides on that primitive.
- **chain(F)** is the `owner_box` calls with `owner_of` from F up to T. They are metadata calls inside the
  same Sequence.
- **route(p)**: section 4.4; one round trip per nesting level.
- **tree** comes from the scope (no round trip) or from one `frames` call when F is not T.

| # | Command | Primitives, in order | RT |
|---|---|---|---|
| 1 | DescribeDocuments | if `settle_ms > 0`: `call(T, observe visual_settle <= 1000)`, errors ignored; then `frames`; then bind locators locally (current iff frame and loader are in the inventory); then route(p) per point, first hop in page coordinates; then, if focused: `call Parallel(focus_probe over supported docs)` | 1-4+ |
| 2 | InstallPageRuntime | never at level 4; `Protocol` | 0 |
| 3 | InDocuments | `Scope::admit`; the inner command runs with the scope; returns `InDocuments{observation, result}`; nesting is `Protocol` | +0 |
| 4 | ListTabs | `list_tabs{}` (all tabs; `tab_ids` is reserved for when Work carries ids) | 1 |
| 5 | FocusTab | `focus_tab` | 1 |
| 6 | OpenTab | `open_tab{url, group_key: "g_" + hex(sha256(group_title))[..16], group_title, adopt_tab_id, focus_window, ready_timeout_ms: 8000}`; `reused = adopted` | 1 |
| 7 | Navigate | `navigate{to:url}` | 1 |
| 8 | TraverseHistory | `navigate{to: back or forward}`; any other direction is a plain failure, nothing sent | 1 |
| 9 | Reload | `navigate{to: reload{bypass_cache}}` | 1 |
| 10 | CloseTab | `close_tab{released}`; `local_interlock` becomes `LocalInterlock` | 1 |
| 11 | NavigateDiscardingBeforeUnload | `navigate{to:url, discard_beforeunload:true}` | 1 |
| 12 | ReadText | `call(F or T, read_text)` G | 1 |
| 13 | Inspect | `call Parallel(A, inspect)` G; rescope locators; merge DFS under `max_items`; `limited` when over | 1 |
| 14 | ReadDocument | section 4.6 | 1-3 |
| 15 | InspectTree | section 4.6 | 1-2 |
| 16 | Find | section 4.6 | 1 |
| 17 | Screenshot | section 4.6 | 1-3 |
| 18 | ScreenshotRegion | [settle call(T)]; `capture{page_rect, magnify}`; compare `view` with `expected_viewport` (tolerances 1 px, 0.02, 0.001); a mismatch is "stale screenshot view" and the image is discarded | 1-2 |
| 19 | DescribeTargets | `call Parallel(describe per document group)` G; rescope locators | 1 |
| 20 | QuerySemantic | as 13, with cap 8 | 1 |
| 21, 23 | Activate, ActivateModified | section 4.6 | 2-3+depth |
| 22, 24 | ActivatePoint, ActivatePointModified | `view` (validate before any effect); `call(T, scroll_point)`; route(viewport point); scoped: the subject must be allowed; `input{click steps, settle 250}` G | 3+depth |
| 25 | WheelAt | as 22; `input{wheel x ticks, deltaY +-120, settle 120}` | 3+depth |
| 26 | Scroll | `call Sequence[scroll(F or T), scroll_offset(T) when F is not T]` G | 1 |
| 27 | SetZoom | `set_zoom` | 1 |
| 28 | ResizeWindow | `resize_window` | 1 |
| 29 | Hover | [tree]; `call Sequence[hover(F), chain(F)]` G; scoped and restricted: route; `input{moved}` G | 2-3 |
| 30 | HoverPoint | `view`; `call(T, scroll_point)`; route; `input{moved}` G | 3+depth |
| 31 | Fill | section 4.6 | 2+T+L |
| 32 | TypeText | `call(F, type_text)` G | 1 |
| 33 | DescribeFocused | `call Parallel(describe_focused over subject docs, or all supported)` G; the first ok in DFS order | 1 |
| 34 | TypeFocused | `call Parallel(verify_focused_text)`; D = the first ok in DFS; if clear_first: `call Sequence[clear_focused(D), verify_focused_text(D)]`; `input{precondition: verify_focused_text(D), insert_text}` G | 2-3 |
| 35 | PressKey | `input{precondition: locator ? focus(F) : scoped ? require_focused(subject) : none, key down, key up}` G; text is dropped when Alt, Control or Meta is held (`service-worker.js:2313-2322`) | 1 |
| 36 | Drag | section 4.6 | 2-3 |
| 37 | DragPoints | `view`; `call Sequence[viewport_point(T, start), viewport_point(T, end)]`; route both (subjects); `input{drag path 12}` G | 3+ |
| 38 | UploadFiles | `call(F, upload_files)` G; over 768 KiB uses `command_chunk` | 1 |
| 39 | DropImageAt | `view`; `call(T, scroll_point)`; route; `call(deepest doc, drop_files{local x, y})` G | 3+depth |
| 40 | EvaluateScript | `evaluate_main{settle 100}` G; `exception` becomes `EffectUnknown(description)` (today's semantics) | 1 |
| 41 | Observe | section 4.6 | 1-2 |
| 42 | InspectDialog | `dialog{status}` becomes `Dialog{present, dialog_type, message, url}`; no inventory; `url` is authorized | 1 |
| 43 | HandleDialog | `dialog{status}`, authorize `url`, then `dialog{handle}` becomes `DialogHandled`, or `DialogAbsent` when no dialog was showing or `handled:false`; no inventory | 1-2 |
| 44 | ReadDiagnostics | `diagnostics{read{binding: hex(sha256(sorted scope ids)), guard}}` | 1 |
| 45 | ClearDiagnostics | `diagnostics{clear}` | 1 |
| 46 | StartRecording | a held recording is checked with `recording{status}` first (still capturing the same tab: answer it; gone: forget it; stopped and already saved: `recording{discard}` then go ahead; otherwise `RecordingHeld`); then `recording{start{tab_id, watch: scope.watch_changes then the real scope docs as GuardDoc}}`; record the id in RecordingIndex | 1-2 |
| 47-50 | Status, Stop, Export, Discard Recording | take the workspace's id from RecordingIndex (none: `RecordingNotFound`), then `recording{op}`; a Target export becomes `page{tab_id, call: attach_file at F (Scope::call), guard}` | 0-1 |
| 51 | Cancel | write the `cancel` frame for that correlation; `Cancelled` | 0 |
| 52 | Present | section 4.7; write `notice`; `Presented{rendered:false}` at once | 0 |

**Outcome mapping.**
- `Tab` converts to `PhysicalTab` (dropping `opener_tab_id`).
- `TabOpened` becomes `TabOpened{tab, committed_urls: l.committed(), reused: adopted}`.
- Effect commands take `committed_urls` from the Link.
- `Captured` becomes `Screenshot{viewport: geometry::viewport(scope, clip, view, output_scale)}`.
- Recordings and diagnostics map field for field.

### 4.4 Coordinates, frame offsets and point routing

- **Page rectangles** are each document's own viewport CSS pixels (`getBoundingClientRect`).
- **offset(F)** is the sum of `owner_box` origins for every frame f from F up to T, exclusive. Each one is read
  in parent(f) with `owner_of: f`.
  - The owner is exact (`DOM.getFrameOwner`), so identical embeds no longer cause a refusal. Today they do
    (`service-worker.js:1374-1376`).
  - CSS transforms on iframes stay unsupported, as today.
- **Input coordinates** are top-viewport CSS pixels sent to the root session, unchanged.
- **`route(point)`** descends at most 8 levels, one round trip per level, from D = T:
  1. Send `call Parallel[hit_point(D, local, page = first hop and caller gave page coordinates),
     owner_probe(D, owner_of C, local) for each child C of D in the tree]`.
  2. If `hit_point.embed` is false, D is the subject document, and `hit_point.subject` is the subject.
  3. Otherwise exactly one probe must report `hit`. Zero or several leave the point unresolved. An unseen
     OOPIF ends here too, which is conservative.
  4. Set `local -= probe origin` and continue in that child.
- **`Scope::needs_point_check()`** is false when every tree document is supported and allowed (all-open).
  Every point then lands in an allowed document, so the route round trips are skipped. All-open is not taxed
  by governance.
- **View validation** compares `view` facts with `expected_viewport` using today's tolerances
  (`service-worker.js:2188-2195`). It reads the same source capture used: CDP metrics, the isolated-world DPR
  and `tabs.getZoom`.

### 4.5 Scope enforcement with `DocumentGuard`

Authority stays in `work/documents.rs:221-435`, unchanged. Level 4 replaces only what `documents.js` did in the
extension.

```rust
pub struct Scope {
    tree: FrameTree,                  // from scope.documents: "{frame}.{loader}" ids, plus "unseen.{n}" placeholders
    allowed: BTreeSet<FrameRef>,
    subjects: Vec<FrameRef>,
    input_subjects: Vec<FrameRef>,
    strict: bool,
    settle_until: Instant,
    settle_requested: bool,           // the agent's intent; false only for an explicit visual_settle false
    observation: Observation,         // visited, unavailable, limited
}

impl Scope {
    pub fn admit(scope: &DocumentScope) -> Result<Self, BrowserError> {
        let tree = FrameTree::from_documents(&scope.documents)?;            // bad id or graph: DocumentUnavailable
        let allowed = scope.allowed.iter()
            .map(|id| tree.supported_ref(id).ok_or(BrowserError::DocumentUnavailable))   // placeholders are never targetable
            .collect::<Result<_, _>>()?;
        let subjects = scope.subjects.iter().filter_map(|id| tree.real_ref(id)).collect();
        Ok(Self { tree, allowed, subjects, input_subjects: vec![], strict: scope.strict_tree,
                  settle_until: Instant::now() + Duration::from_millis(scope.settle_ms.min(1_000)),
                  settle_requested: scope.settle_requested,
                  observation: Observation::default() })
    }

    /// Every world call goes through here: nothing reaches a document the authority did not admit.
    pub fn call(&self, doc: &FrameRef, message: world::Message) -> Result<PlannedCall, BrowserError> {
        let admitted = match message.scope_class() {
            ScopeClass::Content => self.allowed.contains(doc),
            ScopeClass::Metadata => self.tree.contains(doc),
        };
        if !admitted { return Err(BrowserError::DocumentUnavailable); }      // nothing sent
        let message = if message.takes_settle() { message.with_settle(self.remaining_settle_ms(), self.settle_requested) } else { message };
        Ok(PlannedCall { call: WorldCall { frame: Some(doc.clone()), owner_of: None, message: message.to_json() },
                         doc: doc.clone(), scope: message.scope_class(), effect: message.effect_class() })
    }

    /// The guard rides on the first primitive of a command and on every input, evaluate_main,
    /// diagnostics read and recording page export.
    pub fn guard(&self) -> DocumentGuard {
        if self.strict {
            DocumentGuard { documents: self.tree.real_documents().map(GuardDoc::from).collect(), exact: true,
                            unseen: self.tree.placeholder_count() }
        } else {   // subjects + top + input subjects + their ancestors (documents.js:31-42)
            DocumentGuard { documents: self.tree.closed_under_ancestors(self.required()).map(GuardDoc::from).collect(),
                            exact: false, unseen: 0 }
        }
    }

    /// Point input: one route before the input; the subject must be allowed.
    pub fn admit_input_subject(&mut self, subject: &FrameRef) -> Result<(), BrowserError> {
        if !self.allowed.contains(subject) { return Err(BrowserError::DocumentUnavailable); }
        self.input_subjects.push(subject.clone());
        Ok(())
    }

    pub fn record(&mut self, planned: &PlannedCall, answer: &CallAnswer) {
        if planned.scope != ScopeClass::Content { return; }
        match answer {
            CallAnswer::Skipped => {}                                          // race losers and skipped steps record nothing
            a if a.ok() || a.error_code() == Some(AssetErrorCode::Unsatisfied) => { self.observation.visited(&planned.doc); }
            _ => { self.observation.unavailable(&planned.doc); }               // as documents.js:150
        }
        if answer.value_flag("truncated") { self.observation.limited = true; }
    }

    pub fn observation(&self, result: &BrowserOutcome) -> DocumentObservation {
        DocumentObservation { visited: self.observation.visited_ids(), unavailable: self.observation.unavailable_ids(),
                              limited_by_size: self.observation.limited || truncated(result) }
    }
}
```

Rules:
- Every `Link::call` made with a scope records its answers through `Scope::record`.
- An extension `document_changed`, from the guard or from a call failure with `started:false`, becomes
  `BrowserError::DocumentUnavailable` when nothing effectful was dispatched, and `EffectUnknown` otherwise.
  This matches today (`documents.js:126`).
- Unbounded commands (drag, script, diagnostics, recording) need no per-packet check. Admission
  already refused any denied document (`work/documents.rs:333-358`). Under `strict_tree`, the exact guard pins
  the whole tree.
- Key input: the precondition (`require_focused`, `verify_focused_text`, `focus_text_field` or `focus`) runs
  in the extension immediately before the first key event, in the subject document.

### 4.6 Composite commands (pseudo-Rust)

Helpers:
- `scoped(s)` returns `s.guard()` when a scope is present, else `None`.
- `plan(s, doc, msg)` is `Scope::call`, or an unscoped `PlannedCall` when there is no scope.
- `Link::call(tab, mode, planned, guard, settle_ms, budget, scope)` sends one `call` primitive and returns
  `Answers`. It sets `effect_started` per Effect-class call with `dispatched_effect()` (section 5).

```rust
fn fill(l: &mut Link, s: &mut Scope, tab: u64, fields: &[PhysicalField], submit: Option<&str>,
        timeout_ms: u64, creds: bool) -> Result<BrowserOutcome, BrowserError> {
    let budget = l.deadline().min(Instant::now() + Duration::from_millis(timeout_ms));
    let groups = forms::group_by_document(fields)?;          // first-appearance order; "field target is not frame-scoped"
    let submit = submit.map(Locator::decode).transpose()?;   // malformed: "stale browser locator"
    if submit.as_ref().is_some_and(|x| !groups.iter().any(|g| g.doc == x.doc)) {
        return Err(BrowserError::Primitive("submit control is not contained in the resolved form".into()));
    }
    // RT 1: every document validates (and waits for a stable form) before any edit. Parallel:
    // each call carries the same remaining settle budget, so their waits overlap inside it.
    let prepare = groups.iter().map(|g| plan(s, &g.doc, world::prepare_fill(&g.fields, creds,
        submit.as_ref().filter(|x| x.doc == g.doc).map(|x| &x.local)))).collect::<Result<Vec<_>, _>>()?;
    let plans = l.call(tab, CallMode::Parallel, prepare, Some(s.guard()), 0, budget, Some(s))?;
    let kinds = forms::field_kinds(&plans, &groups)?;         // "content primitive returned an invalid fill plan"
    let mut filled = 0;
    for (g, kinds) in groups.iter().zip(kinds) {
        let mut run = Vec::new();                             // consecutive local fields share one Sequence call
        for (field, kind) in g.fields.iter().zip(kinds) {
            l.require_budget(budget)?;                        // after an effect: EffectUnknown("form fill exhausted its physical execution budget")
            match kind {
                FieldKind::Local => run.push(plan(s, &g.doc, world::fill_local(field, creds))?),
                FieldKind::BrowserText => {
                    l.flush(tab, &mut run, s, budget)?;
                    let done = l.input(tab, Some(plan(s, &g.doc, world::focus_text_field(field, creds))?.call),
                                       keys::replace_text(&field.value), Some(s.guard()), 0, budget)?;
                    done.require_precondition()?;             // refused focus: plain failure while nothing was dispatched
                    done.require_complete()?;                 // a dialog mid-typing: EffectUnknown
                }
            }
            filled += 1;
        }
        l.flush(tab, &mut run, s, budget)?;
    }
    let limit = FILL_RETAINED_LIMIT_MS.min(remaining_ms(budget));
    let verify = groups.iter().map(|g| plan(s, &g.doc, world::verify_fill_retained(&g.fields, creds, limit)))
                       .collect::<Result<Vec<_>, _>>()?;
    let retained = l.call(tab, CallMode::Parallel, verify, None, if submit.is_none() { 25 } else { 0 }, budget, Some(s))?;
    let held = verified(l, &groups, &retained, filled)?;  // FieldsReverted when the page changed fields back
    let submitted = match &submit {
        Some(x) => l.call(tab, CallMode::Sequence,
                          vec![plan(s, &x.doc, world::submit_fill(groups.fields_of(&x.doc), &x.local, creds))?],
                          None, 250, budget, Some(s))?.value::<world::Submitted>(0)?.submitted,
        None => false,
    };
    Ok(BrowserOutcome::Filled { tab: l.tab(tab)?.into(), filled_count: filled, submitted, committed_urls: l.committed() })
}

fn activate(l: &mut Link, s: Option<&mut Scope>, tab: u64, locator: &str, button: &str, clicks: u8,
            mods: &[String]) -> Result<BrowserOutcome, BrowserError> {
    let t = Locator::decode(locator)?;
    let tree = tree_for(l, s.as_deref(), tab, &[&t.doc])?;   // scope tree, or one `frames` when F is not T
    let mut calls = vec![plan(s.as_deref(), &t.doc, world::activate(&t.local))?];   // Prepare: scroll; rect and subject
    calls.extend(geometry::chain(&tree, &t.doc)?);            // owner_box metadata calls
    let a = l.call(tab, CallMode::Sequence, calls, scoped(s.as_deref()), 0, l.deadline(), s.as_deref_mut())?;
    let prepared: world::Prepared = a.value(0)?;               // failing here is plain: nothing effectful dispatched
    let point = prepared.rectangle.center() + geometry::offset(&a.values_from(1)?)?;
    if let Some(s) = s.as_deref_mut() {
        if s.needs_point_check() { geometry::route_and_admit(l, s, &tree, tab, point)?; }
    }
    let out = l.input(tab, None, keys::click(point, button, clicks, keys::mask(mods)), scoped(s.as_deref()), 250, l.deadline())?;
    out.require_complete_or_dialog()?;                         // a dialog proves the click landed
    Ok(BrowserOutcome::Activated { tab: l.tab(tab)?.into(), subject: prepared.subject, committed_urls: l.committed() })
}

fn observe(l: &mut Link, s: &mut Scope, tab: u64, condition: &str, value: Option<String>, locator: Option<String>,
           timeout_ms: u64) -> Result<BrowserOutcome, BrowserError> {
    let call_ms = timeout_ms + CALL_MARGIN_MS;                 // the page loop owns the timeout; this is a backstop
    let (satisfied, elapsed, readiness) = match condition {
        "visual_settle"                    => observe_one(l, s, tab, s.doc_or_top(&locator)?, condition, &value, &locator, call_ms)?,
        "load_ready" | "url_contains"     => observe_one(l, s, tab, s.top(), condition, &value, &None, call_ms)?,
        "target_present" | "target_absent" => observe_one(l, s, tab, s.doc_of(&locator)?, condition, &value, &locator, call_ms)?,
        "text_present" => {                                    // the first satisfied document wins; the rest get CANCEL by token
            let calls = s.content_docs().map(|d| s.call(d, world::observe(condition, &value, None, timeout_ms, true)))
                         .collect::<Result<Vec<_>, _>>()?;
            let a = l.call(tab, CallMode::Race, calls, Some(s.guard()), 0, call_ms, Some(s))?;
            (a.any_ok(), a.max_elapsed(), readiness(l, s, tab)?)
        }
        "text_absent" => {                                     // every document; an unavailable one counts as absent (service-worker.js:1635)
            let calls = s.content_docs().map(|d| s.call(d, world::observe(condition, &value, None, timeout_ms, false)))
                         .collect::<Result<Vec<_>, _>>()?;
            let a = l.call(tab, CallMode::Parallel, calls, Some(s.guard()), 0, call_ms, Some(s))?;
            (a.all(|x| x.satisfied_or_failed()), a.max_elapsed(), readiness(l, s, tab)?)
        }
        other => return Err(BrowserError::Primitive(format!("unknown wait condition {other}"))),
    };
    Ok(BrowserOutcome::Observed { tab_id: tab, satisfied, elapsed_ms: elapsed, readiness })
}

fn screenshot(l: &mut Link, tab: u64, locator: Option<&str>, full_page: bool, settle: Option<bool>)
    -> Result<BrowserOutcome, BrowserError> {
    let settle = (settle != Some(false)).then(|| world::observe_settle(1_000));   // never fails: answers satisfied:false
    let (clip, scope) = match locator {
        Some(x) => {
            let t = Locator::decode(x)?;
            let tree = tree_for(l, None, tab, &[&t.doc])?;
            let mut calls: Vec<PlannedCall> = settle.map(PlannedCall::top).into_iter().collect();
            let first = calls.len();
            calls.push(PlannedCall::unscoped(&t.doc, world::box_(&t.local)));
            calls.extend(geometry::chain(&tree, &t.doc)?);
            let a = l.call(tab, CallMode::Sequence, calls, None, 0, l.deadline(), None)?;
            let r = a.rect(first)?.translate(geometry::offset(&a.values_from(first + 1)?)?);
            (CaptureClip::ViewportRect { left: r.left, top: r.top, width: r.width.max(1.0), height: r.height.max(1.0) },
             CaptureScope::Target)                             // the extension adds pageX/Y and clamps at 0 (:2097-2098)
        }
        None => {
            if let Some(c) = settle {                          // best effort; frame absent = current top
                let _ = l.call(tab, CallMode::Sequence, vec![PlannedCall::top(c)], None, 0, l.deadline().min(soon(1_500)), None);
            }
            if full_page { (CaptureClip::FullPage, CaptureScope::FullPage) } else { (CaptureClip::Viewport, CaptureScope::Viewport) }
        }
    };
    let c = l.send(Capture { tab_id: tab, clip, budget: geometry::ordinary_budget() })?;   // 2400, 4M px, 0.05, [55, 30], 6M chars
    Ok(BrowserOutcome::Screenshot { tab_id: tab, mime_type: c.mime_type, data: c.data, width: c.width, height: c.height,
                                    viewport: geometry::viewport(scope, &c) })
}

fn drag(l: &mut Link, s: Option<&mut Scope>, tab: u64, src: &str, dst: &str) -> Result<BrowserOutcome, BrowserError> {
    let (a, b) = (Locator::decode(src)?, Locator::decode(dst)?);
    let tree = tree_for(l, s.as_deref(), tab, &[&a.doc, &b.doc])?;
    // service-worker.js:1957-1962 order: scroll the destination, then the source, then measure both.
    let mut calls = vec![plan(s.as_deref(), &b.doc, world::hover(&b.local))?, plan(s.as_deref(), &a.doc, world::hover(&a.local))?,
                         plan(s.as_deref(), &a.doc, world::box_(&a.local))?, plan(s.as_deref(), &b.doc, world::box_(&b.local))?];
    let (ca, cb) = (geometry::chain(&tree, &a.doc)?, geometry::chain(&tree, &b.doc)?);
    let (na, nb) = (ca.len(), cb.len());
    calls.extend(ca); calls.extend(cb);
    let r = l.call(tab, CallMode::Sequence, calls, scoped(s.as_deref()), 0, l.deadline(), s.as_deref_mut())?;
    let start = r.rect(2)?.center() + geometry::offset(&r.values(4..4 + na)?)?;
    let end = r.rect(3)?.center() + geometry::offset(&r.values(4 + na..4 + na + nb)?)?;
    let out = l.input(tab, None, vec![InputStep::Drag { path: keys::drag_path(start, end, 12), modifiers: 0 }],
                      scoped(s.as_deref()), 0, l.deadline())?;
    out.require_complete()?;
    Ok(BrowserOutcome::Dragged { tab: l.tab(tab)?.into(), source_subject: r.subject(2)?, destination_subject: r.subject(3)?,
                                 committed_urls: l.committed() })
}

fn read_document(l: &mut Link, s: &mut Scope, tab: u64, mode: &str, max_chars: usize) -> Result<BrowserOutcome, BrowserError> {
    let top = s.top();
    if mode == "article" {                                     // explicitly top-first (frames.js:97-108)
        let a = l.call(tab, CallMode::Sequence, vec![s.call(&top, world::read_text(None, "article", max_chars))?],
                       Some(s.guard()), 0, l.deadline(), Some(s))?;
        if let Ok(article) = a.value::<world::Article>(0) { if article.article_found { return Ok(article.into_text(tab)); } }
    }
    let first = l.call(tab, CallMode::Sequence, vec![s.call(&top, world::read_text(None, "visible", max_chars))?],
                       Some(s.guard()), 0, l.deadline(), Some(s))?;
    let mut sections = vec![(top.clone(), first.value::<world::Text>(0).ok())];
    let remaining = merge::remaining_after(&sections, max_chars);
    let children: Vec<FrameRef> = s.content_docs().filter(|d| *d != top).cloned().collect();
    if remaining > 0 && !children.is_empty() {
        let calls = children.iter().map(|d| s.call(d, world::read_text(None, "visible", remaining))).collect::<Result<Vec<_>, _>>()?;
        let rest = l.call(tab, CallMode::Parallel, calls, None, 0, l.deadline(), Some(s))?;
        sections.extend(children.into_iter().zip(rest.values::<world::Text>()));
    }
    let merged = merge::text_sections(&sections, max_chars, remaining == 0 && s.content_docs().count() > 1);   // DFS order
    Ok(BrowserOutcome::Text { tab_id: tab, text: merged.text, truncated: merged.truncated,
                              title: bounded(first.title(0), 500), url: first.url(0) })
}

fn inspect_tree(l: &mut Link, s: &mut Scope, tab: u64, locator: Option<&str>, max_depth: usize) -> Result<BrowserOutcome, BrowserError> {
    if let Some(x) = locator {
        let t = Locator::decode(x)?;
        let a = l.call(tab, CallMode::Sequence, vec![s.call(&t.doc, world::inspect_tree(Some(&t.local), max_depth, NODE_LIMIT))?],
                       Some(s.guard()), 0, l.deadline(), Some(s))?;
        return a.tree(0).map(|t| t.into_outcome(tab));
    }
    let top = s.top();
    let root = l.call(tab, CallMode::Sequence, vec![s.call(&top, world::inspect_tree(None, max_depth, NODE_LIMIT))?],
                      Some(s.guard()), 0, l.deadline(), Some(s))?.tree(0).ok();
    let used = root.as_ref().map_or(0, |t| t.nodes);
    let children: Vec<FrameRef> = s.content_docs().filter(|d| *d != top).cloned().collect();
    let subtrees = if used < NODE_LIMIT && max_depth > 1 && !children.is_empty() {
        let calls = children.iter().map(|d| s.call(d, world::inspect_tree(None, max_depth - 1, NODE_LIMIT - used)))
                            .collect::<Result<Vec<_>, _>>()?;
        l.call(tab, CallMode::Parallel, calls, None, 0, l.deadline(), Some(s))?.trees()
    } else { vec![] };
    let merged = merge::append_and_trim(root, subtrees, NODE_LIMIT, max_depth);   // DFS order; pre-order trim (frames.js:136-162)
    Ok(BrowserOutcome::DocumentTree { tab_id: tab, tree: serde_json::to_string(&merged.tree)?, truncated: merged.truncated })
}

fn find(l: &mut Link, s: &mut Scope, tab: u64, text: &str, kind: &str, max_results: usize) -> Result<BrowserOutcome, BrowserError> {
    let docs: Vec<FrameRef> = s.content_docs().cloned().collect();
    let calls = docs.iter().map(|d| s.call(d, world::find(text, kind, max_results))).collect::<Result<Vec<_>, _>>()?;
    let a = l.call(tab, CallMode::Parallel, calls, Some(s.guard()), 0, l.deadline(), Some(s))?;
    let per_doc = docs.iter().zip(a.values::<world::Targets>())
        .map(|(d, t)| (d.clone(), t.map(|t| locator::rescope(d, t.targets)).unwrap_or_default()));
    let (targets, over) = merge::targets(per_doc, max_results);   // DFS order, top first, one ceiling
    if over { s.limit(); }
    Ok(BrowserOutcome::Targets { tab_id: tab, targets })
}
```

`Inspect` and `QuerySemantic` are `find` with their own kind and cap. `QuerySemantic` uses the cap of 8
(`service-worker.js:82`).

### 4.7 Presentation, tab context and recordings

- **Notice tab.** `BrowserPresentation::present` (`presentation/mod.rs:47-66`) fills a missing `tab_id` for
  `denial` signals, only when the browser's `BrowserSummary.protocol` is 4. It uses:
  - the workspace's only tab; or
  - the active one among its tabs, from one `list_tabs{tab_ids}` call with a 500 ms budget.

  At level 3 the extension still chooses, so level 3 is unchanged.
- **The notice frame.** `Present{signal}` becomes `notice`:
  - `tab_id`;
  - `frame` is the locator's document;
  - `signal` carries the frame-local locator and `label`, the service's word for the activity, which the
    popup's activity list shows (the extension's own table answers only for a frame without one);
  - `defer_until_visible` is `signal.signal == denial`.

  A signal with no tab is dropped. The port returns `Presented{rendered:false}` without waiting.
- **Stale locators.** A level-3 locator that reaches a level-4 connection, for example after the browser
  updated mid-session, fails `Locator::decode` as "stale browser locator", which is a plain failure.

---

## 5. Cancellation and effect truth

**Before every primitive**, in order:
1. Check cancellation.
2. Check the deadline.
3. For an effectful primitive, call `admit()`. This re-checks runtime control, so a Pause takes effect between
   fill fields.

If nothing effectful has been dispatched in this command, the failures are `CancelledBeforeDispatch`,
`DeadlineBeforeDispatch` and `RuntimeControl`. Otherwise they are `CancelledAfterDispatch`,
`DeadlineAfterDispatch` and `EffectUnknown("held during a multi-step action")`.

**In flight.**
- On cancel the Link writes `cancel` and waits up to `min(CANCEL_GRACE, deadline)` for the receipt.
  - The extension adds the correlation to `cancelled`, which the Sequence, Input and ready-wait loops check.
  - It sends CANCEL to every in-flight world token. Page loops end with `cancelled`.
  - The interrupted primitive answers `failure:"cancelled"` with true facts.
- If no receipt arrives, an effect-class primitive counts as dispatched.
- On the deadline the Link writes `cancel` and returns at once. Primitive budgets are the remaining time minus
  `RECEIPT_MARGIN`, so the extension normally answers `timeout` with facts before the service deadline.

**Classification (service only).**

| Class | World kinds | Primitives |
|---|---|---|
| Read | the Read rows in section 3.5, including every metadata kind | `list_tabs`, `frames`, `view`, `capture`, dialog `status`, `diagnostics`, recording ops except page export, `accessibility_snapshot` |
| Prepare | `activate`, `hover`, `focus`, `scroll_point` | none |
| Effect | `fill_local`, `focus_text_field`, `submit_fill`, `type_text`, `clear_focused`, `scroll`, `upload_files`, `drop_files`, `attach_file` | `open_tab`, `navigate`, `focus_tab`, `close_tab`, `set_zoom`, `resize_window`, `input`, `evaluate_main`, dialog `handle`, recording page export |

**Rules.**
1. A step **dispatched an effect** when either:
   - an Effect primitive's receipt says `dispatched:true`; or
   - an Effect-class world call has an answer that is `Returned` with `ok:true`, `Returned` with a code that is
     not a pre-effect code, or `Failed{started:true}`.

   `Failed{started:false}`, `Skipped` and pre-effect codes are not effects.
2. The first such step sets `effect_started` for the rest of the command.
3. After that, any failure becomes `EffectUnknown(message)`, with today's message text. Cancel and deadline
   become their After variants instead.
4. With nothing dispatched, the receipt `failure` maps as follows:

| `failure` | `BrowserError` |
|---|---|
| `local_interlock` | `LocalInterlock(message)` |
| `document_changed` | `DocumentUnavailable` |
| `cancelled` | `CancelledBeforeDispatch` |
| `timeout` | `DeadlineBeforeDispatch` |
| `tab_missing` | `Primitive("browser tab is missing")` |
| `unsupported` | `CapabilityVersion{capability: name, required: 1, advertised: 0}` |
| `invalid_request` | `Protocol(message)` (a translation bug) |
| `debugger_unavailable`, `browser_refused`, `dialog_open`, `capture_too_large`, `session_lost`, `other` | `Primitive(message)` |
| asset envelope errors | `Primitive(message)` with today's text |

5. An input stopped by a dialog is success for activation (`service-worker.js:2240-2252`), and
   `EffectUnknown` for typing.
6. A lost socket is `DisconnectedAfterDispatch` if the request was written. Otherwise it is
   `DisconnectedBeforeDispatch`.
7. `committed_urls` and `tab` come from receipt facts, unioned in order and de-duplicated.

**Model-visible deltas.** All of them move toward the truth, and each gets a language golden in C10:
- navigate, history or reload to a missing tab is a plain failure (today: effect unknown);
- an activate, hover or drag that fails in preparation (stale or not visible) is a plain failure (today every
  activate error is effect unknown, `service-worker.js:1662-1664`);
- a guard failure after preparation but before input is `DocumentUnavailable`;
- an `upload_files` failure after the files were attached is `EffectUnknown` (today: plain);
- a cancelled or timed-out read is a Before variant (today: After).

**Behaviour deltas** (documented, and pinned by tests):
- `text_present` returns at the first satisfied document (today `Promise.all`).
- Merges follow DFS order. Today they follow webNavigation creation order. The top document stays first.
- Identical embeds now resolve instead of refusing.

---

## 6. Implementation plan

**Starting point.** dev, with ADR-0196 step 1. Nothing else is required.
- M1, M2, M4a and M4b may land before, after or in between.
- M2 moved the level-3 assets into `adapter/chromium/assets/` and pinned the bundle, so C1 is done.
- The plan touches level-3 code only in C1, C4 and C6. Each of those adds a byte golden.

**Gate on every commit:**
- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `npm test` from `extension/`
- `node --check` on every changed `.js`

**Level 3 stays working throughout.**
- No extension file that the current `service-worker.js` loads changes behaviour until C15. The one exception
  is `debugger.js` in C11, which is additive and pinned by the existing `debugger.test.js`.
- The service serves level 3 at every commit.
- Nothing sends a protocol-4 hello until C15, so C4 to C14 run only in tests.

---

**C1 `test(adapter): pin the protocol-3 page bundle inside the service`**
- Change:
  - copy the six level-3 sources into `page_runtime/level3/`, or into `adapter/chromium/level3/assets/` if M2
    created it: `extension/lib/{shared,form-diagnostics,presentation-css,presentation}.js` and
    `page_runtime/{sensor,content}.js`;
  - `bundle()` reads only these copies;
  - `extension/tests/content.test.js` and `page-runtime.test.js` point at the copies.
- Tests:
  - `level3_bundle_sha256_is_pinned`, with the hex recorded from the current bundle before the copy;
  - the existing runtime and relay tests.

**C2 `feat(adapter): protocol-4 wire vocabulary and shared golden fixtures`**
- Change:
  - `browser/adapter/{mod.rs, chromium/mod.rs, chromium/level4/{mod.rs, wire.rs}}` (types only);
  - `tests/golden/protocol4/**`;
  - `extension/lib/protocol.js`.
- Rust tests (`golden.rs`):
  - typed requests serialize to the fixtures;
  - receipts decode into their outcomes;
  - frames round-trip;
  - tolerance fixtures decode, with unknown kinds, events and codes mapped to `Unknown` or `Other`;
  - enum names equal `vocabulary.json`.
- JS tests (`golden-protocol4.test.js`):
  - `protocol.js` equals `vocabulary.json`;
  - every fixture parses.

**C3 `feat(adapter): isolated-world page assets for protocol 4`**
- Change:
  - `level4/assets/*.js`, with the section 3.4 edits applied to a copy of `content.js`;
  - `level4/assets.rs`;
  - `level4/world.rs` (constructors, `AssetErrorCode`, `ScopeClass`, `EffectClass`).
- Node tests (`extension/tests/world-assets.test.js`, reusing the vm fake DOM of `content.test.js`):
  - the envelope never rejects;
  - every throw site carries its code;
  - for every Effect kind, pre-effect codes are thrown before any mutation (setter and `execCommand` spies);
  - tokens end `observe`, `prepare_fill`, `verify_fill_retained` and sensor settles;
  - `dispose` removes the host, listeners, timers and the global;
  - a second install with the same id is a no-op, and a different id disposes the previous one;
  - a foreign-epoch locator is `stale_locator`;
  - the owner kinds use the supplied `owner`;
  - `attach_file` reads the attachment;
  - `submit_fill` clicks only after returning;
  - the forbidden-API scan.
- Rust tests:
  - `assets().id == sha256(source)`;
  - source size at most 512 KiB, and the `hello_accepted` frame under 768 KiB;
  - `CODES` in `world.js` equals `AssetErrorCode`;
  - every `dispatch` kind in `content.js` has a constructor in `world.rs`, and vice versa (text scan);
  - every constructor's JSON matches its fixture in `tests/golden/protocol4/world/`.

**C4 `feat(browser): accept protocol-4 hellos`**
- Change:
  - `adapter/mod.rs::attach` with the first-frame peek;
  - `level4/connection.rs` (attach, reader, heartbeat, pending);
  - `Connection.level`;
  - `BrowserSummary.protocol`;
  - every command on a level-4 connection answers `CapabilityVersion` (no translations yet);
  - the `adapter_incompatible` error frame;
  - `level4/fake.rs` FakeExtension (scripted TCP adapter: hello, heartbeat ack, events, scripted receipts,
    recorded requests).
- Tests:
  - a level-4 hello gets `hello_accepted` with `assets.id`, becomes ready, and is listed with `protocol:4`;
  - level-3 byte goldens recorded before the change are unchanged: `hello_accepted`, `install_page_runtime`,
    one request frame and one acknowledge;
  - `protocol:5` and `major:2` get an error frame and are closed;
  - a malformed level-4 hello is closed;
  - FakeExtension events reach the sink and unknown frame kinds are ignored;
  - heartbeat acks keep liveness;
  - a level-3 and a level-4 browser connected together route by browser id;
  - the existing relay contract suite is unchanged.

**C5 `feat(adapter): Chromium translation helpers`**
- Change: `level4/{tree,locator,scope,merge,keys,geometry}.rs`, pure Rust.
- Tests:
  - ported case for case from `extension/tests/{frames,documents,shared,screenshot}.test.js`;
  - DFS order with out-of-process children;
  - unseen placeholders;
  - strict guards (url) versus required guards (origin);
  - the route reducer requires exactly one owner;
  - inspect-tree trim parity;
  - key descriptor parity with the `shared.test.js` vectors;
  - output-scale parity with `screenshot.test.js`;
  - the scope refuses content calls outside `allowed` before anything is sent;
  - `unsatisfied` counts as visited.

**C6 `feat(adapter): link and browser-native commands at protocol 4`**
- Change:
  - `level4/link.rs`;
  - `run` with rows 2-11, 27-28 and 42-52 (others answer `CapabilityVersion`);
  - `OpenTab` skip fields filled by `work/navigation.rs`;
  - `BrowserOutcome::Dialog.message`;
  - the protocol-4 notice tab choice in `BrowserPresentation`;
  - `RecordingIndex`.
- Tests (FakeExtension):
  - each row's request equals its golden request;
  - outcomes map to `BrowserOutcome`;
  - `tab_missing` is a plain failure;
  - `local_interlock`;
  - a cancel during navigate writes `cancel`, and the receipt facts decide Before (`dispatched:false`) or
    After;
  - the deadline writes `cancel` and returns at once;
  - the workspace's one recording resolves, or answers not found, and a second start is held;
  - a denial notice carries `defer_until_visible`, and no receipt is awaited;
  - level-3 `OpenTab` bytes are unchanged (golden);
  - a level-3 receipt without `message` still decodes.

**C7 `feat(adapter): documents and reading at protocol 4`**
- Change: rows 1, 3, 12-16, 19-20, 33, 41, 44 and 46.
- Tests:
  - `work/tests/documents.rs` fixtures re-run through a `RelayBrowserPort` with a FakeExtension attached,
    via a `fixture_with(port)` variant, with the same assertions as FakeBrowser;
  - `text_present` completes at the first satisfied document, and the others receive CANCEL tokens;
  - `text_absent` needs all documents;
  - an unseen placeholder makes a restricted unbounded command refuse;
  - a strict guard mismatch is `DocumentUnavailable`;
  - visited is a subset of admitted;
  - read and tree merges equal the `frames.test.js` cases;
  - the recording watch carries GuardDocs;
  - a diagnostics binding change resets the ring.

**C8 `feat(adapter): pointer input at protocol 4`**
- Change: rows 21-26, 29-30, 36-37 and 39.
- Tests:
  - offsets through two nested frames;
  - identical embeds resolved by owner;
  - zero or two owners are unresolved;
  - a dialog during a click is success;
  - a stale view refuses before input;
  - a scoped point outside `allowed` is refused before input;
  - all-open skips the route round trips;
  - a preparation failure is plain;
  - a failure after the click is `EffectUnknown`;
  - the drag path has 12 steps;
  - wheel tick count and sign.

**C9 `feat(adapter): keyboard, typing, capture, upload and script at protocol 4`**
- Change: rows 17-18, 32, 34-35, 38, 40 and 47-50 (target export).
- Tests:
  - modified keys drop their text;
  - TypeFocused picks the first focused document and clears only there;
  - a `focus_lost` precondition refuses with no key sent;
  - target clip composition;
  - a region view mismatch after capture is "stale screenshot view";
  - capture is a single primitive with no hide calls from the service;
  - the script guard, and script exception mapping;
  - an upload over 768 KiB uses `command_chunk`;
  - a page export carries `attach_file` and its guard.

**C10 `feat(adapter): form fill at protocol 4`**
- Change: row 31, and `run` becomes exhaustive with no `CapabilityVersion` fallback. `tests/process-journey.mjs`
  gains a protocol-4 fake-adapter lane that drives real executables through hello, list, navigate, a `call`
  and a `capture`.
- Tests:
  - a multi-frame fill;
  - prepare refuses before any edit;
  - local runs are batched into one Sequence;
  - a text field is one `input` with a `focus_text_field` precondition;
  - a retention failure after edits is `EffectUnknown`;
  - a Pause (`admit` fails) between fields is `EffectUnknown`;
  - budget exhaustion is `EffectUnknown`;
  - an orphan submit is refused before dispatch;
  - language goldens for every section 5 delta.

**C11 `feat(extension): debugger sessions, registrations and CDP allowlist`**
- Change: `lib/debugger.js`, additive:
  - `cdp()` with the allowlist; all internal calls go through it, and every legacy method is listed;
  - the child-session registry;
  - registration identifiers;
  - the `installer` and `onRelease` options (default: today's MAIN-world installer);
  - `forEachSession`, `rootSession`, `childSessions`;
  - dialog `{type, message, sessionId}`.
- Tests:
  - the existing `debugger.test.js` is unchanged and green, which pins the legacy CDP sequence;
  - a disallowed method fails with `browser_refused`;
  - a child session is added on `attachedToTarget` and dropped on `detachedFromTarget`;
  - the order is `Page.enable`, installer, nested `setAutoAttach`, then `runIfWaitingForDebugger` last;
  - `onRelease` runs before a voluntary detach;
  - identifiers are recorded per session.

**C12 `feat(extension): isolated world, frame inventory and input mechanics`** (not loaded by the worker)
- Change: `lib/world.js`, `lib/inventory.js`, `lib/input.js`.
- Tests (mocked `chrome.debugger` and `webNavigation`):
  - registration removes old identifiers, then registers in every session with `worldName:"ghostlight"` and
    the id-wrapped source;
  - a checksum mismatch is refused;
  - handles verify the loader and cache an `objectId`;
  - `callFunctionOn` never carries `executionContextId`;
  - `runtime_missing` triggers exactly one installer call and one retry;
  - `exceptionDetails` maps to `page_exception`;
  - "Cannot find context" maps to `document_changed` with `started:false`; a context destroyed mid-await has
    `started:true`;
  - the deadline sends CANCEL, then `timeout`;
  - race cancels pending losers and reports them skipped;
  - sequence skips after the first non-ok answer;
  - `owner_of` uses `getFrameOwner` and `resolveNode` on the parent session and re-checks the loader;
  - out-of-process ownership and unseen URLs;
  - the guard in exact (url) and required (origin) modes;
  - the input batch stops at a dialog;
  - a non-ok precondition sends no input;
  - drag with interception, without it, and with late interception (`cancelDragging`);
  - `Runtime.enable` is never sent.

**C13 `feat(extension): protocol-4 primitive handlers`** (not loaded by the worker)
- Change:
  - `lib/tabs.js`, `lib/capture.js`, `lib/notice.js`, `lib/buffers.js`, `lib/primitives.js`;
  - `script-evaluator.js` gains an additive `prepare` and `run(deadline, terminate)`.
- Tests (mocked chrome APIs):
  - every golden request produces its golden receipt;
  - an unknown name answers `unsupported` with `dispatched:false`;
  - bad arguments are `invalid_request`;
  - the preserve-tabs veto, and `released` marks the tab;
  - `open_tab`: adopt versus create, the group by key with a title fallback after reload, duplicate merge,
    `focus_window`;
  - capture unhides in `finally`, including on failure, and concurrent holds stay hidden;
  - the quality ladder and `capture_too_large`;
  - a notice defers, retires its spinner, flushes on visibility, and never attaches;
  - a recording watch interrupts on a boundary;
  - the diagnostics binding reset;
  - `evaluate_main` on its deadline calls `terminateExecution` and answers `timeout` with `dispatched:true`;
  - two concurrent primitives on one tab both see a commit;
  - a cancel that beats chunk reassembly still cancels.

**C14 `feat(extension): protocol-4 worker entry`** (`extension/worker.js`; the manifest still points at
`service-worker.js`)
- Change: `worker.js` (section 2.2).
- Tests (worker sliced into vm, like `native-startup.test.js`):
  - the hello is the first frame and matches the golden;
  - a second `hello_accepted` on one port re-registers assets, and settles state only on a new epoch;
  - each request is answered exactly once;
  - unknown frame kinds are ignored;
  - events go only for controlled tabs, and a released tab reports one commit and then stops;
  - with no `hello_accepted` in 10 s the outdated-service message shows;
  - `backend_unavailable` and a disconnect send AMBIENT `disconnected` and cancel in-flight work;
  - control `ended` detaches everything.

**C15 `feat(extension): speak adapter protocol 4 only`**
- Change:
  - manifest `background.service_worker: "worker.js"`, `version: "2.0.0"`, `scripting` removed;
  - delete `service-worker.js`, `lib/{engine,documents,frames,topology,form-diagnostics,presentation,presentation-css,screenshot}.js`
    and their tests: `documents`, `frames`, `engine`, `form-diagnostics*`, `click-worker`, `fill-worker`,
    `focus-worker`, `keyboard-worker`, `settlement-worker`, `presentation-routing`, `page-runtime`,
    `diagnostics-toggle`, `onboarding` if it slices the worker, `native-startup`;
  - trim `shared.js`;
  - drop the `debugger.js` legacy default installer and `installPageRuntime`;
  - remove the popup diagnostics row and the options form-diagnostics export.
- Journeys:
  - `frame-browser-journey.mjs:272-276` is inverted, so the page cannot see the runtime;
  - an OOPIF read, click and type;
  - identical embeds;
  - a world shared by name across re-attach;
  - prerender activation and a back/forward restore;
  - `alert` during a world call and on submit;
  - HTML5 and pointer drags;
  - a Trusted Types and strict-CSP page;
  - fix `keyboard-browser-journey.mjs:104`;
  - the process journey runs both of its adapter lanes.
- Docs:
  - `docs/STATUS.md`;
  - the `docs/1.0/ARCHITECTURE.md` adapter section;
  - a MEMORY learning ("calls go through objectIds");
  - the store disclosure text, drafted and not submitted (ADR-0196 Consequences).
- Tests: the full `npm test` without the legacy suites; goldens; worker; `node --check`.
- After this commit the owner's unpacked extension speaks protocol 4. Published 1.3.x extensions keep using
  level 3. Store publication of 2.0.0 waits for service spread (SYNTHESIS M5).

---

## 7. Risks

| Risk | Mitigation and test |
|---|---|
| **A protocol-4 extension meets a 1.3.x service.** The service closes the socket; there is no fallback (D1) | The store item publishes only after services spread (deferred publishing, SYNTHESIS M5). After 10 s the extension says "Update Ghostlight on this computer". C14 test |
| **A new service meets an old extension** | The level-3 path is unchanged: the pinned bundle (C1), byte goldens (C4, C6), the existing relay suite, and the process journey's level-3 lane |
| **Store review refuses service-supplied page code** (CRITIQUE SG-1, EXT-4) | Named fallback: embed the asset set in the package and select it by `assets.id` (the extension refuses unknown ids). It costs a store release per asset change. Owner decision before submission |
| **A numeric context id aliases another process's context** | Calls go through `objectId`, which V8 rejects in a foreign isolate. Numeric ids are used only before a loader re-check. C12 tests; C15 cross-site navigation during calls |
| **World identity differs across Chrome 125+ builds** | Correctness depends on neither behaviour: the lazy install on `runtime_missing`, plus epochs that make old handles `stale_locator`. C15 re-attach journey |
| **Another debugger client uses the world name `ghostlight`** | Only debugger-privileged clients can; the page cannot. Accepted |
| **A registration-created world has universal access** | The assets make no network or extension calls; C3 forbidden-API scan |
| **An OOPIF Chrome will not attach is invisible to CDP** | `unseen` placeholders keep today's conservative governance. A load race may refuse a restricted command once; a retry works. C7 test |
| **A dialog blocks `callFunctionOn`** | Calls refuse early when a dialog is known. The dialog and deadline race the call. Tail clicks run through `setTimeout`. C12; C15 journeys |
| **`terminateExecution` cannot stop an awaited promise** | The receipt is `timeout` with `dispatched:true` either way; the service reports effect unknown |
| **More round trips** | Coarse batches: fill is 2+T+L, activate 2-3, reads 1-2, observe one race. All-open skips route checks. The C10 process-journey lane records timings |
| **Drag interception timing** (10 ms per step, 100 ms grace) | Late data is replayed or cancelled. C12 tests; C15 HTML5 and pointer fixtures |
| **Prerender activation swaps the page and drops registrations** | Re-attach re-registers; lazy install; guards answer `document_changed`, never the wrong document. C15 |
| **The isolated world cannot see page-defined JS properties** | Native DOM accessors work; attribute and ARIA fallbacks exist; text goes through trusted keys. C15 controlled-form fixture |
| **Trusted Types or CSP block presentation `innerHTML`** (`presentation.js:150`) | C15 journey. If it fails, the fix is asset-only (build the SVG with DOM calls), with no protocol change |
| **A leftover MAIN-world runtime after a 3-to-4 switch** | Registrations die with the old debugger sessions. Page globals die at the next navigation and are never called. Low impact |
| **`hello_accepted` grows past the native-message cap** | C3 size test (source at most 512 KiB, frame under 768 KiB). If assets ever outgrow it, a new optional `assets_chunk` frame kind is an allowed addition |
| **Effect-classification drift changes model wording** | The class table in `world.rs`, the C3 pre-effect invariant tests, and the C10 language goldens |
| **Merge order changes** (DFS instead of creation order) | The top stays first. Documented and pinned in C5 |
| **The level-3 decode path now goes through `serde_json::Value`** | Same types and same results; the C4 byte goldens and the relay suite. Only duplicate JSON keys behave differently (last wins), which no adapter sends |
| **`RecordingIndex` is lost on a service restart** | Recordings are interrupted on an epoch change anyway; a status or stop then answers `RecordingNotFound` |
| **`list_tabs` still returns every tab** | Unchanged from today. `tab_ids` is reserved, so the privacy improvement needs no store release once Work carries ids |
| **The worker is killed mid-primitive** | The port closes, and the service maps that to `DisconnectedAfterDispatch` (effect unknown). Correlations are never resent, so no ledger is needed |
| **The relay replays hellos until M4a** | A repeated `hello_accepted` on one port is idempotent and re-registers new assets. C14 test |

## 8. Owner checks

1. **OD14.** `hello_accepted` carries no diagnostics state, so the popup diagnostics toggle is gone at
   protocol 4. The workbench and CLI keep it. Adding it back later is an optional field.
2. **OD13.** Recording ops require ids on the wire. A workspace holds one recording and the service supplies
   its id (section 4.3, rows 46-50).
3. **Wording for Before variants.** A cancelled or timed-out read maps to a Before variant (FINAL open choice
   4). Its sentence should say "nothing changed", not "nothing reached the browser". The typed
   `Cancelled{dispatched}` in S6 settles this later.
4. **Store fallback.** Decide before submitting 2.0.0 (section 7).
5. **`host_permissions`.** Keep them until a live check proves the debugger-only path works without them.
