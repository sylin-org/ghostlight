//! Whether this process runs with an elevated token, through the audited FFI boundary.

use std::ffi::c_void;
use std::io;
use std::mem::size_of;
use std::ptr::null_mut;

type Handle = *mut c_void;
const TOKEN_QUERY: u32 = 0x0008;
const TOKEN_ELEVATION_CLASS: u32 = 20;

#[repr(C)]
struct TokenElevation {
    token_is_elevated: u32,
}

#[link(name = "advapi32")]
extern "system" {
    fn OpenProcessToken(process: Handle, access: u32, token: *mut Handle) -> i32;
    fn GetTokenInformation(
        token: Handle,
        class: u32,
        output: *mut c_void,
        length: u32,
        needed: *mut u32,
    ) -> i32;
}
#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcess() -> Handle;
    fn CloseHandle(handle: Handle) -> i32;
}

/// Closes the token handle it owns.
struct Token(Handle);

impl Drop for Token {
    fn drop(&mut self) {
        // SAFETY: this guard exclusively owns a valid token handle and closes it once.
        unsafe { CloseHandle(self.0) };
    }
}

/// Whether the current process token is elevated (`TokenElevation`).
pub(crate) fn is_elevated() -> io::Result<bool> {
    let mut handle: Handle = null_mut();
    // SAFETY: GetCurrentProcess returns a pseudo-handle that needs no closing, and `handle` is a
    // valid out-pointer for the token handle.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut handle) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let token = Token(handle);
    let mut elevation = TokenElevation {
        token_is_elevated: 0,
    };
    let mut needed = 0_u32;
    // SAFETY: the token is open for query, and the output buffer is a `TokenElevation` of exactly
    // the length passed.
    let read = unsafe {
        GetTokenInformation(
            token.0,
            TOKEN_ELEVATION_CLASS,
            (&mut elevation as *mut TokenElevation).cast(),
            size_of::<TokenElevation>() as u32,
            &mut needed,
        )
    };
    if read == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(elevation.token_is_elevated != 0)
}
