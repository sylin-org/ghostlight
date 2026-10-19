//! Owner-private file creation: a new file whose protected access list admits only this user and
//! SYSTEM from the moment it exists, so nothing written to it is ever open to anyone else.

use std::ffi::c_void;
use std::fs::File;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::FromRawHandle;
use std::path::Path;
use std::ptr::null_mut;

use crate::token::{LocalMemory, ProcessToken};

type Handle = *mut c_void;
const SDDL_REVISION: u32 = 1;
const GENERIC_WRITE: u32 = 0x4000_0000;
const FILE_SHARE_READ_DELETE: u32 = 0x0000_0005;
const CREATE_NEW: u32 = 1;
const FILE_ATTRIBUTE_NORMAL: u32 = 0x0000_0080;

#[repr(C)]
struct SecurityAttributes {
    length: u32,
    descriptor: *mut c_void,
    inherit: i32,
}

#[link(name = "advapi32")]
extern "system" {
    fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
        text: *const u16,
        revision: u32,
        output: *mut *mut c_void,
        length: *mut u32,
    ) -> i32;
}
#[link(name = "kernel32")]
extern "system" {
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

/// Create the new file `path`, writable by this process and closed to everyone but this user and
/// SYSTEM. An existing path, a link included, is refused.
pub(super) fn create(path: &Path) -> io::Result<File> {
    let sid = ProcessToken::current()?.user_sid()?;
    // Protected DACL: only this user and SYSTEM. Never inherit a broad parent-directory grant.
    let descriptor: Vec<u16> = format!("O:{sid}D:P(A;;FA;;;{sid})(A;;FA;;;SY)")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut raw_descriptor = null_mut();
    // SAFETY: descriptor is NUL-terminated; the conversion owns and initializes the returned block.
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            descriptor.as_ptr(),
            SDDL_REVISION,
            &mut raw_descriptor,
            null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let _descriptor = LocalMemory(raw_descriptor);
    let security = SecurityAttributes {
        length: std::mem::size_of::<SecurityAttributes>() as u32,
        descriptor: raw_descriptor,
        inherit: 0,
    };
    let mut name: Vec<u16> = path.as_os_str().encode_wide().collect();
    if name.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "NUL in file path",
        ));
    }
    name.push(0);
    // SAFETY: both inputs remain live through CreateFileW. CREATE_NEW refuses existing files and
    // links; the descriptor is applied at creation, before anything is written to the file.
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            GENERIC_WRITE,
            FILE_SHARE_READ_DELETE,
            &security,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL,
            null_mut(),
        )
    };
    if handle == (-1_isize) as Handle {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: CreateFileW returned a newly owned valid handle; File becomes its sole closer.
    Ok(unsafe { File::from_raw_handle(handle) })
}
