//! Ghostlight's one atomic file replace and its one private-file creator.
//!
//! [`Replace`] is Ghostlight's one atomic file replace (ADR-0203 Decision 11.2), so a reader sees
//! the old file or the new one, never a partial one. Its options say who may read the new file,
//! whether a backup is kept, and whether a link is written through or refused. [`atomic_replace`]
//! and [`atomic_replace_keeping_access`] name its two common uses.
//!
//! [`create_private`] is the one creator for a file Ghostlight makes that holds a secret or a
//! record about the person (ADR-0203 Decision 11.1). On Windows the file is created with a
//! protected access list that grants the current user and SYSTEM full access and inherits nothing
//! from its directory, so a broad grant on the directory never reaches it. On Unix it is created
//! with mode 0600. A file that already exists keeps the access it has.

use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use ghostlight_relay::private;

use crate::language::deployment as words;

/// How the name of a replace's temporary file begins; a simple UUID and [`TEMPORARY_SUFFIX`]
/// follow. Every replace names its temporary this way, beside the file it replaces, so the stray
/// sweep knows one name ([`is_temporary`]).
const TEMPORARY_PREFIX: &str = ".ghostlight-";
/// How the name of a replace's temporary file ends.
const TEMPORARY_SUFFIX: &str = ".tmp";
/// How many hexadecimal digits the UUID in a temporary's name has.
const TEMPORARY_ID_DIGITS: usize = 32;
/// What follows the replaced file's extension in the name of the copy a backup keeps.
const BACKUP_SUFFIX: &str = "ghostlight-backup";
/// The extension a backup's name uses for a file that has none.
const BACKUP_BARE_EXTENSION: &str = "config";

/// Who may read the file a [`Replace`] publishes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Access {
    /// Only the current user, whatever the old file allowed: on Windows the new file is created
    /// with [`create_private`]'s protected access list, on Unix with mode 0600. Missing parent
    /// directories are created closed to everyone else on Unix.
    Private,
    /// The access the old file had, for a file other accounts or programs read. On Unix the new
    /// file gets the old file's mode; on Windows it inherits from its directory. A file that did
    /// not exist gets its directory's default access, and missing parent directories are created
    /// with the default access too.
    Kept,
}

/// What a [`Replace`] does when its path is a symbolic link.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Link {
    /// Replace the file the link names, so the link stays a link to it. A link whose target is
    /// gone fails the replace.
    Follow,
    /// Fail with [`io::ErrorKind::InvalidInput`] and change nothing, so no bytes go where a link
    /// could send them.
    Refuse,
}

/// The stated options of Ghostlight's one atomic file replace (ADR-0203 Decision 11.2).
///
/// [`Replace::write`] puts the bytes in a new file beside the one it replaces, flushes them to
/// disk, and renames that file over the path. A reader sees the old file or the new one, never a
/// partial one. A failure at any step leaves the old file and no temporary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Replace {
    /// Who may read the new file.
    pub access: Access,
    /// Copy the old file to `<name>.<extension>.ghostlight-backup` beside it ([`backup_path`])
    /// before the new one is published. The system's copy makes it, so it keeps the mode on Unix
    /// and takes its directory's access on Windows.
    pub backup: bool,
    /// What a link at the path means.
    pub link: Link,
}

impl Replace {
    /// Ghostlight's own file, which holds a secret or a record about the person: private, no
    /// backup, and a link refused. What [`atomic_replace`] does.
    pub const OWN: Self = Self {
        access: Access::Private,
        backup: false,
        link: Link::Refuse,
    };

    /// A file other accounts or programs also read: its access kept, no backup, and written
    /// through a link. What [`atomic_replace_keeping_access`] does.
    pub const SHARED: Self = Self {
        access: Access::Kept,
        backup: false,
        link: Link::Follow,
    };

    /// Replace the file at `path` with `bytes`, atomically, under these options.
    pub fn write(self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        let path = &self.target(path)?;
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no parent"))?;
        match self.access {
            Access::Private => private::create_dir_all(parent)?,
            Access::Kept => fs::create_dir_all(parent)?,
        }
        let temporary = parent.join(format!(
            "{TEMPORARY_PREFIX}{}{TEMPORARY_SUFFIX}",
            uuid::Uuid::new_v4().simple()
        ));
        let published = (|| {
            let mut file = match self.access {
                Access::Private => create_private(&temporary)?,
                Access::Kept => create_like(&temporary, path)?,
            };
            file.write_all(bytes)?;
            file.sync_all()?;
            drop(file);
            #[cfg(test)]
            steps::take(steps::Step::Synced(temporary.clone()));
            if self.backup && path.exists() {
                fs::copy(path, backup_path(path))?;
            }
            fs::rename(&temporary, path)?;
            #[cfg(test)]
            steps::take(steps::Step::Renamed(temporary.clone(), path.clone()));
            Ok(())
        })();
        if published.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        published
    }

