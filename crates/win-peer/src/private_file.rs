//! Owner-private files: creating a new file whose protected access list admits only this user and
//! SYSTEM from the moment it exists, so nothing written to it is ever open to anyone else, and
//! asking whether a file's access list lets anyone else read it.

use std::ffi::c_void;
use std::fs::File;
use std::io;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::FromRawHandle;
use std::path::Path;
use std::ptr::null_mut;

use crate::token::{sid_text, LocalMemory, ProcessToken};

type Handle = *mut c_void;
const SDDL_REVISION: u32 = 1;
const GENERIC_WRITE: u32 = 0x4000_0000;
const FILE_SHARE_READ_DELETE: u32 = 0x0000_0005;
const CREATE_NEW: u32 = 1;
const FILE_ATTRIBUTE_NORMAL: u32 = 0x0000_0080;
/// `SE_OBJECT_TYPE` value for a file or directory.
const SE_FILE_OBJECT: u32 = 1;
/// `SECURITY_INFORMATION` flag selecting the discretionary access list.
const DACL_SECURITY_INFORMATION: u32 = 0x0000_0004;
/// The SID of the operating system's own account, SYSTEM.
const SYSTEM_SID: &str = "S-1-5-18";
/// Access entry type that grants, with a body of a mask followed by the trustee's SID.
const ACCESS_ALLOWED_ACE_TYPE: u8 = 0x00;
/// Access entry type that grants under a condition, with the same leading body.
const ACCESS_ALLOWED_CALLBACK_ACE_TYPE: u8 = 0x09;
/// Access entry types that deny rather than grant: plain, object, callback, callback object.
const DENY_ACE_TYPES: [u8; 4] = [0x01, 0x06, 0x0A, 0x0C];
/// Entry flag: the entry only passes to children and does not apply to the file itself.
const INHERIT_ONLY_ACE: u8 = 0x08;
/// Rights that let a trustee read the file's contents, directly (`FILE_READ_DATA`,
/// `GENERIC_READ`, `GENERIC_ALL`) or by rewriting its access list or taking its ownership
/// (`WRITE_DAC`, `WRITE_OWNER`).
const READ_REACHING_RIGHTS: u32 =
    0x0000_0001 | 0x8000_0000 | 0x1000_0000 | 0x0004_0000 | 0x0008_0000;
/// Bytes of a plain access entry before its trustee's SID: the header and the access mask.
const SID_OFFSET: usize = size_of::<AceHeader>() + size_of::<u32>();

/// The C `ACE_HEADER` every access entry begins with.
#[repr(C)]
struct AceHeader {
    kind: u8,
    flags: u8,
    size: u16,
}

/// The C `ACL` header: revision and size, then the entry count.
#[repr(C)]
struct Acl {
    _revision_and_size: [u8; 4],
    count: u16,
    _reserved: u16,
}

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
    fn GetNamedSecurityInfoW(
        name: *const u16,
        object: u32,
        information: u32,
        owner: *mut *mut c_void,
        group: *mut *mut c_void,
        dacl: *mut *mut Acl,
        sacl: *mut *mut c_void,
        descriptor: *mut *mut c_void,
    ) -> u32;
    fn GetAce(acl: *const Acl, index: u32, ace: *mut *mut c_void) -> i32;
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
    create_with(path, &format!("O:{sid}D:P(A;;FA;;;{sid})(A;;FA;;;SY)"))
}

