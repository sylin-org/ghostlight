//! Linux detached process creation with no inherited descriptors, the credentials the relay
//! endpoint compares, and the process facts an ancestor walk reads from `/proc`.

use std::ffi::{c_int, c_long, c_ulong, c_void, OsStr};
use std::fs;
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;

use super::ancestry::ProcessFacts;

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

/// The directory the kernel describes every process in.
const PROC_DIRECTORY: &str = "/proc";

/// The file below a process's directory that holds its status line.
const STAT_FILE: &str = "stat";

/// The link below a process's directory that names its executable.
const EXE_LINK: &str = "exe";

/// The parent id's place among the status fields after the command name (field 4 of `stat`).
const PARENT_FIELD: usize = 1;

/// The start time's place among the status fields after the command name (field 22 of `stat`).
const START_TIME_FIELD: usize = 19;

/// What the kernel appends to the executable link of a program whose file was removed.
const DELETED_SUFFIX: &str = " (deleted)";

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

/// Who is on the other end of a connected Unix socket, as the kernel recorded it when the
/// connection was made.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PeerCredentials {
    /// The peer's user id.
    pub user_id: u32,
    /// The peer's process id, or none when the peer lives in another process id namespace (a
    /// sandbox), where the kernel reports zero.
    pub process_id: Option<u32>,
}

/// The credentials of the process on the other end of the connected Unix socket `stream`.
pub fn peer_credentials(stream: &UnixStream) -> io::Result<PeerCredentials> {
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
    Ok(PeerCredentials {
        user_id: credentials.user_id,
        process_id: u32::try_from(credentials.process_id)
            .ok()
            .filter(|id| *id != 0),
    })
}

/// A reader of one process's facts for [`super::ancestry`], from `/proc/<id>/stat` (the parent id
/// and start time) and the `/proc/<id>/exe` link (the executable).
pub(super) fn process_facts_reader() -> impl Fn(u32) -> Option<ProcessFacts> {
    process_facts
}

/// The facts `/proc` holds about `process_id`, or none when it has exited or cannot be read.
fn process_facts(process_id: u32) -> Option<ProcessFacts> {
    let directory = Path::new(PROC_DIRECTORY).join(process_id.to_string());
    let stat = fs::read_to_string(directory.join(STAT_FILE)).ok()?;
    // The command name, in parentheses, may itself hold spaces and parentheses: the fields
    // that follow start after the last closing one.
    let fields: Vec<&str> = stat
        .get(stat.rfind(')')? + 1..)?
        .split_whitespace()
        .collect();
    let parent = fields
        .get(PARENT_FIELD)?
        .parse::<u32>()
        .ok()
        .filter(|id| *id != 0);
    let started = fields.get(START_TIME_FIELD)?.parse::<u64>().ok()?;
    let image = fs::read_link(directory.join(EXE_LINK)).ok().map(|image| {
        let kept = image
            .to_string_lossy()
            .strip_suffix(DELETED_SUFFIX)
            .map(PathBuf::from);
        kept.unwrap_or(image)
    });
    Some(ProcessFacts {
        parent,
        started,
        image,
    })
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
///
/// It refuses, spawning nothing, while the session is ending (see [`super::session`]).
pub fn spawn_detached(
    executable: &Path,
    arguments: &[&OsStr],
    directory: &Path,
) -> io::Result<u32> {
    super::session::refuse_start()?;
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
