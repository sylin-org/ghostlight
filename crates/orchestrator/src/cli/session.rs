//! Who owns a command-line session (ADR-0106).
//!
//! A session is the caller, not the socket. That is what lets a person type one command after
//! another, and an application shell out repeatedly, and have every call reach the same tabs.

use std::env;

use ghostlight_bridge::service::SessionMarker;
use sysinfo::{get_current_pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// The environment variable a caller sets to pin its own session.
///
/// Environment is inherited through intermediaries, so this survives a throwaway shell between a
/// program and `ghostlight call` -- the case where the parent process is the wrong answer because
/// it is a different `cmd.exe` on every invocation.
pub const SESSION_VARIABLE: &str = "GHOSTLIGHT_SESSION";

/// Describe what owns this command's session.
///
/// An explicit key wins when one is set. Otherwise the caller is this process's parent, identified
/// by the pair the operating system keeps unique.
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

/// This process's parent, as pid, start time, and file name.
///
/// Returns `None` when the parent cannot be identified, which leaves the workspace bound to the
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
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[parent]),
        true,
        ProcessRefreshKind::nothing(),
    );
    let process = system.process(parent)?;
    Some(SessionMarker::Process {
        pid: parent.as_u32(),
        started_at: process.start_time(),
        name: process.name().to_string_lossy().chars().take(120).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::{caller, marker, SESSION_VARIABLE};
    use ghostlight_bridge::service::SessionMarker;

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
