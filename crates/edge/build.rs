//! Embeds the edge's application manifest on MSVC targets.
//!
//! The manifest sets `consoleAllocationPolicy` to `detached`, so on Windows 11 24H2 and later a
//! double-click on the console-subsystem edge shows no console window (ADR-0199 Decision 4). Other
//! targets have no manifest.

use std::env;
use std::path::PathBuf;

/// The manifest file beside the crate's `Cargo.toml`.
const MANIFEST: &str = "manifest.xml";

fn main() {
    println!("cargo:rerun-if-changed={MANIFEST}");
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        return;
    }
    let directory = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("Cargo supplies CARGO_MANIFEST_DIR"),
    );
    println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
        directory.join(MANIFEST).display()
    );
}
