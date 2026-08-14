# T07: Doctor subcommand fusing debug state into one diagnosis

## Goal

When the browser connection fails, the user has no one-command answer to "where did it
die". Extend the existing `browser-mcp doctor` subcommand so it fuses everything the binary
already knows (installer registration state, the per-pid debug state files, a live probe of
the IPC endpoint) into one plain-text diagnosis that ends with verdict lines and a truthful
exit code. Along the way, add the small pieces of instrumentation the diagnosis needs: a
role tag and a recorded MCP client name in the debug snapshot, and debug state files for
the native-host role, which today writes none.

## Project context

Browser MCP is a governed browser automation system. A single Rust binary is both the MCP
server (JSON-RPC 2.0 over stdio, hand-rolled, tokio) and the Chrome native-messaging host.
A thin Manifest V3 extension executes CDP commands. The chain is:

    MCP Client <--stdio--> Binary <--native messaging--> Extension <--CDP--> Browser

The two binary roles run as separate OS processes bridged by tokio-native named-pipe (on
Windows) or Unix-domain-socket (elsewhere) IPC:

- The mcp-server role (default, no subcommand) is launched by the MCP client over stdio.
  It owns the IPC endpoint, serves it, and runs the JSON-RPC loop.
- The native-host role is launched by Chrome via `connectNative`; Chrome passes the calling
  extension origin (`chrome-extension://<id>/`) as a positional argument, which is how the
  role is detected. It connects to the mcp-server endpoint and relays native-messaging
  frames both ways.
- `install` / `uninstall` / `doctor` / `status` are synchronous subcommands with no async
  runtime.

Files relevant to this task (all paths relative to the repo root):

- `src/main.rs`: clap CLI, role dispatch, the debug-sink construction. You will modify it.
- `src/debug.rs`: the observability sink. Writes `debug-state-<pid>.json` (live snapshot)
  and `debug-events-<pid>.jsonl` (append-only event stream) under the log directory when
  debug mode is on. You will extend it.
- `src/install/mod.rs`: the installer, including the current registration-only
  `run_doctor`. You will remove `run_doctor` and `DoctorOptions` from it and widen one
  helper's visibility.
- `src/install/native_host.rs` and `src/install/clients.rs`: detection and registration
  helpers (`BROWSERS`, `detect_browser`, `win_reg_key`, `read_default`, `CLIENTS`,
  `detect`, `config_path`). All already `pub`; read-only for this task.
- `src/native/ipc.rs`: the IPC transport (serve/connect/relay). You will add a synchronous
  endpoint probe and thread a debug sink through the relay.
- `src/mcp/server.rs`: the JSON-RPC loop. You will add clientInfo capture to the
  `initialize` handler.
- `src/lib.rs`: module declarations. You will add one line.
- `src/doctor.rs`: NEW FILE. The fused doctor lives here.
- `src/mcp/schemas/tools.json`: SACRED. Byte-frozen official Claude-in-Chrome v1.0.78 tool
  schemas. Never edit it, never touch tool names, parameters, or description strings.
- `tests/tool_schema_fidelity.rs`, `tests/mcp_protocol.rs`, `tests/peer_death.rs`: existing
  integration tests. All must keep passing WITHOUT modification.
- `extension/`: not part of this task. Do not touch anything under it.

Build and test:

- Run `cargo test` from the repo root; all tests must pass. Also run `cargo fmt` and
  `cargo clippy --all-targets -- -D warnings`; both must be clean.
- Binary changes require an MCP client restart to observe in a live session.
- If `target/debug/browser-mcp.exe` is locked by a running session, rename it aside first
  (for example: `mv target/debug/browser-mcp.exe target/debug/browser-mcp.exe.old-1`) and
  rebuild.

## Current behavior

All line numbers verified against the working tree as it stands now.

`src/main.rs` (282 lines):

- Lines 43-53: the `Command` enum already has `Doctor(DoctorArgs)` (line 50) and
  `Status(StatusArgs)` (line 52). `DoctorArgs` (lines 105-110) carries one flag,
  `--verbose`.
- Lines 163-164: debug mode is detected before clap from the `BROWSER_MCP_DEBUG` env var
  or a `--debug` argument in any position.
