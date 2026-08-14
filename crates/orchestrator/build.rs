#[cfg(not(any(target_os = "windows", target_os = "linux")))]
compile_error!("Ghostlight supports Windows and Linux only.");

fn main() {
    #[cfg(target_os = "windows")]
    {
        let icon = windows_icon().expect("prepare the original Ghostlight Windows icon");
        let attributes = tauri_build::Attributes::new()
            .windows_attributes(tauri_build::WindowsAttributes::new().window_icon_path(icon));
        tauri_build::try_build(attributes).expect("build Tauri desktop resources");
    }
    #[cfg(target_os = "linux")]
    tauri_build::build();
}

#[cfg(target_os = "windows")]
fn windows_icon() -> std::io::Result<std::path::PathBuf> {
    use std::fs;
    use std::path::PathBuf;

    let source = PathBuf::from("../../extension/icons/icon128.png");
    println!("cargo:rerun-if-changed={}", source.display());
    let png = fs::read(source)?;
    let output = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("ghostlight.ico");
    let mut icon = Vec::with_capacity(22 + png.len());
    icon.extend_from_slice(&0_u16.to_le_bytes());
    icon.extend_from_slice(&1_u16.to_le_bytes());
    icon.extend_from_slice(&1_u16.to_le_bytes());
    icon.push(128);
    icon.push(128);
    icon.push(0);
    icon.push(0);
    icon.extend_from_slice(&1_u16.to_le_bytes());
    icon.extend_from_slice(&32_u16.to_le_bytes());
    icon.extend_from_slice(
        &u32::try_from(png.len())
            .expect("the Ghostlight icon fits an ICO entry")
            .to_le_bytes(),
    );
    icon.extend_from_slice(&22_u32.to_le_bytes());
    icon.extend_from_slice(&png);
    fs::write(&output, icon)?;
    Ok(output)
}
