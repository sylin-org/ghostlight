# exe-split SPEC: the normative implementation pins for ADR-0046

This document is NORMATIVE for the exe-split batch (tasks S1..S10). Task prompts CITE it instead
of restating semantics. Where a task file and this SPEC disagree, THIS SPEC WINS. Where this SPEC
and the live tree disagree on a fact (not a requirement), STOP and record in the LEDGER.

Authority chain: ADR-0046 (the decision) -> this SPEC (implementation pins) -> task files
(procedure). ADR-0044 (instance identity) and ADR-0045 (resilient adapter) remain in force;
ADR-0046 refines both.

## Provenance (decided; never re-litigate)

- Split into three executables named `ghostlight`, `ghostlight-adapter-agent`,
  `ghostlight-adapter-browser`: decided by the owner 2026-07-08 (names chosen by the owner
  explicitly; "adapter-" prefix + the side each faces). ADR-0046 Decision 1.
- Two library crates `ghostlight-transport` (stable) / `ghostlight-core` (churny); adapters depend
  on transport ONLY: ADR-0046 Decision 2.
- Instance identity: ADR-0044 (Accepted). Default byte-identical; org policy machine-wide.
- Resilient adapter semantics: ADR-0045 (Accepted). Reconnect patience constants: this SPEC
  section 8 (S8 amends ADR-0045 with the same numbers).
- Bare `ghostlight` (no subcommand) stops being an MCP adapter: ADR-0046 Decision 1 / migration
  step 5. Guidance message pinned in section 9.
- `--no-supervisor` install flag: owner-approved direction (dev instances run the service in a
  terminal; a dev supervisor would resurrect and lock the exe during rebuilds).

## 1. Workspace layout (end state)

Root package `ghostlight` STAYS at the repo root (src/, tests/ unchanged in place); the workspace
adds member crates:

```
Cargo.toml                      # [package] ghostlight + [workspace]
src/main.rs                     # CLI + service shell only (after S7)
src/lib.rs                      # thin facade re-exporting core + transport (section 6)
tests/                          # integration tests, UNCHANGED imports (facade keeps ghostlight:: paths)
crates/transport/               # package ghostlight-transport (lib)
crates/core/                    # package ghostlight-core   (lib)
crates/adapter-agent/           # package ghostlight-adapter-agent   (bin only)
crates/adapter-browser/         # package ghostlight-adapter-browser (bin only)
```

Root `Cargo.toml` gains:

```toml
[workspace]
members = [".", "crates/transport", "crates/core", "crates/adapter-agent", "crates/adapter-browser"]
resolver = "2"
```

Every member crate sets `publish = false` and `edition = "2021"`. License metadata per crate:
- transport, adapter-agent, adapter-browser: `license = "Apache-2.0 OR MIT"`.
- core: `license-file = "../../LICENSE"` (it contains `src/governance`, the commercial module;
  the root LICENSE holds the split notice).
- root package keeps its existing `license-file = "LICENSE"` and `[package.metadata.binstall]`.

Dependency direction (the load-bearing rule): `adapter-agent` and `adapter-browser` depend on
`ghostlight-transport` ONLY (plus tokio/tracing). `core` depends on `transport`. Root depends on
`core` + `transport`. A core dependency in an adapter crate is a design error; STOP if a task seems
to require one.

## 2. Crate `ghostlight-transport`: module disposition

Flat module layout (no nested `native/` dir). Source files MOVE (git mv) from the root package;
each keeps its SPDX header (`Apache-2.0 OR MIT` for every file listed here).