- Line 169: the native-host role is selected when any argument starts with
  `chrome-extension://`.
- Lines 182-185: the doctor arm calls `browser_mcp::install::run_doctor(args.into())`. It
  can only exit 0 (run_doctor always returns `Ok`).
- Lines 212-226: `run_native_host_role()` builds NO debug sink. It runs
  `ipc::relay_native_host(&ipc::default_endpoint())` and then calls
  `std::process::exit(0)` (line 225). The comment above it explains why the direct exit is
  load-bearing: tokio's stdin reader parks a blocking thread on Chrome's still-open stdin
  and dropping the runtime would hang forever (the native-host zombie fix). This exit MUST
  survive your change.
- Lines 230-259: `run_server` builds the sink via `build_debug_sink(debug_on)` (line 236)
  and flushes it after the MCP loop ends (line 255).
- Lines 263-281: `build_debug_sink(debug: bool) -> DebugSink` returns a disabled sink when
  debug is off or no log dir is available, otherwise `DebugSink::enabled(&dir)`.

`src/debug.rs` (657 lines):

- The module serves the mcp-server role ONLY (module doc, lines 1-17). Do not assume
  both roles write debug files: today the native-host role writes nothing.
- Lines 53-58: `log_dir()` is `BROWSER_MCP_LOG_DIR` if set, else
  `<data-local>/browser-mcp`. Already `pub`.
- Lines 73-84: `fmt_ms` formats a millisecond duration ("3m 12s", "800ms"). Private.
- Lines 87-104: `session_state_files(dir)` lists `debug-state-*.json` newest-mtime-first.
  Private.
- Lines 45-50: `now_ms()`. Private.
- Lines 107-129: `cleanup_stale` removes session files older than 24h (`STALE_AFTER`,
  line 42) when a new debug session starts. So doctor may see files up to 24h old.
- Lines 139-247: `status_report()` (backs `browser-mcp status`) reads only the NEWEST state
  file, whatever it is.
- Lines 281-290: the `Snapshot` struct serialized to `debug-state-<pid>.json` has `pid`,
  `started_ms`, `updated_ms`, `extension_connected`, `in_flight`, `counters`, `recent`.
  There is NO role field and NO client field.
- Lines 308-322: `Inner::record` appends to the JSONL log and rewrites the snapshot when
  forced or when 200ms (`STATE_THROTTLE_MS`, line 39) have passed.
- Lines 325-346: `write_state` uses `serde_json::to_string_pretty` (line 335) and a
  temp-plus-rename. The pretty format matters: `tests/peer_death.rs` line 87 greps the raw
  file for the exact substring `"extension_connected": true`.
- Lines 363-389: `DebugSink::enabled(dir: &Path)` takes no role.
- Lines 518-525: `frame_out` / `frame_in` bump counters only; they never rewrite the
  snapshot, so `updated_ms` does NOT track frame traffic today.
- Lines 529-556: `set_connected` forces a snapshot write and records an "ipc" event.
- Tests at lines 618 and 647 call `DebugSink::enabled(&dir)`.

`src/install/mod.rs` (1015 lines):

- Lines 58-61: `DoctorOptions { verbose: bool }`.
- Lines 719-757: `run_doctor` prints `browser-mcp doctor`, a `Binary: <exe>` line, a
  `Browsers:` section (`{:<16} detected={:<5} registered={}` rows; on Windows registered
  means the registry key default value exists in HKCU native view or HKLM both views; on
  Unix it means the host manifest file exists), and an `MCP clients:` section (registered
  means the client config file contains the substring `"browser-mcp"`). It ignores
  `--verbose` (binding is `_opts`) and always returns `Ok(())`.

`src/native/ipc.rs` (343 lines):

- Line 27: `DEFAULT_ENDPOINT` is `org.sylin.browser_mcp.v1`; lines 30-32:
  `default_endpoint()` honors the `BROWSER_MCP_ENDPOINT` env override.
- Lines 43-71: `relay_native_host(endpoint)` connects, splits, and runs two forwarding
  loops under `tokio::select!`. The comment at lines 38-42 forbids adding a
  post-select `shutdown().await` (it would hang on a dead Windows pipe). No sink is
  threaded through.
