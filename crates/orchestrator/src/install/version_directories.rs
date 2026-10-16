//! The 1.x per-version payload directories under `bin`.
//!
//! The 1.x launchers and download installers unpacked each release into `bin/v<version>`
//! (ADR-0193). 2.0 keeps one set of executables in `bin` itself (ADR-0197 Decision 13), so these
//! directories hold nothing that runs. Every install finds them here, stops what still runs from
//! one (a stray of its own kind, see `deployment::StrayKind`), archives the audit log a directory
//! holds into `home/state` beside the break migration's archive, and removes the directory. An
//! audit log that cannot be archived keeps its directory. Only a directory directly under `bin`
//! whose name is `v` followed by a semantic version counts; anything else in `bin` is never
//! touched. Nothing here stops a process or changes a registration: the installer does that
//! first.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ghostlight_relay::private;

use super::entry::same_path;
use super::layout::Layout;
use super::migration::{self, Move, AUDIT_FILE_NAME};
use super::record::Version;

/// The letter every 1.x version directory name starts with, before the version.
const VERSION_PREFIX: char = 'v';

/// What became of the directories one retirement handled.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Retired {
    /// The directories that are gone.
    pub removed: Vec<PathBuf>,
    /// The directories that stay: their audit log could not be archived, or they could not be
    /// removed. The next install tries again.
    pub kept: Vec<PathBuf>,
    /// The audit logs this run moved into `state`: only a log that was there and now sits at its
    /// archive file.
    pub archived: Vec<Move>,
}

/// Whether `name` is the name of a 1.x version directory: `v` followed by a semantic version.
fn is_version_name(name: &str) -> bool {
    name.strip_prefix(VERSION_PREFIX)
        .is_some_and(|version| Version::parse(version).is_some())
}

/// Whether `path` is a 1.x version directory of `layout`: directly under `bin`, with a version
/// directory name. The disk is not read.
#[must_use]
pub fn is_version_directory(layout: &Layout, path: &Path) -> bool {
    path.parent()
        .is_some_and(|parent| same_path(parent, &layout.bin()))
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(is_version_name)
}

/// The 1.x version directories that exist in `layout`'s `bin`, in name order.
///
/// A file, a link and any other name are not listed. A `bin` that cannot be read lists nothing.
#[must_use]
pub fn find(layout: &Layout) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(layout.bin()) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.path())
        .filter(|path| is_version_directory(layout, path))
        .collect();
    found.sort();
    found
}

/// The audit logs `directories` hold, each with the file in `home/state` that keeps it. The
/// archive's name says which version directory the log came from.
#[must_use]
pub fn archives(layout: &Layout, directories: &[PathBuf]) -> Vec<Move> {
    directories
        .iter()
        .filter_map(|directory| {
            let log = directory.join(AUDIT_FILE_NAME);
            let name = directory.file_name()?.to_str()?;
            log.is_file().then(|| Move {
                from: log,
                to: layout.state().join(format!("audit-{name}.jsonl")),
            })
        })
        .collect()
}

/// The move in `archives` that keeps the audit log of `directory`, when it has one.
#[must_use]
pub fn archive_of<'a>(archives: &'a [Move], directory: &Path) -> Option<&'a Move> {
    archives
        .iter()
        .find(|transfer| transfer.from.parent() == Some(directory))
}

/// Move each log of `archives` to its archive file, the way the break migration moves the 1.x
/// log. Returns the moves this run carried out and the directories whose log could not be
/// archived, which must stay.
///
/// A move an earlier run began is finished. An archive file that already holds other content is
/// never overwritten: the log stays and so does its directory. A log that is no longer there,
/// because the break migration archived it first, is neither moved nor listed.
fn archive(archives: &[Move]) -> (Vec<Move>, Vec<PathBuf>) {
    let mut archived = Vec::new();
    let mut unarchived = Vec::new();
    for transfer in archives {
        let present = transfer.from.is_file();
        let moved = transfer
            .to
            .parent()
            .map_or(Ok(()), private::create_dir_all)
            .map_err(|_| ())
            .and_then(|()| migration::archive_transfer(transfer).map_err(|_| ()));
        match moved {
            Ok(()) if present && transfer.to.is_file() => archived.push(transfer.clone()),
            Ok(()) => {}
            Err(()) => unarchived.extend(transfer.from.parent().map(Path::to_path_buf)),
        }
    }
    (archived, unarchived)
}

