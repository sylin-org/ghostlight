//! This process's own token, read through the audited FFI boundary: whether it is elevated and
//! which user it belongs to, and the text of a SID. Every other module that needs a token fact
//! asks this one.

use std::ffi::c_void;
use std::io;
use std::mem::size_of;
use std::ptr::null_mut;

/// A Windows `HANDLE`.
type Handle = *mut c_void;

/// Access right: query a token.
const TOKEN_QUERY: u32 = 0x0008;

/// `TOKEN_INFORMATION_CLASS` value for the token's user.
const TOKEN_USER_CLASS: u32 = 1;

/// `TOKEN_INFORMATION_CLASS` value for the token's elevation.
const TOKEN_ELEVATION_CLASS: u32 = 20;

/// The largest token information block this module accepts, in bytes.
const MAX_INFORMATION_BYTES: u32 = 64 * 1024;

/// The C `TOKEN_ELEVATION`.
#[repr(C)]
struct TokenElevation {
    token_is_elevated: u32,
}

/// The C `TOKEN_USER`: the user's SID and its attributes.
#[repr(C)]
struct TokenUser {
    sid: *mut c_void,
    attributes: u32,
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
    fn ConvertSidToStringSidW(sid: *const c_void, output: *mut *mut u16) -> i32;
}
#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcess() -> Handle;
    fn CloseHandle(handle: Handle) -> i32;
    fn LocalFree(memory: *mut c_void) -> *mut c_void;
}

/// Memory a security call allocated with `LocalAlloc` and handed to the caller, freed once when
/// the guard drops.
pub(crate) struct LocalMemory(pub(crate) *mut c_void);

impl Drop for LocalMemory {
    fn drop(&mut self) {
        // SAFETY: the guard owns a block a conversion call allocated with LocalAlloc and frees it
        // once.
        unsafe { LocalFree(self.0) };
    }
}

/// The SID at `sid` as a SID string (`S-1-5-21-...`).
///
/// # Safety
///
/// `sid` points at a valid SID that stays live for the call.
pub(crate) unsafe fn sid_text(sid: *const c_void) -> io::Result<String> {
    let mut text = null_mut();
    // SAFETY: the caller guarantees sid is a valid SID, and text is writable.
    if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let _memory = LocalMemory(text.cast());
    let mut length = 0;
    // SAFETY: a successful conversion returns a NUL-terminated UTF-16 string, owned above until
    // this function returns.
    unsafe {
        while *text.add(length) != 0 {
            length += 1;
        }
        String::from_utf16(std::slice::from_raw_parts(text, length))
            .map_err(|_| io::Error::other("invalid SID"))
    }
}

/// This process's token, opened for query and closed once when it drops.
pub(crate) struct ProcessToken(Handle);

impl Drop for ProcessToken {
    fn drop(&mut self) {
        // SAFETY: this guard exclusively owns a valid token handle and closes it once.
        unsafe { CloseHandle(self.0) };
    }
}

impl ProcessToken {
    /// Open this process's token for query.
    pub(crate) fn current() -> io::Result<Self> {
        let mut handle: Handle = null_mut();
        // SAFETY: GetCurrentProcess returns a pseudo-handle that needs no closing, and `handle` is
        // a valid out-pointer for the token handle.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut handle) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(handle))
    }

    /// Whether the token is elevated (`TokenElevation`).
    pub(crate) fn is_elevated(&self) -> io::Result<bool> {
        let buffer = self.query(TOKEN_ELEVATION_CLASS, size_of::<TokenElevation>())?;
        // SAFETY: a successful elevation query wrote a TOKEN_ELEVATION at the start of the aligned
        // buffer, which is at least that long and outlives the borrow.
        let elevation = unsafe { &*buffer.as_ptr().cast::<TokenElevation>() };
        Ok(elevation.token_is_elevated != 0)
    }

    /// The token's user as a SID string (`S-1-5-21-...`).
    pub(crate) fn user_sid(&self) -> io::Result<String> {
        let buffer = self.query(TOKEN_USER_CLASS, size_of::<TokenUser>())?;
        // SAFETY: a successful user query wrote a TOKEN_USER at the start of the aligned buffer,
        // which is at least that long and outlives the borrow.
        let user = unsafe { &*buffer.as_ptr().cast::<TokenUser>() };
        // SAFETY: user.sid points at the SID inside the buffer, which lives until this returns.
        unsafe { sid_text(user.sid) }
    }

    /// Read one token information class of at least `at_least` bytes into storage aligned for
    /// the structure and the pointers it holds.
    fn query(&self, class: u32, at_least: usize) -> io::Result<Vec<usize>> {
        let mut needed = 0_u32;
        // SAFETY: a null buffer of length zero is the documented size query; needed is writable.
        unsafe { GetTokenInformation(self.0, class, null_mut(), 0, &mut needed) };
        if (needed as usize) < at_least || needed > MAX_INFORMATION_BYTES {
            return Err(io::Error::other("invalid token information size"));
        }
        let mut buffer = vec![0_usize; (needed as usize).div_ceil(size_of::<usize>())];
        // SAFETY: the token is open for query, and the aligned buffer covers `needed` bytes.
        let read = unsafe {
            GetTokenInformation(
                self.0,
                class,
                buffer.as_mut_ptr().cast(),
                needed,
                &mut needed,
            )
        };
        if read == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::ProcessToken;

    /// The token answers both questions, and its user is a SID string.
    #[test]
    fn the_process_token_says_whether_it_is_elevated_and_whose_it_is() {
        let token = ProcessToken::current().expect("the process token opens");
        token.is_elevated().expect("the elevation can be read");
        let sid = token.user_sid().expect("the user can be read");
        assert!(sid.starts_with("S-1-"), "{sid} is a SID string");
    }
}