- Lines 76-78: `pipe_path(endpoint)` (Windows, private) is `\\.\pipe\<endpoint>`.
- Lines 83-117: Windows `serve` creates the first pipe instance
  (`first_pipe_instance(true)`; raw OS errors 5 and 231 map to `Error::SessionBusy`) and
  pre-creates the next instance before attaching each accepted connection.
- Lines 229-234: `socket_path(endpoint)` (Unix, private) is
  `<runtime-or-cache-dir>/browser-mcp/<endpoint>.sock`.
- Lines 244-285: Unix `serve` binds, treating a connect-able existing socket as
  `SessionBusy` and a stale file as removable.

`src/mcp/server.rs` (156 lines):

- Line 86: the `initialize` arm ignores its params entirely, so the MCP client's
  self-reported `clientInfo` (`{ name, version }` inside the initialize params) is never
  recorded anywhere.

`src/browser.rs`:

- Lines 120-150: `attach` sets `set_connected(true)` on entry (line 127) and
  `set_connected(false)` when the stream closes (line 145). Consequence for the probe you
  will build: a probe connection against an idle live server gets accepted and attached
  for an instant, producing one phantom connect/disconnect pair in that server's debug
  state; against a server with a live native-host attached, the probe just queues and is
  drained harmlessly later. It never breaks a live attachment. The doctor output must
  disclose that it probed.

Tests:

- `tests/mcp_protocol.rs` line 51 sends `initialize` with `params: {}` (no clientInfo);
  your capture must tolerate that silently.
- `tests/peer_death.rs` spawns the server WITH `BROWSER_MCP_DEBUG=1` and
  `BROWSER_MCP_LOG_DIR` but spawns the native-host WITHOUT them (lines 26-47), then polls
  the newest state file for `"extension_connected": true`. Your native-host
  instrumentation is env-gated, so that test sees no host file and keeps working. Do not
  edit it.

`Cargo.toml`: package version is currently 0.1.0; print it via `env!("CARGO_PKG_VERSION")`.

## Required behavior

Seven parts. Do them in this order; each later part depends on the earlier ones.

### Part A: role and client in the debug snapshot (`src/debug.rs`)

1. Change `DebugSink::enabled(dir: &Path)` to
   `DebugSink::enabled(dir: &Path, role: &'static str)`. Store the role in `Inner` and
   serialize it in `Snapshot` as a new field `role` (a plain string). The two callers in
   this file's tests (lines 618 and 647) pass `"mcp-server"`.
2. Add `client: Option<String>` to `Inner` (initialized `None`) and a matching `Snapshot`
   field serialized as `client`, with `#[serde(skip_serializing_if = "Option::is_none")]`,
   so old-format consumers see no change when it is unset.
3. Add a public method:

       /// Record the MCP client's self-reported identity (from the initialize params).
       pub fn set_client(&self, client: &str)

   Behavior: under the lock, set `Inner.client` to the `ident`-clipped value and record an
   event with `kind: "mcp"`, `dir: "-"`, summary `client <clipped value>`, no detail, with
   `force_state: true` so the snapshot immediately carries it.
4. Add a public method:

       /// Record a one-line IPC lifecycle note (used by the native-host role).
       pub fn ipc_note(&self, summary: &str)

   Behavior: record an event with `kind: "ipc"`, `dir: "-"`, the `ident`-clipped summary,
   no detail, `force_state: true`.
5. Make `frame_in` and `frame_out` refresh `updated_ms`: after incrementing the counter,
   perform the same throttled snapshot write `record` does (write only when 200ms have
   passed since `last_state_ms`, updating `last_state_ms`). Extract a small
   `Inner::touch(&mut self)` helper for it. Do NOT log an event per frame (the existing
   comment explains frames are too chatty for the event stream); this is a state-file
   refresh only. `record`'s own semantics stay identical.
6. Widen visibility to `pub(crate)` on exactly three items so the doctor module can reuse
   them: `session_state_files`, `fmt_ms`, `now_ms`. Do not make them `pub`.
