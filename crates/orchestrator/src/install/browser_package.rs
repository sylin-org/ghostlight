//! Read-only Linux browser-package provenance for native-host usability.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// Package form of one supported browser installation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserPackage {
    /// A host executable outside known sandbox roots is available.
    Native,
    /// Only a Snap installation was found.
    Snap,
    /// Only a Flatpak installation was found.
    Flatpak,
    /// Both Snap and Flatpak installations were found, with no native installation.
    MultipleSandboxes,
    /// No supported package form was found.
    NotDetected,
    /// Package provenance is not inspected on this operating system.
    NotChecked,
}

impl BrowserPackage {
    /// Whether this package form can start an ordinary host native-messaging executable.
    #[must_use]
    pub fn native_messaging_usable(self) -> bool {
        matches!(self, Self::Native | Self::NotChecked)
    }

    /// Whether the browser was found only inside an unsupported sandbox.
    #[must_use]
    pub fn sandboxed(self) -> bool {
        matches!(self, Self::Snap | Self::Flatpak | Self::MultipleSandboxes)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct BrowserPackageSpec {
    pub(crate) executables: &'static [&'static str],
    pub(crate) snap_executable: &'static str,
    pub(crate) flatpak_ids: &'static [&'static str],
}

#[derive(Clone, Debug)]
pub(crate) struct BrowserPackageContext {
    linux: bool,
    path_entries: Vec<PathBuf>,
    snap_bin: PathBuf,
    user_flatpak_applications: PathBuf,
    system_flatpak_applications: PathBuf,
}

impl BrowserPackageContext {
    /// The package places of the person's directories `dirs`, on Linux when `linux`.
    pub(super) fn system(dirs: &super::user_dirs::UserDirs, linux: bool) -> Self {
        let data_home = dirs.xdg_data();
        Self {
            linux,
            path_entries: dirs.path_entries().to_vec(),
            snap_bin: PathBuf::from("/snap/bin"),
            user_flatpak_applications: data_home.join("flatpak/exports/share/applications"),
            system_flatpak_applications: PathBuf::from(
                "/var/lib/flatpak/exports/share/applications",
            ),
        }
    }

    #[cfg(test)]
    pub(crate) fn isolated(root: &Path) -> Self {
        Self {
            linux: true,
            path_entries: vec![root.join("native-bin")],
            snap_bin: root.join("snap-bin"),
            user_flatpak_applications: root.join("user-flatpak-applications"),
            system_flatpak_applications: root.join("system-flatpak-applications"),
        }
    }
}

pub(crate) fn inspect(context: &BrowserPackageContext, spec: BrowserPackageSpec) -> BrowserPackage {
    if !context.linux {
        return BrowserPackage::NotChecked;
    }
    if spec
        .executables
        .iter()
        .any(|name| native_executable_path(context, name).is_some())
    {
        return BrowserPackage::Native;
    }
    let snap = context.snap_bin.join(spec.snap_executable).is_file();
    let flatpak = spec.flatpak_ids.iter().any(|id| {
        let desktop_entry = format!("{id}.desktop");
        context
            .user_flatpak_applications
            .join(&desktop_entry)
            .is_file()
            || context
                .system_flatpak_applications
                .join(desktop_entry)
                .is_file()
    });
    match (snap, flatpak) {
        (true, true) => BrowserPackage::MultipleSandboxes,
        (true, false) => BrowserPackage::Snap,
        (false, true) => BrowserPackage::Flatpak,
        (false, false) => BrowserPackage::NotDetected,
    }
}

pub(crate) fn detail(name: &str, package: BrowserPackage) -> String {
    match package {
        BrowserPackage::Native => {
            format!("A native {name} package can start Ghostlight.")
        }
        BrowserPackage::Snap => format!(
            "{name} is installed as a Snap. Its sandbox cannot connect to Ghostlight; install a supported native browser package."
        ),
        BrowserPackage::Flatpak => format!(
            "{name} is installed as a Flatpak. Its sandbox cannot connect to Ghostlight; install a supported native browser package."
        ),
        BrowserPackage::MultipleSandboxes => format!(
            "{name} is installed as Snap and Flatpak packages. Neither sandbox can start Ghostlight; install a supported native browser package."
        ),
        BrowserPackage::NotDetected => format!("No supported {name} installation was detected."),
        BrowserPackage::NotChecked => {
            "Browser package provenance is not checked on this operating system.".into()
        }
    }
}

