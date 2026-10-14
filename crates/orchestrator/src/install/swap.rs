//! Replacing an executable that may be running.
//!
//! The installer never overwrites an image in place. [`rename_swap`] copies the new image beside
//! the old one, moves the old one aside under a numbered name, and renames the new one into
//! place. Windows and Unix both allow renaming a running image, so the old service keeps
//! running from its `.old-<n>` name until it exits (ADR-0197 Decision 14). The service removes
//! what is left with [`remove_old_images`] when it starts.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Suffix of the copy being prepared.
const NEW_SUFFIX: &str = ".new";

/// Separator between an image name and the number of the swap that moved it aside.
const OLD_MARKER: &str = ".old-";

/// Replace `dir/name` with the file at `source`.
///
/// The source is copied to `name.new`, the current image is renamed to `name.old-<n>`, and the
/// copy is renamed to `name`. If the last rename fails, the old image gets its name back, so a
/// failed swap leaves the previous image in place. A missing current image is a first install.
/// Missing directories are created.
pub fn rename_swap(dir: &Path, name: &str, source: &Path) -> io::Result<()> {
    swap(dir, name, source, &|from, to| fs::rename(from, to))
}

/// [`rename_swap`] with the rename it uses supplied, so a test can make one rename fail.
fn swap(
    dir: &Path,
    name: &str,
    source: &Path,
    rename: &dyn Fn(&Path, &Path) -> io::Result<()>,
) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let target = dir.join(name);
    let prepared = dir.join(format!("{name}{NEW_SUFFIX}"));
    remove_if_present(&prepared)?;
    if let Err(error) = fs::copy(source, &prepared) {
        let _ = fs::remove_file(&prepared);
        return Err(error);
    }
    let moved_aside = if target.exists() {
        let old = unused_old_name(dir, name);
        if let Err(error) = rename(&target, &old) {
            let _ = fs::remove_file(&prepared);
            return Err(error);
        }
        Some(old)
    } else {
        None
    };
    if let Err(error) = rename(&prepared, &target) {
        if let Some(old) = &moved_aside {
            let _ = rename(old, &target);
        }
        let _ = fs::remove_file(&prepared);
        return Err(error);
    }
    Ok(())
}

/// Remove the images earlier swaps moved aside in `dir`, as far as the system allows.
///
/// An image that is still running cannot be removed on Windows; it stays for the next call.
pub fn remove_old_images(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let is_old_image = entry
            .file_name()
            .to_str()
            .is_some_and(|name| old_image_number(name).is_some());
        if is_old_image {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// The first `name.old-<n>` path in `dir` that nothing occupies.
fn unused_old_name(dir: &Path, name: &str) -> PathBuf {
    (1u32..)
        .map(|number| dir.join(format!("{name}{OLD_MARKER}{number}")))
        .find(|candidate| !candidate.exists())
        .expect("a counter that cannot be exhausted yields a free name")
}

/// The swap number when `file_name` is `<name>.old-<digits>`.
fn old_image_number(file_name: &str) -> Option<u32> {
    let (name, number) = file_name.rsplit_once(OLD_MARKER)?;
    if name.is_empty() || number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    number.parse().ok()
}

/// Remove a file, treating one that is already gone as success.
fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::super::scratch::Scratch;
    use super::*;

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn a_first_install_places_the_image() {
        let scratch = Scratch::new();
        let source = scratch.path().join("payload");
        fs::write(&source, b"one").unwrap();
        let bin = scratch.path().join("bin");
        rename_swap(&bin, "ghostlight", &source).unwrap();
        assert_eq!(fs::read(bin.join("ghostlight")).unwrap(), b"one");
        assert_eq!(names(&bin), ["ghostlight"]);
    }

    #[test]
    fn a_swap_moves_the_current_image_aside_under_a_number() {
        let scratch = Scratch::new();
        let source = scratch.path().join("payload");
        let bin = scratch.path().join("bin");
        for contents in [&b"one"[..], b"two", b"three"] {
            fs::write(&source, contents).unwrap();
            rename_swap(&bin, "ghostlight", &source).unwrap();
        }
        assert_eq!(fs::read(bin.join("ghostlight")).unwrap(), b"three");
        assert_eq!(
            names(&bin),
            ["ghostlight", "ghostlight.old-1", "ghostlight.old-2"]
        );
        assert_eq!(fs::read(bin.join("ghostlight.old-1")).unwrap(), b"one");
        assert_eq!(fs::read(bin.join("ghostlight.old-2")).unwrap(), b"two");
    }

    #[test]
    fn a_missing_source_changes_nothing() {
        let scratch = Scratch::new();
        let bin = scratch.path().join("bin");
        fs::create_dir(&bin).unwrap();
        fs::write(bin.join("ghostlight"), b"current").unwrap();
        assert!(rename_swap(&bin, "ghostlight", &scratch.path().join("absent")).is_err());
        assert_eq!(fs::read(bin.join("ghostlight")).unwrap(), b"current");
        assert_eq!(names(&bin), ["ghostlight"]);
    }

    #[test]
    fn a_failed_last_rename_restores_the_old_name() {
        let scratch = Scratch::new();
        let source = scratch.path().join("payload");
        fs::write(&source, b"new").unwrap();
        let bin = scratch.path().join("bin");
        fs::create_dir(&bin).unwrap();
        fs::write(bin.join("ghostlight"), b"current").unwrap();
        let target = bin.join("ghostlight");
        let prepared = bin.join("ghostlight.new");
        let refuse_the_target = |from: &Path, to: &Path| {
            if from == prepared {
                Err(io::Error::other("refused"))
            } else {
                fs::rename(from, to)
            }
        };
        assert!(swap(&bin, "ghostlight", &source, &refuse_the_target).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"current");
        assert_eq!(names(&bin), ["ghostlight"]);
    }

    #[test]
    fn a_failed_move_aside_leaves_the_current_image() {
        let scratch = Scratch::new();
        let source = scratch.path().join("payload");
        fs::write(&source, b"new").unwrap();
        let bin = scratch.path().join("bin");
        fs::create_dir(&bin).unwrap();
        fs::write(bin.join("ghostlight"), b"current").unwrap();
        let refuse_everything = |_: &Path, _: &Path| Err(io::Error::other("refused"));
        assert!(swap(&bin, "ghostlight", &source, &refuse_everything).is_err());
        assert_eq!(fs::read(bin.join("ghostlight")).unwrap(), b"current");
        assert_eq!(names(&bin), ["ghostlight"]);
    }

    #[test]
    fn only_numbered_old_images_are_removed() {
        let scratch = Scratch::new();
        let dir = scratch.path();
        for name in [
            "ghostlight",
            "ghostlight.old-1",
            "ghostlight.old-12",
            "ghostlight-mcp-connector.old-3",
            "ghostlight.old-x",
            "ghostlight.new",
            "install.json",
        ] {
            fs::write(dir.join(name), b"x").unwrap();
        }
        remove_old_images(dir);
        assert_eq!(
            names(dir),
            [
                "ghostlight",
                "ghostlight.new",
                "ghostlight.old-x",
                "install.json"
            ]
        );
    }

    #[test]
    fn removing_old_images_from_a_missing_directory_is_quiet() {
        let scratch = Scratch::new();
        remove_old_images(&scratch.path().join("absent"));
    }
}
