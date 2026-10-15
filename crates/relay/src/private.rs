//! Owner-private files and directories under the Ghostlight home.
//!
//! On Linux every file Ghostlight creates is readable only by the user who runs it and every
//! directory it creates is closed to everyone else. On Windows the home sits inside the user
//! profile, so the ordinary calls already give that and these helpers change nothing there.

use std::fs::{DirBuilder, OpenOptions};
use std::io;
use std::path::Path;

/// File mode of a file Ghostlight creates on Unix: read and write for the owner only.
#[cfg(unix)]
pub const FILE_MODE: u32 = 0o600;

/// Directory mode of a directory Ghostlight creates on Unix: the owner only.
#[cfg(unix)]
pub const DIRECTORY_MODE: u32 = 0o700;

/// Open options for a file that only the current user can read.
///
/// The caller adds the access flags. Existing files keep the mode they already have.
#[must_use]
pub fn file_options() -> OpenOptions {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut options = OpenOptions::new();
        options.mode(FILE_MODE);
        options
    }
    #[cfg(not(unix))]
    OpenOptions::new()
}

/// Create `path` and any missing parent directories, each closed to everyone but the user.
pub fn create_dir_all(path: &Path) -> io::Result<()> {
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(DIRECTORY_MODE);
    }
    builder.create(path)
}

/// Make an existing file readable only by the current user.
///
/// For a file that arrived by rename and so still has the mode it was created with. On Windows
/// this does nothing.
pub fn restrict_file(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(FILE_MODE))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}
