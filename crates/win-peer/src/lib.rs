//! The audited FFI boundary for Windows process, shell and file-security calls.
//!
//! This is the one audited FFI crate in the workspace. The workspace sets
//! `unsafe_code = "forbid"` through the shared lints table, which Cargo applies as command-line
//! flags no in-source allow can override; this crate's manifest deliberately does not inherit
//! that table, so every line of `unsafe` in Ghostlight lives here, is reviewed as a unit, and a
//! repository guard test fails if `unsafe` appears anywhere else. The owner decision recorded in
//! ADR-0105 chose this shape over wrapping the same calls behind a new third-party dependency on
//! a security-sensitive path: every foreign function is declared by hand below against system
//! link libraries, with a `// SAFETY:` note at each call site.
//!
//! The crate holds process identity and liveness in `process` (including the executable paths of a
//! process's ancestors, from which the installer learns whether it runs inside an app's
//! container), owner-private runtime-file creation in `private_file` (ADR-0160), the shell's item
//! identifier list of a file in `item_id_list` (named by the installer's Start-menu shortcut),
//! `clear_standard_handle_inheritance` in `standard_handles` (which keeps a detached child from
//! holding its starter's standard pipes open), and session shutdown events in `shutdown`.
//! Non-Windows targets compile the portable surface returning `None` or an empty answer.

/// Stable identity for one Windows process across process-id reuse.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessIdentity {
    /// Operating-system process id.
    pub process_id: u32,
    /// Windows process creation time, expressed as the raw FILETIME value.
    pub created: u64,
}

#[cfg(target_os = "windows")]
mod item_id_list;
#[cfg(target_os = "windows")]
mod private_file;
#[cfg(target_os = "windows")]
mod process;
#[cfg(target_os = "windows")]
mod shutdown;
#[cfg(target_os = "windows")]
mod standard_handles;

/// Session shutdown events from the operating system.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShutdownEvent {
    /// System is querying whether session can end (WM_QUERYENDSESSION).
    Query,
    /// System cancelled the shutdown sequence (WM_ENDSESSION with wParam == FALSE).
    Cancelled,
    /// System is terminating (WM_ENDSESSION with wParam != FALSE or console shutdown).
    Terminating,
}

#[cfg(target_os = "windows")]
pub use shutdown::{listen_for_system_shutdown, SystemShutdownListener};

#[cfg(not(target_os = "windows"))]
pub struct SystemShutdownListener;

#[cfg(not(target_os = "windows"))]
pub fn listen_for_system_shutdown(
    _callback: Box<dyn Fn(ShutdownEvent) + Send + Sync + 'static>,
) -> std::io::Result<SystemShutdownListener> {
    Ok(SystemShutdownListener)
}

/// Capture the current process's parent with creation time bound to its process id.
#[must_use]
pub fn parent_process() -> Option<ProcessIdentity> {
    #[cfg(target_os = "windows")]
    return process::parent();
    #[cfg(not(target_os = "windows"))]
    None
}

/// Capture one process with creation time bound to its process id.
#[must_use]
pub fn process_identity(process_id: u32) -> Option<ProcessIdentity> {
    #[cfg(target_os = "windows")]
    return process::identity(process_id);
    #[cfg(not(target_os = "windows"))]
    {
        let _ = process_id;
        None
    }
}

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

/// Return whether the exact captured Windows process is still running.
#[must_use]
pub fn process_is_alive(process: ProcessIdentity) -> bool {
    #[cfg(target_os = "windows")]
    return process::is_alive(process);
    #[cfg(not(target_os = "windows"))]
    {
        let _ = process;
        false
    }
}

/// Create a new file with a protected current-user/SYSTEM DACL before it can contain secrets.
/// Existing paths are refused. Runtime publication belongs to the caller (ADR-0160).
#[cfg(target_os = "windows")]
pub fn create_private_file(path: &std::path::Path) -> std::io::Result<std::fs::File> {
    private_file::create(path)
}

/// Clear the inherit flag on this process's standard input, output and error handles, so a child
/// created afterwards does not inherit them (for a detached service, its starter's pipes).
/// `Stdio::inherit` still forwards them: it duplicates a fresh inheritable copy for each child.
#[cfg(target_os = "windows")]
pub fn clear_standard_handle_inheritance() {
    standard_handles::clear_inheritance();
}

/// The shell's serialized item identifier list for the existing file at `path`, terminator
/// included: what a shell link stores to name its target.
#[cfg(target_os = "windows")]
pub fn item_id_list(path: &std::path::Path) -> std::io::Result<Vec<u8>> {
    item_id_list::of_file(path)
}
