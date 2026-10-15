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
//! elevated in `elevation` (the service refuses to run elevated), owner-private runtime-file
//! creation in `private_file` (ADR-0160), the shell's item identifier list of a file in
//! `item_id_list` (named by the installer's Start-menu shortcut), and joining the parent
//! process's console in `console`. Starting the service, process identity and the session
//! listener live in the relay's audited `os` module (ADR-0199 Decisions
//! 3 and 14). Non-Windows targets compile the portable surface returning an empty answer.

#[cfg(target_os = "windows")]
mod console;
#[cfg(target_os = "windows")]
mod elevation;
#[cfg(target_os = "windows")]
mod item_id_list;
#[cfg(target_os = "windows")]
mod private_file;
#[cfg(target_os = "windows")]
mod process;

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

/// Attach this process to the console of the process that started it, so a typed command run
/// from a terminal writes there and receives Ctrl-C itself. A failure is ignored: the parent has
/// no console, or this process already has one. Does nothing off Windows.
///
/// ADR-0199 Decision 4: the service image is a GUI-subsystem program and so owns no console.
/// The desktop shell track deletes this call when the image becomes console subsystem.
pub fn attach_parent_console() {
    #[cfg(target_os = "windows")]
    console::attach_parent();
}
