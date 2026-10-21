//! Build script of the service: its icon, its version information and its application manifest on
//! Windows targets.
//!
//! The version resource gives the service a description of its own, so Task Manager reads
//! "Ghostlight service" for it, "Ghostlight" for the window and "Ghostlight connection" for each
//! edge (ADR-0199 Decisions 1 and 4). The service is a console-subsystem program. On MSVC targets
//! it embeds the edge's application manifest, whose `consoleAllocationPolicy` is `detached`, so
//! on Windows 11 24H2 and later a start that has no console to join shows none. The service's
//! needs are exactly the edge's (only that one setting), and the edge is frozen, so the build
//! reads the edge crate's manifest instead of keeping a second one in the tree. Other targets
//! embed nothing.

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
compile_error!("Ghostlight supports Windows and Linux only.");

use std::env;
use std::path::PathBuf;

/// The product icon the executable carries, relative to this crate.
const ICON_PNG: &str = "../../extension/icons/icon128.png";

/// The application manifest the service shares with the edge, relative to this crate.
const MANIFEST: &str = "../edge/manifest.xml";

/// The name of the manifest's copy in the build output directory.
const MANIFEST_COPY: &str = "manifest.xml";

/// The Cargo target operating system the resources are for.
const TARGET_OS_WINDOWS: &str = "windows";

/// The Cargo target environment whose linker embeds the manifest.
const TARGET_ENV_MSVC: &str = "msvc";

fn main() {
    println!("cargo:rerun-if-changed={ICON_PNG}");
    println!("cargo:rerun-if-changed={MANIFEST}");
    println!("cargo:rerun-if-changed=build.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok(TARGET_OS_WINDOWS) {
        return;
    }
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok(TARGET_ENV_MSVC) {
        // The linker reads the manifest's copy in the output directory. Cargo keeps this script's
        // output when the same crate builds from another checkout of the tree, so a path into the
        // checkout that last ran it would outlive that checkout, and the link would fail.
        let manifest = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
            .join(MANIFEST_COPY);
        std::fs::copy(MANIFEST, &manifest).expect("copy the application manifest");
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
            manifest.display()
        );
    }
    embed_resources();
}

/// Compiles the icon and the version resource into the executable.
#[cfg(windows)]
fn embed_resources() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"));
    let icon = write_icon(&out_dir).expect("prepare the Ghostlight Windows icon");
    let script = out_dir.join("service.rc");
    std::fs::write(&script, resource_script(&icon)).expect("write the resource script");
    embed_resource::compile(&script, embed_resource::NONE)
        .manifest_optional()
        .expect("compile the Windows resources");
}

/// A host that cannot compile Windows resources embeds none.
#[cfg(not(windows))]
fn embed_resources() {}

/// The size of an ICO header with one directory entry; the PNG follows it.
#[cfg(windows)]
const ICO_HEADER_BYTES: u32 = 22;

/// Wraps the product PNG in a single-image ICO file and returns its path.
#[cfg(windows)]
fn write_icon(out_dir: &std::path::Path) -> std::io::Result<PathBuf> {
    let png = std::fs::read(ICON_PNG)?;
    let length = u32::try_from(png.len()).expect("the Ghostlight icon fits an ICO entry");
    let mut icon = Vec::with_capacity(ICO_HEADER_BYTES as usize + png.len());
    icon.extend_from_slice(&0_u16.to_le_bytes());
    icon.extend_from_slice(&1_u16.to_le_bytes());
    icon.extend_from_slice(&1_u16.to_le_bytes());
    icon.extend_from_slice(&[128, 128, 0, 0]);
    icon.extend_from_slice(&1_u16.to_le_bytes());
    icon.extend_from_slice(&32_u16.to_le_bytes());
    icon.extend_from_slice(&length.to_le_bytes());
    icon.extend_from_slice(&ICO_HEADER_BYTES.to_le_bytes());
    icon.extend_from_slice(&png);
    let path = out_dir.join("ghostlight.ico");
    std::fs::write(&path, icon)?;
    Ok(path)
}

/// The resource script: the icon and the version block Task Manager reads.
#[cfg(windows)]
fn resource_script(icon: &std::path::Path) -> String {
    let number = |name: &str| env::var(name).expect("Cargo supplies the package version");
    let major = number("CARGO_PKG_VERSION_MAJOR");
    let minor = number("CARGO_PKG_VERSION_MINOR");
    let patch = number("CARGO_PKG_VERSION_PATCH");
    let version = number("CARGO_PKG_VERSION");
    let icon = icon.display().to_string().replace('\\', "\\\\");
    format!(
        r#"1 ICON "{icon}"

1 VERSIONINFO
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
      VALUE "FileDescription", "Ghostlight service"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "ghostlight-service"
      VALUE "OriginalFilename", "ghostlight-service.exe"
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
