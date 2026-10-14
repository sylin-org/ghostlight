//! Who owns a command-line session (ADR-0106).
//!
//! A session is the caller, not the socket. That is what lets a person type one command after
//! another, and an application shell out repeatedly, and have every call reach the same tabs.

use std::env;

use crate::intake::session::SessionMarker;
use ghostlight_relay::home::SERVICE_EXECUTABLE_NAME;
use sysinfo::{get_current_pid, Pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// The environment variable a caller sets to pin its own session.
///
/// Environment is inherited through intermediaries, so this survives a throwaway shell between a
/// program and `ghostlight call` -- the case where the parent process is the wrong answer because
/// it is a different `cmd.exe` on every invocation.
pub const SESSION_VARIABLE: &str = "GHOSTLIGHT_SESSION";

/// Describe what owns this command's session.
///
/// An explicit key wins when one is set. Otherwise the caller is the nearest ancestor that is not a
/// Ghostlight executable, widened to the outermost of the same-named ancestors above it (a
/// shell and its per-command stubs), identified by the pair the operating system keeps unique.
#[must_use]
pub fn marker() -> Option<SessionMarker> {
    if let Some(key) = declared() {
        return Some(SessionMarker::Declared { key });
    }
    caller()
}

fn declared() -> Option<String> {
    let value = env::var(SESSION_VARIABLE).ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(120).collect())
}

/// How many ancestors the caller walk reads before it stops.
///
/// The bound covers the whole climb: the Ghostlight hops (a carrier hands over to the installed
/// command, which may hand over to a development build) and the run of same-named ancestors above
/// them. It keeps a malformed process tree from looping the walk.
const MAX_ANCESTORS_READ: usize = 8;

/// One process as the caller walk sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Ancestor {
    pid: u32,
    started_at: u64,
    name: String,
    parent: Option<u32>,
}

/// Whether a process file name is the Ghostlight service executable.
fn is_ghostlight(name: &str) -> bool {
    name.eq_ignore_ascii_case(SERVICE_EXECUTABLE_NAME)
}

/// Climb from `start` past every Ghostlight executable, then to the outermost of the run of
/// same-named ancestors the caller sits in.
///
/// A command is forwarded through fresh child processes (a carrier, the installed command, a
/// development build), so the immediate parent is a throwaway Ghostlight process that differs on
/// every call. Past those, MSYS and Cygwin run every native program through a per-command stub with
/// the same image as the shell, so the first foreign ancestor differs on every command while the
/// shell above it is stable. A run of consecutive ancestors sharing one image name (compared
/// case-insensitively) is therefore one terminal, and its outermost member is the caller. That also
/// treats a shell nested in the same shell as one terminal, which is the intended model.
///
/// Operating systems recycle pids, so a parent link can point at an unrelated process. A parent
/// that started after its child cannot be that child's real parent (equal times are allowed), so
/// the walk treats it as unknown and stops before it, for the hop skip and the fold alike.
///
/// `start` is the first parent to read and `started_at` is when the process below it began, so the
/// first link is held to the same rule as every later one. The walk does not begin at the starting
/// process itself, which may not be named like Ghostlight and would become its own caller.
///
/// `lookup` answers one process by pid. The whole climb reads at most `MAX_ANCESTORS_READ`
/// ancestors. Returns `None` when the chain ends, turns untrustworthy, or the budget is spent
/// before any foreign ancestor is found; once one is found, any of those settles on the outermost
/// member reached.
fn nearest_foreign_ancestor(
    start: u32,
    started_at: u64,
    mut lookup: impl FnMut(u32) -> Option<Ancestor>,
) -> Option<Ancestor> {
    let mut next = start;
    let mut caller: Option<Ancestor> = None;
    let mut child_started_at = started_at;
    for _ in 0..MAX_ANCESTORS_READ {
        let Some(process) = lookup(next) else {
            return caller;
        };
        if process.started_at > child_started_at {
            return caller;
        }
        child_started_at = process.started_at;
        let parent = process.parent;
        match &caller {
            None if is_ghostlight(&process.name) => {}
            Some(inner) if !inner.name.eq_ignore_ascii_case(&process.name) => return caller,
            _ => caller = Some(process),
        }
        match parent {
            Some(parent) => next = parent,
            None => return caller,
        }
    }
    caller
}

