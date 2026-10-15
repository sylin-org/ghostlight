//! Windows handles, process tokens and security identifiers, as the named-pipe endpoint needs
//! them: who this process is, who a peer process is, and a security descriptor that admits only
//! the current user.

use std::ffi::c_void;
use std::io;
use std::ptr::{null, null_mut};

/// A Windows `HANDLE`.
pub(super) type Handle = *mut c_void;

/// A pointer to a security identifier.
type Sid = *mut c_void;

/// Access right: query a token.
const TOKEN_QUERY: u32 = 0x0008;

/// Access right: read a limited set of process facts, enough to open its token.
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x0000_1000;

/// `TOKEN_INFORMATION_CLASS` value for the token's user.
const TOKEN_USER_CLASS: u32 = 1;

/// `TOKEN_INFORMATION_CLASS` value for the token's integrity level.
const TOKEN_INTEGRITY_LEVEL_CLASS: u32 = 25;

/// `GetTokenInformation` failure code: the buffer is too small.
const ERROR_INSUFFICIENT_BUFFER: i32 = 122;

/// The only security descriptor string revision there is.
const SDDL_REVISION_1: u32 = 1;

/// The invalid-handle sentinel some calls return.
pub(super) const INVALID_HANDLE_VALUE: Handle = -1_isize as Handle;

/// The C `SID_AND_ATTRIBUTES`, which both `TOKEN_USER` and `TOKEN_MANDATORY_LABEL` are.
#[repr(C)]
struct SidAndAttributes {
    sid: Sid,
    attributes: u32,
}

#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcess() -> Handle;
    fn OpenProcess(desired_access: u32, inherit_handle: i32, process_id: u32) -> Handle;
    fn CloseHandle(handle: Handle) -> i32;
    fn CreateEventW(
        attributes: *const c_void,
        manual_reset: i32,
        initial_state: i32,
        name: *const u16,
    ) -> Handle;
    fn SetEvent(handle: Handle) -> i32;
    fn LocalFree(memory: *mut c_void) -> *mut c_void;
}

#[link(name = "advapi32")]
extern "system" {
    fn OpenProcessToken(process: Handle, desired_access: u32, token: *mut Handle) -> i32;
    fn GetTokenInformation(
        token: Handle,
        class: u32,
        information: *mut c_void,
        length: u32,
        returned: *mut u32,
    ) -> i32;
    fn ConvertSidToStringSidW(sid: Sid, string: *mut *mut u16) -> i32;
    fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
        text: *const u16,
        revision: u32,
        descriptor: *mut *mut c_void,
        size: *mut u32,
    ) -> i32;
    fn EqualSid(first: Sid, second: Sid) -> i32;
    fn GetSidSubAuthorityCount(sid: Sid) -> *mut u8;
    fn GetSidSubAuthority(sid: Sid, index: u32) -> *mut u32;
}

/// A handle closed exactly once when the guard drops.
pub(super) struct Owned(pub(super) Handle);

// SAFETY: a handle is a process-wide value, not tied to the thread that opened it. Closing it
// from another thread is allowed, and the guard closes it once.
unsafe impl Send for Owned {}
// SAFETY: the guard exposes the raw value only; every call that takes it is itself thread-safe.
unsafe impl Sync for Owned {}

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: this guard exclusively owns a valid handle and closes it once.
        unsafe { CloseHandle(self.0) };
    }
}

/// A manual-reset event: signalled with [`Event::set`], never cleared.
pub(super) struct Event(Owned);

impl Event {
    /// Create an unsignalled manual-reset event.
    pub(super) fn manual_reset() -> io::Result<Self> {
        // SAFETY: no security attributes and no name; the result is checked below.
        let handle = unsafe { CreateEventW(null(), 1, 0, null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(Owned(handle)))
    }

    /// The raw handle, for a wait or an `OVERLAPPED`.
    pub(super) fn raw(&self) -> Handle {
        (self.0).0
    }

    /// Signal the event.
    pub(super) fn set(&self) {
        // SAFETY: the event handle is valid for as long as this value lives.
        unsafe { SetEvent(self.raw()) };
    }
}

/// A buffer whose start is aligned for any structure the token calls fill in.
struct Buffer(Vec<u64>);

impl Buffer {
    /// Room for at least `bytes` bytes.
    fn with_capacity_for(bytes: usize) -> Self {
        Self(vec![0; bytes.div_ceil(size_of::<u64>()).max(1)])
    }

    /// The buffer's size in bytes.
    fn bytes(&self) -> usize {
        self.0.len() * size_of::<u64>()
    }

    /// The first `SID_AND_ATTRIBUTES` in the buffer.
    fn first_sid_entry(&self) -> &SidAndAttributes {
        // SAFETY: a successful GetTokenInformation for the user or integrity class wrote a
        // SID_AND_ATTRIBUTES at the aligned start, and the buffer outlives the borrow.
        unsafe { &*(self.0.as_ptr().cast::<SidAndAttributes>()) }
    }
}

/// A process token opened for query.
pub(super) struct Token(Owned);

impl Token {
    /// This process's own token.
    pub(super) fn of_current_process() -> io::Result<Self> {
        let mut token: Handle = null_mut();
        // SAFETY: GetCurrentProcess is a pseudo-handle that needs no closing, and token is a
        // writable out-pointer.
        let ok = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(Owned(token)))
    }

