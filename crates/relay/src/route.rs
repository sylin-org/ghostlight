//! Where an edge's invocation goes (ADR-0199 Decision 2).
//!
//! One edge executable serves four jobs, and this module is the whole decision, frozen with the
//! relay. The caller resolves both paths with [`crate::home::canonical`] and names the edge's own
//! file, so this module names no file itself. The carrier check comes first: an edge that does
//! not run from `<home>/bin` under the edge's file name is a carrier copy, and it runs the
//! service image found beside it. Then three rules, in order: the first argument `mcp`, a first
//! argument that is a Chrome extension origin, anything else.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// The first argument that selects the MCP connector. It matches exactly, case included.
pub const MCP_ARGUMENT: &str = "mcp";

/// What a browser passes first when it starts a native messaging host: the extension's origin.
pub const BROWSER_ORIGIN_PREFIX: &str = "chrome-extension://";

/// What an invocation of the edge does.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Route {
    /// This copy is not the installed edge: run the service image found in `directory`, the
    /// directory that holds this executable, with the same arguments. An executable with no
    /// parent directory gives an empty `directory`.
    Carrier {
        /// The directory of the carrier copy.
        directory: PathBuf,
    },
    /// Run the MCP connector on standard input and output.
    Mcp,
    /// Run the browser connector for the extension at `origin`. The browser may pass more
    /// arguments after it.
    Browser {
        /// The first argument, `chrome-extension://<id>/`.
        origin: String,
    },
    /// Run the service image in `<home>/bin` with the same arguments.
    Service,
}

/// Decide what an invocation of the edge does.
///
/// `this_exe` is the running edge and `bin_dir` is `<home>/bin`, both already canonical.
/// `edge_file_name` is the edge's installed file name; it compares ASCII case-insensitively on
/// Windows and exactly elsewhere. `arguments` are the arguments after the program name.
pub fn route(
    this_exe: &Path,
    bin_dir: &Path,
    edge_file_name: &OsStr,
    arguments: &[OsString],
) -> Route {
    let directory = this_exe.parent();
    let installed = directory == Some(bin_dir)
        && this_exe
            .file_name()
            .is_some_and(|name| same_file_name(name, edge_file_name));
    if !installed {
        return Route::Carrier {
            directory: directory.map(Path::to_path_buf).unwrap_or_default(),
        };
    }
    match arguments.first() {
        Some(first) if first == MCP_ARGUMENT => Route::Mcp,
        Some(first) if first.to_str().is_some_and(is_browser_origin) => Route::Browser {
            origin: first.to_string_lossy().into_owned(),
        },
        _ => Route::Service,
    }
}

/// Whether `argument` is a Chrome extension origin.
fn is_browser_origin(argument: &str) -> bool {
    argument.starts_with(BROWSER_ORIGIN_PREFIX)
}

/// Whether two file names are the same file name on this platform.
fn same_file_name(left: &OsStr, right: &OsStr) -> bool {
    if cfg!(windows) {
        left.eq_ignore_ascii_case(right)
    } else {
        left == right
    }
}

#[cfg(test)]
mod tests;