| transport module | moved from | notes |
|---|---|---|
| `error` | `src/error.rs` | whole file (Error, Result, ToolError). |
| `proc` | `src/proc.rs` | whole file. |
| `instance` | `src/instance.rs` | whole file; S6 adds `from_exe_stem_with_base` (section 7). |
| `observability` | `src/observability.rs` | whole file, PLUS `build_debug_sink` moved in from `src/hub/mod.rs`. |
| `watchdog` | `src/transport/watchdog.rs` | whole file. |
| `role` | `src/hub/role.rs` | whole file. |
| `host` | `src/transport/native/host.rs` | whole file (framing + `host_name()` + `HOST_DESCRIPTION` + `MAX_MESSAGE_LEN`). |
| `handshake` | `src/hub/handshake.rs` | whole file (HUB_PROTO, ROLE_ADAPTER, ROLE_CONTROL, ROLE_SERVICE_PROOF). |
| `antisquat` | `src/hub/antisquat.rs` | whole file (hub-key read/create, mac compute/verify, REFUSAL_MESSAGE). |
| `session_guid` | split from `src/hub/session.rs` | ONLY `SessionGuid` (struct + mint/parse/as_str + Display/Debug + its unit tests). The registry/PeerCred/owned-tab parts stay behind. |
| `supervisor` | `src/hub/supervisor.rs` | whole file (task/label/unit name fns, SELF_HEAL_* consts, SELF_HEAL_FAILURE_MESSAGE, supervisor_start_command, start_service). |
| `ipc` | ADAPTER half of `src/transport/native/ipc.rs` | items listed below. |

`transport::ipc` receives exactly these items from the current `src/transport/native/ipc.rs`:
`default_endpoint`, `adapter_endpoint_name`, `pipe_path` (windows), `socket_path` + `set_mode` +
`short_endpoint_hash` (unix), `dial_once` (both platforms), `probe_endpoint` + `endpoint_display` +
`EndpointProbe` (both), `connect` (both platforms; the native-host 30s-retry dial),
`relay_native_host`, `relay_adapter` + `RelaySide` + `HandshakePreamble` + `read_line_unbuffered` +
`relay_session`, `verify_service_proof`, `try_connect_once`, `connect_and_handshake`, and the
adapter-side unit tests (`preamble_captures_only_the_handshake`,
`read_line_unbuffered_reads_exactly_one_line_and_leaves_the_rest`,
`probe_reports_absent_for_an_unused_endpoint`). Items that were `pub(crate)` or private and are now
needed across the crate boundary (`pipe_path`, `socket_path`, `set_mode`, `adapter_endpoint_name`)
become `pub`.

The SERVICE half STAYS BEHIND (S3) and moves to core as `hub::endpoint` (S4): `serve`,
`claim_adapter_endpoint`, `serve_adapters`, `handle_adapter_connection`, `AdapterListener`,
`capture_peer_cred` (all platforms), `send_service_proof`, the `win_security` module, and the
service-side tests (`serve_bridges_a_tool_call_over_the_real_ipc`,
`probe_reports_accepts_against_a_live_server`).

`transport/src/lib.rs` (exact):

```rust
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Ghostlight transport: the small, stable substrate the role executables share (ADR-0046).
//! Wire framing, dialing, the resilient relay, identity, and process-lifecycle primitives.
//! The adapters depend on THIS crate only; a dependency on ghostlight-core here or in an
//! adapter is a design error (it would reintroduce the exe-lock ADR-0046 removes).

pub mod antisquat;
pub mod error;
pub mod handshake;
pub mod host;
pub mod instance;
pub mod ipc;
pub mod observability;
pub mod proc;
pub mod role;
pub mod session_guid;
pub mod supervisor;
pub mod watchdog;

pub use error::{Error, Result, ToolError};

/// Initialize operational (debug) logging to stderr (moved from the root crate; same body).
pub fn init_tracing(verbose: bool) {
    use tracing_subscriber::EnvFilter;
    let default = if verbose { "debug" } else { "info" };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}
```

Intra-transport path rewrites when files move: `crate::observability` etc. still resolve (same
crate); `crate::hub::handshake` -> `crate::handshake`; `crate::hub::session::SessionGuid` ->
`crate::session_guid::SessionGuid`; `crate::hub::antisquat` -> `crate::antisquat`;
`crate::hub::supervisor` -> `crate::supervisor`; `crate::hub::role` -> `crate::role`;
`crate::transport::native::host` / `super::host` -> `crate::host`; `crate::proc` unchanged;
`crate::{Error, Result}` unchanged (re-exported at the transport root). `relay_native_host`'s doc
references stay as prose.

`transport` Cargo dependencies (exact):

