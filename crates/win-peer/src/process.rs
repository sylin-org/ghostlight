//! The executable paths of a Windows process's ancestors through the audited FFI boundary.

use std::collections::HashMap;
use std::ffi::OsString;
use std::mem;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;

type Handle = *mut core::ffi::c_void;

const TH32CS_SNAPPROCESS: u32 = 0x0000_0002;
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x0000_1000;
const MAX_PATH: usize = 260;
/// UTF-16 units reserved for one image path query.
const IMAGE_PATH_UNITS: usize = 1024;

#[repr(C)]
struct FileTime {
    low: u32,
    high: u32,
}

#[repr(C)]
struct ProcessEntry32W {
    size: u32,
    usage: u32,
    process_id: u32,
    default_heap_id: usize,
    module_id: u32,
    threads: u32,
    parent_process_id: u32,
    priority_class_base: i32,
    flags: u32,
    executable: [u16; MAX_PATH],
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> Handle;
    fn Process32FirstW(snapshot: Handle, entry: *mut ProcessEntry32W) -> i32;
    fn Process32NextW(snapshot: Handle, entry: *mut ProcessEntry32W) -> i32;
    fn OpenProcess(desired_access: u32, inherit_handle: i32, process_id: u32) -> Handle;
    fn GetProcessTimes(
        process: Handle,
        creation: *mut FileTime,
        exit: *mut FileTime,
        kernel: *mut FileTime,
        user: *mut FileTime,
    ) -> i32;
    fn QueryFullProcessImageNameW(
        process: Handle,
        flags: u32,
        exe_name: *mut u16,
        size_pointer: &mut u32,
    ) -> i32;
    fn CloseHandle(handle: Handle) -> i32;
}

struct OwnedHandle(Handle);

impl OwnedHandle {
    fn process(process_id: u32, access: u32) -> Option<Self> {
        // SAFETY: OpenProcess receives a numeric id and no borrowed pointers. This guard closes
        // every non-null handle exactly once.
        let handle = unsafe { OpenProcess(access, 0, process_id) };
        (!handle.is_null()).then_some(Self(handle))
    }

    fn snapshot() -> Option<Self> {
        // SAFETY: the process snapshot has no borrowed inputs and is closed by this guard.
        let handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        let invalid = (-1_isize) as Handle;
        (handle != invalid).then_some(Self(handle))
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: this guard exclusively owns a valid handle and closes it once.
        unsafe { CloseHandle(self.0) };
    }
}

/// Every running process as `(process id, parent process id)`, from one snapshot.
fn process_table() -> Vec<(u32, u32)> {
    let Some(snapshot) = OwnedHandle::snapshot() else {
        return Vec::new();
    };
    let mut entry = ProcessEntry32W {
        size: mem::size_of::<ProcessEntry32W>() as u32,
        usage: 0,
        process_id: 0,
        default_heap_id: 0,
        module_id: 0,
        threads: 0,
        parent_process_id: 0,
        priority_class_base: 0,
        flags: 0,
        executable: [0; MAX_PATH],
    };
    let mut rows = Vec::new();
    // SAFETY: entry has the required size and remains writable for the complete snapshot walk.
    let mut found = unsafe { Process32FirstW(snapshot.0, &mut entry) } != 0;
    while found {
        rows.push((entry.process_id, entry.parent_process_id));
        // SAFETY: snapshot and entry remain valid for the next row.
        found = unsafe { Process32NextW(snapshot.0, &mut entry) } != 0;
    }
    rows
}

/// Executable paths of this process's ancestors, nearest first, walking at most `limit` parents.
///
/// The walk ends at the first parent that cannot be identified or that started after its child,
/// which is a parent id the system has since reused for an unrelated process. An ancestor whose
/// image cannot be queried is skipped and the walk continues past it.
pub(super) fn ancestor_images(limit: usize) -> Vec<PathBuf> {
    let parents: HashMap<u32, u32> = process_table().into_iter().collect();
    let mut images = Vec::new();
    let mut child = std::process::id();
    for _ in 0..limit {
        let Some(&parent) = parents.get(&child) else {
            break;
        };
        let (Some(child_created), Some(parent_created)) =
            (creation_time(child), creation_time(parent))
        else {
            break;
        };
        if parent_created > child_created {
            break;
        }
        if let Some(image) = full_image_path(parent) {
            images.push(image);
        }
        child = parent;
    }
    images
}

/// A process image's full path. Never logged, never audited.
fn full_image_path(process_id: u32) -> Option<PathBuf> {
    let process = OwnedHandle::process(process_id, PROCESS_QUERY_LIMITED_INFORMATION)?;
    let mut wide = [0u16; IMAGE_PATH_UNITS];
    let mut size = wide.len() as u32;
    // SAFETY: process is a valid query handle; wide outlives the call and size names its exact
    // length in UTF-16 units.
    let ok = unsafe { QueryFullProcessImageNameW(process.0, 0, wide.as_mut_ptr(), &mut size) };
    if ok == 0 || size == 0 || size as usize > wide.len() {
        return None;
    }
    Some(PathBuf::from(OsString::from_wide(&wide[..size as usize])))
}

/// When `process_id` was created, as the raw FILETIME value.
fn creation_time(process_id: u32) -> Option<u64> {
    if process_id == 0 {
        return None;
    }
    let process = OwnedHandle::process(process_id, PROCESS_QUERY_LIMITED_INFORMATION)?;
    let mut creation = FileTime { low: 0, high: 0 };
    let mut exit = FileTime { low: 0, high: 0 };
    let mut kernel = FileTime { low: 0, high: 0 };
    let mut user = FileTime { low: 0, high: 0 };
    // SAFETY: all output structures outlive the call and process is a valid query handle.
    let ok =
        unsafe { GetProcessTimes(process.0, &mut creation, &mut exit, &mut kernel, &mut user) };
    (ok != 0).then_some((u64::from(creation.high) << 32) | u64::from(creation.low))
}

#[cfg(test)]
mod tests {
    use super::ancestor_images;

    #[test]
    fn the_ancestor_walk_is_bounded_and_starts_at_the_parent() {
        let one = ancestor_images(1);
        assert!(one.len() <= 1);
        assert!(ancestor_images(0).is_empty());
        let all = ancestor_images(16);
        assert!(all.len() <= 16);
        assert!(!all.is_empty(), "a test process has a parent to name");
        assert_eq!(all.first(), one.first());
    }
}
