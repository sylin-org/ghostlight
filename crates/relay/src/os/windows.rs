//! Windows detached process creation with no inherited handles, process identity, and the
//! process facts an ancestor walk reads.

use std::collections::HashMap;
use std::ffi::{c_void, OsStr, OsString};
use std::io;
use std::mem;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};

use super::ancestry::ProcessFacts;

type Handle = *mut c_void;

/// Process creation flag: the child gets no console window.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Process creation flag: the child does not inherit the parent's console.
const DETACHED_PROCESS: u32 = 0x0000_0008;

/// Process creation flag: the child is not part of the parent's Ctrl+C group.
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

/// Process creation flag: the environment block is UTF-16.
const CREATE_UNICODE_ENVIRONMENT: u32 = 0x0000_0400;

/// The creation flags of a detached process.
pub(super) const CREATION_FLAGS: u32 =
    CREATE_NO_WINDOW | DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_UNICODE_ENVIRONMENT;

const TH32CS_SNAPPROCESS: u32 = 0x0000_0002;
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x0000_1000;
const SYNCHRONIZE: u32 = 0x0010_0000;
const WAIT_TIMEOUT: u32 = 0x0000_0102;
const MAX_PATH: usize = 260;

/// The longest executable path read, in UTF-16 units: the longest path Windows allows.
const MAX_IMAGE_UNITS: u32 = 32 * 1024;

/// `QueryFullProcessImageNameW` flag: the path in Win32 form, not the native device form.
const WIN32_PATH_FORMAT: u32 = 0;

const SPACE: u16 = b' ' as u16;
const TAB: u16 = b'\t' as u16;
const QUOTE: u16 = b'"' as u16;
const BACKSLASH: u16 = b'\\' as u16;

/// Stable identity for one Windows process across process-id reuse.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessIdentity {
    /// Operating-system process id.
    pub process_id: u32,
    /// Process creation time, as the raw FILETIME value.
    pub created: u64,
}

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

#[repr(C)]
struct StartupInfoW {
    size: u32,
    reserved: *mut u16,
    desktop: *mut u16,
    title: *mut u16,
    x: u32,
    y: u32,
    x_size: u32,
    y_size: u32,
    x_count_chars: u32,
    y_count_chars: u32,
    fill_attribute: u32,
    flags: u32,
    show_window: u16,
    reserved_2_size: u16,
    reserved_2: *mut u8,
    standard_input: Handle,
    standard_output: Handle,
    standard_error: Handle,
}

#[repr(C)]
struct ProcessInformation {
    process: Handle,
    thread: Handle,
    process_id: u32,
    thread_id: u32,
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateProcessW(
        application_name: *const u16,
        command_line: *mut u16,
        process_attributes: *const c_void,
        thread_attributes: *const c_void,
        inherit_handles: i32,
        creation_flags: u32,
        environment: *const c_void,
        current_directory: *const u16,
        startup_info: *const StartupInfoW,
        process_information: *mut ProcessInformation,
    ) -> i32;
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
        name: *mut u16,
        size: *mut u32,
    ) -> i32;
    fn WaitForSingleObject(handle: Handle, milliseconds: u32) -> u32;
    fn CloseHandle(handle: Handle) -> i32;
}

