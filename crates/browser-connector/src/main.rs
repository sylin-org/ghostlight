// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Browser-only native-messaging pass-through between Chromium and the persistent service.
//!
//! MCP lifecycle and stdio moved to `ghostlight-mcp-connector` under ADR-0096. This executable now has one
//! responsibility: preserve Chrome native framing, extension identity, reconnect, and parent-death
//! behavior without holding governance or browser policy.

use ghostlight_transport::instance::Instance;
use ghostlight_transport::observability::build_debug_sink;
use ghostlight_transport::proc;
use ghostlight_transport::{ipc, watchdog};

fn main() {
    run_browser()
}

/// The Chrome native-messaging pass-through: resolve the instance, relay extension frames to the
/// service as a stateless byte pipe, then `process::exit(0)` (tokio's stdin reader parks a blocking
/// ReadFile on Chrome's still-open stdin; dropping the runtime would hang joining it).
///
/// ADR-0058: identifies itself to the service with a `ROLE_BROWSER` session-hello carrying its
/// PARENT process's identity (the browser that spawned it via `connectNative`) and races
/// the relay loop against a parent-death watchdog, so the process exits the moment the browser
/// itself is gone rather than depending solely on stdin/pipe EOF (today's only signal for that,
/// and the weaker of the two).
fn run_browser() -> ! {
    // Chrome launches this with a bare path plus the extension origin (`chrome-extension://<id>/`)
    // and `--parent-window=<hwnd>` -- positional/flag args this role simply ignores.
    let instance = resolve_browser_instance();

    // Chrome never passes `--debug`; the only debug signal is an inherited GHOSTLIGHT_DEBUG.
    let debug = std::env::var_os("GHOSTLIGHT_DEBUG").is_some();
    ghostlight_transport::init_tracing(debug);

    tracing::info!("ghostlight starting (native-host role, launched by the browser)");
    let sink = build_debug_sink(debug, "native-host");
    let browser_parent = proc::parent();
    let hello =
        ghostlight_transport::handshake::browser_hello_bytes(std::process::id(), browser_parent);
    let rt = tokio::runtime::Runtime::new().expect("build the native-host tokio runtime");
    let endpoints = ipc::endpoint_candidates(&instance);
    let result = rt.block_on(async {
        tokio::select! {
            r = ipc::relay_native_host(&endpoints, &hello, &sink) => r,
            _ = async {
                match browser_parent {
                    Some(p) => watchdog::wait_until_orphaned(p).await,
                    // No determinable parent: never fires, so this arm simply never wins the
                    // select -- the relay loop's own EOF detection remains the sole exit trigger.
                    None => std::future::pending().await,
                }
            } => {
                tracing::warn!("the browser that launched this relay has exited; ending");
                Ok(())
            }
        }
    });
    if let Err(e) = result {
        tracing::warn!(error = %e, "native-host relay ended with error");
    }
    sink.flush();
    tracing::info!("native-host relay ended; exiting");
    std::process::exit(0);
}

/// Resolve the browser role's instance (ADR-0044/0064): an inherited, explicit `GHOSTLIGHT_INSTANCE`
/// wins (the reserved word `default` is the default; an invalid value is non-fatal -- Chrome launched
/// us with no console, so warn and fall through); else a `ghostlight-browser-connector-<n>` per-instance copy
/// pins `<n>` via its own argv[0] (the ADR-0044 Decision 4 launcher -- this is how the dev extension's
/// `ghostlight-browser-connector-dev` copy targets the dev service, ADR-0064); else the DEFAULT instance. There
/// is no "unpinned, resolve-at-connect, prefer dev" state anymore.
fn resolve_browser_instance() -> Instance {
    if let Ok(raw) = std::env::var(Instance::ENV_VAR) {
        let name = raw.trim();
        if !name.is_empty() {
            if name.eq_ignore_ascii_case("default") {
                std::env::remove_var(Instance::ENV_VAR);
                return Instance::default();
            }
            match Instance::from_name(name) {
                Ok(i) => {
                    std::env::set_var(Instance::ENV_VAR, name);
                    return i;
                }
                Err(e) => {
                    tracing::warn!(value = %name, error = %e, "ignoring an invalid GHOSTLIGHT_INSTANCE; using the default instance");
                    std::env::remove_var(Instance::ENV_VAR);
                }
            }
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(inst) = Instance::from_exe_stem_with_base(&exe, "ghostlight-browser-connector")
        {
            if let Some(name) = inst.name() {
                std::env::set_var(Instance::ENV_VAR, name);
                return inst;
            }
        }
    }
    std::env::remove_var(Instance::ENV_VAR);
    Instance::default()
}