    /// The file this replace writes for `path`: the path itself, or the file a link there names.
    /// A link refused, or one whose target is gone, is an error.
    pub fn target(self, path: &Path) -> io::Result<PathBuf> {
        let linked = fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_symlink());
        match (linked, self.link) {
            (false, _) => Ok(path.to_path_buf()),
            (true, Link::Follow) => fs::canonicalize(path),
            (true, Link::Refuse) => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                words::link_refused(&path.display().to_string()),
            )),
        }
    }
}

/// Replace the file at `path` with `bytes`, atomically, as Ghostlight's own: [`Replace::OWN`].
///
/// The new file is readable only by the current user, missing parent directories are created
/// closed to everyone else on Unix, and a link at `path` is refused.
pub fn atomic_replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    Replace::OWN.write(path, bytes)
}

/// Replace the file at `path` with `bytes`, atomically, for a file that is not Ghostlight's own:
/// [`Replace::SHARED`].
///
/// The new file has the access the old one had instead of being private, so a policy file that
/// other accounts must read stays readable by them, and a new file gets its directory's default
/// access. A link at `path` is written through.
pub fn atomic_replace_keeping_access(path: &Path, bytes: &[u8]) -> io::Result<()> {
    Replace::SHARED.write(path, bytes)
}

/// Whether `name` is the name of a [`Replace`]'s temporary file, which an interrupted write may
/// leave beside the file it was replacing.
#[must_use]
pub fn is_temporary(name: &OsStr) -> bool {
    name.to_str()
        .and_then(|name| name.strip_prefix(TEMPORARY_PREFIX))
        .and_then(|name| name.strip_suffix(TEMPORARY_SUFFIX))
        .is_some_and(|id| {
            id.len() == TEMPORARY_ID_DIGITS && id.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
}

/// Where a [`Replace`] with a backup keeps the copy of the file at `path`.
#[must_use]
pub fn backup_path(path: &Path) -> PathBuf {
    let extension = path
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or(BACKUP_BARE_EXTENSION);
    path.with_extension(format!("{extension}.{BACKUP_SUFFIX}"))
}

/// Create a new file with the access of `existing`, which it is about to replace, or with the
/// directory's default access when there is none.
fn create_like(path: &Path, existing: &Path) -> io::Result<File> {
    let file = File::options().create_new(true).write(true).open(path)?;
    #[cfg(unix)]
    match fs::metadata(existing) {
        Ok(metadata) => file.set_permissions(metadata.permissions())?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    #[cfg(not(unix))]
    let _ = existing;
    Ok(file)
}

/// The steps a [`Replace`] took on this thread, for the tests that prove their order.
#[cfg(test)]
pub(crate) mod steps {
    use std::cell::RefCell;
    use std::path::PathBuf;

    /// One step of a replace.
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub(crate) enum Step {
        /// The temporary's bytes reached the disk.
        Synced(PathBuf),
        /// The temporary was renamed over the file it replaces.
        Renamed(PathBuf, PathBuf),
    }

    thread_local! {
        static TAKEN: RefCell<Vec<Step>> = const { RefCell::new(Vec::new()) };
    }

    /// Note that this thread's replace took `step`.
    pub(super) fn take(step: Step) {
        TAKEN.with(|taken| taken.borrow_mut().push(step));
    }

    /// The steps this thread's replaces took since the last call, in order.
    pub(crate) fn taken() -> Vec<Step> {
        TAKEN.with(RefCell::take)
    }
}

/// Create a new file at `path`, open for writing, that only the current user can read.
///
/// On Windows the file gets a protected access list for the current user and SYSTEM, applied at
/// creation and inheriting nothing from the directory; on Unix it gets mode 0600. An existing
/// path, a link included, is refused with [`io::ErrorKind::AlreadyExists`], so this never
/// overwrites. The parent directory must exist. The caller writes the bytes and syncs them.
pub fn create_private(path: &Path) -> io::Result<File> {
    #[cfg(windows)]
    return ghostlight_win_peer::create_private_file(path);
    #[cfg(not(windows))]
    private::file_options()
        .create_new(true)
        .write(true)
        .open(path)
}

/// Assert that only the current user can read the file at `path`: on Windows no trustee but the
/// current user and SYSTEM is granted read access, on Unix its mode is 0600.
#[cfg(test)]
pub(crate) fn assert_owner_only(path: &Path) {
    #[cfg(windows)]
    assert!(
        !ghostlight_win_peer::others_can_read(path).unwrap(),
        "another account can read {}",
        path.display()
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "{}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    /// Protects ADR-0203 Decision 11.2: a replace publishes the whole new file or leaves the old
    /// one. Its temporary sits beside the file under the one name the stray sweep recognises and
    /// is gone afterwards, a failed replace keeps what was there, a backup keeps the old file
    /// beside the new one, and an own file refuses a link while a shared one writes through it.
    #[test]
    fn a_replace_publishes_the_whole_file_or_leaves_the_old_one() {
        let scratch = Scratch::new();
        {
            let path = scratch.path().join("nested").join("file.json");
            atomic_replace(&path, b"first").unwrap();
            atomic_replace(&path, b"second").unwrap();
            assert_eq!(fs::read(&path).unwrap(), b"second");
            let names: Vec<_> = fs::read_dir(path.parent().unwrap())
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect();
            assert_eq!(names, ["file.json"]);
        }
        {
            for (replace, name) in [(Replace::OWN, "own.json"), (Replace::SHARED, "shared.json")] {
                let path = scratch.path().join(name);
                steps::taken();
                replace.write(&path, b"{}").unwrap();
                let taken = steps::taken();
                let [steps::Step::Synced(temporary), steps::Step::Renamed(_, published)] =
                    taken.as_slice()
                else {
                    panic!("one synced write then one rename, not {taken:?}");
                };
                assert_eq!(temporary.parent(), path.parent());
                assert!(
                    is_temporary(temporary.file_name().unwrap()),
                    "{temporary:?}"
                );
                assert_eq!(published, &path);
            }
            for other in [
                ".ghostlight-desktop-0123456789abcdef0123456789abcdef.tmp",
                ".ghostlight-0123456789abcdef.tmp",
                ".ghostlight-0123456789abcdef0123456789abcdef.json",
                "ghostlight-0123456789abcdef0123456789abcdef.tmp",
                "settings.json",
            ] {
                assert!(!is_temporary(OsStr::new(other)), "{other}");
            }
        }
        {
            let failed = Scratch::new();
            let path = failed.path().join("directory");
            fs::create_dir(&path).unwrap();
            assert!(atomic_replace(&path, b"bytes").is_err());
            assert!(path.is_dir());
            assert_eq!(fs::read_dir(failed.path()).unwrap().count(), 1);
        }
        {
            let path = scratch.path().join("settings.json");
            let replace = Replace {
                backup: true,
                ..Replace::SHARED
            };
            replace.write(&path, b"first").unwrap();
            assert!(!backup_path(&path).exists());
            replace.write(&path, b"second").unwrap();
            assert_eq!(fs::read(&path).unwrap(), b"second");
            assert_eq!(
                backup_path(&path),
                scratch.path().join("settings.json.ghostlight-backup")
            );
            assert_eq!(fs::read(backup_path(&path)).unwrap(), b"first");
        }
        {
            let absent = scratch.path().join("absent.json");
            assert_eq!(Replace::OWN.target(&absent).unwrap(), absent);
            assert_eq!(Replace::SHARED.target(&absent).unwrap(), absent);

            let real = scratch.path().join("real.json");
            fs::write(&real, b"old").unwrap();
            let link = scratch.path().join("link.json");
            #[cfg(unix)]
            let linked = std::os::unix::fs::symlink(&real, &link).is_ok();
            // Windows needs the symlink privilege (Developer Mode or an elevated shell); without
            // it the Unix run proves the same replace.
            #[cfg(windows)]
            let linked = std::os::windows::fs::symlink_file(&real, &link).is_ok();
            if linked {
                let refused = atomic_replace(&link, b"secret").unwrap_err();
                assert_eq!(refused.kind(), io::ErrorKind::InvalidInput);
                assert_eq!(fs::read(&real).unwrap(), b"old");
                assert!(fs::symlink_metadata(&link).unwrap().is_symlink());

                atomic_replace_keeping_access(&link, b"new").unwrap();
                assert!(fs::symlink_metadata(&link).unwrap().is_symlink());
                assert_eq!(fs::read(&real).unwrap(), b"new");
            }
        }
    }

    /// Protects ADR-0203 Decision 11.1: an own replace is readable only by the current user, with
    /// the directories made for it closed on Unix, while a shared replace creates its file and
    /// directories with the default access and keeps the access an existing file had.
    #[test]
    fn a_replaced_file_is_private_or_keeps_its_access_as_asked() {
        let scratch = Scratch::new();
        let own = scratch.path().join("state").join("settings.json");
        atomic_replace(&own, b"{}").unwrap();
        assert_owner_only(&own);

        let shared = scratch.path().join("shared").join("policy.json");
        atomic_replace_keeping_access(&shared, b"first").unwrap();
        assert_eq!(fs::read(&shared).unwrap(), b"first");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(own.parent().unwrap()), 0o700);

            let plain = scratch.path().join("plain");
            fs::create_dir(&plain).unwrap();
            fs::write(plain.join("file"), b"").unwrap();
            assert_eq!(mode(shared.parent().unwrap()), mode(&plain));
            assert_eq!(mode(&shared), mode(&plain.join("file")));

            fs::set_permissions(&shared, fs::Permissions::from_mode(0o644)).unwrap();
            atomic_replace_keeping_access(&shared, b"second").unwrap();
            assert_eq!(mode(&shared), 0o644);
        }
    }
}
