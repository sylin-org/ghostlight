//! Process test: Windows loads the built `ghostlight-desktop.exe`.
//!
//! The window's libraries import functions that only Common Controls 6 has, so an image built
//! without its application manifest is refused by the loader before `main` runs. Started with an
//! argument it does not take, a loadable image reaches `main` and exits with the usage code at
//! once, without opening a window or reaching the service.

#![cfg(windows)]

use std::process::Command;

use ghostlight_desktop::EXIT_USAGE;

/// An argument the window does not take.
const UNKNOWN_ARGUMENT: &str = "--no-such-argument";

#[test]
fn the_window_image_loads_and_reaches_main() {
    let status = Command::new(env!("CARGO_BIN_EXE_ghostlight-desktop"))
        .arg(UNKNOWN_ARGUMENT)
        .status()
        .expect("start the built window image");
    assert_eq!(
        status.code(),
        Some(i32::from(EXIT_USAGE)),
        "the loader refused the image or main did not run: {status}"
    );
}
