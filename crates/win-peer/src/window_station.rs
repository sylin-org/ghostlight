//! Whether this process can show a window to a person, through the audited FFI boundary.
//!
//! A process belongs to one window station. Only the interactive station, `WinSta0`, is visible:
//! a service in session 0 and a command run over an OpenSSH session sit in a station no person
//! looks at, so a window opened there is invisible. The service asks before it starts the shell
//! (ADR-0199 Decision 11).

use std::ffi::c_void;
use std::mem::size_of;

type Handle = *mut c_void;

/// `GetUserObjectInformationW` index for the object's flags (`USEROBJECTFLAGS`).
const UOI_FLAGS: i32 = 1;

/// The window-station flag that marks the interactive, visible station.
const WSF_VISIBLE: u32 = 0x0001;

/// The `USEROBJECTFLAGS` structure `GetUserObjectInformationW` fills for `UOI_FLAGS`.
#[repr(C)]
struct UserObjectFlags {
    inherit: i32,
    reserved: i32,
    flags: u32,
}

#[link(name = "user32")]
extern "system" {
    fn GetProcessWindowStation() -> Handle;
    fn GetUserObjectInformationW(
        object: Handle,
        index: i32,
        information: *mut c_void,
        length: u32,
        needed: *mut u32,
    ) -> i32;
}

/// Whether the window station of this process is visible. A station that cannot be read counts as
/// not visible.
pub(crate) fn is_visible() -> bool {
    // SAFETY: GetProcessWindowStation takes no arguments. The handle it returns belongs to the
    // process and is not closed by the caller; it is only passed to the call below.
    let station = unsafe { GetProcessWindowStation() };
    if station.is_null() {
        return false;
    }
    let mut information = UserObjectFlags {
        inherit: 0,
        reserved: 0,
        flags: 0,
    };
    let mut needed = 0_u32;
    // SAFETY: `station` is the process window station and `information` is a `UserObjectFlags`
    // of exactly the length passed, which is the layout UOI_FLAGS writes.
    let read = unsafe {
        GetUserObjectInformationW(
            station,
            UOI_FLAGS,
            (&mut information as *mut UserObjectFlags).cast(),
            size_of::<UserObjectFlags>() as u32,
            &mut needed,
        )
    };
    read != 0 && information.flags & WSF_VISIBLE != 0
}
