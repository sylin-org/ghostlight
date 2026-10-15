//! Linux detached process creation with no inherited descriptors, and the user identities the
//! relay endpoint compares.

use std::ffi::{c_int, c_long, c_ulong, c_void, OsStr};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;

/// The `close_range` system call number, the same on every architecture Ghostlight supports.
const SYS_CLOSE_RANGE: c_long = 436;

/// `close_range` flag: mark the range close-on-exec instead of closing it now.
const CLOSE_RANGE_CLOEXEC: u32 = 4;

/// The first descriptor the child must not inherit. Descriptors 0 to 2 are the child's null
/// standard streams.
const FIRST_UNINHERITED: u32 = 3;

/// `getrlimit` resource: the number of open files.
const RLIMIT_NOFILE: c_int = 7;

/// `fcntl` command: set the descriptor flags.
const F_SETFD: c_int = 2;

/// `fcntl` descriptor flag: close the descriptor when the process executes another program.
const FD_CLOEXEC: c_int = 1;

/// The descriptor ceiling used when the limit cannot be read.
const FALLBACK_DESCRIPTOR_LIMIT: u32 = 1024;

/// The most descriptors the per-descriptor fallback sweeps, however high the limit is set.
const MAX_SWEEP: u32 = 1 << 20;

/// Socket option level: the socket layer itself.
const SOL_SOCKET: c_int = 1;

/// Socket option: the credentials of the peer of a connected Unix socket.
const SO_PEERCRED: c_int = 17;

/// The C `struct ucred`.
#[repr(C)]
struct Credentials {
    process_id: i32,
    user_id: u32,
    group_id: u32,
}

/// The C `struct rlimit`.
#[repr(C)]
struct RLimit {
    current: c_ulong,
    maximum: c_ulong,
}

extern "C" {
    fn syscall(number: c_long, ...) -> c_long;
    fn fcntl(descriptor: c_int, command: c_int, ...) -> c_int;
    fn getrlimit(resource: c_int, limit: *mut RLimit) -> c_int;
    fn getuid() -> u32;
    fn getsockopt(
        socket: c_int,
        level: c_int,
        option: c_int,
        value: *mut c_void,
        length: *mut u32,
    ) -> c_int;
}

/// The user id this process runs as.
#[must_use]
pub fn current_uid() -> u32 {
    // SAFETY: getuid takes no arguments, cannot fail and touches no memory.
    unsafe { getuid() }
}

/// The user id of the process on the other end of the connected Unix socket `stream`, as the
/// kernel recorded it when the connection was made.
pub fn peer_uid(stream: &UnixStream) -> io::Result<u32> {
    let mut credentials = Credentials {
        process_id: 0,
        user_id: 0,
        group_id: 0,
    };
    let mut length = size_of::<Credentials>() as u32;
    // SAFETY: the descriptor is open for the borrow; value and length describe the writable
    // credentials struct for the whole call.
    let read = unsafe {
        getsockopt(
            stream.as_raw_fd(),
            SOL_SOCKET,
            SO_PEERCRED,
            (&raw mut credentials).cast(),
            &mut length,
        )
    };
    if read != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(credentials.user_id)
}

/// Start `executable` with `arguments` in its own process group, with `directory` as its working
/// directory, null standard streams and no other open descriptor, and return its process id.
///
/// Every descriptor from 3 up is made close-on-exec in the child just before it executes the new
/// program, so nothing the starter holds (a pipe, a file, a socket) outlives it in the child. A
/// failure to execute the program is returned, not hidden: the standard library reports it
/// through its own close-on-exec pipe, which the marking leaves open until the exec.
///
/// A thread collects the child's exit status so an early exit does not linger as a zombie.
pub fn spawn_detached(
    executable: &Path,
    arguments: &[&OsStr],
    directory: &Path,
) -> io::Result<u32> {
    let limit = descriptor_limit();
    let mut command = Command::new(executable);
    command
        .args(arguments)
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    // SAFETY: the closure runs between fork and exec, so it may only call async-signal-safe
    // functions. `mark_close_on_exec` makes two raw system calls and allocates nothing.
    unsafe {
        command.pre_exec(move || {
            mark_close_on_exec(limit);
            Ok(())
        });
    }
    let mut child = command.spawn()?;
    let process_id = child.id();
    thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(process_id)
}

/// The soft limit on open files, read before the fork so the child never calls anything that
/// allocates.
fn descriptor_limit() -> u32 {
    let mut limit = RLimit {
        current: 0,
        maximum: 0,
    };
    // SAFETY: limit is a writable struct of the C layout for the whole call.
    let read = unsafe { getrlimit(RLIMIT_NOFILE, &mut limit) } == 0;
    if !read {
        return FALLBACK_DESCRIPTOR_LIMIT;
    }
    u32::try_from(limit.current)
        .unwrap_or(MAX_SWEEP)
        .min(MAX_SWEEP)
}

/// Mark every descriptor from [`FIRST_UNINHERITED`] up close-on-exec.
///
/// One `close_range` call covers the whole range; a kernel older than 5.11 refuses the flag, and
/// each descriptor below `descriptor_limit` is then marked on its own.
fn mark_close_on_exec(descriptor_limit: u32) {
    // SAFETY: close_range takes only integers and changes flags on this process's own
    // descriptors; it touches no memory.
    let marked = unsafe {
        syscall(
            SYS_CLOSE_RANGE,
            FIRST_UNINHERITED,
            u32::MAX,
            CLOSE_RANGE_CLOEXEC,
        )
    } == 0;
    if marked {
        return;
    }
    for descriptor in FIRST_UNINHERITED..descriptor_limit {
        // SAFETY: fcntl with F_SETFD takes integers only. A descriptor that is not open is
        // refused with an error that is ignored.
        unsafe { fcntl(descriptor as c_int, F_SETFD, FD_CLOEXEC) };
    }
}