pub(crate) fn native_executable_path(
    context: &BrowserPackageContext,
    name: &str,
) -> Option<PathBuf> {
    context.path_entries.iter().find_map(|directory| {
        let candidate = directory.join(name);
        (executable_file(&candidate)
            && !sandbox_path(&candidate, context)
            && !small_snap_wrapper(&candidate))
        .then_some(candidate)
    })
}

fn executable_file(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn sandbox_path(path: &Path, context: &BrowserPackageContext) -> bool {
    path.starts_with(&context.snap_bin)
        || path.starts_with(&context.user_flatpak_applications)
        || path.starts_with(&context.system_flatpak_applications)
        || fs::canonicalize(path).is_ok_and(|resolved| {
            resolved.starts_with("/snap") || resolved.starts_with("/var/lib/flatpak")
        })
}

fn small_snap_wrapper(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if metadata.len() > 64 * 1024 {
        return false;
    }
    fs::read(path).is_ok_and(|bytes| {
        let source = String::from_utf8_lossy(&bytes);
        source.contains("snap run ") || source.contains("/snap/bin/")
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    use crate::scratch::Scratch;

    use super::{inspect, BrowserPackage, BrowserPackageContext, BrowserPackageSpec};

    const SPEC: BrowserPackageSpec = BrowserPackageSpec {
        executables: &["chromium", "chromium-browser"],
        snap_executable: "chromium",
        flatpak_ids: &["org.chromium.Chromium"],
    };

    fn context(name: &str) -> (Scratch, BrowserPackageContext) {
        let root = Scratch::named(&format!("browser-package-{name}"));
        let context = BrowserPackageContext::isolated(&root);
        (root, context)
    }

    fn touch(path: &std::path::Path, bytes: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    #[cfg(unix)]
    fn make_executable(path: &std::path::Path) {
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    /// ACCEPTANCE Desktop 16: a Linux browser is told apart by how it is packaged, and only a
    /// native one can start Ghostlight's native host. A native executable on `PATH` wins over the
    /// sandboxed forms; a Snap, a Flatpak and both together each read as their own form; a launcher
    /// script that runs the Snap is the Snap; and a file on `PATH` that cannot run (defect 8ac06f75)
    /// is no native browser.
    #[test]
    fn package_forms_are_told_apart_and_native_wins() {
        let (_root, forms) = context("forms");
        assert_eq!(inspect(&forms, SPEC), BrowserPackage::NotDetected);
        touch(&forms.snap_bin.join("chromium"), b"snap");
        assert_eq!(inspect(&forms, SPEC), BrowserPackage::Snap);
        touch(
            &forms
                .user_flatpak_applications
                .join("org.chromium.Chromium.desktop"),
            b"flatpak",
        );
        assert_eq!(inspect(&forms, SPEC), BrowserPackage::MultipleSandboxes);
        let native = forms.path_entries[0].join("chromium");
        touch(&native, b"native binary");
        #[cfg(unix)]
        make_executable(&native);
        assert_eq!(inspect(&forms, SPEC), BrowserPackage::Native);

        let (_root, flatpak) = context("flatpak");
        touch(
            &flatpak
                .system_flatpak_applications
                .join("org.chromium.Chromium.desktop"),
            b"flatpak",
        );
        assert_eq!(inspect(&flatpak, SPEC), BrowserPackage::Flatpak);

        let (_root, wrapper) = context("wrapper");
        touch(
            &wrapper.path_entries[0].join("chromium-browser"),
            b"#!/bin/sh\nexec snap run chromium \"$@\"\n",
        );
        touch(&wrapper.snap_bin.join("chromium"), b"snap");
        assert_eq!(inspect(&wrapper, SPEC), BrowserPackage::Snap);

        #[cfg(unix)]
        {
            let (_root, stub) = context("non-executable");
            touch(&stub.path_entries[0].join("chromium"), b"not executable");
            assert_eq!(inspect(&stub, SPEC), BrowserPackage::NotDetected);
        }
    }
}