/// Start `executable` with `arguments`, detached, with `directory` as its working directory, and
/// return its process id.
///
/// The child is created with handle inheritance off and no standard handles, no console and no
/// window, in its own process group, with this process's environment. Nothing the starter holds
/// (a pipe, a file, a socket) reaches it, so a caller reading the starter's output sees it end
/// when the starter does. The handles `CreateProcessW` returns are closed at once.
///
/// It refuses, spawning nothing, while the session is ending (see [`super::session`]).
pub fn spawn_detached(
    executable: &Path,
    arguments: &[&OsStr],
    directory: &Path,
) -> io::Result<u32> {
    super::session::refuse_start()?;
    let application = nul_terminated(executable.as_os_str().encode_wide().collect())?;
    let mut line = nul_terminated(command_line(executable, arguments))?;
    let directory = nul_terminated(directory.as_os_str().encode_wide().collect())?;
    let startup = StartupInfoW {
        size: mem::size_of::<StartupInfoW>() as u32,
        reserved: null_mut(),
        desktop: null_mut(),
        title: null_mut(),
        x: 0,
        y: 0,
        x_size: 0,
        y_size: 0,
        x_count_chars: 0,
        y_count_chars: 0,
        fill_attribute: 0,
        flags: 0,
        show_window: 0,
        reserved_2_size: 0,
        reserved_2: null_mut(),
        standard_input: null_mut(),
        standard_output: null_mut(),
        standard_error: null_mut(),
    };
    let mut created = ProcessInformation {
        process: null_mut(),
        thread: null_mut(),
        process_id: 0,
        thread_id: 0,
    };
    // SAFETY: application, line and directory are NUL-terminated UTF-16 buffers that outlive the
    // call, and line is writable as CreateProcessW requires. startup and created have the C
    // layouts. Inheritance is off (0), there are no security attributes, and a null environment
    // means this process's own.
    let ok = unsafe {
        CreateProcessW(
            application.as_ptr(),
            line.as_mut_ptr(),
            null(),
            null(),
            0,
            CREATION_FLAGS,
            null(),
            directory.as_ptr(),
            &startup,
            &mut created,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: CreateProcessW succeeded, so both handles are valid, owned by this call and closed
    // exactly once here.
    unsafe {
        CloseHandle(created.process);
        CloseHandle(created.thread);
    }
    Ok(created.process_id)
}

/// The UTF-16 command line for `executable` and `arguments`, quoted by the rules the Microsoft C
/// runtime parses: an argument that is empty or holds a space, a tab or a quote is wrapped in
/// quotes; a quote inside is preceded by a backslash; the backslashes before a quote, or before
/// the closing quote, are doubled. The result has no terminator.
pub(super) fn command_line(executable: &Path, arguments: &[&OsStr]) -> Vec<u16> {
    let mut line = Vec::new();
    push_quoted(executable.as_os_str(), &mut line);
    for argument in arguments {
        line.push(SPACE);
        push_quoted(argument, &mut line);
    }
    line
}

/// Append `argument` to `line`, quoted when the C runtime would otherwise split or alter it.
fn push_quoted(argument: &OsStr, line: &mut Vec<u16>) {
    let units: Vec<u16> = argument.encode_wide().collect();
    let needs_quotes = units.is_empty()
        || units
            .iter()
            .any(|&unit| unit == SPACE || unit == TAB || unit == QUOTE);
    if !needs_quotes {
        line.extend(units);
        return;
    }
    line.push(QUOTE);
    let mut backslashes = 0;
    for unit in units {
        if unit == BACKSLASH {
            backslashes += 1;
            continue;
        }
        let escaped = if unit == QUOTE {
            backslashes * 2 + 1
        } else {
            backslashes
        };
        line.extend(std::iter::repeat_n(BACKSLASH, escaped));
        backslashes = 0;
        line.push(unit);
    }
    line.extend(std::iter::repeat_n(BACKSLASH, backslashes * 2));
    line.push(QUOTE);
}

/// `units` followed by the terminator, or an error when a NUL inside would cut the string short.
fn nul_terminated(mut units: Vec<u16>) -> io::Result<Vec<u16>> {
    if units.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a process path or argument holds a NUL",
        ));
    }
    units.push(0);
    Ok(units)
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