```toml
[dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "macros", "io-std", "io-util", "net", "sync", "time"] }
serde = { version = "1", features = ["derive"] }
serde_json = { version = "1", features = ["preserve_order"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
thiserror = "2"
dirs = "6"
sha2 = "0.11"
hmac = "0.13"
getrandom = "0.4"
uuid = { version = "1", features = ["v4"] }

[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.61", features = [
    "Win32_Foundation",
    "Win32_System_Threading",
    "Win32_System_Diagnostics_ToolHelp",
] }

[target.'cfg(unix)'.dependencies]
libc = "0.2"
```

(If a moved file fails to compile for a MISSING feature/dep, add the smallest missing item and log
a LEDGER deviation; do not restructure.)

## 3. Crate `ghostlight-core`: module disposition

Everything else moves to `crates/core/src/` preserving RELATIVE layout and file SPDX headers
(governance keeps `LicenseRef-Ghostlight-Commercial`):

| core module | moved from |
|---|---|
| `browser` | `src/browser/` |
| `governance` | `src/governance/` |
| `hub` | `src/hub/` (minus role.rs, handshake.rs, antisquat.rs, supervisor.rs, and SessionGuid, which went to transport) |
| `hub::endpoint` | NEW file `crates/core/src/hub/endpoint.rs` = the service half of the old ipc.rs (section 2) |
| `install` | `src/install/` |
| `mcp` | `src/transport/mcp/` |
| `messages` | `src/transport/native/messages.rs` |
| `origin` | `src/origin.rs` |

`core/src/lib.rs` (exact):

```rust
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Ghostlight core: the churny brain (governance, tools, browser protocol, hub composition,
//! installer, CLI support). Depends on ghostlight-transport; the adapter executables must
//! NEVER depend on this crate (ADR-0046 Decision 2).

pub mod browser;
pub mod governance;
pub mod hub;
pub mod install;
pub mod mcp;
pub mod messages;
pub mod origin;

pub use ghostlight_transport::error::{Error, Result, ToolError};
```

Path-rewrite rules inside moved core files (mechanical, apply everywhere):

| old path (in root crate) | new path (in core) |
|---|---|
| `crate::observability` | `ghostlight_transport::observability` |
| `crate::proc` | `ghostlight_transport::proc` |
| `crate::instance` | `ghostlight_transport::instance` |
| `crate::transport::watchdog` | `ghostlight_transport::watchdog` |
| `crate::hub::role` | `ghostlight_transport::role` |
| `crate::hub::handshake` | `ghostlight_transport::handshake` |
| `crate::hub::antisquat` | `ghostlight_transport::antisquat` |
| `crate::hub::supervisor` | `ghostlight_transport::supervisor` |
| `crate::transport::native::host` (and `crate::native::host`) | `ghostlight_transport::host` |
| `crate::native::ipc` / `crate::transport::native::ipc`, adapter-half items | `ghostlight_transport::ipc` |
| same, service-half items (serve, claim_adapter_endpoint, serve_adapters, probe is transport) | `crate::hub::endpoint` |
| `crate::hub::session::SessionGuid` | keep path: `hub/session.rs` adds `pub use ghostlight_transport::session_guid::SessionGuid;` |
| `crate::transport::mcp` (and `crate::mcp`) | `crate::mcp` |
| `crate::transport::native::messages` (and `crate::native::messages`) | `crate::messages` |
| `crate::{Error, Result, ToolError}` | unchanged (re-exported at core root) |
| `crate::browser`, `crate::governance`, `crate::install`, `crate::hub`, `crate::origin` | unchanged |

`core` keeps `build_debug_sink` call sites but the fn now lives in transport::observability;
`hub::run_service` etc. call `ghostlight_transport::observability::build_debug_sink`.

`core` Cargo dependencies: everything in the CURRENT root `[dependencies]` MINUS clap, anyhow,
tracing-subscriber, getrandom, hmac, sha2 (sha2 stays IF core still uses it -- grep; manifest
hashing in governance uses sha2, so keep sha2 in core too), PLUS
`ghostlight-transport = { path = "../transport" }`. Windows deps: winreg + windows-sys with the
full current feature list. Unix: libc. When in doubt keep the dep; pruning is not the goal.