    /// The token of the process `process_id`.
    pub(super) fn of_process(process_id: u32) -> io::Result<Self> {
        // SAFETY: OpenProcess takes a numeric id and returns an owned handle or null.
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
        if process.is_null() {
            return Err(io::Error::last_os_error());
        }
        let process = Owned(process);
        let mut token: Handle = null_mut();
        // SAFETY: process is a valid handle with query access and token is a writable
        // out-pointer.
        let ok = unsafe { OpenProcessToken(process.0, TOKEN_QUERY, &mut token) };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(Owned(token)))
    }

    /// The user this token belongs to.
    pub(super) fn user(&self) -> io::Result<UserSid> {
        self.query(TOKEN_USER_CLASS).map(UserSid)
    }

    /// The integrity level of this token, as its relative identifier (medium is `0x2000`).
    pub(super) fn integrity_rid(&self) -> io::Result<u32> {
        let buffer = self.query(TOKEN_INTEGRITY_LEVEL_CLASS)?;
        let sid = buffer.first_sid_entry().sid;
        // SAFETY: sid points into the buffer GetTokenInformation filled, which is alive, and is a
        // well-formed SID. The count and the sub-authority it indexes are read before the buffer
        // drops.
        unsafe {
            let count = *GetSidSubAuthorityCount(sid);
            if count == 0 {
                return Err(io::Error::from(io::ErrorKind::InvalidData));
            }
            Ok(*GetSidSubAuthority(sid, u32::from(count) - 1))
        }
    }

    /// Fetch one token information class into an aligned buffer.
    fn query(&self, class: u32) -> io::Result<Buffer> {
        let mut needed: u32 = 0;
        // SAFETY: a null buffer of length zero asks only for the required size.
        unsafe { GetTokenInformation((self.0).0, class, null_mut(), 0, &mut needed) };
        let failure = io::Error::last_os_error();
        if failure.raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER) || needed == 0 {
            return Err(failure);
        }
        let mut buffer = Buffer::with_capacity_for(needed as usize);
        let length = u32::try_from(buffer.bytes()).map_err(|_| io::ErrorKind::OutOfMemory)?;
        // SAFETY: the buffer is writable for `length` bytes and aligned for 64-bit values.
        let ok = unsafe {
            GetTokenInformation(
                (self.0).0,
                class,
                buffer.0.as_mut_ptr().cast(),
                length,
                &mut needed,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(buffer)
    }
}

/// The security identifier of a token's user, with the buffer it points into.
pub(super) struct UserSid(Buffer);

impl UserSid {
    fn sid(&self) -> Sid {
        self.0.first_sid_entry().sid
    }

    /// The identifier as text, such as `S-1-5-21-1-2-3-1001`.
    pub(super) fn text(&self) -> io::Result<String> {
        let mut string: *mut u16 = null_mut();
        // SAFETY: the SID is valid for the call and string is a writable out-pointer.
        if unsafe { ConvertSidToStringSidW(self.sid(), &mut string) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: on success string points at a NUL-terminated UTF-16 string the system
        // allocated. It is read once, then freed with LocalFree exactly once.
        let text = unsafe {
            let mut length = 0;
            while *string.add(length) != 0 {
                length += 1;
            }
            let text = String::from_utf16_lossy(std::slice::from_raw_parts(string, length));
            LocalFree(string.cast());
            text
        };
        Ok(text)
    }

    /// Whether both identifiers name the same user.
    pub(super) fn same_user(&self, other: &UserSid) -> bool {
        // SAFETY: both SIDs are valid and live in buffers that outlive the call.
        unsafe { EqualSid(self.sid(), other.sid()) != 0 }
    }
}

/// A security descriptor, freed when it drops.
pub(super) struct SecurityDescriptor(*mut c_void);

// SAFETY: the descriptor is immutable memory the system allocated; reading it from any thread is
// fine, and it is freed once by the owner.
unsafe impl Send for SecurityDescriptor {}
// SAFETY: see above; nothing mutates it after creation.
unsafe impl Sync for SecurityDescriptor {}

impl SecurityDescriptor {
    /// A protected descriptor that grants all access to the user `user_sid_text` and to no one
    /// else.
    pub(super) fn for_user_only(user_sid_text: &str) -> io::Result<Self> {
        let text: Vec<u16> = format!("D:P(A;;GA;;;{user_sid_text})")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let mut descriptor: *mut c_void = null_mut();
        // SAFETY: text is NUL-terminated and outlives the call; descriptor is a writable
        // out-pointer and the size is not wanted.
        let ok = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                text.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(descriptor))
    }

    /// The raw descriptor, for a `SECURITY_ATTRIBUTES`.
    pub(super) fn as_ptr(&self) -> *mut c_void {
        self.0
    }
}

impl Drop for SecurityDescriptor {
    fn drop(&mut self) {
        // SAFETY: the descriptor was allocated by the system for this guard and is freed once.
        unsafe { LocalFree(self.0) };
    }
}
