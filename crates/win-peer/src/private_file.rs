//! Atomic owner-private runtime-file creation through the existing audited Win32 FFI boundary.

use std::ffi::c_void;
use std::fs::File;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::FromRawHandle;
use std::path::Path;
use std::ptr::null_mut;

type Handle = *mut c_void;
const TOKEN_QUERY: u32 = 0x0008;
const TOKEN_USER: u32 = 1;
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
    fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
        text: *const u16,
        revision: u32,
        output: *mut *mut c_void,
        length: *mut u32,
    ) -> i32;
}
#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcess() -> Handle;
    fn CloseHandle(handle: Handle) -> i32;
    fn LocalFree(memory: *mut c_void) -> *mut c_void;
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

struct Token(Handle);
impl Drop for Token {
    fn drop(&mut self) {
        // SAFETY: this guard owns the non-null handle returned by OpenProcessToken.
        unsafe {
            CloseHandle(self.0);
        }
    }
}
struct LocalMemory(*mut c_void);
impl Drop for LocalMemory {
    fn drop(&mut self) {
        // SAFETY: conversion APIs allocate with LocalAlloc and transfer ownership to the caller.
        unsafe {
            LocalFree(self.0);
        }
    }
}

fn user_sid() -> io::Result<String> {
    let mut raw_token = null_mut();
    // SAFETY: current-process pseudo-handle is valid; raw_token points to writable handle storage.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw_token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let token = Token(raw_token);
    let mut needed = 0;
    // SAFETY: null/zero output is the documented size query; needed is writable.
    unsafe {
        GetTokenInformation(token.0, TOKEN_USER, null_mut(), 0, &mut needed);
    }
    if needed < std::mem::size_of::<TokenUser>() as u32 || needed > 64 * 1024 {
        return Err(io::Error::other("invalid token-user size"));
    }
    // usize storage supplies alignment for TOKEN_USER and its embedded pointer.
    let mut buffer = vec![0_usize; (needed as usize).div_ceil(std::mem::size_of::<usize>())];
    // SAFETY: aligned storage covers needed bytes and remains alive through SID conversion.
    if unsafe {
        GetTokenInformation(
            token.0,
            TOKEN_USER,
            buffer.as_mut_ptr().cast(),
            needed,
            &mut needed,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: a successful TOKEN_USER query initialized the leading structure in aligned storage.
    let user = unsafe { &*buffer.as_ptr().cast::<TokenUser>() };
    let mut text = null_mut();
    // SAFETY: user.sid refers to the SID returned in the live token buffer; text is writable.
    if unsafe { ConvertSidToStringSidW(user.sid, &mut text) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let _memory = LocalMemory(text.cast());
    let mut length = 0;
    // SAFETY: successful conversion returns a NUL-terminated UTF-16 SID string, owned above.
    unsafe {
        while *text.add(length) != 0 {
            length += 1;
        }
        String::from_utf16(std::slice::from_raw_parts(text, length))
            .map_err(|_| io::Error::other("invalid user SID"))
    }
}

pub(super) fn create(path: &Path) -> io::Result<File> {
    let sid = user_sid()?;
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
            "NUL in runtime path",
        ));
    }
    name.push(0);
    // SAFETY: both inputs remain live through CreateFileW. CREATE_NEW refuses existing files and
    // links; the descriptor is applied at creation, before any authentication token is written.
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