/// Every process's parent id, by process id, from one process snapshot. Empty when no snapshot
/// can be taken.
fn parent_ids() -> HashMap<u32, u32> {
    let mut parents = HashMap::new();
    let Some(snapshot) = OwnedHandle::snapshot() else {
        return parents;
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
    // SAFETY: entry has the required size and remains writable for the complete snapshot walk.
    let mut found = unsafe { Process32FirstW(snapshot.0, &mut entry) } != 0;
    while found {
        parents.insert(entry.process_id, entry.parent_process_id);
        // SAFETY: snapshot and entry remain valid for the next row.
        found = unsafe { Process32NextW(snapshot.0, &mut entry) } != 0;
    }
    parents
}

/// The parent process id of `process_id`, from one process snapshot.
fn parent_id(process_id: u32) -> Option<u32> {
    parent_ids().get(&process_id).copied()
}

/// A reader of one process's facts for [`super::ancestry`]: the parent ids come from one snapshot
/// taken now, and each process is opened for query only when it is read.
pub(super) fn process_facts_reader() -> impl Fn(u32) -> Option<ProcessFacts> {
    let parents = parent_ids();
    move |process_id| process_facts(process_id, parents.get(&process_id).copied())
}

/// The creation time and executable path of `process_id`, whose parent the snapshot named as
/// `parent`; none when the process cannot be opened for query.
fn process_facts(process_id: u32, parent: Option<u32>) -> Option<ProcessFacts> {
    if process_id == 0 {
        return None;
    }
    let process = OwnedHandle::process(process_id, PROCESS_QUERY_LIMITED_INFORMATION)?;
    Some(ProcessFacts {
        parent: parent.filter(|id| *id != 0),
        started: creation_time(&process)?,
        image: image_path(&process),
    })
}

/// The creation time of the open `process`, as the raw FILETIME value.
fn creation_time(process: &OwnedHandle) -> Option<u64> {
    let mut creation = FileTime { low: 0, high: 0 };
    let mut exit = FileTime { low: 0, high: 0 };
    let mut kernel = FileTime { low: 0, high: 0 };
    let mut user = FileTime { low: 0, high: 0 };
    // SAFETY: all output structures outlive the call and process is a valid query handle.
    let ok =
        unsafe { GetProcessTimes(process.0, &mut creation, &mut exit, &mut kernel, &mut user) };
    (ok != 0).then_some((u64::from(creation.high) << 32) | u64::from(creation.low))
}

/// The full Win32 path of the open `process`'s executable.
fn image_path(process: &OwnedHandle) -> Option<PathBuf> {
    let mut buffer = vec![0_u16; MAX_IMAGE_UNITS as usize];
    let mut length = MAX_IMAGE_UNITS;
    // SAFETY: buffer holds `length` writable units for the call, and process is a valid query
    // handle. On success the call writes the path's length, without the terminator, to length.
    let ok = unsafe {
        QueryFullProcessImageNameW(
            process.0,
            WIN32_PATH_FORMAT,
            buffer.as_mut_ptr(),
            &mut length,
        )
    };
    if ok == 0 {
        return None;
    }
    buffer.truncate(length as usize);
    Some(PathBuf::from(OsString::from_wide(&buffer)))
}

/// Capture the current process's parent with its creation time bound to its process id.
#[must_use]
pub fn parent_process() -> Option<ProcessIdentity> {
    process_identity(parent_id(std::process::id())?)
}

/// Capture one process with its creation time bound to its process id, or `None` when it cannot
/// be opened.
#[must_use]
pub fn process_identity(process_id: u32) -> Option<ProcessIdentity> {
    if process_id == 0 {
        return None;
    }
    let process = OwnedHandle::process(process_id, PROCESS_QUERY_LIMITED_INFORMATION)?;
    Some(ProcessIdentity {
        process_id,
        created: creation_time(&process)?,
    })
}

/// Whether the exact captured process is still running.
#[must_use]
pub fn process_is_alive(expected: ProcessIdentity) -> bool {
    let Some(process) = OwnedHandle::process(
        expected.process_id,
        SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
    ) else {
        return false;
    };
    // A terminated Windows process remains openable while another process retains a handle.
    // Its handle is signalled, so a zero-time wait is the authoritative liveness check.
    // SAFETY: process is a valid synchronizable handle and the zero timeout never blocks.
    if unsafe { WaitForSingleObject(process.0, 0) } != WAIT_TIMEOUT {
        return false;
    }
    process_identity(expected.process_id).is_some_and(|actual| actual == expected)
}
