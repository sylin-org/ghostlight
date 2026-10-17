//! Where a Microsoft Store (MSIX) package is installed, asked of Windows's app model by its
//! package family name.
//!
//! The family name (`<name>_<publisher id>`) names every version and architecture of one app.
//! Windows lists the full names of the family's packages installed for this user, and says where
//! each one's files are. Only those two kernel32 calls are made; nothing here starts the app or
//! uses WinRT.

use std::ffi::OsString;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;

/// The call succeeded.
const ERROR_SUCCESS: i32 = 0;

/// The buffers given are too small; the sizes needed were written back.
const ERROR_INSUFFICIENT_BUFFER: i32 = 122;

/// How many times the list is asked for when it grows between the size query and the read.
const ATTEMPTS: usize = 3;

/// The most packages of one family read: a family normally has one per architecture.
const MAX_PACKAGES: u32 = 16;

/// The longest full name list or install path read, in UTF-16 units.
const MAX_UNITS: u32 = 32 * 1024;

#[link(name = "kernel32")]
extern "system" {
    fn GetPackagesByPackageFamily(
        family: *const u16,
        count: *mut u32,
        full_names: *mut *mut u16,
        buffer_length: *mut u32,
        buffer: *mut u16,
    ) -> i32;
    fn GetPackagePathByFullName(
        full_name: *const u16,
        path_length: *mut u32,
        path: *mut u16,
    ) -> i32;
}

/// The install folders of the packages of family `family` installed for this user; empty when
/// none is, or when Windows cannot say.
pub(crate) fn install_folders(family: &str) -> Vec<PathBuf> {
    full_names(family)
        .iter()
        .filter_map(|full_name| install_folder(full_name))
        .collect()
}

/// The NUL-terminated full names of the family's packages installed for this user.
fn full_names(family: &str) -> Vec<Vec<u16>> {
    let family: Vec<u16> = std::ffi::OsStr::new(family)
        .encode_wide()
        .chain(Some(0))
        .collect();
    for _ in 0..ATTEMPTS {
        let mut count = 0u32;
        let mut length = 0u32;
        // SAFETY: `family` is NUL-terminated and outlives the call; with null buffers and zero
        // sizes the call only writes the sizes it needs into `count` and `length`.
        let status = unsafe {
            GetPackagesByPackageFamily(
                family.as_ptr(),
                &mut count,
                std::ptr::null_mut(),
                &mut length,
                std::ptr::null_mut(),
            )
        };
        if status != ERROR_INSUFFICIENT_BUFFER
            || count == 0
            || count > MAX_PACKAGES
            || length == 0
            || length > MAX_UNITS
        {
            return Vec::new();
        }
        let mut pointers = vec![std::ptr::null_mut::<u16>(); count as usize];
        let mut buffer = vec![0u16; length as usize];
        // SAFETY: `pointers` holds `count` writable slots and `buffer` holds `length` writable
        // units, the sizes the call is told; both outlive it. The pointers it writes point into
        // `buffer` and are read below only as offsets into it.
        let status = unsafe {
            GetPackagesByPackageFamily(
                family.as_ptr(),
                &mut count,
                pointers.as_mut_ptr(),
                &mut length,
                buffer.as_mut_ptr(),
            )
        };
        match status {
            ERROR_SUCCESS => {
                return pointers
                    .iter()
                    .take(count as usize)
                    .filter_map(|pointer| name_at(&buffer, *pointer))
                    .collect();
            }
            ERROR_INSUFFICIENT_BUFFER => {}
            _ => return Vec::new(),
        }
    }
    Vec::new()
}

/// The NUL-terminated name that starts where `pointer` points into `buffer`, read as an offset so
/// nothing outside `buffer` is touched.
fn name_at(buffer: &[u16], pointer: *mut u16) -> Option<Vec<u16>> {
    let bytes = (pointer as usize).checked_sub(buffer.as_ptr() as usize)?;
    if bytes % std::mem::size_of::<u16>() != 0 {
        return None;
    }
    let rest = buffer.get(bytes / std::mem::size_of::<u16>()..)?;
    let end = rest.iter().position(|unit| *unit == 0)?;
    (end > 0).then(|| rest[..=end].to_vec())
}

/// The install folder of the package with the NUL-terminated `full_name`.
fn install_folder(full_name: &[u16]) -> Option<PathBuf> {
    let mut length = 0u32;
    // SAFETY: `full_name` is NUL-terminated and outlives the call; with a null buffer and a zero
    // length the call only writes the length it needs.
    let status =
        unsafe { GetPackagePathByFullName(full_name.as_ptr(), &mut length, std::ptr::null_mut()) };
    if status != ERROR_INSUFFICIENT_BUFFER || length == 0 || length > MAX_UNITS {
        return None;
    }
    let mut path = vec![0u16; length as usize];
    // SAFETY: `path` holds `length` writable units, the size the call is told, and outlives it.
    let status =
        unsafe { GetPackagePathByFullName(full_name.as_ptr(), &mut length, path.as_mut_ptr()) };
    if status != ERROR_SUCCESS {
        return None;
    }
    let end = path
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(path.len());
    (end > 0).then(|| PathBuf::from(OsString::from_wide(&path[..end])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_family_nothing_installed_has_no_install_folder() {
        assert!(install_folders("Ghostlight.NoSuchPackage_0000000000000").is_empty());
        assert!(install_folders("").is_empty());
        assert!(install_folders("not a family name").is_empty());
    }

    #[test]
    fn a_name_is_read_only_inside_its_buffer() {
        let mut buffer: Vec<u16> = "first\0second\0".encode_utf16().collect();
        let start = buffer.as_mut_ptr();
        // The pointers are made only to be compared as addresses, never dereferenced.
        let second = start.wrapping_add(6);
        let outside = start.wrapping_add(buffer.len() + 4);
        assert_eq!(
            name_at(&buffer, start),
            Some("first\0".encode_utf16().collect())
        );
        assert_eq!(
            name_at(&buffer, second),
            Some("second\0".encode_utf16().collect())
        );
        assert_eq!(name_at(&buffer, outside), None);
        assert_eq!(name_at(&buffer, start.wrapping_sub(1)), None);
        assert_eq!(name_at(&buffer, start.wrapping_add(5)), None, "empty name");
    }
}
