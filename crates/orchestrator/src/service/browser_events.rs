//! What a connected browser reports, applied to the workspaces and the workbench, and the reveal of
//! the workbench off the thread that asked for it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use crate::browser::adapter::BrowserEvent;
use crate::browser::BrowserEventSink;
use crate::language::outcome::DialogKind;
use crate::workbench::{PageAddress, RuntimeSource, WorkbenchFacade};
use crate::workspace::WorkspaceStore;

/// The name of the short-lived thread that carries out a request to show the window.
const SHOW_REQUEST_THREAD: &str = "ghostlight-show-request";

/// Applies what a connected browser reports: its tabs, its dialogs and its toolbar's pause, and
/// tells the workbench when who is connected may have changed.
pub(super) struct ServiceBrowserEvents {
    workbench: WorkbenchFacade,
    workspaces: WorkspaceStore,
    /// Whether a show request is being carried out. One request in flight absorbs the presses
    /// that arrive meanwhile.
    pub(super) showing: Arc<AtomicBool>,
}

impl ServiceBrowserEvents {
    pub(super) fn new(workbench: WorkbenchFacade, workspaces: WorkspaceStore) -> Self {
        Self {
            workbench,
            workspaces,
            showing: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Carry out the browser's request to be shown without holding the browser's reader.
    ///
    /// Revealing can wait for a shell to start and attach, and the reader that delivered the
    /// request also delivers the receipts of work in flight on that browser. The answer is not
    /// read: with nothing to show, or nothing to show it, the person is no worse off. While setup
    /// is incomplete the window opens at the setup view.
    fn request_show(&self) {
        reveal_once(
            &self.workbench,
            self.workbench.show_address(),
            &self.showing,
        );
    }
}

/// Reveal the workbench at `address` off the calling thread, unless a reveal guarded by
/// `in_flight` is already under way. One reveal in flight absorbs the requests that arrive
/// meanwhile, so repeated presses do not stack threads or windows.
pub(super) fn reveal_once(
    workbench: &WorkbenchFacade,
    address: Option<PageAddress>,
    in_flight: &Arc<AtomicBool>,
) {
    if in_flight.swap(true, Ordering::SeqCst) {
        return;
    }
    let done = Arc::clone(in_flight);
    let spawned = reveal_off_thread(workbench.clone(), address, move || {
        done.store(false, Ordering::SeqCst);
    });
    if spawned.is_err() {
        in_flight.store(false, Ordering::SeqCst);
    }
}

/// Reveal the workbench at `address` on a short-lived thread, then run `finished`.
///
/// Revealing can wait for a shell to start and attach, so a reader that must keep reading never
/// does it inline. The answer is not read: with nothing to show, or nothing to show it, the
/// person is no worse off. `finished` does not run when the thread cannot be started.
fn reveal_off_thread(
    workbench: WorkbenchFacade,
    address: Option<PageAddress>,
    finished: impl FnOnce() + Send + 'static,
) -> std::io::Result<()> {
    thread::Builder::new()
        .name(SHOW_REQUEST_THREAD.into())
        .spawn(move || {
            let _ = workbench.reveal(address);
            finished();
        })
        .map(|_| ())
}

impl BrowserEventSink for ServiceBrowserEvents {
    fn on_event(&self, browser: &str, event: BrowserEvent) {
        match event {
            BrowserEvent::DocumentCommitted { tab_id, url } => {
                // A commit is browser state, not proof of a Ghostlight action. Never authorize,
                // audit, or react to it as work. Only stale our own references; the executor
                // checks the current destination when an agent actually requests an operation.
                self.workspaces.note_browser_landing(browser, tab_id, &url);
            }
            BrowserEvent::ReadinessChanged { tab_id, readiness } => self
                .workspaces
                .apply_browser_readiness(browser, tab_id, readiness),
            BrowserEvent::RuntimeControlRequested { intent } => {
                self.workbench
                    .apply_runtime_control(intent, RuntimeSource::Browser);
            }
            BrowserEvent::ShowRequested => self.request_show(),
            BrowserEvent::TabClosed { tab_id } => {
                self.workspaces.apply_browser_close(browser, tab_id);
                self.workbench.refresh_live_parts();
            }
            // A browser that comes back may have restarted and reused its tab ids, so every tab
            // binding it held waits for it to confirm it.
            BrowserEvent::Disconnected { life } => {
                self.workspaces
                    .apply_browser_disconnect(browser, life.as_deref());
                self.workbench.refresh_live_parts();
            }
            // Attention never reaches this sink: it is browser routing state, and the port that
            // knows which connection reported it is the only thing that needs it.
            BrowserEvent::Attended => {}
            BrowserEvent::DialogChanged {
                tab_id,
                present,
                dialog_type,
            } => self.workspaces.apply_browser_dialog(
                browser,
                tab_id,
                present.then(|| DialogKind::classify(&dialog_type)),
            ),
        }
    }
}