/// Archive the logs of `archives`, then remove each of `directories`. A directory whose log
/// could not be archived is not removed. The result names what was done, not what was planned.
///
/// The programs running from the directories must already have been stopped.
#[must_use]
pub fn retire(directories: &[PathBuf], archives: &[Move]) -> Retired {
    let (archived, unarchived) = archive(archives);
    let mut retired = Retired {
        archived,
        ..Retired::default()
    };
    for directory in directories {
        let removed = !unarchived.iter().any(|kept| same_path(kept, directory))
            && match fs::remove_dir_all(directory) {
                Ok(()) => true,
                Err(error) => error.kind() == io::ErrorKind::NotFound,
            };
        if removed {
            retired.removed.push(directory.clone());
        } else {
            retired.kept.push(directory.clone());
        }
    }
    retired
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::scratch::Scratch;

    fn layout_with(directories: &[&str], files: &[&str]) -> (Scratch, Layout) {
        let scratch = Scratch::new();
        let layout = Layout::new(scratch.path());
        fs::create_dir_all(layout.bin()).unwrap();
        for name in directories {
            fs::create_dir_all(layout.bin().join(name)).unwrap();
        }
        for name in files {
            fs::write(layout.bin().join(name), b"file").unwrap();
        }
        (scratch, layout)
    }

    #[test]
    fn only_a_v_and_a_semantic_version_make_a_version_name() {
        for name in ["v1.3.5", "v2.0.0-rc.1", "v10.20.30+build"] {
            assert!(is_version_name(name), "{name}");
        }
        for name in [
            "vnext", "v", "1.3.5", "V1.3.5", "v1.3", "v1.3.5.6", "v1.3.x", "",
        ] {
            assert!(!is_version_name(name), "{name}");
        }
    }

    #[test]
    fn find_lists_version_directories_in_order_and_nothing_else() {
        let (_scratch, layout) = layout_with(
            &["v1.3.6", "v1.3.5", "vnext", "tools", "v1.4"],
            &["v1.3.7", "install.json"],
        );
        assert_eq!(
            find(&layout),
            [layout.bin().join("v1.3.5"), layout.bin().join("v1.3.6")]
        );
    }

    #[test]
    fn a_home_without_bin_has_no_version_directories() {
        let scratch = Scratch::new();
        assert!(find(&Layout::new(scratch.path())).is_empty());
    }

    #[test]
    fn a_directory_is_a_version_directory_only_directly_under_bin() {
        let layout = Layout::new("home");
        assert!(is_version_directory(&layout, &layout.bin().join("v1.3.5")));
        assert!(!is_version_directory(
            &layout,
            &layout.bin().join("v1.3.5").join("v1.3.5")
        ));
        assert!(!is_version_directory(
            &layout,
            &layout.home().join("v1.3.5")
        ));
        assert!(!is_version_directory(&layout, &layout.bin().join("vnext")));
    }

    #[test]
    fn a_log_is_archived_under_a_name_that_says_where_it_came_from() {
        let (_scratch, layout) = layout_with(&["v1.3.5", "v1.3.6"], &[]);
        let directories = find(&layout);
        fs::write(directories[0].join(AUDIT_FILE_NAME), b"one\n").unwrap();
        assert_eq!(
            archives(&layout, &directories),
            [Move {
                from: directories[0].join(AUDIT_FILE_NAME),
                to: layout.state().join("audit-v1.3.5.jsonl"),
            }]
        );
    }

    #[test]
    fn retiring_keeps_the_log_and_removes_the_directory() {
        let (_scratch, layout) = layout_with(&["v1.3.5", "v1.3.6"], &[]);
        let directories = find(&layout);
        fs::write(directories[0].join(AUDIT_FILE_NAME), b"one\n").unwrap();
        let moves = archives(&layout, &directories);
        let retired = retire(&directories, &moves);
        assert_eq!(retired.removed, directories);
        assert!(retired.kept.is_empty());
        assert_eq!(
            fs::read(layout.state().join("audit-v1.3.5.jsonl")).unwrap(),
            b"one\n"
        );
        assert!(find(&layout).is_empty());
    }

    #[test]
    fn an_archive_file_with_other_content_is_not_overwritten_and_keeps_the_directory() {
        let (_scratch, layout) = layout_with(&["v1.3.5", "v1.3.6"], &[]);
        let directories = find(&layout);
        fs::write(directories[0].join(AUDIT_FILE_NAME), b"one\n").unwrap();
        fs::create_dir_all(layout.state()).unwrap();
        let archive_file = layout.state().join("audit-v1.3.5.jsonl");
        fs::write(&archive_file, b"other\n").unwrap();
        let retired = retire(&directories, &archives(&layout, &directories));
        assert_eq!(retired.kept, [directories[0].clone()]);
        assert_eq!(retired.removed, [directories[1].clone()]);
        assert_eq!(fs::read(&archive_file).unwrap(), b"other\n");
        assert_eq!(
            fs::read(directories[0].join(AUDIT_FILE_NAME)).unwrap(),
            b"one\n"
        );
    }

    #[test]
    fn retiring_twice_is_harmless() {
        let (_scratch, layout) = layout_with(&["v1.3.5"], &[]);
        let directories = find(&layout);
        fs::write(directories[0].join(AUDIT_FILE_NAME), b"one\n").unwrap();
        let moves = archives(&layout, &directories);
        assert!(retire(&directories, &moves).kept.is_empty());
        assert!(retire(&directories, &moves).kept.is_empty());
    }
}
