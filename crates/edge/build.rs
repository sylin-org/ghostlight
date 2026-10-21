//! Build script of the edge: its application manifest and its version information on MSVC targets.
//!
//! The manifest sets `consoleAllocationPolicy` to `detached`, so on Windows 11 24H2 and later a
//! double-click on the console-subsystem edge shows no console window (ADR-0199 Decision 4). The
//! version resource gives every connection process a description of its own, so Task Manager
//! reads "Ghostlight connection" for each MCP client or browser the edge serves. The edge's
//! file and product version are the edge package's version, which is the relay revision. Other
//! targets have neither.

use std::env;
use std::path::PathBuf;

/// The manifest file beside the crate's `Cargo.toml`.
const MANIFEST: &str = "manifest.xml";

/// The resource script file name written into the build output directory.
#[cfg(windows)]
const RESOURCE_FILE: &str = "edge.rc";

/// The Cargo target environment the manifest and the resources are for.
const TARGET_ENV_MSVC: &str = "msvc";

fn main() {
    println!("cargo:rerun-if-changed={MANIFEST}");
    println!("cargo:rerun-if-changed=build.rs");
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok(TARGET_ENV_MSVC) {
        return;
    }
    // The linker reads the manifest's copy in the output directory. Cargo keeps this script's
    // output when the same crate builds from another checkout of the tree, so a path into the
    // checkout that last ran it would outlive that checkout, and the link would fail.
    let manifest =
        PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR")).join(MANIFEST);
    std::fs::copy(MANIFEST, &manifest).expect("copy the application manifest");
    println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
        manifest.display()
    );
    embed_version();
}

/// Compiles the version resource into the executable.
#[cfg(windows)]
fn embed_version() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"));
    let script = out_dir.join(RESOURCE_FILE);
    std::fs::write(&script, resource_script()).expect("write the resource script");
    embed_resource::compile(&script, embed_resource::NONE)
        .manifest_optional()
        .expect("compile the Windows resources");
}

/// A host that cannot compile Windows resources embeds no version information.
#[cfg(not(windows))]
fn embed_version() {}

/// The resource script: the version block Task Manager reads.
#[cfg(windows)]
fn resource_script() -> String {
    let number = |name: &str| env::var(name).expect("Cargo supplies the package version");
    let major = number("CARGO_PKG_VERSION_MAJOR");
    let minor = number("CARGO_PKG_VERSION_MINOR");
    let patch = number("CARGO_PKG_VERSION_PATCH");
    let version = number("CARGO_PKG_VERSION");
    format!(
        r#"1 VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEOS 0x4
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "CompanyName", "Sylin"
      VALUE "FileDescription", "Ghostlight connection"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "ghostlight"
      VALUE "OriginalFilename", "ghostlight.exe"
      VALUE "ProductName", "Ghostlight"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#
    )
}
