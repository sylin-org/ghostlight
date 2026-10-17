//! The audited FFI boundary for Windows process, shell and file-security calls.
//!
//! This is one of the two audited `unsafe` surfaces in the workspace; the other is the relay's
//! `os` module. The workspace sets `unsafe_code = "forbid"` through the shared lints table, which
//! Cargo applies as command-line flags no in-source allow can override; this crate's manifest
//! deliberately does not inherit that table, so its `unsafe` is reviewed as a unit, and a
//! repository guard test fails if `unsafe` appears anywhere else. The owner decision recorded in
//! ADR-0105 chose this shape over wrapping the same calls behind a new third-party dependency on
//! a security-sensitive path: every foreign function is declared by hand below against system
//! link libraries, with a `// SAFETY:` note at each call site.
//!
//! The crate holds the executable paths of a process's ancestors in `process` (from which the
//! installer learns whether it runs inside an app's container), whether the process token is
//! elevated in `elevation` (the service refuses to run elevated), owner-private file
//! creation in `private_file` (ADR-0160), the shell's item identifier list of a file in
//! `item_id_list` (named by the installer's Start-menu shortcut), whether the process sits in the
//! visible window station in `window_station` (the service starts the shell only where a person
//! could see it), announcing a change of the user's environment variables to running programs
//! in `settings_change`, a program's own icon read from its icon resource in `program_icon`, and
//! the install folders of a Store package family in `package` (the window shows each agent's
//! app icon). Starting the service, process identity and the session listener live in
//! the relay's audited `os` module (ADR-0199 Decisions 3 and 14). Non-Windows targets compile the
//! portable surface returning an empty answer.

#[cfg(target_os = "windows")]
mod elevation;
#[cfg(target_os = "windows")]
mod item_id_list;
#[cfg(target_os = "windows")]
mod package;
#[cfg(target_os = "windows")]
mod private_file;
#[cfg(target_os = "windows")]
mod process;
#[cfg(target_os = "windows")]
mod program_icon;
#[cfg(test)]
mod scratch;
#[cfg(target_os = "windows")]
mod settings_change;
#[cfg(target_os = "windows")]
mod window_station;

/// Executable paths of this process's ancestors, nearest parent first, walking at most `limit`
/// parents. Empty where the platform cannot answer.
///
/// The paths decide where this process runs (the installer asks whether an ancestor is a packaged
/// app). They are never logged or audited.
#[must_use]
pub fn ancestor_images(limit: usize) -> Vec<std::path::PathBuf> {
    #[cfg(target_os = "windows")]
    return process::ancestor_images(limit);
    #[cfg(not(target_os = "windows"))]
    {
        let _ = limit;
        Vec::new()
    }
}

/// Create a new file with a protected current-user/SYSTEM DACL before it can contain secrets.
/// Existing paths are refused. Runtime publication belongs to the caller (ADR-0160).
#[cfg(target_os = "windows")]
pub fn create_private_file(path: &std::path::Path) -> std::io::Result<std::fs::File> {
    private_file::create(path)
}

/// Whether this process runs with an elevated token. `false` where the platform has no such
/// notion.
pub fn is_elevated() -> std::io::Result<bool> {
    #[cfg(target_os = "windows")]
    return elevation::is_elevated();
    #[cfg(not(target_os = "windows"))]
    Ok(false)
}

/// The shell's serialized item identifier list for the existing file at `path`, terminator
/// included: what a shell link stores to name its target.
#[cfg(target_os = "windows")]
pub fn item_id_list(path: &std::path::Path) -> std::io::Result<Vec<u8>> {
    item_id_list::of_file(path)
}

/// Tell the programs running in this session that the user's environment variables changed, so
/// Explorer and the terminals opened afterwards read the new `Path`. A program that is hung or
/// slow is skipped after a bounded wait, and every failure is ignored: the change itself is
/// already stored. Programs that already read their environment keep what they read.
#[cfg(target_os = "windows")]
pub fn broadcast_environment_change() {
    settings_change::environment_changed();
}

/// The icon of the program at `path` as the bytes of a `.ico` file of at most `max_bytes`: the
/// largest images of the program's first icon group, read with the program mapped as a data file,
/// so none of its code runs. `Ok(None)` when the program has no icon resource, no image fits, or
/// the platform has no icon resources; an error when the file cannot be mapped as a program.
pub fn program_icon(path: &std::path::Path, max_bytes: usize) -> std::io::Result<Option<Vec<u8>>> {
    #[cfg(target_os = "windows")]
    return program_icon::of_program(path, max_bytes);
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (path, max_bytes);
        Ok(None)
    }
}

/// The install folders of the Store (MSIX) packages of the package family `family`
/// (`<name>_<publisher id>`) installed for this user, as Windows's app model reports them. Empty
/// when none is installed, when Windows cannot say, or where the platform has no packages.
#[must_use]
pub fn package_install_folders(family: &str) -> Vec<std::path::PathBuf> {
    #[cfg(target_os = "windows")]
    return package::install_folders(family);
    #[cfg(not(target_os = "windows"))]
    {
        let _ = family;
        Vec::new()
    }
}

/// Whether a window this process opens could be seen by a person: the process is in the visible,
/// interactive window station. False in session 0 and over an OpenSSH session, where a window
/// would be invisible. Always true where the platform has no such notion; Linux asks its
/// display variables instead.
#[must_use]
pub fn interactive_window_station() -> bool {
    #[cfg(target_os = "windows")]
    return window_station::is_visible();
    #[cfg(not(target_os = "windows"))]
    true
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    /// An ordinary test run happens in a person's desktop session, whose station is visible.
    ///
    /// A run with no desktop (a build service in session 0) reports false. The test then skips
    /// with a note when the `CI` variable is set, which the project's CI sets, and fails
    /// otherwise, so a developer's machine that reports false is looked at.
    #[test]
    fn a_desktop_session_has_the_visible_window_station() {
        if super::interactive_window_station() {
            return;
        }
        assert!(
            std::env::var_os("CI").is_some(),
            "this run has no visible window station and is not CI"
        );
        eprintln!("skipped: CI runs this test without a desktop session");
    }
}