## 4. Root package after the split

- `src/lib.rs` = the facade (section 6). `src/main.rs` = CLI + service shell (S7 slims it).
- Root Cargo `[dependencies]`: `ghostlight-core = { path = "crates/core" }`,
  `ghostlight-transport = { path = "crates/transport" }`, `clap`, `anyhow`, `tokio`, `tracing`,
  `serde_json` (main.rs uses json! nowhere? keep if compile needs), and NOTHING else unless the
  compiler demands it (log a deviation if so).
- tests/ keep importing `ghostlight::...` and spawning `env!("CARGO_BIN_EXE_ghostlight")`.

## 5. Adapter executables

### 5.1 `ghostlight-adapter-agent` (crates/adapter-agent/src/main.rs)

Purpose: the MCP stdio adapter (was `run_mcp_server` + `run_as_adapter` in `src/hub/mod.rs`).
Behavior pins:
- Resolve instance: scan argv for `--instance <v>` / `--instance=<v>` (same forms as root main),
  else `GHOSTLIGHT_INSTANCE` env, else default. Invalid name: print
  `ghostlight-adapter-agent: <validation error>` to stderr, exit 2. Fold the winner into the env
  (`std::env::set_var(Instance::ENV_VAR, ...)`) exactly like root `resolve_instance`.
- `ghostlight_transport::init_tracing(debug)` where debug = env `GHOSTLIGHT_DEBUG` set OR argv
  contains `--debug`.
- `role::set_role(Role::Adapter)`.
- Debug sink: `observability::build_debug_sink(debug, "adapter")` (role string UNCHANGED --
  doctor/status parse it).
- NO orphan sweep (doctor --fix and the watchdog cover it; core is unavailable here by design).
- Parent watchdog + relay: same structure as the current `run_as_adapter`: capture
  `proc::parent()`, build a tokio Runtime, `select!` `ipc::relay_adapter(&ipc::default_endpoint(), &sink)`
  against the watchdog (`watchdog::wait_until_orphaned(parent)`), then `sink.flush()` and
  `std::process::exit(code)` (0 on clean end/watchdog, 1 on relay error).
