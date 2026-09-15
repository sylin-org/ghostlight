//! Ghostlight 1.0 orchestrator and integrated desktop workbench process.

// The mandatory npm launcher retains CLI stdio and waits for this child. Release desktop launches
// therefore use the native Windows application subsystem without flashing a console window.
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

fn main() -> anyhow::Result<()> {
    let mode = ghostlight::cli::parse::launch_mode(std::env::args_os().skip(1))?;
    ghostlight::cli::dispatch(mode)
}
