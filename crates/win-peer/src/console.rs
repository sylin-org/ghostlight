//! Joining the console of the parent process, through the audited FFI boundary.
//!
//! A GUI-subsystem image run by the console edge receives the edge's console handles as its
//! standard streams. After the image attaches to that console, those inherited handles still do
//! not work in it: writes through them vanish (seen live on Windows 11 build 26300). So once the
//! image has attached, every standard stream that is not a working console handle and not a
//! redirection (a pipe or a file) is pointed at the attached console's own `CONOUT$` or `CONIN$`.
//! A redirected stream is left alone, so `ghostlight doctor > report.txt` and scripted callers
//! keep their output.

use core::ffi::c_void;

use crate::private_file::SecurityAttributes;

type Handle = *mut c_void;

#[link(name = "kernel32")]
extern "system" {
    fn AttachConsole(process_id: u32) -> i32;
    fn GetStdHandle(which: u32) -> Handle;
    fn SetStdHandle(which: u32, handle: Handle) -> i32;
    fn GetFileType(handle: Handle) -> u32;
    fn GetConsoleMode(handle: Handle, mode: *mut u32) -> i32;
    fn CreateFileW(
        path: *const u16,
        access: u32,
        share: u32,
        security: *const SecurityAttributes,
        creation: u32,
        flags: u32,
        template: Handle,
    ) -> Handle;
}

/// `ATTACH_PARENT_PROCESS`: the console of the process that started this one.
const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
/// `STD_INPUT_HANDLE` (-10).
const STD_INPUT_HANDLE: u32 = -10i32 as u32;
/// `STD_OUTPUT_HANDLE` (-11).
const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
/// `STD_ERROR_HANDLE` (-12).
const STD_ERROR_HANDLE: u32 = -12i32 as u32;
/// `FILE_TYPE_DISK`: a file.
const FILE_TYPE_DISK: u32 = 1;
/// `FILE_TYPE_CHAR`: a character device, such as a console or `NUL`.
const FILE_TYPE_CHAR: u32 = 2;
/// `FILE_TYPE_PIPE`: a pipe.
const FILE_TYPE_PIPE: u32 = 3;
/// `GENERIC_READ | GENERIC_WRITE`.
const READ_WRITE: u32 = 0x8000_0000 | 0x4000_0000;
/// `FILE_SHARE_READ | FILE_SHARE_WRITE`.
const SHARE_READ_WRITE: u32 = 0x1 | 0x2;
/// `OPEN_EXISTING`.
const OPEN_EXISTING: u32 = 3;
/// The attached console's output device.
const CONSOLE_OUTPUT: &str = "CONOUT$";
/// The attached console's input device.
const CONSOLE_INPUT: &str = "CONIN$";

/// What a standard stream is, as far as the rebind decision needs to know.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Stream {
    /// A pipe or a file: a redirection the caller chose.
    Redirected,
    /// A console handle that works in this process.
    WorkingConsole,
    /// A character handle that is not a working console here (an inherited console handle that
    /// no longer works after attaching), an unknown type, or no handle at all.
    Unusable,
}

/// Whether a standard stream is pointed at the attached console.
fn needs_console(stream: Stream) -> bool {
    stream == Stream::Unusable
}

/// Classify a standard stream from its file type and whether `GetConsoleMode` accepts it.
fn classify(file_type: Option<u32>, console_mode_works: bool) -> Stream {
    match file_type {
        Some(FILE_TYPE_PIPE | FILE_TYPE_DISK) => Stream::Redirected,
        Some(FILE_TYPE_CHAR) if console_mode_works => Stream::WorkingConsole,
        _ => Stream::Unusable,
    }
}

/// Attach to the parent process's console and make the standard streams reach it. Every failure
/// is ignored: a parent with no console, a console this process already has, or a device that
/// cannot be opened all leave the process as it was.
pub(crate) fn attach_parent() {
    // SAFETY: AttachConsole takes no pointers. A parent with no console, or a console this
    // process already has, makes it return zero, which leaves the process as it was.
    let attached = unsafe { AttachConsole(ATTACH_PARENT_PROCESS) } != 0;
    if !attached {
        return;
    }
    rebind(STD_OUTPUT_HANDLE, CONSOLE_OUTPUT);
    rebind(STD_ERROR_HANDLE, CONSOLE_OUTPUT);
    rebind(STD_INPUT_HANDLE, CONSOLE_INPUT);
}

/// Point the standard stream `which` at `device` when it is not a working console or a
/// redirection.
fn rebind(which: u32, device: &str) {
    // SAFETY: GetStdHandle takes no pointers and returns a handle value or null or
    // INVALID_HANDLE_VALUE; every one of those is only compared or passed back to the system.
    let handle = unsafe { GetStdHandle(which) };
    let present = !handle.is_null() && handle as isize != -1;
    let file_type = if present {
        // SAFETY: GetFileType reads no memory of ours; an unusable handle yields
        // FILE_TYPE_UNKNOWN, which `classify` treats as unusable.
        Some(unsafe { GetFileType(handle) })
    } else {
        None
    };
    let mut mode = 0u32;
    // SAFETY: `mode` is a live u32 owned by this frame for the duration of the call; a handle
    // that is not a console makes the call fail and leaves `mode` untouched.
    let console_mode_works = present && unsafe { GetConsoleMode(handle, &mut mode) } != 0;
    if !needs_console(classify(file_type, console_mode_works)) {
        return;
    }
    let name: Vec<u16> = device.encode_utf16().chain(Some(0)).collect();
    // SAFETY: `name` is a NUL-terminated UTF-16 string that outlives the call; the security and
    // template arguments are null, which CreateFileW accepts. The result is checked below.
    let console = unsafe {
        CreateFileW(
            name.as_ptr(),
            READ_WRITE,
            SHARE_READ_WRITE,
            core::ptr::null(),
            OPEN_EXISTING,
            0,
            core::ptr::null_mut(),
        )
    };
    if console.is_null() || console as isize == -1 {
        return;
    }
    // SAFETY: `console` is a handle this process just opened; SetStdHandle stores it as the
    // standard stream and takes ownership of nothing else. A failure leaves the old stream.
    let _ = unsafe { SetStdHandle(which, console) };
}

#[cfg(test)]
mod tests {
    use super::{classify, needs_console, Stream, FILE_TYPE_CHAR, FILE_TYPE_DISK, FILE_TYPE_PIPE};

    #[test]
    fn a_redirection_is_kept_and_a_working_console_is_kept() {
        assert_eq!(classify(Some(FILE_TYPE_PIPE), false), Stream::Redirected);
        assert_eq!(classify(Some(FILE_TYPE_DISK), false), Stream::Redirected);
        assert_eq!(classify(Some(FILE_TYPE_CHAR), true), Stream::WorkingConsole);
        assert!(!needs_console(Stream::Redirected));
        assert!(!needs_console(Stream::WorkingConsole));
    }

    #[test]
    fn an_inherited_console_that_does_not_work_or_a_missing_stream_reaches_the_console() {
        assert_eq!(classify(Some(FILE_TYPE_CHAR), false), Stream::Unusable);
        assert_eq!(classify(Some(0), false), Stream::Unusable);
        assert_eq!(classify(None, false), Stream::Unusable);
        assert!(needs_console(Stream::Unusable));
    }
}