- The manifest no-op warning fires when the `--manifest` argument OR the `GHOSTLIGHT_MANIFEST`
  env var is present (mirroring the current `run_mcp_server` exactly), with the existing text
  ("a --manifest on a client invocation is ignored; the running Ghostlight service's policy
  governs all sessions").
- No clap; parse by scanning `std::env::args()` (the bin must tolerate unknown args).
- NAMING FENCE: no function in either adapter main may be named `run_mcp_server`,
  `run_as_adapter`, or `run_native_host_role` (S7's verification greps those names to prove the
  roles are gone; keep the adapter main's helper, if any, named `relay_with_watchdog`).

Cargo: `ghostlight-transport = { path = "../transport" }`,
`tokio = { version = "1", features = ["rt-multi-thread", "macros"] }`, `tracing = "0.1"`.
`[[bin]] name = "ghostlight-adapter-agent"` (package name the same).

### 5.2 `ghostlight-adapter-browser` (crates/adapter-browser/src/main.rs)

Purpose: the Chrome native-messaging relay (was `run_native_host_role` in `src/main.rs`).
Behavior pins:
- Chrome passes the extension origin (`chrome-extension://<id>/`) and `--parent-window=<hwnd>`;
  the bin ignores both (no clap; never error on unknown args).
- Resolve instance: env `GHOSTLIGHT_INSTANCE` if set (validate; on invalid, WARN and use default --
  Chrome-launched, so exiting helps no one), else
  `Instance::from_exe_stem_with_base(&current_exe, "ghostlight-adapter-browser")` (section 7),
  else default. Fold winner into env.
- debug = env `GHOSTLIGHT_DEBUG` only (Chrome never passes --debug).
- Sink role string: `"native-host"` (UNCHANGED).
- Body: exactly the current `run_native_host_role`: init tracing, build sink, Runtime,
  `ipc::relay_native_host(&ipc::default_endpoint(), &sink)`, warn on error, `sink.flush()`,
  `std::process::exit(0)` (the direct exit is load-bearing; keep the comment about the parked
  stdin read).

Cargo: same three deps as adapter-agent. `[[bin]] name = "ghostlight-adapter-browser"`.

### 5.3 Sibling-binary resolution (installer + tests)

The three bins are siblings in the same directory. Pin one helper in `install/native_host.rs`:

```rust
/// The path of a sibling role executable next to the running one (ADR-0046): same directory,
/// platform suffix appended on Windows.
pub fn sibling_bin(current_exe: &Path, name: &str) -> PathBuf {
    let file = if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
    normalize_exe_path(current_exe)
        .parent()
        .map(|d| d.join(&file))
        .unwrap_or_else(|| PathBuf::from(file))
}
```

- Client entry command (`install::clients::server_entry`): `sibling_bin(exe, "ghostlight-adapter-agent")`.
- Host manifest `path` (default instance): `sibling_bin(exe, "ghostlight-adapter-browser")`.
- Host launcher copy (non-default instance): copy FROM the adapter-browser sibling TO
  `<local>/<dir_leaf>/ghostlight-adapter-browser-<n>[.exe]` (`instance_launcher` reworked in S6).
- Tests: `tests/support/mod.rs` gains
  `pub fn adapter_bin() -> PathBuf { sibling of env!("CARGO_BIN_EXE_ghostlight") named ghostlight-adapter-agent[.exe] }`
  and `spawn_adapter` switches to it. Sibling bins exist under `cargo test --workspace`
  because the workspace builds every member's bins before running tests.

## 6. The root facade (`src/lib.rs`, final form at S4)

```rust
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Ghostlight -- facade crate. The implementation lives in ghostlight-core (churny brain) and
//! ghostlight-transport (stable substrate); this crate re-exports both under the historical
//! `ghostlight::` paths so integration tests and external references keep compiling, and hosts
//! the `ghostlight` executable (CLI + service).

pub use ghostlight_core::{browser, governance, hub, install, mcp, messages, origin};
pub use ghostlight_transport::{error, instance, observability, proc};
pub use ghostlight_transport::error::{Error, Result, ToolError};
pub use ghostlight_transport::init_tracing;

/// Historical path continuity (`ghostlight::native::...`).
pub mod native {
    pub use ghostlight_core::messages;
    pub use ghostlight_transport::host;
    /// The two halves of the old ipc module, merged back under the historical path.
    pub mod ipc {
        pub use ghostlight_core::hub::endpoint::*;
        pub use ghostlight_transport::ipc::*;
    }
}

/// Historical path continuity (`ghostlight::transport::...`).
pub mod transport {
    pub use crate::native;
    pub use ghostlight_core::mcp;
    pub use ghostlight_transport::watchdog;
}
```

(S2/S3 use intermediate facades that re-export the already-moved pieces while the rest still
lives in the root crate; each task file pins its interim facade.)

## 7. Instance: the base-parameterized exe-stem (added in S6)

In `transport::instance`:

```rust
/// [`from_exe_stem`] generalized over the executable's base name (ADR-0046: each role executable
/// resolves argv[0] against ITS OWN base, so `ghostlight-adapter-browser` is that bin's DEFAULT
/// instance, never a bogus instance named "adapter-browser").
pub fn from_exe_stem_with_base(exe: &std::path::Path, base: &str) -> Option<Self> {
    let stem = exe.file_stem()?.to_str()?;
    if stem == base {
        return Some(Self::default());
    }
    let name = stem.strip_prefix(base)?.strip_prefix('-')?;
    Self::from_name(name).ok()
}
```

`from_exe_stem(exe)` becomes `Self::from_exe_stem_with_base(exe, LEAF_BASE)` (behavior for the
root bin unchanged). Pinned new tests (S6, in transport::instance). Path literals MUST be
forward-slash (a backslash is not a separator on unix; the CI matrix runs all three OSes --
this exact mistake reddened CI once already):
- `from_exe_stem_with_base_resolves_the_browser_adapter_family`:
  `("/x/ghostlight-adapter-browser", base)` -> default;
  `("/x/ghostlight-adapter-browser-dev.exe", base)` -> Some("dev");
  `("/x/ghostlight-adapter-browser-qa-staging", base)` -> Some("qa-staging");
  `("/x/ghostlight", base)` -> None (not this family);
  where `base = "ghostlight-adapter-browser"`. A `#[cfg(windows)]` block may add
  backslash-path variants; nothing backslashed outside such a block.

## 8. Reconnect patience (S8; amends ADR-0045 in place with a dated note)

In `transport::ipc`:
- FIRST connect keeps today's behavior: one `try_connect_once`, then `start_service()` once, then
  retry every `SELF_HEAL_RETRY_INTERVAL` (200ms) for `SELF_HEAL_RETRY_WINDOW` (3s), then error out.
- RECONNECT (a session existed and the service dropped) is patient:
  `pub const RECONNECT_RETRY_WINDOW: Duration = Duration::from_secs(120);`
  `pub const RECONNECT_RETRY_INTERVAL: Duration = Duration::from_millis(500);`
  On entering a reconnect episode call `supervisor::start_service()` exactly ONCE, then retry
  `try_connect_once` every RECONNECT_RETRY_INTERVAL until RECONNECT_RETRY_WINDOW elapses; if it
  elapses, log `SELF_HEAL_FAILURE_MESSAGE` and return the last error (adapter exits; the client
  reload path is the fallback, exactly today's behavior).
- Implementation shape: `connect_and_handshake(adapter_endpoint, reconnect: bool)` (add the flag;
  `relay_adapter` passes `first == false` as `reconnect == true`).
- Pinned new test in `tests/adapter_reconnect.rs`:
  `adapter_survives_a_five_second_service_gap` -- identical to the existing restart test but with
  `std::thread::sleep(Duration::from_secs(5))` between `service1.kill()/wait()` and spawning
  service2 (5s exceeds the old 3s window, proving the patient path), and a 30s `recv` timeout for
  the post-restart reply.

## 9. Retiring the roles from `ghostlight` (S7)

- Delete from root `src/main.rs`: `run_native_host_role`, the `chrome-extension://` argv
  detection, and the `Command: None => run_mcp_server(...)` arm.
- Delete from core `hub/mod.rs`: `run_mcp_server` and `run_as_adapter` (the agent bin owns them).
  `build_debug_sink` was already moved to transport (S2). `run_service`, `run_service_loop`,
  `idle_grace_watch`, ServiceContext etc. all stay.
- The bare invocation (`ghostlight` with no subcommand) prints EXACTLY this to stderr and exits 2:

```
ghostlight no longer serves MCP directly; your MCP client launches ghostlight-adapter-agent.
Run `ghostlight install` to update client registrations, then restart your editor.
```

- Pinned test (root tests/, new file `tests/bare_invocation.rs`):
  `bare_invocation_prints_guidance_and_exits_2` -- spawn `CARGO_BIN_EXE_ghostlight` with no args,
  stdin null; assert exit code 2 and stderr contains
  `"ghostlight no longer serves MCP directly"`.

## 10. `--no-supervisor` (S9)

- `InstallArgs`/`InstallOptions` gain `no_supervisor: bool` (clap long flag `--no-supervisor`,
  doc: "Skip registering the OS auto-start supervisor (dev instances run 'ghostlight service' in
  a terminal instead)").
- In `run_install`: when set, print the supervisor section header followed by exactly
  `  (skipped: --no-supervisor)` and do not call `supervisor::apply_steps`. Uninstall is
  unchanged (removal of a never-registered task is already a warn-level no-op).
- Pinned test in `tests/install_instance.rs`:
  `no_supervisor_flag_plans_no_supervisor_steps` -- run the dry-run with `--no-supervisor` added;
  assert stdout contains `(skipped: --no-supervisor)` and does NOT contain `schtasks` /
  `launchctl` / `systemctl` (platform-agnostic: assert all three absent).
- Dev workflow doc: new `docs/DEV-LOOP.md` (plain, human style; ASCII; no em-dashes) describing:
  build (`cargo build -p ghostlight`), dev install
  (`ghostlight --instance dev install --no-supervisor --debug --extension-id <id>`), terminal
  service (`ghostlight --instance dev service --keep-warm --debug`), the edit loop (Ctrl-C ->
  build -> rerun; the agent adapter reconnects within 120s), and why `-p ghostlight` (does not
  relink the running adapters).

## 11. Packaging end-state (S10)

- `release.yml`: build becomes `cargo build --release --locked --workspace --target <t>`; the
  Package step copies all THREE bins into the archive dir; the raw uploads become three files per
  target: `ghostlight-<target>`, `ghostlight-adapter-agent-<target>`,
  `ghostlight-adapter-browser-<target>` (Windows: `.exe` inserted before `-<target>`? NO --
  keep the existing raw naming convention exactly as release.yml does today for the single bin,
  extended to the three names; re-read the workflow's existing raw-upload step and mirror it).
- `scripts/get.sh` / `scripts/get.ps1`: download all three raw binaries into the same install dir,
  chmod each on unix, then run `ghostlight install` as today.
- `packaging/npm`: the launcher downloads/spawns... the MCP entry now must exec
  `ghostlight-adapter-agent` (the npm bin is what MCP clients launch via `npx ghostlight`).
  End-state: the npm package downloads all three binaries on first run and its bin script execs
  `ghostlight-adapter-agent`, passing argv through. ALSO: `ghostlight install` config entries
  written by the npm flow point at the downloaded adapter (sibling rule handles it: they sit in
  one dir).
- `packaging/winget|scoop|homebrew` templates and `docs/business/DISTRIBUTION.md`: list the three
  binaries (template edits are textual; keep shapes).
- `tests/e2e/run-smoke.mjs`: host-manifest wrapper wraps the adapter-browser bin; the MCP stdio
  spawn uses the adapter-agent bin; the service spawn stays `ghostlight service`. (Job remains
  quarantined; verification is `node --check tests/e2e/run-smoke.mjs`.)
- README quick-install + CLAUDE.md structure notes: mention the three executables (small,
  surgical edits; do not rewrite the docs).

## 12. Verification commands (every task, unless the task narrows them)

```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
cargo check --target x86_64-unknown-linux-gnu --workspace --all-targets
```

All four must be clean/green. `cargo test --workspace` output must contain no `FAILED` and no
`test result:` line with a nonzero `failed` count. The linux cross-check target is installed on
this machine. macOS cannot be checked locally; CI covers it -- do not guess-edit mac-only code
beyond the mechanical path rewrites.

## 13. Facts as of authoring (2026-07-08, dev @ fccca60)

- The batch BASE COMMIT is `fccca60` (`fix(instance): cross-platform from_exe_stem test`); the
  authoring red-team found the prior commit's instance test reddened the unix CI matrix, and the
  fix landed as the base. Commits after the base that touch only `docs/` are expected (the batch
  files themselves).
- Rust file inventory and module tree: see section 2/3 tables; verified by `find src -name "*.rs"`.
- `Cargo.lock` is committed and CI uses `--locked`: run plain cargo once per task so the lock
  updates, and COMMIT the lock changes with the task.
- CI (`.github/workflows/ci.yml`): clippy + `cargo test --locked --no-fail-fast` on a 3-OS matrix;
  S5 switches both to `--workspace` (and the e2e job's `cargo build --locked` line too).
- Three integration tests read SOURCE PATHS as text and are re-pointed by pinned edits when their
  files move: `tests/hub_lifecycle.rs` (reads `src/hub/supervisor.rs`; S3),
  `tests/architecture.rs` (asserts `src/governance/` exists; S4), `tests/hub_role_wiring.rs`
  (reads `src/transport/mcp/server.rs`; S4).
- The local suite is green at the base (fmt, clippy -D warnings, tests, linux cross-check); the
  unix CI matrix is expected green with the base fix.
- The machine is Windows; PowerShell primary, bash available. ASCII ONLY in all code and docs (no
  em-dashes, no unicode arrows/quotes).