7. Make `status_report()` role-aware: read and parse the state files newest-first and keep
   only those that parse as JSON and whose `role` field is missing (old format) or equals
   `"mcp-server"`. Report from the first such candidate exactly as today. The trailing
   multi-session note now counts candidates, not raw files. New failure texts:
   - state files exist but no candidate parses or has an mcp-server role:
     `no mcp-server debug state under <dir> (state files exist for other roles or are unreadable)`
   - everything else (no dir, no files at all) keeps the existing messages.
8. Tests in `src/debug.rs`: update the two existing `enabled` calls, and extend
   `enabled_sink_tracks_state_and_writes_files` (or add one new test) to assert that after
   `set_client("claude-code 1.2.3")` and `flush()`, the snapshot has
   `snap["role"] == "mcp-server"` and `snap["client"] == "claude-code 1.2.3"`.

### Part B: capture clientInfo on initialize (`src/mcp/server.rs`)

In `handle_line`, change only the `initialize` arm (line 86). Before building the
response, read `params.clientInfo` from the raw request value: if `clientInfo.name` is a
string, call `browser.debug().set_client(...)` with `"<name> <version>"` when
`clientInfo.version` is also a string, else just `"<name>"`. Missing params, missing
clientInfo, or non-string fields are silently fine (record nothing). The response itself
is byte-identical to today. `set_client` is already a no-op when debug is off, so no
`is_enabled` guard is needed.

### Part C: native-host instrumentation (`src/main.rs`, `src/native/ipc.rs`)

1. `build_debug_sink` gains a role parameter:
   `fn build_debug_sink(debug: bool, role: &'static str) -> DebugSink`. `run_server`
   passes `"mcp-server"`.
2. `run_native_host_role` gains the debug flag: `fn run_native_host_role(debug: bool)`,
   and the call at line 170 becomes `return run_native_host_role(debug);`. Inside, build
   `let sink = build_debug_sink(debug, "native-host");`, pass `&sink` to the relay, and
   call `sink.flush()` after `block_on` returns, BEFORE the `std::process::exit(0)`. The
   `std::process::exit(0)` and its zombie-fix comment stay exactly as they are.
3. `relay_native_host` signature becomes
   `pub async fn relay_native_host(endpoint: &str, debug: &crate::debug::DebugSink) -> Result<()>`.
   Instrumentation, and nothing else, changes inside:
   - immediately after `connect` succeeds: `debug.ipc_note("connected to mcp-server endpoint");`
   - in the upstream loop, after each successful read from Chrome's stdin:
     `debug.frame_in();`
   - in the downstream loop, after each successful write to Chrome's stdout:
     `debug.frame_out();`
   - after the `tokio::select!` completes: `debug.ipc_note("relay ended");`
   Do NOT add a post-select `shutdown()` (the existing comment forbids it) and do not
   alter the forwarding logic, the retry loop, or the connect timeout.
4. Honesty note you must reflect in doc comments and in the doctor's wording: the
   native-host process inherits Chrome's environment, and Chrome does not pass `--debug`,
   so native-host state files appear only when Chrome itself was launched with
   `BROWSER_MCP_DEBUG=1` in its environment. Their absence is normal and must never be a
   doctor finding by itself.

### Part D: synchronous endpoint probe (`src/native/ipc.rs`)

Add, with doc comments on every public item:

    /// Result of a one-shot, synchronous probe of the IPC endpoint.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum EndpointProbe {
        /// No pipe/socket of this name exists: no mcp-server currently owns the endpoint.
        Absent,
        /// The endpoint exists and accepted a connection (opened and closed immediately).
        Accepts,
        /// The endpoint exists but the probe could not connect (detail explains why).
        Rejects(String),
    }

    pub fn probe_endpoint(endpoint: &str) -> EndpointProbe
    pub fn endpoint_display(endpoint: &str) -> String

Both are synchronous (no tokio; doctor has no runtime) and cfg-gated per platform like
`serve`/`connect`:

