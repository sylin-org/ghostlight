//! How released tabs are cleaned up: a released workspace's browser diagnostics are cleared before
//! its tabs are offered to the close interlock, and the sessions whose owning process is gone are
//! reaped.

use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use crate::browser::adapter::BrowserCommand;
use crate::browser::{BrowserError, BrowserPort};
use crate::intake::session::SessionMarker;
use crate::workspace::{ReleasedTabs, WorkspaceStore};

use super::DIAGNOSTIC_CLEAR_BATCH_SIZE;

/// The longest one release command may take: a diagnostics clear or a tab close.
const RELEASE_CALL_LIMIT: Duration = Duration::from_secs(2);

/// Close the tabs of every session whose owning process is gone.
pub(super) fn reap_finished_sessions(workspaces: &WorkspaceStore, browser: &dyn BrowserPort) {
    for released in workspaces.reap(&owner_alive) {
        cleanup_released_tabs(&released, browser, workspaces, None);
    }
}

/// Erase service-owned browser diagnostics before released tabs are offered to the close interlock,
/// then settle each tab's place in the released pool from its close answer.
///
/// Cleanup goes to the browser that holds the tabs and nowhere else. A workspace that never opened
/// a browser has nothing to clean up. A tab becomes reusable only when the person's preserve-tabs
/// setting refused its close: the browser then already dropped the dead workspace's binding and
/// debugger hold and keeps the tab open. Until its close answers, no other session can claim it,
/// so this close can never act on a tab another session took (ADR-0196 Decisions 8 and 9).
///
/// Each command has [`RELEASE_CALL_LIMIT`]. A service end also bounds the whole cleanup by
/// `until`: a command that would start after it is not sent, and its tab leaves the pool as one
/// whose close did not answer.
pub(super) fn cleanup_released_tabs(
    released: &ReleasedTabs,
    browser: &dyn BrowserPort,
    workspaces: &WorkspaceStore,
    until: Option<Instant>,
) {
    let Some(target) = released.browser.as_deref() else {
        return;
    };
    let call_deadline = || {
        let now = Instant::now();
        let deadline = now + RELEASE_CALL_LIMIT;
        match until {
            Some(until) if until <= now => None,
            Some(until) => Some(deadline.min(until)),
            None => Some(deadline),
        }
    };
    let cancelled = AtomicBool::new(false);
    for tab_ids in released.physical_ids.chunks(DIAGNOSTIC_CLEAR_BATCH_SIZE) {
        let Some(deadline) = call_deadline() else {
            break;
        };
        let _ = browser.call(
            target,
            BrowserCommand::ClearDiagnostics {
                tab_ids: tab_ids.to_vec(),
            },
            deadline,
            &cancelled,
        );
    }
    for &tab_id in &released.physical_ids {
        let kept_open = call_deadline().is_some_and(|deadline| {
            let closed = browser.call(
                target,
                BrowserCommand::CloseTab {
                    tab_id,
                    released: true,
                },
                deadline,
                &cancelled,
            );
            matches!(closed, Err(BrowserError::LocalInterlock(_)))
        });
        workspaces.settle_released(target, tab_id, kept_open);
    }
}

/// Whether the process that owns a session is still running.
///
/// A declared key names no process, so it lives until the service exits, by a quit or by the idle
/// exit ([`idle`](super::idle)). The workspace registry is in memory, so the key goes with the service.
fn owner_alive(marker: &SessionMarker) -> bool {
    let SessionMarker::Process {
        pid, started_at, ..
    } = marker
    else {
        return true;
    };
    let Ok(pid) = usize::try_from(*pid).map(sysinfo::Pid::from) else {
        return false;
    };
    let mut system = sysinfo::System::new();
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::Some(&[pid]),
        true,
        sysinfo::ProcessRefreshKind::nothing(),
    );
    // The start time is the half that matters: a recycled pid is a different owner.
    system
        .process(pid)
        .is_some_and(|process| process.start_time() == *started_at)
}
