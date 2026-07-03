# G06: Audit flight recorder: JSONL records at the dispatch chokepoint

## Goal

Implement the audit flight recorder (ADR-0018 step 1; groundwork for ADR-0020
commitments 5 and 6): a new `src/audit/` subsystem that writes exactly one JSON Lines
record for every MCP tool call, in the exact record shape defined by
`docs/tasks/stage-2/00-shared-format.md` section 6, to a destination selected by the
`audit.destination` configuration key (`file` or `stderr`), gated by `audit.enabled`,
with the file path resolved from `audit.file.path`. Wire it at the dispatch chokepoint
(`src/dispatch.rs` / `src/mcp/server.rs`) so every call is recorded, allow and deny
alike. Today every decision is `allow`: no enforcement exists yet, so the
manifest-dependent fields (`identity`, `domain`, `grant_id`, `denial_id`, `manifest`)
are `null` by construction. A failure to write an audit record must never break a tool
call, but must be reported via `tracing`.

## Depends on

- `docs/tasks/stage-2/00-shared-format.md` -- the reconciled format reference. Read it
  FIRST. The load-bearing sections for this task are 1.4 (default audit file path),
  3.4 (the `audit.enabled` / `audit.destination` / `audit.file.path` keys and their
  Minimal defaults), 6 (the audit record: fields, order, JSONL rules, sensitive-parameter
  omission), and 8 (the read/write classification the `rw` field uses). Use ITS field
  names verbatim; do not improvise names.
- G05 (`docs/tasks/stage-2/g05-rw-classification.md`): `src/policy/classify.rs` with
  `pub fn classify(tool: &str, action: Option<&str>) -> Option<RwClass>` and
  `RwClass::as_str()` returning `"observe"` / `"mutate"`. If `src/policy/classify.rs`
  does not exist in the tree, STOP and report that G05 must land first; do not
  re-implement classification inside this task.
- All release-1 (stage-1) tasks in `docs/tasks/release-1/` are assumed landed.
- This task deliberately does NOT depend on the stage-2 configuration-registry growth
  task, the manifest/enforcement tasks, or the shadow-mode task (G15). It is the first
  governance subsystem to land (observe-then-enforce, ADR-0018), so it adds only the
  minimal typed config it needs and leaves everything manifest-shaped as `null`.

## Project context

Browser MCP is governed browser automation. A single Rust binary is BOTH the MCP server
(JSON-RPC 2.0 over stdio, hand-rolled, tokio) AND the Chrome native-messaging host; a
thin Manifest V3 extension executes CDP commands. Architecture:

```
MCP Client <--stdio--> Binary <--native messaging--> Extension <--CDP--> Browser
```

The two binary roles are separate OS processes bridged by tokio-native named-pipe/UDS
IPC. Stage 1 (docs/tasks/release-1/) hardened the engine. Stage 2 is the governance
layer per ADR-0013 (separable overlay; all-open stays first-class), ADR-0018
(observe-then-enforce: the flight recorder lands BEFORE any enforcement), ADR-0019
(layered configuration and typed key registry), and ADR-0020 (org policy experience:
manifest identity in every record, stable denial ids).

Audit trust boundary (SPEC section 7, shared format section 6): records are written by
the binary only; the extension never logs. Every tool call produces exactly one record.
This record shape is designed once and reused later by `policy simulate`, the local
activity ledger, and session recap; get the shape exactly right now.

Authority order: where `docs/SPEC.md` and the ADRs disagree, the ADRs win. The shared
format doc is the reconciled single source. Concretely: SPEC 7.1/7.2 describe an OLDER
record schema (with `url`, `parameters`, `screenshot`, `access_tier_*` fields). That is
superseded. The only correct record shape is shared format section 6, reproduced in
Required behavior below.

Key files for this task:

- `src/dispatch.rs` -- the single dispatch chokepoint (31 lines today). `policy_check`
  (lines 23-25) and `audit` (line 30) are documented no-ops. This task makes `audit`
  real and leaves `policy_check` and `PolicyDecision` (single `Allow` variant,
  lines 13-17) untouched.
- `src/mcp/server.rs` -- the JSON-RPC loop. `run` (line 22) builds
  `Config::default()` (line 28); `handle_line` (line 55) routes methods; the
  `initialize` arm (line 86) currently IGNORES its params (this is where `clientInfo`
  must be captured); `handle_tools_call` (lines 116-155) calls the no-op seams at
  lines 132-133.
