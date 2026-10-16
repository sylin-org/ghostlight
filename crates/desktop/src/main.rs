//! The `ghostlight-desktop` executable.
//!
//! On Windows it is a GUI-subsystem program, in debug builds too, so nothing a person clicks opens
//! a console. It runs the Ghostlight window and tray. `--tray` and `--updating` say how the service
//! or a previous shell started it; any other argument is refused.
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::ffi::OsString;
use std::process::ExitCode;

use ghostlight_desktop::{app, Launch, EXIT_USAGE};

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    match Launch::from_arguments(&args) {
        Some(launch) => app::run(launch),
        None => ExitCode::from(EXIT_USAGE),
    }
}
