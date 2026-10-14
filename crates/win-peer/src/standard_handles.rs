//! The current process's standard handles, made non-inheritable (ADR-0197).
//!
//! Rust's Windows `Command::spawn` calls `CreateProcessW` with handle inheritance on, so a child
//! inherits every inheritable handle of its parent. `Stdio::null` only replaces the child's
//! three standard handles; the parent's own standard handles, which are often the pipe ends of
//! whoever captures its output, are inherited all the same. A detached service started that way
//! holds its starter's pipes open until it exits, and the starter never sees end of output.
//!
//! Clearing the inherit flag on the originals closes that leak. It does not break forwarding:
//! `Stdio::inherit` duplicates the standard handle into a fresh inheritable copy for each child
//! instead of passing the original through, and the test below proves it.

use std::ffi::c_void;

type Handle = *mut c_void;

const STD_INPUT_HANDLE: u32 = -10_i32 as u32;
const STD_OUTPUT_HANDLE: u32 = -11_i32 as u32;
const STD_ERROR_HANDLE: u32 = -12_i32 as u32;
const HANDLE_FLAG_INHERIT: u32 = 0x0000_0001;

/// Every standard handle, in the order input, output, error.
const STANDARD_HANDLES: [u32; 3] = [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE];

#[link(name = "kernel32")]
extern "system" {
    fn GetStdHandle(standard_handle: u32) -> Handle;
    fn SetHandleInformation(handle: Handle, mask: u32, flags: u32) -> i32;
}

/// Clear the inherit flag on this process's standard input, output and error handles.
///
/// A standard handle that does not exist (null) or is invalid is skipped. A handle the system
/// refuses to change, such as a legacy console pseudo-handle, is left as it is: it cannot be
/// inherited that way either.
pub(super) fn clear_inheritance() {
    for which in STANDARD_HANDLES {
        // SAFETY: GetStdHandle takes a numeric selector and returns a handle owned by the
        // process; it borrows nothing.
        let handle = unsafe { GetStdHandle(which) };
        if handle.is_null() || handle as isize == -1 {
            continue;
        }
        // SAFETY: handle is the process's own standard handle, not closed by this call. Only the
        // inherit bit is selected by the mask, and it is cleared.
        unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0) };
    }
}

#[cfg(test)]
mod tests {
    use std::fs::OpenOptions;
    use std::os::windows::io::AsRawHandle;
    use std::process::{Command, Stdio};

    use super::*;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetHandleInformation(handle: Handle, flags: *mut u32) -> i32;
        fn SetStdHandle(standard_handle: u32, handle: Handle) -> i32;
    }

    fn inherit_flag(handle: Handle) -> Option<bool> {
        let mut flags = 0_u32;
        // SAFETY: flags is writable for the call and handle is a live handle of this process.
        let ok = unsafe { GetHandleInformation(handle, &mut flags) };
        (ok != 0).then_some(flags & HANDLE_FLAG_INHERIT != 0)
    }

    fn standard(which: u32) -> Handle {
        // SAFETY: GetStdHandle borrows nothing.
        unsafe { GetStdHandle(which) }
    }

    /// One test, because it swaps this process's standard error handle and a second test
    /// reading the standard handles would race it.
    #[test]
    fn clearing_removes_inheritance_and_inherited_stdio_still_reaches_a_child() {
        // Every standard handle that exists reports the flag cleared afterwards.
        clear_inheritance();
        for which in STANDARD_HANDLES {
            let handle = standard(which);
            if handle.is_null() || handle as isize == -1 {
                continue;
            }
            if let Some(inheritable) = inherit_flag(handle) {
                assert!(
                    !inheritable,
                    "standard handle {which:#x} is still inheritable"
                );
            }
        }

        // Point standard error at a file whose handle is inheritable, then clear it.
        let path = std::env::temp_dir().join(format!("ghostlight-std-{}.txt", std::process::id()));
        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .read(true)
            .open(&path)
            .unwrap();
        let original = standard(STD_ERROR_HANDLE);
        // SAFETY: file outlives the swap and the original handle is put back below.
        unsafe {
            assert_ne!(SetStdHandle(STD_ERROR_HANDLE, file.as_raw_handle()), 0);
            SetHandleInformation(
                file.as_raw_handle(),
                HANDLE_FLAG_INHERIT,
                HANDLE_FLAG_INHERIT,
            );
        }
        assert_eq!(inherit_flag(file.as_raw_handle()), Some(true));
        clear_inheritance();
        assert_eq!(inherit_flag(file.as_raw_handle()), Some(false));

        // A child with inherited stdio still writes to that file: the standard library hands it
        // a fresh inheritable duplicate, not the cleared original.
        let status = Command::new("cmd")
            .args(["/c", "echo forwarded 1>&2"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .status();
        // SAFETY: restores the handle that was current before the swap.
        unsafe { SetStdHandle(STD_ERROR_HANDLE, original) };
        assert!(status.unwrap().success());
        let written = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(written.contains("forwarded"), "{written:?}");
    }
}
