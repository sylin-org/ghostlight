//! The `ghostlight` executable: the service, its desktop workbench and the command line.
//!
//! Before any command runs, the entry rule decides whether this copy does the job, hands it to
//! the development build or the installed service, or installs from its own package first
//! (ADR-0197 Decisions 10, 13 and 15).

// The mandatory npm launcher retains CLI stdio and waits for this child. Release desktop launches
// therefore use the native Windows application subsystem without flashing a console window.
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use ghostlight::cli::start::{start, ProcessLauncher, Start};
use ghostlight::install::layout::Layout;

fn main() -> anyhow::Result<()> {
    let arguments = std::env::args_os().skip(1).collect();
    let current_exe = std::env::current_exe()?;
    match start(
        arguments,
        &current_exe,
        &Layout::current()?,
        &ProcessLauncher,
    )? {
        Start::Dispatch(mode) => ghostlight::cli::dispatch(mode),
        Start::Exit(code) => std::process::exit(code),
    }
}