- Windows `probe_endpoint`: open the pipe with
  `std::fs::OpenOptions::new().read(true).write(true).open(pipe_path(endpoint))`.
  `Ok(file)`: drop the handle at once and return `Accepts`. `ErrorKind::NotFound`:
  `Absent`. `raw_os_error() == Some(231)` (all instances busy): return
  `Rejects("all pipe instances are busy".into())`. Any other error:
  `Rejects(e.to_string())`.
- Unix `probe_endpoint`: if `socket_path(endpoint)` errors, return `Rejects(<the error
  message>)`. If the path does not exist, `Absent`. Otherwise
  `std::os::unix::net::UnixStream::connect(&path)`: `Ok` means `Accepts` (drop it);
  `ErrorKind::ConnectionRefused` means
  `Rejects("socket file exists but nothing is listening (stale)".into())`; any other
  error, `Rejects(e.to_string())`.
- `endpoint_display`: Windows returns `pipe_path(endpoint)`; Unix returns the socket path
  display string, or, when `socket_path` errors, `(unresolvable: <error>)`.

The probe writes no bytes; it only opens and closes. Document (one comment) the known,
harmless side effect: a live idle server briefly attaches the probe, logging one phantom
connect/disconnect pair in its own debug state.

Tests to add in the existing `mod tests` of `ipc.rs`:

- `probe_reports_absent_for_an_unused_endpoint`: plain `#[test]`; a pid-unique endpoint
  name yields `EndpointProbe::Absent`.
- `probe_reports_accepts_against_a_live_server`: `#[tokio::test]`; spawn
  `serve(Browser::new(), endpoint)` on a pid-unique endpoint, then poll
  `tokio::task::spawn_blocking` around `probe_endpoint` (small sleeps between attempts,
  overall bound of a few seconds) until it returns `Accepts`, panicking on timeout.

### Part E: the fused doctor (`src/doctor.rs`, new file)

Create `src/doctor.rs` with a module doc comment describing it as the one-shot, read-only
diagnosis that fuses registration state, debug sessions, and a live endpoint probe.
Declare it in `src/lib.rs` as `pub mod doctor;` (alphabetically between `dispatch` and
`error`). Public API:

    /// Options for `browser-mcp doctor`.
    pub struct DoctorOptions { pub verbose: bool }

    /// Run the diagnosis; prints the report and returns Ok(true) when healthy.
    pub fn run(opts: DoctorOptions) -> Result<bool>

`DoctorOptions` MOVES here from `src/install/mod.rs` (delete it and `run_doctor` there;
also delete the now-unused `yesno` helper if nothing else uses it). In
`src/install/mod.rs`, change `host_file_path` from private to `pub(crate)` so doctor can
call it. Doctor never writes, deletes, or kills anything; its only side effect is the one
probe connection.

Data gathering, in order (an `Err` may come only from `PlanCtx::resolve()`; everything
else degrades to a printed line, never an early return):

