//! The process's explicit AppUserModelID (ADR-0199 Decisions 3, 4 and 7).
//!
//! Windows groups a window with a pinned shortcut, and shows a toast from an unpackaged
//! application, only when the process's explicit id equals the id on its Start-menu shortcut.
//! The id itself belongs to the desktop shell; this module only offers the call.

use std::io;

/// Name the calling process's AppUserModelID `id`.
///
/// An id that is empty or holds a NUL is refused with [`io::ErrorKind::InvalidInput`] before any
/// system call. On Windows it calls `SetCurrentProcessExplicitAppUserModelID` and turns a failing
/// result into an error that names it in hex. On Linux the window is identified by its GTK
/// program name, which the shell sets, so the call does nothing.
pub fn set_app_user_model_id(id: &str) -> io::Result<()> {
    if id.is_empty() || id.contains('\0') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "an AppUserModelID is not empty and holds no NUL",
        ));
    }
    set_for_platform(id)
}

#[cfg(windows)]
fn set_for_platform(id: &str) -> io::Result<()> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "shell32")]
    extern "system" {
        fn SetCurrentProcessExplicitAppUserModelID(app_id: *const u16) -> i32;
    }

    let wide: Vec<u16> = OsStr::new(id).encode_wide().chain(Some(0)).collect();
    // SAFETY: `wide` is a NUL-terminated UTF-16 buffer that outlives the call, and the call keeps
    // no pointer to it after it returns.
    let result = unsafe { SetCurrentProcessExplicitAppUserModelID(wide.as_ptr()) };
    if result < 0 {
        return Err(io::Error::other(format!(
            "SetCurrentProcessExplicitAppUserModelID failed with HRESULT {:#010x}",
            result as u32
        )));
    }
    Ok(())
}

#[cfg(not(windows))]
fn set_for_platform(_id: &str) -> io::Result<()> {
    Ok(())
}
