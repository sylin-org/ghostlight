//! The Ghostlight window process (ADR-0199 Decisions 1 and 7, and its amendment of 2026-10-15).
//!
//! `ghostlight-desktop` is the third executable of one build, next to the service and the
//! connectors. It owns the window and the tray, and nothing else. It decides nothing and opens no
//! network port: the window renders the service's loopback workbench, and every fact it shows or
//! acts on comes from the service. The shell learns the workbench address and the bearer that
//! makes the page the window's over the relay's `desktop` stream, and hands the bearer to that
//! origin's page only.
//!
//! The shell is built on `wry` (the operating system webview), `tao` (window and event loop) and
//! `tray-icon` (tray and menu). It is not built on Tauri: Tauri brings `tokio`, `reqwest` and
//! `http`, plus its runtime, configuration, capability codegen, build step and plugins, which is a
//! larger binary and more upgrade surface for a process that must decide nothing. It is not built
//! on raw platform bindings either (WebView2 COM calls, Win32 windows and `Shell_NotifyIcon`,
//! hand-driven WebKitGTK): that is hundreds of lines of unsafe code, which the workspace forbids
//! outside its audited FFI crate. `wry`, `tao` and `tray-icon` come from the same upstream as
//! Tauri and are already locked at these exact versions in this tree, so the shell adds no
//! package to `Cargo.lock` except itself.
//!
//! The dependency tree holds exactly one name on the network-crate ban list of ADR-0028 and
//! ADR-0197: `http`, which `wry`'s own API types bring. The shell registers no custom scheme and
//! no IPC handler, does no I/O of its own over it and opens no socket.
//! `crates/desktop/guards/network.test.mjs` names it as the one exception and fails if it ever
//! leaves the tree.

use std::ffi::OsString;

pub mod app;
pub mod link;
pub mod session;
pub mod tray;
pub mod webview;
pub mod window;

/// The exit code of a run with arguments the program does not accept.
pub const EXIT_USAGE: u8 = 2;

/// Whether the command line, without the program name, is one the program accepts.
///
/// The window process takes no arguments: everything it does is decided by the service.
pub fn accepts_arguments(args: &[OsString]) -> bool {
    args.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_arguments_are_accepted_and_any_argument_is_refused() {
        assert!(accepts_arguments(&[]));
        assert!(!accepts_arguments(&[OsString::from("--help")]));
        assert!(!accepts_arguments(&[OsString::new()]));
    }
}
