//! Peer executable image lookup through `OpenProcess` and `QueryFullProcessImageNameW`.

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;

/// `PROCESS_QUERY_LIMITED_INFORMATION`.
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(
        desired_access: u32,
        inherit_handle: i32,
        process_id: u32,
    ) -> *mut core::ffi::c_void;
    fn CloseHandle(handle: *mut core::ffi::c_void) -> i32;
    fn QueryFullProcessImageNameW(
        process: *mut core::ffi::c_void,
        flags: u32,
        exe_name: *mut u16,
        size_pointer: &mut u32,
    ) -> i32;
}

/// The peer image's full path, for signature verification only. Never logged, never audited.
pub(super) fn full_image_path(process_id: u32) -> Option<PathBuf> {
    // SAFETY: OpenProcess with only query-limited access takes no ownership of anything and the
    // handle is closed on every path below.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
    if process.is_null() {
        return None;
    }
    let mut wide = [0u16; 1024];
    let mut size = wide.len() as u32;
    // SAFETY: process is a valid handle from the call above; wide outlives the call and size
    // names its exact length in UTF-16 units.
    let ok = unsafe { QueryFullProcessImageNameW(process, 0, wide.as_mut_ptr(), &mut size) };
    // SAFETY: the handle came from OpenProcess and is closed exactly once here.
    unsafe { CloseHandle(process) };
    if ok == 0 || size == 0 || size as usize > wide.len() {
        return None;
    }
    Some(PathBuf::from(OsString::from_wide(&wide[..size as usize])))
}

/// Bounded lowercase file name of an image path (ADR-0105 Decision 2: the name only).
pub(super) fn bounded_name(path: &std::path::Path) -> Option<String> {
    let raw = path.file_name()?.to_str()?;
    let mut bounded: String = raw.chars().take(120).collect();
    bounded.make_ascii_lowercase();
    (!bounded.is_empty()).then_some(bounded)
}