- `src/policy/mod.rs` -- the typed key registry seed: `KeyDef` (lines 25-33), `KEYS`
  (lines 43-47, one entry), `Config` (lines 51-76, one field, `Copy`).
- `src/policy/classify.rs` -- G05's classifier (prerequisite; see Depends on).
- `src/lib.rs` -- library crate root; module list at lines 15-24 (no `audit` module
  today).
- `src/mcp/schemas/tools.json` -- SACRED, byte-frozen, guarded by
  `tests/tool_schema_fidelity.rs`. Never touched by this task.
- `Cargo.toml` -- current deps (lines 9-18): tokio, serde, serde_json (with
  `preserve_order`), clap, tracing, tracing-subscriber, thiserror, anyhow, dirs.
  NOTE: `uuid` and `chrono` are NOT present today; this task adds them (sanctioned by
  the shared format doc's crate note). `sha2` is NOT needed here (no manifest hash, no
  denial ids in this task).

## Current behavior

All facts verified against the working tree at authoring time.

- There is no `src/audit/` directory and no audit code anywhere. `src/lib.rs` declares
  modules `browser, debug, dispatch, error, install, mcp, native, origin, policy, tools`
  (lines 15-24).
- `src/dispatch.rs` line 30: `pub fn audit(_tool: &str) {}` -- a documented no-op.
- `src/mcp/server.rs` `handle_tools_call` lines 132-133:
  `let _decision = dispatch::policy_check(name);` then `dispatch::audit(name);` --
  called BEFORE the browser call, so today's seam cannot carry a duration or outcome.
- The `initialize` arm (server.rs line 86) is
  `"initialize" => Some(JsonRpcResponse::success(id, initialize_result()))` -- the
  request `params` (which carry `clientInfo` per MCP) are dropped.
- `src/policy/mod.rs` `Config` is `#[derive(Debug, Clone, Copy)]` with a single field
  `secrets_redact: bool`; `KeyDef.minimal_default` is `bool`-typed, so only boolean
  keys can be registered in `KEYS` today. The registry has one key,
  `content.security.secrets.redact`.
- `src/browser.rs` `Browser::call` fails fast with
  `Error::NativeMessaging("browser extension is not connected")` when no extension is
  attached (lines 90-95); `Browser::new()` builds an unconnected handle. Inline server
  tests can use this to exercise a full `tools/call` without a browser.
- `tests/mcp_protocol.rs` spawns the real binary over stdio (with an isolated
  `BROWSER_MCP_ENDPOINT`) and asserts, among other things, that a `tools/call` with no
  extension returns an `isError` tool result whose text contains `not connected`.
- `serde_json` is built with `preserve_order` (Cargo.toml line 12), so struct field
  order is preserved in serialized output and `Value::Object` preserves key order when
  parsing. The record's field order relies on this.

## Required behavior

### 1. Dependencies (`Cargo.toml`)

Add exactly two dependencies:

```toml
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", default-features = false, features = ["clock", "std"] }
```

`uuid` provides the v4 `event_id`; `chrono` provides the RFC 3339 UTC timestamp. Both
are sanctioned by the shared format doc's crate note for stage 2. Do not add `sha2`,
`time`, `regex`, or anything else. `Cargo.lock` will change accordingly.

### 2. Config keys (`src/policy/mod.rs`)

Add three key-name constants with doc comments, following the existing
`CONTENT_SECURITY_SECRETS_REDACT` style:

```rust
/// `audit.enabled` -- when true, the binary writes one audit record (JSON line) for
/// every tool call: the flight recorder (ADR-0018 step 1). Default: `true`.
pub const AUDIT_ENABLED: &str = "audit.enabled";

/// `audit.destination` -- where audit records are written: `"file"` or `"stderr"`.
/// Not yet in [`KEYS`]: registration awaits typed (non-bool) `KeyDef` values
/// (shared format doc section 3.3; configuration-registry task). Default: `"file"`.
pub const AUDIT_DESTINATION: &str = "audit.destination";

/// `audit.file.path` -- absolute path of the audit file; the empty string means the
/// platform default (shared format doc section 1.4). Not yet in [`KEYS`] (same reason
/// as `audit.destination`). Default: `""`.
pub const AUDIT_FILE_PATH: &str = "audit.file.path";
```

