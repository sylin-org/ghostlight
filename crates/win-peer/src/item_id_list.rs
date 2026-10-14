//! The shell's item identifier list for an existing file.
//!
//! A shell link names its target by this list; Windows does not resolve a link that only carries
//! the target's path. The installer writes the Start-menu shortcut itself (ADR-0197 Decision 14)
//! and asks the shell for the list here instead of assembling shell items by hand.

use std::ffi::c_void;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

#[link(name = "shell32")]
extern "system" {
    fn ILCreateFromPathW(path: *const u16) -> *mut c_void;
    fn ILGetSize(list: *const c_void) -> u32;
    fn ILFree(list: *mut c_void);
}

/// An item identifier list the shell allocated; freed on drop.
struct ItemIdList(*mut c_void);

impl Drop for ItemIdList {
    fn drop(&mut self) {
        // SAFETY: the pointer is the non-null list ILCreateFromPathW returned, freed only here.
        unsafe { ILFree(self.0) }
    }
}

/// The serialized item identifier list of the file at `path`, including its terminator.
pub(crate) fn of_file(path: &Path) -> io::Result<Vec<u8>> {
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: `wide` is a NUL-terminated UTF-16 string that outlives the call.
    let list = unsafe { ILCreateFromPathW(wide.as_ptr()) };
    if list.is_null() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("the shell cannot name {}", path.display()),
        ));
    }
    let list = ItemIdList(list);
    // SAFETY: `list.0` is a valid list owned by the guard.
    let size = unsafe { ILGetSize(list.0) } as usize;
    // SAFETY: ILGetSize reports the byte length of the contiguous list, terminator included, so
    // the whole range is readable while the guard holds the list.
    let bytes = unsafe { std::slice::from_raw_parts(list.0.cast::<u8>(), size) };
    Ok(bytes.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_existing_file_has_a_terminated_list_and_a_missing_one_has_none() {
        let list = of_file(&std::env::current_exe().unwrap()).unwrap();
        assert!(list.len() > 2);
        assert_eq!(&list[list.len() - 2..], &[0, 0]);
        let missing =
            std::env::temp_dir().join(format!("ghostlight-no-such-file-{}", std::process::id()));
        assert!(of_file(&missing).is_err());
    }
}
