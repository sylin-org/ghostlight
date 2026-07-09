// SPDX-License-Identifier: Apache-2.0 OR MIT
//! ghostlight-relay: the single thin pass-through executable (ADR-0051 Phase 3), merging the two
//! ADR-0046 adapters (`ghostlight-adapter-agent` + `ghostlight-adapter-browser`) into one binary
//! with two roles selected at launch:
//!
//! - AGENT: the MCP-side resilient stdio pass-through an editor (Claude Code, Cursor, ...) launches.
//!   Selected by an explicit `--role agent` (the installer writes it into the client config; tests
//!   pass it too), and the safe default for a bare stdio launch.
//! - BROWSER: the Chrome native-messaging pass-through the browser launches via
//!   `chrome.runtime.connectNative`. AUTO-DETECTED from the `chrome-extension://` origin Chrome
//!   passes as an argument, because a native-messaging host manifest hands Chrome a BARE executable
//!   path with no room for a `--role` flag.
//!
//! Either role resolves the active instance, connects to the already-running `ghostlight` SERVICE
//! over the local IPC, and relays its stdio. It holds NO governance and depends ONLY on
//! ghostlight-transport, so a service rebuild never relinks (locks) this binary (ADR-0046
//! Decision 2, preserved by the merge).

use ghostlight_transport::instance::{Instance, Selection};
use ghostlight_transport::observability::{build_debug_sink, DebugSink};
use ghostlight_transport::proc::{self, ProcId};
use ghostlight_transport::role::{self, Role};
use ghostlight_transport::{ipc, watchdog};

/// The relay's two roles (ADR-0051 Phase 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RelayRole {
    Agent,
    Browser,
}

/// Decide the role from argv (ADR-0051 Phase 3):
///   1. an explicit `--role agent` / `--role browser` / `--role=<v>` (the agent's client config
///      passes this; the E2E tests pass it for both roles),
///   2. else the BROWSER role if any argument is a `chrome-extension://` origin -- Chrome launches
///      the native host with a bare path plus the extension origin, no room for a flag,
///   3. else the AGENT role -- the safe default for a stdio launch.
fn role_from_args(args: &[String]) -> RelayRole {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let explicit = a
            .strip_prefix("--role=")
            .map(str::to_string)
            .or_else(|| (a == "--role").then(|| it.next().cloned().unwrap_or_default()));
        if let Some(v) = explicit {
            if v.eq_ignore_ascii_case("browser") {
                return RelayRole::Browser;
            }
            if v.eq_ignore_ascii_case("agent") {
                return RelayRole::Agent;
            }
        }
    }
    if args.iter().any(|a| a.starts_with("chrome-extension://")) {
        return RelayRole::Browser;
    }
    RelayRole::Agent
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match role_from_args(&args) {
        RelayRole::Agent => run_agent(&args),
        RelayRole::Browser => run_browser(),
    }
}

// --- AGENT role (former ghostlight-adapter-agent) ---

/// The MCP-side pass-through: resolve the instance, then relay the client's stdio to the service,
/// dying with its editor via the ADR-0029 parent-death watchdog. `process::exit` rather than
/// unwinding: the stdin read may still be parked in a blocking ReadFile, and dropping the runtime
/// would hang joining that thread.
fn run_agent(args: &[String]) -> ! {
    // Resolve the instance from the same precedence root `ghostlight` uses (ADR-0044) and fold the
    // winner back into GHOSTLIGHT_INSTANCE so every point-of-use `Instance::resolve()` agrees.
    let selection = resolve_agent_selection();

    let debug =
        std::env::var_os("GHOSTLIGHT_DEBUG").is_some() || args.iter().any(|a| a == "--debug");
    ghostlight_transport::init_tracing(debug);
    role::set_role(Role::Adapter);

    // A `--manifest` on a client invocation is a no-op: only the running SERVICE loads policy
    // (PINS.md SS5.1).
    if args.iter().any(|a| a == "--manifest") || std::env::var_os("GHOSTLIGHT_MANIFEST").is_some() {
        tracing::warn!(
            "a --manifest on a client invocation is ignored; the running Ghostlight service's \
             policy governs all sessions"
        );
    }

    let sink = build_debug_sink(debug, "adapter");
    // The MCP client that spawned us, captured before the runtime starts (ADR-0029). None (no
    // resolvable parent) skips the watchdog and leaves stdin EOF as the sole exit trigger.
    let parent = proc::parent();

    let rt = tokio::runtime::Runtime::new().expect("build the adapter tokio runtime");
    let block_sink = sink.clone();
    let endpoints = ipc::endpoint_candidates(&selection);
    let code = rt.block_on(relay_with_watchdog(&endpoints, block_sink, parent));

    sink.flush();
    std::process::exit(code)
}

/// Resolve the agent's instance SELECTION (ADR-0048 D2): `--instance <name>` / `--instance=<name>`
/// wins over `GHOSTLIGHT_INSTANCE`; the reserved word `default` pins the default; NOTHING pins
/// nothing (resolve at connect time, preferring a live dev instance, ADR-0048 D1). An invalid name
/// is fatal: print the validation error and exit 2.
fn resolve_agent_selection() -> Selection {
    match Selection::resolve_from(instance_flag_value().as_deref()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("ghostlight-relay: {e}");
            std::process::exit(2);
        }
    }
}

/// Scan argv for `--instance <value>` or `--instance=<value>` (no clap: this bin tolerates unknown
/// args, e.g. a stray `--manifest`).
fn instance_flag_value() -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if let Some(v) = a.strip_prefix("--instance=") {
            return Some(v.to_string());
        }
        if a == "--instance" {
            return args.next();
        }
    }
    None
}