/// Create the new file `path`, writable by this process, with the security descriptor `sddl`.
/// An existing path, a link included, is refused.
fn create_with(path: &Path, sddl: &str) -> io::Result<File> {
    let descriptor: Vec<u16> = sddl.encode_utf16().chain(Some(0)).collect();
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
    let name = wide_path(path)?;
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

/// Whether the access list of the file at `path` lets a trustee other than this user and SYSTEM
/// read the file's contents, or rewrite the list or take ownership so it could. A file with no
/// access list grants everyone everything and answers true; so does a granting entry whose
/// trustee this cannot read. Deny entries are not subtracted, so the answer never understates.
pub(super) fn others_can_read(path: &Path) -> io::Result<bool> {
    let user = ProcessToken::current()?.user_sid()?;
    let name = wide_path(path)?;
    let mut dacl: *mut Acl = null_mut();
    let mut raw_descriptor = null_mut();
    // SAFETY: name is NUL-terminated and live for the call; the out-pointers are writable, and
    // the unused ones are null, which the call allows. The returned dacl points into the
    // descriptor, which the guard below frees only after the last read of the list.
    let status = unsafe {
        GetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            &mut dacl,
            null_mut(),
            &mut raw_descriptor,
        )
    };
    if status != 0 {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    let _descriptor = LocalMemory(raw_descriptor);
    if dacl.is_null() {
        return Ok(true);
    }
    // SAFETY: dacl is non-null and points at the ACL header inside the live descriptor.
    let count = unsafe { (*dacl).count };
    for index in 0..u32::from(count) {
        let mut ace = null_mut();
        // SAFETY: index is below the list's entry count, and ace is writable.
        if unsafe { GetAce(dacl, index, &mut ace) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: GetAce returned a pointer to an entry inside the live descriptor, and every
        // entry begins with an ACE_HEADER.
        let header = unsafe { &*ace.cast::<AceHeader>() };
        if DENY_ACE_TYPES.contains(&header.kind) || header.flags & INHERIT_ONLY_ACE != 0 {
            continue;
        }
        let plain = matches!(
            header.kind,
            ACCESS_ALLOWED_ACE_TYPE | ACCESS_ALLOWED_CALLBACK_ACE_TYPE
        );
        if !plain || usize::from(header.size) <= SID_OFFSET {
            return Ok(true);
        }
        let bytes = ace.cast::<u8>();
        // SAFETY: a plain allow entry is its header, then a four-byte mask, then the trustee's
        // SID, all inside the entry, whose size exceeds the SID's offset.
        let mask = unsafe {
            bytes
                .add(size_of::<AceHeader>())
                .cast::<u32>()
                .read_unaligned()
        };
        // SAFETY: as above, the trustee's SID starts at SID_OFFSET inside the live entry.
        let trustee = unsafe { sid_text(bytes.add(SID_OFFSET).cast()) }?;
        if mask & READ_REACHING_RIGHTS != 0 && trustee != user && trustee != SYSTEM_SID {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `path` as a NUL-terminated wide string for a Win32 call; a path holding a NUL is refused.
fn wide_path(path: &Path) -> io::Result<Vec<u16>> {
    let mut name: Vec<u16> = path.as_os_str().encode_wide().collect();
    if name.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "NUL in file path",
        ));
    }
    name.push(0);
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::{create, create_with, others_can_read};
    use crate::scratch::Scratch;
    use crate::token::ProcessToken;

    /// An owner-private file is closed to everyone else, and the check sees each way a file can
    /// be open to someone: a read grant, a full grant, a grant to rewrite its list, a generic
    /// read grant, and no list at all. A grant to write alone reads nothing.
    #[test]
    fn only_an_owner_private_file_is_closed_to_everyone_else() {
        let scratch = Scratch::named("private-file");
        let private = scratch.join("private");
        drop(create(&private).expect("the private file is created"));
        assert!(!others_can_read(&private).expect("its access list is read"));

        let sid = ProcessToken::current().unwrap().user_sid().unwrap();
        let owner_only = format!("O:{sid}D:P(A;;FA;;;{sid})(A;;FA;;;SY)");
        for (label, grant, open) in [
            ("read", "(A;;FR;;;BU)", true),
            ("full", "(A;;FA;;;WD)", true),
            ("rewrite", "(A;;WD;;;AU)", true),
            ("generic", "(A;;GR;;;BA)", true),
            ("write", "(A;;FW;;;BU)", false),
        ] {
            let path = scratch.join(label);
            drop(create_with(&path, &format!("{owner_only}{grant}")).unwrap());
            assert_eq!(others_can_read(&path).unwrap(), open, "{label} grant");
        }
        let unlisted = scratch.join("unlisted");
        drop(create_with(&unlisted, &format!("O:{sid}D:NO_ACCESS_CONTROL")).unwrap());
        assert!(others_can_read(&unlisted).unwrap(), "a file with no list");
    }
}
