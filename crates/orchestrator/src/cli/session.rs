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
/// Ghostlight executable, identified by the pair the operating system keeps unique.
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

/// How many forwarding hops the caller walk will climb before it gives up.
///
/// A carrier hands over to the installed command, which may hand over to a development build: two
/// hops in practice. The bound keeps a malformed process tree from looping the walk.
const MAX_FORWARDING_HOPS: usize = 8;

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

/// Climb from `start` past every Ghostlight executable to the first other ancestor.
///
/// A command is forwarded through fresh child processes (a carrier, the installed command, a
/// development build), so the immediate parent is a throwaway Ghostlight process that differs on
/// every call. The caller is the nearest ancestor that is not Ghostlight. `lookup` answers one
/// process by pid. Returns `None` when the chain ends or the hop bound is reached first.
fn nearest_foreign_ancestor(
    start: u32,
    mut lookup: impl FnMut(u32) -> Option<Ancestor>,
) -> Option<Ancestor> {
    let mut next = start;
    for _ in 0..=MAX_FORWARDING_HOPS {
        let process = lookup(next)?;
        if !is_ghostlight(&process.name) {
            return Some(process);
        }
        next = process.parent?;
    }
    None
}

/// The nearest ancestor of this process that is not a Ghostlight executable.
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
    let parent = system.process(me)?.parent()?;
    let found = nearest_foreign_ancestor(parent.as_u32(), |pid| {
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
        caller, marker, nearest_foreign_ancestor, Ancestor, MAX_FORWARDING_HOPS,
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

    fn walk(chain: &[Ancestor], start: u32) -> Option<u32> {
        nearest_foreign_ancestor(start, |pid| chain.iter().find(|p| p.pid == pid).cloned())
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
        let hops = u32::try_from(MAX_FORWARDING_HOPS).unwrap();
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
    fn the_caller_is_this_process_parent() {
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
