//! Telling running programs that the user's environment variables changed, through the audited FFI
//! boundary.

use std::ffi::c_void;
use std::ptr::without_provenance_mut;

#[link(name = "user32")]
extern "system" {
    fn SendMessageTimeoutW(
        window: *mut c_void,
        message: u32,
        word_parameter: usize,
        long_parameter: isize,
        flags: u32,
        timeout_milliseconds: u32,
        result: *mut usize,
    ) -> isize;
}

/// `HWND_BROADCAST`: every top-level window.
const HWND_BROADCAST: usize = 0xFFFF;

/// `WM_SETTINGCHANGE`: a system-wide setting changed. With the text `Environment` it means the
/// environment variables did.
const WM_SETTINGCHANGE: u32 = 0x001A;

/// `SMTO_ABORTIFHUNG`: do not wait for a window whose thread is hung.
const SMTO_ABORTIFHUNG: u32 = 0x0002;

/// How long each window may take to answer.
const TIMEOUT_MILLISECONDS: u32 = 5000;

/// The text that says the environment variables changed.
const ENVIRONMENT: &str = "Environment";

/// Broadcast that the user's environment variables changed; every failure is ignored.
pub(crate) fn environment_changed() {
    let text: Vec<u16> = ENVIRONMENT.encode_utf16().chain([0]).collect();
    let mut result = 0_usize;
    // SAFETY: `text` is a NUL-terminated UTF-16 string that outlives the call, and its address is
    // the message's long parameter, which `WM_SETTINGCHANGE` defines as a string. `result` is a
    // valid out-pointer. The call sends to windows owned by other processes and returns after at
    // most the timeout for each; a failure or a timeout makes it return zero, which changes
    // nothing here.
    let _ = unsafe {
        SendMessageTimeoutW(
            without_provenance_mut(HWND_BROADCAST),
            WM_SETTINGCHANGE,
            0,
            text.as_ptr() as isize,
            SMTO_ABORTIFHUNG,
            TIMEOUT_MILLISECONDS,
            &mut result,
        )
    };
}
