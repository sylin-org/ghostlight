//! The Ghostlight window process (ADR-0199 Decisions 1 and 7).
//!
//! `ghostlight-desktop` is the third executable of one build, next to the service and the
//! connectors. It owns the window, the tray and the notifications, and nothing else. It decides
//! nothing, stores nothing and opens no network port: it renders only pages it serves from memory
//! through a private custom scheme, and every fact it shows or acts on comes from the service.
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
//! ADR-0197: `http`, the request and response types that `wry`'s custom-scheme and IPC callbacks
//! take. It does no I/O and opens no socket. `crates/desktop/guards/network.test.mjs` names it as
//! the one exception and fails if it ever leaves the tree.

pub mod bridge;
pub mod bundle;
pub mod scheme;
pub mod session;
pub mod tray;
pub mod window;