1. `let ctx = crate::install::PlanCtx::resolve()?;`
2. Browser rows: for each `crate::install::native_host::BROWSERS` entry compute detected
   and registered exactly as the old `run_doctor` did (Windows: HKCU native view or HKLM
   both views has the key's default value; Unix: `host_file_path` exists).
3. Client rows: for each `crate::install::clients::CLIENTS` entry, detected via
   `clients::detect`, registered when the config file at `clients::config_path` reads and
   contains the substring `"browser-mcp"` (quotes included), as today.
4. Endpoint: `let endpoint = crate::native::ipc::default_endpoint();` then
   `endpoint_display` and `probe_endpoint`.
5. Sessions: `crate::debug::log_dir()`, then `crate::debug::session_state_files(&dir)`
   (already newest-first), each read and parsed by a testable helper:

       fn parse_session(raw: &str) -> Option<Session>

   where `Session` holds `role: String` (defaulting to `"mcp-server"` when the field is
   absent, for old-format files), `pid: u64` (required; `None` when missing or when the
   input is not JSON), `started_ms: u64`, `updated_ms: u64`, `extension_connected: bool`,
   `client: Option<String>`, and the seven counters (`mcp_requests`, `tool_calls`,
   `tool_errors`, `frames_out`, `frames_in`, `connects`, `disconnects`), all defaulting to
   zero/false/None when absent. Parse with `serde_json::Value` lookups, tolerantly.

Report layout, printed with `println!`, exactly these sections in this order. `<yn>` is
`yes` or `no`; padding matches the existing rows (`{:<16}` names, `{:<5}` detected).

    browser-mcp doctor

    Binary:
      path     <ctx.current_exe display>
      version  <env!("CARGO_PKG_VERSION")>

    Browsers:
      <display>        detected=<yn>   registered=<yn>
      ... one row per known browser ...

    MCP clients:
      <display>        detected=<yn>   registered=<yn>
      ... one row per known client ...

    IPC endpoint:
      path     <endpoint_display>
      state    <state line>

    Debug sessions (<log dir display>):
      <rows, see below>

    Verdict:
      <verdict lines, see below>

State line, by probe result:

- `Accepts`: `accepts connections (doctor made one brief probe connection)`
- `Absent`: `absent (no mcp-server currently owns it)`
- `Rejects(d)`: `exists but rejected the probe: <d>`

Debug session rows, newest first. When `log_dir()` is `None`, the section header is just
`Debug sessions:` and the single row is
`  (no log directory available on this platform)`. When the directory has no state files
(or does not exist), the single row is
`  (none found; a session run with --debug or BROWSER_MCP_DEBUG=1 writes them)`.
A file that fails to read or parse renders as
`  (skipping unreadable state file: <file name>)` and is otherwise ignored.
A parsed session renders as one line; durations use `crate::debug::fmt_ms` applied to
`now_ms().saturating_sub(...)`:

- role `mcp-server`:
  `  mcp-server   pid <pid>  started <S> ago  active <A> ago  client <C>  extension <E>`
  where `<C>` is the recorded client string or `(not recorded)`, and `<E>` is `connected`
  or `not connected`.
- any other role (today only `native-host`):
  `  <role padded to 12> pid <pid>  started <S> ago  active <A> ago`

Without `--verbose`, show at most 6 session rows and then, if more were parsed,
`  (and <n> older; use --verbose to show all)`. With `--verbose`, show all rows, and add
under every row one counters line:
`      counters: requests=<n> tools=<n> errors=<n> frames_out=<n> frames_in=<n> connects=<n> disconnects=<n>`

After the rows, if at least one `native-host` session was parsed, add one line using the
newest such session (this is the extension-last-seen signal: the relay only moves frames
while the extension's port is alive):
`  extension last seen <A> ago (native-host pid <pid>)`

Verdict. Compute the findings with a pure function so it is unit-testable:

    struct Observations {
        any_browser_registered: bool,
        any_client_registered: bool,
        probe: EndpointProbe,
        sessions_present: bool,          // any state file parsed, either role
        newest_server: Option<NewestServer>, // newest parsed mcp-server session
    }
    struct NewestServer { pid: u64, extension_connected: bool, connects: u64 }

    fn findings(obs: &Observations) -> Vec<String>

Rules, evaluated in this order, each appending one string when it fires:

1. `!any_browser_registered`:
   `the native messaging host is not registered for any browser: run browser-mcp install, then reload the extension at chrome://extensions`
2. `!any_client_registered`:
   `browser-mcp is not registered with any MCP client: run browser-mcp install`
3. probe is `Absent`:
   `no mcp-server is running (the IPC endpoint does not exist): start or restart your MCP client so it launches browser-mcp`
4. probe is `Rejects(d)`; with a newest_server pid:
   `the IPC endpoint exists but rejected a connection (<d>): a stale browser-mcp process may still hold it; try killing pid <pid> and restarting your MCP client`
   without one:
   `the IPC endpoint exists but rejected a connection (<d>): find and kill the stale browser-mcp process with your process manager, then restart your MCP client`
5. probe is `Accepts` and `newest_server` is `None`:
   `an mcp-server is running but wrote no debug state: restart the session with --debug (or BROWSER_MCP_DEBUG=1) and re-run doctor for a full diagnosis`
6. probe is `Accepts`, `newest_server` is `Some`, and `extension_connected` is false:
   when `connects == 0`:
   `the extension never connected in the newest session (pid <pid>): check that the extension is loaded and enabled at chrome://extensions and that the browser is running; if it persists, re-run browser-mcp install and restart the browser`
   when `connects > 0`:
   `the extension is disconnected from the mcp-server (pid <pid>; it connected <connects> time(s) earlier in this session): the extension service worker may be stopped; inspect it at chrome://extensions or restart the browser`
7. `!sessions_present` and probe is NOT `Accepts`:
   `no debug instrumentation found: run a session with --debug (or set BROWSER_MCP_DEBUG=1) and re-run doctor`
   (this fires in addition to rule 3 or 4; with `Accepts`, rule 5 already covers it)

Rendering: when the findings vector is empty print exactly
`  OK: mcp-server (pid <pid>) is running, the extension is connected, and the IPC endpoint accepts connections.`
(the pid is `newest_server`'s; note the vector can only be empty when it exists), and
`run` returns `Ok(true)`. Otherwise print `  problem: <finding>` for each, in order, and
return `Ok(false)`.

Unit tests, inline `#[cfg(test)]` in `src/doctor.rs`, covering at minimum:

- all-healthy observations produce an empty findings vector;
- registered=false browsers and clients each produce their finding;
- `Absent` with no sessions produces exactly rules 3 and 7, in that order;
- `Rejects` with a known pid embeds that pid in the text; without one it says
  `process manager`;
- `Accepts` with no server session produces rule 5;
- `Accepts` with `connects == 0` says `never connected`; with `connects == 3` says
  `disconnected` and `3 time(s)`;
- `parse_session` on a full new-format JSON extracts role, pid, client, counters; on an
  old-format JSON (no role, no client) defaults role to `mcp-server` and client to
  `None`; on garbage or on JSON missing `pid` returns `None`.

### Part F: CLI wiring (`src/main.rs`, `src/install/mod.rs`, `src/lib.rs`)

1. `src/lib.rs`: add `pub mod doctor;`.
2. `src/main.rs`: import `DoctorOptions` from `browser_mcp::doctor` instead of
   `browser_mcp::install` (the `From<DoctorArgs>` impl at lines 154-158 stays, retargeted
   to the moved type). Replace the doctor arm with:

       Cli {
           command: Some(Command::Doctor(args)),
           ..
       } => {
           if !browser_mcp::doctor::run(args.into())? {
               std::process::exit(1);
           }
       }

   So the process exits 0 when healthy and 1 when any problem was detected. Update the
   `Doctor` variant's doc comment (line 49) to:
   `/// Diagnose the whole chain: registration, debug sessions, IPC endpoint, extension link.`
3. `src/install/mod.rs`: remove `run_doctor` and `DoctorOptions`; make `host_file_path`
   `pub(crate)`; remove `yesno` if now unused. Touch nothing else in the installer.

### Part G: regression guarantees

- `tests/peer_death.rs`, `tests/mcp_protocol.rs`, and `tests/tool_schema_fidelity.rs`
  pass without any edit. In particular the snapshot stays pretty-printed with the
  `extension_connected` key (peer_death greps for `"extension_connected": true`), and
  `initialize` with empty params still succeeds byte-identically.
- `browser-mcp status` keeps working and now ignores native-host state files (Part A.7).

## Constraints

1. NEVER modify `src/mcp/schemas/tools.json`, tool names, parameters, or description
   strings. `tests/tool_schema_fidelity.rs` must pass unchanged.
2. The extension holds mechanism only: no policy, access, or redaction decisions in
   extension JS. This task touches no extension file at all.
3. ASCII only in all code and docs: no em-dashes, no arrows, no curly quotes, anywhere,
   including comments. All doctor output is plain ASCII.
4. The engine is truthful: never fake success, never silently substitute behavior; when
   something failed or was recovered, say so. Doctor discloses its probe connection,
   reports unreadable files as skipped, and never claims health it cannot attest.
5. No new runtime dependencies. No `sysinfo`, no process-enumeration crates; liveness is
   inferred only from the endpoint probe. The extension stays vanilla JS.
6. Rust: 2021 edition, thiserror for typed errors in library code, doc comments on every
   public item, module doc comments, rustfmt clean, clippy with deny warnings.
7. Comments only for constraints the code cannot express; match the surrounding comment
   density and style (this codebase comments the why generously; follow suit for the
   probe side effect, the env-gated host instrumentation, and the preserved
   `process::exit(0)`).
8. Do NOT copy code from the official Anthropic extension or any other project; implement
   the behavior described above from scratch.

Task-specific:

9. Doctor is one-shot and read-only: it must not spawn a tokio runtime, must not delete
   or write any file, must not kill any process, and must not modify any registration.
   Hints tell the user what to do; doctor never does it for them.
10. Doctor must work without `--debug`: no debug files is a reported finding (rule 7 /
    rule 5), never a crash, and never an `Err`.
11. Keep `std::process::exit(0)` at the end of the native-host role, with its comment;
    flush the sink before it.
12. Do not change `DEFAULT_ENDPOINT`, the serve/connect logic, the pipe DACL, retry
    counts, or timeouts in `src/native/ipc.rs` beyond the additions specified.
13. Exit-code semantics of `install`, `uninstall`, and `status` stay untouched.

## Verification

1. From the repo root: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and
   `cargo test` all clean. No file under `tests/` was edited.
2. Rebuild the binary (rename `target/debug/browser-mcp.exe` aside first if a session
   holds it locked). Binary changes require an MCP client restart to observe.
3. With no MCP session running: run `browser-mcp doctor`. Expect the Binary, Browsers,
   MCP clients, IPC endpoint (state `absent ...`), and Debug sessions sections; the
   verdict must contain `problem:` lines including the no-server line, and the exit code
   must be 1 (`echo $?` in bash, `$LASTEXITCODE` in PowerShell).
4. Start a debug session (the dev install registers the server with
   `BROWSER_MCP_DEBUG=1`; otherwise restart the MCP client after adding it), make one tool
   call so the extension attaches, then run `browser-mcp doctor`:
   - IPC endpoint state is `accepts connections (doctor made one brief probe connection)`;
   - the newest mcp-server row shows the client name/version the MCP client reported in
     initialize, and `extension connected`;
   - the verdict is the single `OK: ...` line and the exit code is 0.
5. Disable the extension at `chrome://extensions` (or stop its service worker), wait for
   the disconnect, re-run doctor: expect the `extension is disconnected` problem naming
   the server pid, exit code 1. Re-enable and confirm doctor returns to OK.
6. Run `browser-mcp doctor --verbose` and confirm every session row gains its
   `counters:` line and the row cap is lifted.
7. Run `browser-mcp status` during the debug session and confirm it still renders the
   mcp-server report (role filtering did not break it).
8. Optional (native-host state files): launch the browser from a shell with
   `BROWSER_MCP_DEBUG=1` set, run a session, and confirm doctor shows a `native-host` row
   and the `extension last seen ... (native-host pid ...)` line. Absence of this file in
   a normal launch must not produce any problem line.

## Out of scope

- Any daemon, watch, polling, or service mode. Doctor runs once, prints, exits. No
  `--watch` flag, no loops, no background threads.
- Auto-fixing anything: no killing processes, no deleting stale sockets or state files,
  no re-registering hosts or clients, no launching browsers. Hints only.
- Any change under `extension/` (service worker, content script, visual indicator,
  manifest).
- Any change to `src/mcp/schemas/tools.json`, the tools/list surface, tool routing, or
  any tool result text.
- New dependencies in `Cargo.toml`, including dev-dependencies.
- Process liveness checks beyond the endpoint probe (no PID probing, no /proc walking,
  no Windows toolhelp snapshots).
- Changing the debug file naming scheme, the 24h stale cleanup, the 200ms throttle, the
  64-event recent ring, the JSONL event format, or the pretty-printed snapshot format
  (only the two additive fields `role` and `client` are allowed).
- Changing `browser-mcp status` beyond the role filtering described in Part A.7; the
  `--json` raw mode stays as it is.
- Reworking installer planning/apply logic, `Selection`, merges, or registration paths;
  the only installer edits are the removals and the one visibility change in Part F.3.
- Network or HTTP anything: no remote reporting, no telemetry, no version checks.
