// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Cross-platform process-liveness primitives for lifecycle hygiene (ADR-0029).
//!
//! The adapter role (ADR-0030 Decision 8 re-scope, PINS.md SS5.5) must end when the MCP client
//! that spawned it goes away, even when the platform does not deliver a stdin EOF (Windows leaves
//! the child's ReadFile parked forever when the parent is killed rather than closed). The
//! parent-death watchdog polls these primitives; the
//! `doctor` diagnosis and its `--fix` reaper use them to tell an exited process from a hung one and
//! to reap only genuinely orphaned sessions.
//!
//! The public surface is intentionally tiny and platform-agnostic; the OS specifics live in the
//! per-platform `imp` module. On Windows a process is identified by pid **plus creation time**, so a
//! reused pid reads as a different (dead) process. On Unix creation time is not recorded here
//! (`created == 0`); parent-death is detected instead via `getppid`, which the kernel updates on
//! reparent and which therefore carries no pid-reuse hazard.

/// A handle to a specific process instance: its pid plus a best-effort creation timestamp that
/// distinguishes it from a later process that reuses the pid. `created == 0` means "creation time
/// unknown on this platform" (Unix), in which case liveness falls back to pid existence alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcId {
    /// OS process id.
    pub pid: u32,
    /// Process creation time in an opaque, platform-specific unit (Windows FILETIME ticks); `0`
    /// when unknown. Only ever compared for equality against another reading for the same pid.
    pub created: u64,
}

impl ProcId {
    /// Construct from a pid, filling in the creation time when the platform can read it.
    pub fn of(pid: u32) -> Self {
        Self {
            pid,
            created: creation_time(pid).unwrap_or(0),
        }
    }
}

/// This process's parent, if it can be determined (with its creation time on Windows).
pub fn parent() -> Option<ProcId> {
    imp::parent()
}

/// Whether a process with this pid is currently **running** (ignores creation time). `false` for
/// pid 0, and -- crucially -- `false` for a process that has terminated but whose object is still
/// held open by another handle (a parent holds handles to its children). This asks "is it running?"
/// via the OS termination signal, not "does its object exist?", so a dead-but-held process reads as
/// dead. See [`imp::is_running`].
pub fn pid_exists(pid: u32) -> bool {
    pid != 0 && imp::is_running(pid)
}

/// A live pid's creation time, or `None` if the pid is not alive or the platform does not record it.
pub fn creation_time(pid: u32) -> Option<u64> {
    if pid == 0 {
        return None;
    }
    imp::creation_time(pid)
}

/// Best-effort terminate a process by pid. Returns `true` if the OS reported success. The caller is
/// responsible for never passing its own pid and for the ADR-0029 "parent-dead orphans only" rule.
pub fn terminate(pid: u32) -> bool {
    pid != 0 && imp::terminate(pid)
}

/// Whether `proc` still names the same **running** process: it is running (not terminated -- see
/// [`pid_exists`] for why "running" is stronger than "object exists") AND, when a creation time is
/// known, still matching it (so a reused pid reads as a different, unrelated process, hence dead).
pub fn is_alive(proc: ProcId) -> bool {
    if proc.pid == 0 {
        return false;
    }
    if !imp::is_running(proc.pid) {
        return false;
    }
    if proc.created != 0 {
        creation_time(proc.pid) == Some(proc.created)
    } else {
        true
    }
}

/// Whether this process has been orphaned: its original parent has exited.
///
/// Windows: the recorded parent [`ProcId`] is no longer alive (pid gone, or a different process now
/// wears that pid -- caught by the creation-time mismatch). Unix: `getppid()` no longer equals the
/// original parent pid, because the kernel reparents an orphan to init/launchd.
pub fn orphaned(original_parent: ProcId) -> bool {
    #[cfg(unix)]
    {
        imp::getppid_u32() != original_parent.pid
    }
    #[cfg(windows)]
    {
        !is_alive(original_parent)
    }
}

// --- Windows ---