/// Relay the client's stdio to the service, ending when the client closes OR the parent-death
/// watchdog fires (ADR-0029/0045). Returns the process exit code (0 on a clean end or watchdog
/// trigger, 1 on a relay error).
async fn relay_with_watchdog(
    endpoints: &[String],
    debug_sink: DebugSink,
    parent: Option<ProcId>,
) -> i32 {
    let shutdown = std::sync::Arc::new(tokio::sync::Notify::new());
    if let Some(parent) = parent {
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            watchdog::wait_until_orphaned(parent).await;
            tracing::warn!(
                parent_pid = parent.pid,
                "MCP client exited; ordering shutdown"
            );
            shutdown.notify_one();
        });
    }

    tokio::select! {
        result = ipc::relay_adapter(endpoints, &debug_sink) => {
            match result {
                Ok(()) => 0,
                Err(e) => {
                    tracing::error!(error = %e, "adapter relay ended with an error");
                    1
                }
            }
        }
        _ = shutdown.notified() => 0,
    }
}

// --- BROWSER role (former ghostlight-adapter-browser) ---

/// The Chrome native-messaging pass-through: resolve the instance, relay extension frames to the
/// service as a stateless byte pipe, then `process::exit(0)` (tokio's stdin reader parks a blocking
/// ReadFile on Chrome's still-open stdin; dropping the runtime would hang joining it).
fn run_browser() -> ! {
    // Chrome launches this with a bare path plus the extension origin (`chrome-extension://<id>/`)
    // and `--parent-window=<hwnd>` -- positional/flag args this role simply ignores.
    let selection = resolve_browser_selection();

    // Chrome never passes `--debug`; the only debug signal is an inherited GHOSTLIGHT_DEBUG.
    let debug = std::env::var_os("GHOSTLIGHT_DEBUG").is_some();
    ghostlight_transport::init_tracing(debug);

    tracing::info!("ghostlight starting (native-host role, launched by the browser)");
    let sink = build_debug_sink(debug, "native-host");
    let rt = tokio::runtime::Runtime::new().expect("build the native-host tokio runtime");
    let endpoints = ipc::endpoint_candidates(&selection);
    let result = rt.block_on(async { ipc::relay_native_host(&endpoints, &sink).await });
    if let Err(e) = result {
        tracing::warn!(error = %e, "native-host relay ended with error");
    }
    sink.flush();
    tracing::info!("native-host relay ended; exiting");
    std::process::exit(0);
}

/// Resolve the browser role's instance SELECTION (ADR-0048 D2/D4): an inherited, explicit
/// `GHOSTLIGHT_INSTANCE` wins (the reserved word `default` pins the default; an invalid value is
/// non-fatal -- Chrome launched us with no console, so warn and fall through); else a
/// `ghostlight-relay-<n>` per-instance copy pins `<n>` via its own argv[0] (the ADR-0044 Decision 4
/// launcher); else UNPINNED -- the plain sibling binary resolves at connect time, preferring a live
/// dev instance.
fn resolve_browser_selection() -> Selection {
    if let Ok(raw) = std::env::var(Instance::ENV_VAR) {
        let name = raw.trim();
        if !name.is_empty() {
            if name.eq_ignore_ascii_case("default") {
                std::env::remove_var(Instance::ENV_VAR);
                return Selection::Pinned(Instance::default());
            }
            match Instance::from_name(name) {
                Ok(i) => {
                    std::env::set_var(Instance::ENV_VAR, name);
                    return Selection::Pinned(i);
                }
                Err(e) => {
                    tracing::warn!(value = %name, error = %e, "ignoring an invalid GHOSTLIGHT_INSTANCE; resolving at connect time");
                    std::env::remove_var(Instance::ENV_VAR);
                }
            }
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(inst) = Instance::from_exe_stem_with_base(&exe, "ghostlight-relay") {
            if let Some(name) = inst.name() {
                std::env::set_var(Instance::ENV_VAR, name);
                return Selection::Pinned(inst);
            }
        }
    }
    std::env::remove_var(Instance::ENV_VAR);
    Selection::Unpinned
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn explicit_role_flag_wins() {
        assert_eq!(
            role_from_args(&args(&["ghostlight-relay", "--role", "agent"])),
            RelayRole::Agent
        );
        assert_eq!(
            role_from_args(&args(&["ghostlight-relay", "--role", "browser"])),
            RelayRole::Browser
        );
        assert_eq!(
            role_from_args(&args(&["ghostlight-relay", "--role=browser"])),
            RelayRole::Browser
        );
    }

    #[test]
    fn a_chrome_extension_origin_selects_browser() {
        // Chrome launches the native host with a bare path + the extension origin and no flag.
        assert_eq!(
            role_from_args(&args(&[
                "ghostlight-relay",
                "chrome-extension://cjcmhepmagomefjggkcohdbfemacojoa/",
                "--parent-window=0"
            ])),
            RelayRole::Browser
        );
    }

    #[test]
    fn a_bare_stdio_launch_defaults_to_agent() {
        assert_eq!(
            role_from_args(&args(&["ghostlight-relay"])),
            RelayRole::Agent
        );
        // A stray --manifest / --instance does not flip the role.
        assert_eq!(
            role_from_args(&args(&[
                "ghostlight-relay",
                "--instance",
                "dev",
                "--manifest",
                "x"
            ])),
            RelayRole::Agent
        );
    }

    #[test]
    fn an_explicit_role_still_wins_over_a_present_origin() {
        assert_eq!(
            role_from_args(&args(&[
                "ghostlight-relay",
                "--role",
                "agent",
                "chrome-extension://cjcmhepmagomefjggkcohdbfemacojoa/"
            ])),
            RelayRole::Agent
        );
    }
}
