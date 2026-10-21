//! Ghostlight's one same-location rule (ADR-0203 Decision 11.2), for every installer part:
//! [`same_path`] by spelling, [`same_location`] once links are resolved, and [`lies_within`] for
//! a path below a directory, each for this system or, with `_on` and a `windows` flag, for the
//! system a context describes.

use std::path::{Path, PathBuf};

/// The path `path` names once links, `.` and `..` are resolved, without the `\?\` prefix
/// Windows puts on resolved paths. A path that cannot be resolved is returned unchanged.
///
/// This is how the entry rule's inputs are made comparable: the running executable (which Linux
/// reports with links resolved) and the installed service both go through it before [`decide`]
/// sees them.
#[must_use]
pub fn canonical_path(path: &Path) -> PathBuf {
    ghostlight_relay::home::canonical(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Whether two paths are spelled as one path on this system: [`spelled_alike`] for the system
/// this build runs on. For paths already resolved with [`canonical_path`], such as the entry
/// rule's executables; [`same_location`] resolves them itself.
#[must_use]
pub fn same_path(left: &Path, right: &Path) -> bool {
    spelled_alike(left, right, cfg!(windows))
}

/// Whether two paths are spelled as one path on Windows (`windows`) or on another system.
///
/// On Windows the comparison ignores case, the difference between `/` and `\`, a trailing
/// separator and the verbatim prefix. Elsewhere it compares the paths' components, so a trailing
/// or doubled `/` and an inner `.` do not matter and case does.
#[must_use]
pub(crate) fn spelled_alike(left: &Path, right: &Path, windows: bool) -> bool {
    if windows {
        windows_spelling(left) == windows_spelling(right)
    } else {
        left == right
    }
}

/// Whether two paths name the same location: Ghostlight's one same-location rule (ADR-0203
/// Decision 11.2), for the system this build runs on.
///
/// They do when they are [`spelled_alike`], or when they are once links, `.` and `..` are
/// resolved ([`canonical_path`]). A path that cannot be resolved is compared as it is spelled.
#[must_use]
pub fn same_location(left: &Path, right: &Path) -> bool {
    same_location_on(left, right, cfg!(windows))
}

/// [`same_location`] for paths of Windows (`windows`) or of another system.
#[must_use]
pub(crate) fn same_location_on(left: &Path, right: &Path, windows: bool) -> bool {
    spelled_alike(left, right, windows)
        || spelled_alike(&canonical_path(left), &canonical_path(right), windows)
}

/// Whether `path` is `directory` or lies below it, spelled as [`spelled_alike`] reads paths of
/// Windows (`windows`) or of another system. Links are not resolved.
#[must_use]
pub(crate) fn lies_within(path: &Path, directory: &Path, windows: bool) -> bool {
    if !windows {
        return path.starts_with(directory);
    }
    let path = windows_spelling(path);
    let directory = windows_spelling(directory);
    path == directory
        || path
            .strip_prefix(&directory)
            .is_some_and(|rest| rest.starts_with('\\'))
}

/// The key every Windows spelling of one path shares: backslashes, no verbatim prefix, no
/// trailing separator, lowercase.
fn windows_spelling(path: &Path) -> String {
    let text = path.to_string_lossy().replace('/', "\\");
    strip_verbatim(&text).trim_end_matches('\\').to_lowercase()
}

/// The text of a Windows path without its verbatim prefix.
pub(super) fn strip_verbatim(text: &str) -> String {
    if let Some(rest) = text.strip_prefix(VERBATIM_UNC_PREFIX) {
        format!("\\\\{rest}")
    } else if let Some(rest) = text.strip_prefix(VERBATIM_PREFIX) {
        rest.to_owned()
    } else {
        text.to_owned()
    }
}

/// Prefix Windows puts on a resolved local path.
pub(super) const VERBATIM_PREFIX: &str = "\\\\?\\";

/// Prefix Windows puts on a resolved network path.
const VERBATIM_UNC_PREFIX: &str = "\\\\?\\UNC\\";