#[cfg(windows)]
mod imp {
    use super::ProcId;
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, ERROR_ACCESS_DENIED, FILETIME, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, TerminateProcess, WaitForSingleObject,
        PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
    };

    /// SYNCHRONIZE access (to wait on a process handle) and the WaitForSingleObject "still running"
    /// result, defined locally as their stable Win32 values so this does not depend on which
    /// windows-sys feature module happens to export each constant.
    const SYNCHRONIZE: u32 = 0x0010_0000;
    const WAIT_TIMEOUT: u32 = 0x0000_0102;

    fn filetime_to_u64(ft: FILETIME) -> u64 {
        ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64)
    }

    pub(super) fn parent() -> Option<ProcId> {
        let me = std::process::id();
        // One snapshot walk to find our own entry's parent pid. Safety: the snapshot handle is
        // closed on every path; `entry` is zero-initialized with its dwSize set, as the API requires.
        let ppid = unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap == INVALID_HANDLE_VALUE {
                return None;
            }
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut found: Option<u32> = None;
            if Process32FirstW(snap, &mut entry) != 0 {
                loop {
                    if entry.th32ProcessID == me {
                        found = Some(entry.th32ParentProcessID);
                        break;
                    }
                    if Process32NextW(snap, &mut entry) == 0 {
                        break;
                    }
                }
            }
            CloseHandle(snap);
            found
        }?;
        if ppid == 0 {
            return None;
        }
        Some(ProcId {
            pid: ppid,
            created: creation_time(ppid).unwrap_or(0),
        })
    }

    pub(super) fn is_running(pid: u32) -> bool {
        // Safety: handle checked for null and closed on every path. The key correctness point: a
        // process object stays queryable via OpenProcess as long as ANY handle to it is open, and a
        // parent holds handles to its children -- so OpenProcess succeeding does NOT mean the process
        // is running. WaitForSingleObject(handle, 0) asks the OS the right question: the process
        // handle becomes signaled (WAIT_OBJECT_0) once the process terminates and stays signaled;
        // WAIT_TIMEOUT means it is still running. Every handle observes the signal, regardless of who
        // holds one, so a terminated-but-held process correctly reads as not running.
        unsafe {
            let handle = OpenProcess(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                // Not openable: a genuinely gone process, or (rare, and never for our same-user
                // targets) access denied, which we treat as still-present rather than risk a false
                // "dead" that could strand a session.
                return GetLastError() == ERROR_ACCESS_DENIED;
            }
            let wait = WaitForSingleObject(handle, 0);
            CloseHandle(handle);
            wait == WAIT_TIMEOUT
        }
    }

    pub(super) fn creation_time(pid: u32) -> Option<u64> {
        // Safety: handle is checked for null and closed on every path; the four FILETIME outs are
        // valid stack storage passed by pointer, as GetProcessTimes requires.
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return None;
            }
            let mut creation: FILETIME = std::mem::zeroed();
            let mut exit: FILETIME = std::mem::zeroed();
            let mut kernel: FILETIME = std::mem::zeroed();
            let mut user: FILETIME = std::mem::zeroed();
            let ok = GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user);
            CloseHandle(handle);
            if ok != 0 {
                Some(filetime_to_u64(creation))
            } else {
                None
            }
        }
    }

    pub(super) fn terminate(pid: u32) -> bool {
        // Safety: handle is checked for null and closed on every path.
        unsafe {
            let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
            if handle.is_null() {
                return false;
            }
            let ok = TerminateProcess(handle, 1);
            CloseHandle(handle);
            ok != 0
        }
    }
}

// --- Unix ---

#[cfg(unix)]
mod imp {
    use super::ProcId;

    pub(super) fn getppid_u32() -> u32 {
        // Safety: getppid is always safe and never fails.
        (unsafe { libc::getppid() }) as u32
    }

    pub(super) fn parent() -> Option<ProcId> {
        let ppid = getppid_u32();
        if ppid == 0 {
            return None;
        }
        // Creation time is not recorded on Unix; parent-death is detected via getppid (see
        // `super::orphaned`), which needs no creation time.
        Some(ProcId {
            pid: ppid,
            created: 0,
        })
    }

    pub(super) fn is_running(pid: u32) -> bool {
        // kill(pid, 0) performs the permission/existence checks without sending a signal: Ok(0)
        // means it exists and we may signal it; EPERM means it exists but we may not; ESRCH means no
        // such process. On Unix an orphaned child is reparented to init and reaped, so a stale
        // terminated entry does not linger the way a handle-held Windows object can. Safety: kill is
        // a plain libc call with no memory arguments.
        let rc = unsafe { libc::kill(pid as libc::pid_t, 0) };
        if rc == 0 {
            return true;
        }
        std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }

    pub(super) fn creation_time(_pid: u32) -> Option<u64> {
        None
    }

    pub(super) fn terminate(pid: u32) -> bool {
        // Safety: kill is a plain libc call with no memory arguments. SIGTERM is the polite default;
        // our server processes do not trap it, so it terminates them.
        (unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) }) == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Child, Command};
    use std::time::Duration;

    /// A child process that lives long enough to probe, without needing stdin.
    fn spawn_sleeper() -> Child {
        #[cfg(windows)]
        {
            use std::process::Stdio;
            // ping runs ~30s and needs no console input; discard its output.
            Command::new("ping")
                .args(["-n", "30", "127.0.0.1"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn ping sleeper")
        }
        #[cfg(unix)]
        {
            Command::new("sleep")
                .arg("30")
                .spawn()
                .expect("spawn sleep sleeper")
        }
    }

    #[test]
    fn current_process_is_alive() {
        let me = ProcId::of(std::process::id());
        assert!(pid_exists(me.pid));
        assert!(is_alive(me));
    }

    #[test]
    fn pid_zero_is_never_alive() {
        assert!(!pid_exists(0));
        assert!(!is_alive(ProcId { pid: 0, created: 0 }));
        assert!(!terminate(0));
    }

    #[test]
    fn this_process_has_a_live_unorphaned_parent() {
        let p = parent().expect("a test process always has a parent");
        assert_ne!(p.pid, 0);
        assert!(is_alive(p), "the test runner (our parent) is alive");
        assert!(!orphaned(p), "we are not orphaned while the runner lives");
    }

    #[test]
    fn terminate_kills_a_child_and_pid_exists_reflects_it() {
        // Scope the Child so its process handle is closed before we probe: on Windows a live
        // std::process::Child handle keeps the (terminated) process object queryable, which is our
        // own handle, not a property of the orphans the reaper targets (it holds no such handles).
        let pid = {
            let mut child = spawn_sleeper();
            let pid = child.id();
            assert!(pid_exists(pid), "the sleeper is alive right after spawn");
            assert!(terminate(pid), "terminate reports success");
            let _ = child.wait(); // reap so the pid is fully released
            pid
        };

        // Give the OS a moment to release the pid, then confirm it reads as dead.
        let mut gone = false;
        for _ in 0..200 {
            if !pid_exists(pid) {
                gone = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(gone, "the killed+reaped pid no longer exists");
    }

    /// The bug that broke the watchdog live-test: on Windows a terminated process's object stays
    /// queryable via OpenProcess while ANY handle to it is open, and a parent holds handles to its
    /// children (VS Code holds the MCP client's handle). Liveness must reflect the termination
    /// SIGNAL, not object existence -- so a killed child whose `std::process::Child` handle we
    /// deliberately keep open must still read as dead. Had this test existed first, the OpenProcess-
    /// success liveness would have failed it immediately.
    #[test]
    fn terminated_process_reads_as_dead_even_while_a_handle_is_held() {
        let mut child = spawn_sleeper();
        let pid = child.id();
        let id = ProcId::of(pid);
        assert!(is_alive(id), "alive right after spawn");

        child.kill().expect("terminate the child");
        child.wait().expect("reap the child's exit status");
        // Deliberately do NOT drop `child`: on Windows it keeps the process handle open, exactly the
        // parent-holds-child-handle case that made a dead process look alive.
        assert!(
            !is_alive(id),
            "a terminated process must read as dead even while its handle is held"
        );
        assert!(
            !pid_exists(pid),
            "pid_exists must reflect actual termination, not object existence"
        );
        drop(child); // release the handle only now
    }

    /// The Windows creation-time discriminator: a live pid carrying the WRONG creation time (as a
    /// reused pid would) reads as dead, not alive. This is the pid-reuse defense the watchdog and
    /// reaper rely on. Unix records no creation time, so this case does not apply there.
    #[cfg(windows)]
    #[test]
    fn wrong_creation_time_reads_as_dead_on_windows() {
        let pid = std::process::id();
        let real = creation_time(pid).expect("own creation time is readable");
        assert!(is_alive(ProcId { pid, created: real }));
        assert!(
            !is_alive(ProcId {
                pid,
                created: real.wrapping_add(1),
            }),
            "a mismatched creation time reads as a different, dead process"
        );
    }
}