Add the typed destination enum next to them:

```rust
/// Typed value of the `audit.destination` key. Registry enum strings: `"file"`,
/// `"stderr"`. `syslog`, `http`, and `none` are deferred beyond stage 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditDestination {
    /// Append JSON lines to the file resolved from `audit.file.path`.
    File,
    /// Write JSON lines to stderr (stdout is reserved for the MCP protocol stream).
    Stderr,
}
```

Register ONE new entry in `KEYS` (only booleans fit today's `KeyDef`):
`AUDIT_ENABLED` with description
`"Write one audit record (JSON line) for every tool call: the flight recorder."` and
`minimal_default: true`.

Grow `Config` with three fields and accessors, keeping `#[derive(Debug, Clone, Copy)]`
(use `&'static str` for the path so `Copy` survives; the layered-config task
generalizes this later):

- `audit_enabled: bool`, accessor `pub fn audit_enabled(&self) -> bool`.
- `audit_destination: AuditDestination`, accessor
  `pub fn audit_destination(&self) -> AuditDestination`.
- `audit_file_path: &'static str`, accessor
  `pub fn audit_file_path(&self) -> &'static str`.

`Config::minimal()` sets `audit_enabled: true`, `audit_destination:
AuditDestination::File`, `audit_file_path: ""` (the shared format doc section 3.4
"safe (= Minimal)" column). Extend the existing test
`minimal_config_matches_the_registry_defaults` to also assert
`Config::minimal().audit_enabled()` equals the `AUDIT_ENABLED` entry's
`minimal_default`, and add asserts that the minimal destination is
`AuditDestination::File` and the minimal path is `""`. Doc comments on every new public
item.

### 3. New module `src/audit/`

Declare `pub mod audit;` in `src/lib.rs`, inserted alphabetically (before
`pub mod browser;`). While editing `src/lib.rs`, update the layering doc sentence at
line 8 from `In v1.0 those seams are no-ops (all-open).` to
`The audit seam records as of stage 2 (ADR-0018 step 1); the policy seam is still a no-op (all-open).`
No other `lib.rs` changes.

Three small files (many small files over one large one):

#### 3a. `src/audit/record.rs` -- the record type

Module doc: this is the shared-format section 6 record, designed once and reused by
`policy simulate`, the activity ledger, and session recap; SPEC 7.1/7.2's older schema
is superseded.

```rust
/// One audit record: exactly one JSON line per tool call (shared format doc sec 6.1).
/// Field ORDER is part of the format; `serde_json` is built with `preserve_order`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AuditRecord {
    /// UUID v4, lowercase, hyphenated. Unique per record.
    pub event_id: String,
    /// RFC 3339 UTC timestamp, millisecond precision, e.g. `2026-07-02T14:32:15.003Z`.
    pub ts: String,
    /// From the active manifest's `identity` block; always `None` until the manifest
    /// task lands.
    pub identity: Option<Identity>,
    /// MCP client identity from the `initialize` request's `clientInfo`; `None` if the
    /// client did not provide it. Captured once per session.
    pub client: Option<ClientInfo>,
    /// MCP tool name as received.
    pub tool: String,
    /// The `computer` sub-action (e.g. `left_click`); `None` for every other tool.
    pub action: Option<String>,
    /// `"observe"` or `"mutate"` (shared format doc sec 8, via policy::classify).
    pub rw: &'static str,
    /// Parser-normalized host of the current tab at decision time; always `None` until
    /// the enforcement task introduces current-tab tracking.
    pub domain: Option<String>,
    /// `"allow"`, `"deny"`, or `"shadow_deny"`. Always `"allow"` in this task.
    pub decision: &'static str,
    /// Grant id that resolved the decision; always `None` until grants exist.
    pub grant_id: Option<String>,
    /// Stable denial id; always `None` until denials exist.
    pub denial_id: Option<String>,
    /// Wall time from dispatch entry to result, in milliseconds.
    pub duration_ms: u64,
    /// Active manifest identity; always `None` until the manifest task lands.
    pub manifest: Option<ManifestIdentity>,
}
```

Also define (with doc comments; constructed by later tasks, serialized shape fixed
now):

```rust
/// `identity` object: `{ "principal": ..., "resolved_by": ... }`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Identity {
    pub principal: String,
    pub resolved_by: String,
}

/// `client` object: `{ "name": ..., "version": ... }` from MCP `initialize`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

/// `manifest` object: `{ "name": ..., "version": ..., "hash": ... }` (hash is the
/// 64-lowercase-hex content hash, shared format doc sec 4.2; computed by a later task).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ManifestIdentity {
    pub name: String,
    pub version: String,
    pub hash: String,
}
```

Rules the type must satisfy (assert in tests):

- Serialized field order is exactly: `event_id, ts, identity, client, tool, action,
  rw, domain, decision, grant_id, denial_id, duration_ms, manifest`.
- Absent values serialize as JSON `null`, never omitted (no `skip_serializing_if`
  anywhere). The record shape is constant.
- Compact serialization (`serde_json::to_string`) yields one line: serde_json escapes
  any newline inside a string value as `\n`, so a raw LF can never appear inside the
  serialized record.
- Sensitive-parameter omission (shared format doc sec 6.2): there is NO `parameters`
  field, NO `screenshot` field, NO `url` field. The only tool-call argument ever
  extracted is the `computer` `action` string, which is a named record field.

#### 3b. `src/audit/destinations.rs` -- where lines go

- `pub fn default_audit_path() -> Option<std::path::PathBuf>`: returns
  `dirs::data_local_dir()` joined with `browser-mcp` then `audit.jsonl`. `dirs` is
  already a dependency and `data_local_dir()` maps exactly to the shared format doc
  section 1.4 table: `%LOCALAPPDATA%` on Windows, `~/Library/Application Support` on
  macOS, `~/.local/share` (or `XDG_DATA_HOME`) on Linux. Doc comment cites section 1.4.
- `pub fn append_line_to_file(path: &std::path::Path, line: &str) -> std::io::Result<()>`:
  `create_dir_all` on the parent (if any), then open with
  `OpenOptions::new().create(true).append(true)` and write the line bytes followed by
  a single `\n` (LF, never CRLF, on every platform; JSON Lines rule from shared format
  doc section 6). One open-append-close per record: simple, rotation-friendly, and
  cheap at tool-call frequency.
- `pub fn write_line_to_stderr(line: &str)`: `eprintln!("{line}")`. Doc comment notes
  stdout is reserved for the MCP protocol stream, and stderr records interleave with
  `tracing` output by design (that is what the `stderr` destination means).

#### 3c. `src/audit/mod.rs` -- the `Recorder`

Module doc: the audit flight recorder (ADR-0018 step 1); records are written by the
binary only, the extension never logs (SPEC sec 7 trust boundary); one record per tool
call; write failures never break tool calls but are reported via `tracing`.

Re-export the record types (`pub use record::{AuditRecord, ClientInfo, Identity,
ManifestIdentity};`) and declare `pub mod destinations; pub mod record;`.

```rust
/// The audit flight recorder. Cheap to share by reference from the server loop.
/// A disabled recorder does nothing (and creates no file).
pub struct Recorder { /* private */ }
```

Internals (private): an `Option<Inner>` where `Inner` holds the resolved sink
(`File(PathBuf)` or `Stderr`) and `client: std::sync::Mutex<Option<ClientInfo>>`.
`None` means disabled. The `Mutex` is `std::sync` (never held across an await; the
server loop is sequential).

Public API, each with a doc comment:

- `pub fn from_config(config: &crate::policy::Config) -> Recorder`:
  - If `!config.audit_enabled()`, return a disabled recorder.
  - If `config.audit_destination()` is `AuditDestination::Stderr`, return a
    stderr-backed recorder.
  - If it is `AuditDestination::File`: the path is `config.audit_file_path()` if
    non-empty, else `destinations::default_audit_path()`. If the path is empty AND
    `default_audit_path()` returns `None`, emit
    `tracing::warn!("no data directory available; audit recording disabled")` and
    return a disabled recorder (audit unavailability is reported, never fatal).
- `pub fn disabled() -> Recorder`.
- `pub fn to_file(path: std::path::PathBuf) -> Recorder` (used by `from_config` and by
  tests).
- `pub fn to_stderr() -> Recorder`.
- `pub fn is_enabled(&self) -> bool`.
- `pub fn set_client(&self, name: &str, version: &str)`: stores
  `ClientInfo { name, version }` if none is stored yet; first capture wins ("captured
  once per session", shared format doc sec 6.1). No-op on a disabled recorder.
- `pub fn record_call(&self, tool: &str, action: Option<&str>, duration_ms: u64)`:
  no-op when disabled; otherwise build the record and write it:
  - `event_id`: `uuid::Uuid::new_v4().to_string()` (Display is lowercase hyphenated).
  - `ts`: `chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)`
    (the trailing `true` renders `Z`, giving exactly the
    `2026-07-02T14:32:15.003Z` shape).
  - `identity: None`, `domain: None`, `grant_id: None`, `denial_id: None`,
    `manifest: None` (each with a short comment naming the later task that populates
    it: manifest loading for `identity`/`manifest`, enforcement for `domain`/
    `grant_id`/`denial_id`).
  - `client`: a clone of the stored `ClientInfo`, or `None`.
  - `tool`: as received; `action`: as received (`Option<&str>` to owned).
  - `rw`: `crate::policy::classify::classify(tool, action)` mapped through
    `RwClass::as_str()`; on a classification miss (`None`: unknown tool, or a
    `computer` call with a missing or unknown action) record `"mutate"`. Comment the
    rationale: the record vocabulary is only `observe`/`mutate` (shared format doc
    sec 6.1) and an unclassifiable call must never be presented as harmless
    observation, so the miss falls to the more conservative class.
  - `decision`: `"allow"` (comment: the only decision until enforcement lands; `deny`
    and `shadow_deny` arrive with the enforcement and shadow-mode tasks).
  - `duration_ms`: as passed in.
  - Serialize with `serde_json::to_string`, then write via the destination function.
    On ANY error (serialization or I/O):
    `tracing::warn!(error = %e, "failed to write audit record")` (include the path for
    file errors) and return normally. Never panic, never propagate, never alter the
    tool result.

### 4. Dispatch wiring (`src/dispatch.rs`)

Replace the no-op `pub fn audit(_tool: &str) {}` with the real hook:

```rust
/// Record one audit record for a completed tool call (ADR-0018 step 1: the flight
/// recorder). Called at the dispatch chokepoint after the call resolves, so the record
/// carries the real duration. Recording failures are contained inside the recorder.
pub fn audit(recorder: &crate::audit::Recorder, tool: &str, action: Option<&str>, duration_ms: u64) {
    recorder.record_call(tool, action, duration_ms);
}
```

Update the module doc comment: the audit seam is now live (stage 2, ADR-0018 step 1);
the policy seam remains a no-op. Do NOT touch `PolicyDecision` (keep the single `Allow`
variant) or `policy_check`.

### 5. Server wiring (`src/mcp/server.rs`)

- In `run` (line 22), directly after `let config = Config::default();` build the
  recorder: `let recorder = crate::audit::Recorder::from_config(&config);` and pass
  `&recorder` to `handle_line`.
- Change `handle_line` to
  `async fn handle_line(browser: &Browser, config: Config, recorder: &Recorder, line: &str) -> Option<JsonRpcResponse>`
  and thread `recorder` to `handle_tools_call`.
- `initialize` arm: before answering, capture the client identity. Add a helper with a
  doc comment:

```rust
/// Capture `clientInfo` from the MCP `initialize` params into the audit recorder
/// (shared format doc sec 6.1 `client` field). Both `name` and `version` must be
/// strings; otherwise the session's records carry `client: null`.
fn capture_client_info(recorder: &Recorder, params: Option<&Value>) {
    let info = params.and_then(|p| p.get("clientInfo"));
    let name = info.and_then(|i| i.get("name")).and_then(Value::as_str);
    let version = info.and_then(|i| i.get("version")).and_then(Value::as_str);
    if let (Some(name), Some(version)) = (name, version) {
        recorder.set_client(name, version);
    }
}
```

  and change the arm to call it, then return the unchanged `initialize_result()`. The
  initialize RESPONSE bytes must not change.
- `handle_tools_call` (new `recorder: &Recorder` parameter): replace the two seam lines
  (currently 132-133) with timing plus post-completion recording:

```rust
// Dispatch chokepoint. The policy seam is still a no-op (all-open); the audit seam
// records every call (ADR-0018 step 1) after it resolves, so the record carries the
// real duration and completion timestamp.
let started = std::time::Instant::now();
let _decision = dispatch::policy_check(name);
let action = if name == "computer" {
    args.get("action").and_then(Value::as_str)
} else {
    None
};
let outcome = browser.call(name, &args).await;
let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
dispatch::audit(recorder, name, action, duration_ms);
match outcome {
    // ... existing Ok / Err arms unchanged (redaction stays in the Ok arm) ...
}
```

  Exactly one record per `tools/call`, whether the browser call succeeds or fails
  (an execution failure is still `decision: "allow"`; the decision field is about
  policy, not outcome). The early `-32602` return for a missing/non-string `name`
  records nothing: with no tool name there is no tool call, and the record's `tool`
  field cannot be fabricated.
- The `action` extraction reads ONLY `arguments.action` and ONLY for `computer`; no
  other argument is read, logged, or stored (shared format doc sec 6.2).

### 6. Tests

Inline unit tests (`#[cfg(test)] mod tests`), by file, with these exact names:

`src/audit/record.rs`:

1. `record_serializes_all_fields_in_shared_format_order`: build a fully null-optional
   record, serialize, parse back into `serde_json::Value` (preserve_order keeps key
   order), and assert the object's keys collect to exactly
   `["event_id","ts","identity","client","tool","action","rw","domain","decision","grant_id","denial_id","duration_ms","manifest"]`.
2. `absent_values_serialize_as_null_not_omitted`: `identity`, `client`, `action`,
   `domain`, `grant_id`, `denial_id`, `manifest` are present and `Value::Null` when
   `None`.
3. `serialized_record_is_a_single_line`: a record whose `tool` contains `"\n"`
   serializes with no raw LF byte in the output.

`src/audit/mod.rs`:

4. `file_destination_appends_one_line_per_record`: a `Recorder::to_file` on a fresh
   temp path; two `record_call`s produce a file with exactly two lines, each parsing
   as a JSON object.
5. `disabled_recorder_writes_nothing`: `Recorder::disabled()` plus `record_call` and
   `set_client`; the temp path is never created.
6. `client_info_is_captured_once_first_wins`: `set_client("a","1")` then
   `set_client("b","2")`; the next record's `client` is `{"name":"a","version":"1"}`.
7. `classification_miss_records_mutate`: `record_call("no_such_tool", None, 0)` and
   `record_call("computer", None, 0)` both produce `rw: "mutate"`.
8. `computer_action_classification_flows_into_rw`:
   `record_call("computer", Some("screenshot"), 0)` gives `rw: "observe"` and
   `action: "screenshot"`; `record_call("computer", Some("left_click"), 0)` gives
   `rw: "mutate"`; `record_call("read_page", None, 0)` gives `rw: "observe"` and
   `action: null`.

`src/audit/destinations.rs`:

9. `default_audit_path_ends_with_browser_mcp_audit_jsonl`: if
   `default_audit_path()` is `Some(p)`, its last two components are `browser-mcp` and
   `audit.jsonl`.

`src/mcp/server.rs` (new inline test module; `#[tokio::test]`, using `Browser::new()`
unconnected so `browser.call` fails fast without any extension):

10. `tools_call_produces_one_audit_record_with_client_identity`: build
    `Recorder::to_file(temp)`; drive `handle_line` with an `initialize` request whose
    params carry `clientInfo: {"name":"test-client","version":"9.9.9"}`, then a
    `tools/call` for `navigate`. Assert the tool response is the `isError` result
    (text contains `not connected`, i.e. today's bytes), and the audit file holds
    exactly one line where `tool == "navigate"`, `action` is null, `rw == "mutate"`,
    `decision == "allow"`, `client.name == "test-client"`,
    `client.version == "9.9.9"`, `identity`/`domain`/`grant_id`/`denial_id`/`manifest`
    are null, `event_id` is a 36-char lowercase UUID with hyphens at offsets
    8/13/18/23, and `ts` is 24 chars, parseable by
    `chrono::DateTime::parse_from_rfc3339`, ending in `Z`.
11. `computer_call_records_action_and_observe_class`: `tools/call` for `computer` with
    `arguments: {"action":"screenshot"}`; the record has `action == "screenshot"` and
    `rw == "observe"`.
12. `invalid_tools_call_without_name_records_nothing`: a `tools/call` whose params lack
    `name` returns the `-32602` error and the audit file is never created.

Integration test, new file `tests/audit_recorder.rs` (public API only; this is the
required "run a call, assert one well-formed JSONL line" check at the recorder
boundary):

13. `a_recorded_call_lands_as_one_wellformed_jsonl_line`: build
    `Recorder::to_file(temp)`, `set_client("claude-code","2.1.0")`, then
    `record_call("computer", Some("left_click"), 42)`. Read the file: exactly one
    line, file content ends with a single `\n`. Parse the line and assert every field:
    the 13 keys in exact shared-format order; `tool == "computer"`,
    `action == "left_click"`, `rw == "mutate"`, `decision == "allow"`,
    `duration_ms == 42`, `client` equals the set identity, the five
    manifest-dependent fields are null, `event_id` and `ts` are well-formed as in
    test 10. Then record a second call and assert the file now has two lines (append,
    not truncate).

Temp-file rule for every test above: use
`std::env::temp_dir().join(format!("browser-mcp-audit-test-{}-{}.jsonl", std::process::id(), <unique-per-test-tag>))`,
delete the file at the start of the test if it exists, and remove it at the end. Never
touch the real default audit path from tests.

## Constraints

Hard rules; every one applies:

1. NEVER modify `src/mcp/schemas/tools.json`, tool names, parameters, or descriptions.
   `tests/tool_schema_fidelity.rs` must pass unchanged. This task does not touch tool
   advertisement.
2. The extension holds mechanism only: no policy, access, redaction, or AUDIT logic in
   extension JS. Records are written by the binary only; no file under `extension/`
   changes.
3. All-open stays first-class: with no manifest and default config, the MCP wire
   behavior (every byte on stdout) is identical to today. The audit JSONL side effect
   is the one sanctioned addition (shared format doc section 4.5: the flight recorder
   records even with no manifest when `audit.enabled` is true; the Minimal default is
   true by design, ADR-0018 step 1). The existing `tests/mcp_protocol.rs` assertions
   must pass unchanged: same responses, same error text, same `tools/list` bytes.
4. ASCII only in ALL code, comments, docs, and this task's outputs: no em-dashes,
   arrows, or curly quotes. Use `--` where the codebase style uses a dash.
5. The engine is truthful: a failed audit write is reported via `tracing::warn!` and a
   recorder disabled at startup (no data dir) is reported via `tracing::warn!`; never
   silently swallowed, and never allowed to break or alter a tool call or its result.
   Do not add any user-facing denial or protection messaging; nothing is being
   protected yet.
6. New runtime dependencies: exactly `uuid` (features `["v4"]`) and `chrono`
   (`default-features = false`, features `["clock", "std"]`), both sanctioned by the
   shared format doc crate note. Nothing else: no `sha2` (not needed here), no `time`,
   no `regex` (assert formats structurally or via `chrono` parsing), no async-log or
   file-rotation crates.
7. Rust 2021 edition; doc comments on every public item (modules, structs, fields kept
   public, enum variants, functions); `cargo fmt` clean;
   `cargo clippy --all-targets -- -D warnings` clean. Unit tests inline; the one new
   integration test file is `tests/audit_recorder.rs`.
8. Do NOT copy code from other projects; implement from the behavior described here.
9. Use the shared format doc's names verbatim: record fields `event_id, ts, identity,
   client, tool, action, rw, domain, decision, grant_id, denial_id, duration_ms,
   manifest`; key names `audit.enabled`, `audit.destination`, `audit.file.path`;
   destination strings `file`, `stderr`; decision string `allow`; rw strings
   `observe`, `mutate`. Do not invent variants, extra fields, or alternate spellings.
10. Field order and null-presence are part of the format: no field reordering, no
    `skip_serializing_if`, compact serialization, single LF terminator.
11. `Config` must remain `Copy` in this task (use `&'static str` for the path field)
    so `handle_line`'s by-value threading keeps compiling without ripple.

## Verification

Run from the repository root:

1. `cargo fmt --check` passes.
2. `cargo clippy --all-targets -- -D warnings` passes.
3. `cargo test` passes, all green, including:
   - the 13 new tests listed above (records, recorder, destinations, server wiring,
     integration);
   - `tests/tool_schema_fidelity.rs` unchanged and passing;
   - `tests/mcp_protocol.rs` unchanged and passing. Note: its subprocess tests spawn
     the real binary with the default config, so they may append a few records to the
     REAL default audit file (`%LOCALAPPDATA%\browser-mcp\audit.jsonl` on Windows).
     That is the designed default behavior of the flight recorder; do not "fix" it
     with env overrides or test-mode flags.
   - every other pre-existing test unchanged and passing.
4. `git status` / `git diff --stat` shows changes ONLY to: `Cargo.toml`, `Cargo.lock`,
   `src/lib.rs`, `src/dispatch.rs`, `src/mcp/server.rs`, `src/policy/mod.rs`, the new
   `src/audit/mod.rs`, `src/audit/record.rs`, `src/audit/destinations.rs`, and the new
   `tests/audit_recorder.rs`. `src/mcp/schemas/tools.json` and everything under
   `extension/` show no diff.
5. Grep the new and changed files for non-ASCII bytes (for example
   `rg -n "[^\x00-\x7F]" src/audit src/dispatch.rs src/mcp/server.rs src/policy/mod.rs tests/audit_recorder.rs`);
   there must be none.
6. Manual smoke check (optional but recommended): rebuild, restart the MCP client
   (binary changes need a client restart), run one tool call from Claude Code, and
   confirm the default audit file gained exactly one line whose `tool`, `rw`,
   `decision: "allow"`, `client`, and `duration_ms` are plausible. If
   `target/debug/browser-mcp.exe` is locked by a running session, rename it aside
   (`mv target/debug/browser-mcp.exe target/debug/browser-mcp.exe.old-1`) and rebuild.

## Out of scope

Fenced off; do not implement any of the following, even partially:

- `syslog`, `http`, and `none` audit destinations. `AuditDestination` has exactly two
  variants; the deferral note lives in its doc comment, nothing more.
- Enforcement of any kind. `PolicyDecision` keeps its single `Allow` variant;
  `policy_check` stays a no-op; no `deny` or `shadow_deny` is ever emitted, no denial
  ids computed (no `sha2`), no denial messages formatted, no sacred-domain checks, no
  domain matching. `decision` is the literal `"allow"` everywhere.
- Populating `identity`, `domain`, `grant_id`, `denial_id`, or `manifest` with anything
  but `None`. No manifest loading, parsing, validation, or content hashing; no
  current-tab URL tracking or extraction of URLs from tool arguments (the `domain`
  field is defined as the CURRENT TAB host, which nothing tracks yet; deriving it from
  `navigate` parameters would be wrong AND a parameter leak).
- The session recap and local activity ledger viewer features, `policy simulate`,
  `policy explain`, and any code that READS audit files. This task only writes them.
- Layered configuration: no user config file loading, no presets, no org policy file,
  no `KeyValue` / `KeyConstraint` growth of `KeyDef`, no registration of
  `audit.destination` / `audit.file.path` in `KEYS` (constants only, as specified), no
  `config list` / `config set` CLI, no native-messaging settings protocol, no JSON
  Schema generation.
- Any environment-variable override for audit settings (for example a
  `BROWSER_MCP_AUDIT_FILE`). The `BROWSER_MCP_ENDPOINT` precedent does NOT extend
  here: an env override would sit above the future layered resolver and let a user
  bypass an org-locked `audit.file.path` (ADR-0019). Configuration reaches the
  recorder through `Config` only.
- Logging tool parameters, screenshots, full URLs, page content, result payloads, or
  any field beyond the 13 defined ones. No opt-in parameters key.
- Buffered, batched, or async audit writers; background flush tasks; file rotation;
  size caps; file locking. One synchronous open-append-close per record.
- Changes to `extension/` (no reload needed), `src/main.rs`, `src/browser.rs`,
  `src/policy/classify.rs`, `src/policy/redact.rs`, the existing test files, or any
  document under `docs/` (SPEC amendments for the superseded audit schema are tracked
  in the shared format doc's "SPEC updates needed" list, items 6 and 9, as a separate
  docs task).
- `Display`, `FromStr`, or `Deserialize` impls for the record types or
  `AuditDestination`; conversion surface waits for a consumer that proves the need.
