//! The `ghostlight-service` executable: the service, its desktop workbench and every typed
//! command (ADR-0199 Decision 1).
//!
//! Before any command runs, the entry rule decides whether this copy does the job, hands it to
//! the development build or the installed service, hands the edge's relay verbs to the installed
//! edge, or installs from its own package first (ADR-0197 Decisions 10, 13 and 15).

// The mandatory npm launcher retains CLI stdio and waits for this child. Release desktop launches
// therefore use the native Windows application subsystem without flashing a console window.
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use ghostlight_service::cli::session_end::listen_unless_serving;
use ghostlight_service::cli::start::{start, ProcessLauncher, Start};
use ghostlight_service::install::layout::Layout;

fn main() -> anyhow::Result<()> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    #[cfg(all(not(debug_assertions), target_os = "windows"))]
    ghostlight_service::cli::console::attach_for_typed_command(&arguments);
    let _session = listen_unless_serving(&arguments);
    let current_exe = std::env::current_exe()?;
    match start(
        arguments,
        &current_exe,
        &Layout::current()?,
        &ProcessLauncher,
    )? {
        Start::Dispatch(mode) => ghostlight_service::cli::dispatch(mode),
        Start::Exit(code) => std::process::exit(code),
    }
}