/// The caller of this process: its nearest non-Ghostlight ancestor, widened to its same-named run.
///
/// Returns `None` when it cannot be identified, which leaves the workspace bound to the
/// connection exactly as it was before. A caller that loses session continuity is a smaller failure
/// than one that guesses an owner.
fn caller() -> Option<SessionMarker> {
    let me = get_current_pid().ok()?;
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[me]),
        true,
        ProcessRefreshKind::nothing(),
    );
    let own = system.process(me)?;
    let parent = own.parent()?;
    let found = nearest_foreign_ancestor(parent.as_u32(), own.start_time(), |pid| {
        let pid = Pid::from_u32(pid);
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[pid]),
            true,
            ProcessRefreshKind::nothing(),
        );
        let process = system.process(pid)?;
        Some(Ancestor {
            pid: pid.as_u32(),
            started_at: process.start_time(),
            name: process.name().to_string_lossy().into_owned(),
            parent: process.parent().map(Pid::as_u32),
        })
    })?;
    Some(SessionMarker::Process {
        pid: found.pid,
        started_at: found.started_at,
        name: found.name.chars().take(120).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        caller, marker, nearest_foreign_ancestor, Ancestor, MAX_ANCESTORS_READ,
        SERVICE_EXECUTABLE_NAME, SESSION_VARIABLE,
    };
    use crate::intake::session::SessionMarker;

    fn process(pid: u32, name: &str, parent: Option<u32>) -> Ancestor {
        Ancestor {
            pid,
            started_at: 1,
            name: name.to_owned(),
            parent,
        }
    }

    /// Walk from `start` as a process that began after everything in the chain.
    fn walk(chain: &[Ancestor], start: u32) -> Option<u32> {
        walk_after(chain, start, u64::MAX)
    }

    /// Walk from `start` as the parent of a process that began at `started_at`.
    fn walk_after(chain: &[Ancestor], start: u32, started_at: u64) -> Option<u32> {
        nearest_foreign_ancestor(start, started_at, |pid| {
            chain.iter().find(|p| p.pid == pid).cloned()
        })
        .map(|p| p.pid)
    }

    #[test]
    fn a_terminal_parent_is_the_caller() {
        let chain = [process(10, "pwsh.exe", Some(1))];
        assert_eq!(walk(&chain, 10), Some(10));
    }

    #[test]
    fn forwarding_hops_are_climbed() {
        let g = SERVICE_EXECUTABLE_NAME;
        let one = [process(20, g, Some(10)), process(10, "pwsh.exe", Some(1))];
        assert_eq!(walk(&one, 20), Some(10));
        let two = [
            process(30, g, Some(20)),
            process(20, &g.to_uppercase(), Some(10)),
            process(10, "pwsh.exe", Some(1)),
        ];
        assert_eq!(walk(&two, 30), Some(10));
    }

    #[test]
    fn a_chain_that_ends_has_no_caller() {
        let g = SERVICE_EXECUTABLE_NAME;
        let no_parent = [process(20, g, None)];
        assert_eq!(walk(&no_parent, 20), None);
        let vanished = [process(20, g, Some(99))];
        assert_eq!(walk(&vanished, 20), None);
    }

    #[test]
    fn the_walk_is_bounded() {
        let g = SERVICE_EXECUTABLE_NAME;
        let hops = u32::try_from(MAX_ANCESTORS_READ).unwrap();
        let mut chain: Vec<Ancestor> = (0..=hops + 3)
            .map(|pid| process(pid, g, Some(pid + 1)))
            .collect();
        chain.push(process(hops + 4, "pwsh.exe", None));
        assert_eq!(
            walk(&chain, 0),
            None,
            "a long chain must not be walked forever"
        );
        assert_eq!(walk(&chain, hops + 3), Some(hops + 4));
    }

    #[test]
    fn a_git_bash_stub_folds_into_its_shell() {
        // Two commands from one bash: the per-command stub differs, the shells above it do not.
        let g = SERVICE_EXECUTABLE_NAME;
        let shells = [
            process(444_668, "bash.exe", Some(447_552)),
            process(447_552, "bash.exe", Some(100)),
            process(100, "claude.exe", Some(1)),
        ];
        let mut first = shells.to_vec();
        first.push(process(448_400, g, Some(448_356)));
        first.push(process(448_356, "bash.exe", Some(444_668)));
        let mut second = shells.to_vec();
        second.push(process(449_200, g, Some(448_296)));
        second.push(process(448_296, "bash.exe", Some(444_668)));
        let a = walk(&first, 448_400);
        let b = walk(&second, 449_200);
        assert_eq!(a, Some(447_552));
        assert_eq!(a, b, "two commands from one shell are one session");
    }

    fn started(pid: u32, name: &str, parent: Option<u32>, started_at: u64) -> Ancestor {
        Ancestor {
            started_at,
            ..process(pid, name, parent)
        }
    }

    #[test]
    fn a_parent_that_started_after_its_child_is_a_recycled_pid() {
        // The stub's recorded parent pid now names a newer bash that is not its real parent.
        let g = SERVICE_EXECUTABLE_NAME;
        let chain = [
            started(40, g, Some(30), 400),
            started(30, "bash.exe", Some(20), 300),
            started(20, "bash.exe", Some(10), 900),
            started(10, "claude.exe", None, 100),
        ];
        assert_eq!(
            walk(&chain, 40),
            Some(30),
            "the fold stops at the recycled parent"
        );
        let orphan = [
            started(40, g, Some(20), 400),
            started(20, "bash.exe", None, 900),
        ];
        assert_eq!(walk(&orphan, 40), None, "the hop skip stops at it too");
        let equal = [
            started(40, g, Some(20), 400),
            started(20, "bash.exe", None, 400),
        ];
        assert_eq!(walk(&equal, 40), Some(20), "equal start times are allowed");
    }

    #[test]
    fn a_first_parent_that_started_after_this_process_is_a_recycled_pid() {
        // The spawning stub exited and its pid now names an unrelated, newer shell.
        let chain = [started(20, "pwsh.exe", Some(10), 900)];
        assert_eq!(walk_after(&chain, 20, 400), None);
        assert_eq!(
            walk_after(&chain, 20, 900),
            Some(20),
            "equal times are allowed"
        );
        let hop = [
            started(20, SERVICE_EXECUTABLE_NAME, Some(10), 900),
            started(10, "pwsh.exe", None, 100),
        ];
        assert_eq!(
            walk_after(&hop, 20, 400),
            None,
            "a Ghostlight first parent is held to the rule too"
        );
    }

    #[test]
    fn a_direct_terminal_chain_is_unchanged() {
        let chain = [
            process(10, "pwsh.exe", Some(5)),
            process(5, "WindowsTerminal.exe", Some(1)),
        ];
        assert_eq!(walk(&chain, 10), Some(10));
    }

    #[test]
    fn a_forwarding_hop_above_a_stub_is_climbed_then_folded() {
        let g = SERVICE_EXECUTABLE_NAME;
        let chain = [
            process(40, g, Some(30)),
            process(30, "bash.exe", Some(20)),
            process(20, "BASH.EXE", Some(10)),
            process(10, "claude.exe", None),
        ];
        assert_eq!(walk(&chain, 40), Some(20));
    }

    #[test]
    fn nested_same_name_shells_are_one_terminal() {
        let chain = [
            process(30, "bash.exe", Some(20)),
            process(20, "bash.exe", Some(10)),
            process(10, "bash.exe", Some(5)),
            process(5, "tmux.exe", None),
        ];
        assert_eq!(walk(&chain, 30), Some(10));
    }

    #[test]
    fn a_same_name_run_is_bounded_by_the_ancestor_budget() {
        let hops = u32::try_from(MAX_ANCESTORS_READ).unwrap();
        let chain: Vec<Ancestor> = (0..hops + 6)
            .map(|pid| process(pid, "bash.exe", Some(pid + 1)))
            .collect();
        assert_eq!(
            walk(&chain, 0),
            Some(hops - 1),
            "the climb stops at the bound"
        );
    }

    #[test]
    fn the_caller_of_this_process_is_identified() {
        let Some(SessionMarker::Process {
            pid,
            started_at,
            name,
        }) = caller()
        else {
            // A parent that cannot be identified is a supported outcome, not a failure.
            return;
        };
        assert!(pid > 0);
        assert!(
            started_at > 0,
            "a start time is what disambiguates a recycled pid"
        );
        assert!(!name.is_empty());
    }

    #[test]
    fn an_explicit_key_wins_and_is_bounded() {
        // Two markers differing only in pid must not collide, and a declared key must not carry an
        // unbounded string into the workspace registry.
        std::env::set_var(SESSION_VARIABLE, format!("  {}  ", "k".repeat(400)));
        let Some(SessionMarker::Declared { key }) = marker() else {
            panic!("an explicit key must win over the parent process");
        };
        assert_eq!(key.len(), 120);
        std::env::set_var(SESSION_VARIABLE, "   ");
        assert!(
            !matches!(marker(), Some(SessionMarker::Declared { .. })),
            "a blank key is not a key"
        );
        std::env::remove_var(SESSION_VARIABLE);
    }

    #[test]
    fn a_key_identifies_a_process_by_pid_and_start_time() {
        let first = SessionMarker::Process {
            pid: 4312,
            started_at: 100,
            name: "pwsh.exe".into(),
        };
        let recycled = SessionMarker::Process {
            pid: 4312,
            started_at: 200,
            name: "pwsh.exe".into(),
        };
        assert_ne!(
            first.key(),
            recycled.key(),
            "a recycled pid running the same program must not inherit the dead session"
        );
        let renamed = SessionMarker::Process {
            pid: 4312,
            started_at: 100,
            name: "other.exe".into(),
        };
        assert_eq!(
            first.key(),
            renamed.key(),
            "the name is attribution, so it must not change identity"
        );
    }
}
