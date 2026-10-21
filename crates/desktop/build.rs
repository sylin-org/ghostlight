//! Build script of the window process.
//!
//! When the target is Windows and the build host can compile resources, it embeds the product icon
//! at 16, 32, 48 and 128 pixels and the version information (the description Task Manager and Explorer show) into
//! `ghostlight-desktop.exe`. On MSVC targets it also embeds the application manifest, which asks
//! for Common Controls 6: the window's libraries import `TaskDialogIndirect`, which only that
//! version has, so without the manifest Windows refuses to load the image. On every other host or
//! target it does nothing.

#[cfg(windows)]
use std::fs;
#[cfg(windows)]
use std::path::{Path, PathBuf};

/// The product icon at each size the executable carries, smallest first, relative to this crate.
/// Explorer, the Start menu and the taskbar each pick the size nearest their own.
#[cfg(windows)]
const ICON_PNGS: [(u8, &str); 4] = [
    (16, "../../extension/icons/icon16.png"),
    (32, "../../extension/icons/icon32.png"),
    (48, "../../extension/icons/icon48.png"),
    (128, "../../extension/icons/icon128.png"),
];

/// The icon file name written into the build output directory.
#[cfg(windows)]
const ICON_FILE: &str = "ghostlight.ico";

/// The resource script file name written into the build output directory.
#[cfg(windows)]
const RESOURCE_FILE: &str = "desktop.rc";

/// The Cargo target operating system the resources are for.
#[cfg(windows)]
const TARGET_WINDOWS: &str = "windows";

/// The application manifest beside the crate's `Cargo.toml`.
#[cfg(windows)]
const MANIFEST: &str = "manifest.xml";

/// The Cargo target environment whose linker embeds the manifest.
#[cfg(windows)]
const TARGET_ENV_MSVC: &str = "msvc";

/// The size of an ICO file's header.
#[cfg(windows)]
const ICO_HEADER_BYTES: u32 = 6;

/// The size of one entry of an ICO file's directory.
#[cfg(windows)]
const ICO_ENTRY_BYTES: u32 = 16;

#[cfg(windows)]
fn main() {
    for (_, png) in ICON_PNGS {
        println!("cargo:rerun-if-changed={png}");
    }
    println!("cargo:rerun-if-changed={MANIFEST}");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok(TARGET_WINDOWS) {
        return;
    }
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"));
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok(TARGET_ENV_MSVC) {
        // The linker reads the manifest's copy in the output directory. Cargo keeps this script's
        // output when the same crate builds from another checkout of the tree, so a path into the
        // checkout that last ran it would outlive that checkout, and the link would fail.
        let manifest = out_dir.join(MANIFEST);
        fs::copy(MANIFEST, &manifest).expect("copy the application manifest");
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
            manifest.display()
        );
    }
    let icon = write_icon(&out_dir).expect("prepare the Ghostlight Windows icon");
    let script = out_dir.join(RESOURCE_FILE);
    fs::write(&script, resource_script(&icon)).expect("write the resource script");
    embed_resource::compile(&script, embed_resource::NONE)
        .manifest_optional()
        .expect("compile the Windows resources");
}

#[cfg(not(windows))]
fn main() {}

/// Packs the product PNGs into one ICO file, one image per size, and returns its path.
#[cfg(windows)]
fn write_icon(out_dir: &Path) -> std::io::Result<PathBuf> {
    let mut images = Vec::with_capacity(ICON_PNGS.len());
    for (size, png) in ICON_PNGS {
        images.push((size, fs::read(png)?));
    }
    let count = u16::try_from(images.len()).expect("the Ghostlight icon sizes fit an ICO");
    let directory = ICO_HEADER_BYTES + ICO_ENTRY_BYTES * u32::from(count);
    let mut icon = Vec::new();
    icon.extend_from_slice(&0_u16.to_le_bytes());
    icon.extend_from_slice(&1_u16.to_le_bytes());
    icon.extend_from_slice(&count.to_le_bytes());
    let mut offset = directory;
    for (size, png) in &images {
        let length = u32::try_from(png.len()).expect("each Ghostlight icon fits an ICO entry");
        icon.extend_from_slice(&[*size, *size, 0, 0]);
        icon.extend_from_slice(&1_u16.to_le_bytes());
        icon.extend_from_slice(&32_u16.to_le_bytes());
        icon.extend_from_slice(&length.to_le_bytes());
        icon.extend_from_slice(&offset.to_le_bytes());
        offset += length;
    }
    for (_, png) in &images {
        icon.extend_from_slice(png);
    }
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
