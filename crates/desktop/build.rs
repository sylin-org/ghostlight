//! Build script of the window process.
//!
//! When the target is Windows and the build host can compile resources, it embeds the product icon
//! and the version information (the description Task Manager and Explorer show) into
//! `ghostlight-desktop.exe`. On every other host or target it does nothing.

#[cfg(windows)]
use std::fs;
#[cfg(windows)]
use std::path::{Path, PathBuf};

/// The product icon the executable carries, relative to this crate.
#[cfg(windows)]
const ICON_PNG: &str = "../../extension/icons/icon128.png";

/// The icon file name written into the build output directory.
#[cfg(windows)]
const ICON_FILE: &str = "ghostlight.ico";

/// The resource script file name written into the build output directory.
#[cfg(windows)]
const RESOURCE_FILE: &str = "desktop.rc";

/// The Cargo target operating system the resources are for.
#[cfg(windows)]
const TARGET_WINDOWS: &str = "windows";

/// The size of an ICO header with one directory entry; the PNG follows it.
#[cfg(windows)]
const ICO_HEADER_BYTES: u32 = 22;

#[cfg(windows)]
fn main() {
    println!("cargo:rerun-if-changed={ICON_PNG}");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok(TARGET_WINDOWS) {
        return;
    }
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"));
    let icon = write_icon(&out_dir).expect("prepare the Ghostlight Windows icon");
    let script = out_dir.join(RESOURCE_FILE);
    fs::write(&script, resource_script(&icon)).expect("write the resource script");
    embed_resource::compile(&script, embed_resource::NONE)
        .manifest_optional()
        .expect("compile the Windows resources");
}

#[cfg(not(windows))]
fn main() {}

/// Wraps the product PNG in a single-image ICO file and returns its path.
#[cfg(windows)]
fn write_icon(out_dir: &Path) -> std::io::Result<PathBuf> {
    let png = fs::read(ICON_PNG)?;
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
    let path = out_dir.join(ICON_FILE);
    fs::write(&path, icon)?;
    Ok(path)
}

/// The resource script: the icon and the version block Task Manager reads.
#[cfg(windows)]
fn resource_script(icon: &Path) -> String {
    let number = |name: &str| std::env::var(name).expect("Cargo supplies the package version");
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
      VALUE "FileDescription", "Ghostlight"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "ghostlight-desktop"
      VALUE "OriginalFilename", "ghostlight-desktop.exe"
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
