//! The `ghostlight-desktop` executable.
//!
//! On Windows it is a GUI-subsystem program, in debug builds too, so nothing a person clicks opens
//! a console. It takes no arguments and runs the standalone spike window.
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::ffi::OsString;
use std::process::ExitCode;

use ghostlight_desktop::{accepts_arguments, spike, EXIT_USAGE};

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    if !accepts_arguments(&args) {
        return ExitCode::from(EXIT_USAGE);
    }
    spike::run()
}
