//! How the service ends on request: [`Quit`] refuses new work, drains what runs and makes the quit
//! signal, and [`Farewell`] decides every session's tabs once and says goodbye on every stream
//! (ADR-0203 Decision 3). The end of the session and the idle exit end through it too
//! (ADR-0199 Decisions 12 and 13).

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use ghostlight_relay::protocol::{GoodbyeReason, QuitReason};

use crate::browser::{BrowserPort, RelayBrowserPort};
use crate::diagnostics::DiagnosticsHub;
use crate::language::outcome::{CancelCause, QuitCause};
use crate::sync::lock;
use crate::workbench::WorkbenchQuitPort;
use crate::workspace::{ReleasedTabs, WorkspaceId, WorkspaceStore};

use super::lifetime::{Drain, END_LIMIT, QUIT_GRACE};
use super::mcp_stream::McpWire;
use super::release::cleanup_released_tabs;
use super::shell::Shells;
use super::{latch, QuitSignal};

/// How the service ends on request: refuse new work, drain what runs, decide every session's tabs
/// and say goodbye ([`Farewell`]), then make the quit signal that lets the process leave.
///
/// A person's Quit also sets the quit latch first, so a browser that restarts does not bring
/// Ghostlight back. A replacement or an uninstall leaves it alone.
#[derive(Clone)]
pub struct Quit {
    pub(super) drain: Arc<Drain>,
    pub(super) signal: Arc<Mutex<Option<QuitSignal>>>,
    diagnostics: Arc<DiagnosticsHub>,
    latch: PathBuf,
    farewell: Arc<Farewell>,
}

/// Make the quit signal, if one is installed. The signal is cloned out of its lock first, so it
/// runs unlocked.
fn fire(signal: &Mutex<Option<QuitSignal>>) {
    let signal = lock(signal).clone();
    if let Some(signal) = signal {
        signal();
    }
}

impl Quit {
    /// A quit that sets `latch` for a person's Quit, decides the tabs of the workspaces in
    /// `workspaces` and closes released ones through `port`, and says goodbye on the MCP streams
    /// it is given and on the streams of `browser`.
    pub(super) fn new(
        diagnostics: Arc<DiagnosticsHub>,
        latch: PathBuf,
        browser: Arc<RelayBrowserPort>,
        workspaces: WorkspaceStore,
        port: Arc<dyn BrowserPort>,
    ) -> Self {
        Self {
            drain: Arc::new(Drain::default()),
            signal: Arc::default(),
            diagnostics,
            latch,
            farewell: Arc::new(Farewell::new(browser, workspaces, port)),
        }
    }

    /// Begin a quit for `reason` unless one is under way: set the latch for a person's Quit,
    /// refuse new work, and once the work in flight has finished or `grace` has passed, end every
    /// session ([`Farewell::end`]) and make the quit signal.
    pub fn begin(&self, reason: QuitReason, grace: Duration) {
        if !self.drain.begin_quit(quit_cause(reason)) {
            return;
        }
        self.farewell.shells.close();
        if reason == QuitReason::Quit {
            if let Err(error) = latch::set(&self.latch) {
                self.diagnostics.sink().emit(
                    ghostlight_bridge::diagnostics::event::PROCESS_FAILED,
                    ghostlight_bridge::diagnostics::Level::Error,
                    None,
                    &format!("the quit latch could not be written: {error}"),
                );
            }
        }
        let drain = Arc::clone(&self.drain);
        let diagnostics = Arc::clone(&self.diagnostics);
        let farewell = Arc::clone(&self.farewell);
        self.end_on_thread("ghostlight-quit-drain", "the quit drain", move || {
            let drained = drain.wait_idle(grace);
            diagnostics.sink().emit(
                ghostlight_bridge::diagnostics::event::PROCESS_EXITED,
                ghostlight_bridge::diagnostics::Level::Info,
                None,
                &format!(
                    "quit: reason={} drained={drained} in_flight={}",
                    goodbye_for(reason).as_str(),
                    drain.in_flight()
                ),
            );
            farewell.end(reason);
        });
    }

    /// Leave because the session is ending: refuse new work, forget every tab without a browser
    /// command, say `session_end` goodbye on every stream, then make the quit signal. Returns at
    /// once, and neither waits for work in flight nor sets the latch, since a sign-out is not a
    /// person's Quit and the caller has already armed the fallback exit that bounds this stop.
    ///
    /// A session end that arrives during a quit still runs: the quit's drain may outlast the
    /// session, and only this path says the `session_end` goodbye the streams must hear before the
    /// fallback exit ends the process.
    pub fn session_end(&self) {
        self.drain.begin_quit(QuitCause::SignOut);
        self.farewell.shells.close();
        let farewell = Arc::clone(&self.farewell);
        self.end_on_thread("ghostlight-session-end", "the session end", move || {
            farewell.end_session();
        });
    }

    /// Run `end` on a thread of its own named `name`, then make the quit signal. A thread that
    /// cannot start is one diagnostics line naming `what`.
    fn end_on_thread(&self, name: &str, what: &str, end: impl FnOnce() + Send + 'static) {
        let signal = Arc::clone(&self.signal);
        let spawned = thread::Builder::new().name(name.into()).spawn(move || {
            end();
            fire(&signal);
        });
        if spawned.is_err() {
            self.diagnostics.sink().emit(
                ghostlight_bridge::diagnostics::event::PROCESS_FAILED,
                ghostlight_bridge::diagnostics::Level::Error,
                None,
                &format!("{what} could not start"),
            );
        }
    }

    /// Leave because no stream of any role was open for the idle limit: refuse new work, forget
    /// every tab without a browser command (no browser stream is open) and make the quit signal.
    /// Returns at once, and neither says goodbye (no stream is open) nor sets the latch, so the
    /// next edge, command or person starts the service again for its own reason. A quit or a
    /// session end already under way is left to finish alone.
    pub(super) fn idle_exit(&self, idle: Duration) {
        if !self.drain.begin_quit(QuitCause::Idle) {
            return;
        }
        self.farewell.shells.close();
        self.farewell.forget_every_session();
        self.diagnostics.sink().emit(
            ghostlight_bridge::diagnostics::event::IDLE_EXIT,
            ghostlight_bridge::diagnostics::Level::Info,
            None,
            &format!("no stream of any role for {} s", idle.as_secs()),
        );
        fire(&self.signal);
    }

    /// Record `wire` as a live MCP stream until the returned guard is dropped, so a quit hands it
    /// its last token and says goodbye on it.
    pub(super) fn enroll(&self, wire: Arc<McpWire>) -> Enrolled {
        self.farewell.enroll(wire)
    }

    /// Release the workspace of a session whose stream closed, unless a service end has taken
    /// over every session; that end then decides the workspace.
    pub(super) fn release_on_close(&self, workspace: &WorkspaceId) {
        self.farewell.release_on_close(workspace);
    }

    /// The Ghostlight shells attached to this service, which a quit says goodbye to and whose
    /// waiting acts it releases.
    pub(super) fn shells(&self) -> Arc<Shells> {
        Arc::clone(&self.farewell.shells)
    }
}

impl WorkbenchQuitPort for Quit {
    fn quit(&self) {
        self.begin(QuitReason::Quit, QUIT_GRACE);
    }
}

/// What a call that a quit for `reason` cancels is told about it.
fn quit_cause(reason: QuitReason) -> QuitCause {
    match reason {
        QuitReason::Quit => QuitCause::Quit,
        QuitReason::Replace => QuitCause::Replace,
        QuitReason::Uninstall => QuitCause::Uninstall,
        QuitReason::Unknown => QuitCause::Unknown,
    }
}

/// The goodbye a quit for `reason` sends.
fn goodbye_for(reason: QuitReason) -> GoodbyeReason {
    match reason {
        QuitReason::Quit => GoodbyeReason::Quit,
        QuitReason::Replace => GoodbyeReason::Replace,
        QuitReason::Uninstall => GoodbyeReason::Uninstall,
        QuitReason::Unknown => GoodbyeReason::Unknown,
    }
}

/// How every service end decides its sessions' tabs, once (ADR-0203 Decision 3), and the live
/// streams it says goodbye on before the service closes them.
///
/// Until an end takes over, a session whose stream closes releases its own workspace. An end takes
/// over before its first goodbye, and from then on it alone decides every workspace, so no release
/// races the goodbye or the process exit.
struct Farewell {
    state: Mutex<FarewellState>,
    /// Told when a release that a closing session began has finished.
    closed: Condvar,
    next: AtomicU64,
    browser: Arc<RelayBrowserPort>,
    shells: Arc<Shells>,
    workspaces: WorkspaceStore,
    /// The port released tabs are closed through.
    port: Arc<dyn BrowserPort>,
}

/// What a service end reads under one lock: the live MCP streams, whether an end took over, and
/// how many releases closing sessions began before it did.
#[derive(Default)]
struct FarewellState {
    streams: BTreeMap<u64, Arc<McpWire>>,
    taken_over: bool,
    releasing: usize,
}

impl Farewell {
    fn new(
        browser: Arc<RelayBrowserPort>,
        workspaces: WorkspaceStore,
        port: Arc<dyn BrowserPort>,
    ) -> Self {
        Self {
            state: Mutex::default(),
            closed: Condvar::new(),
            next: AtomicU64::new(0),
            browser,
            shells: Arc::default(),
            workspaces,
            port,
        }
    }

    /// Record one MCP stream until the guard is dropped.
    fn enroll(self: &Arc<Self>, wire: Arc<McpWire>) -> Enrolled {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        lock(&self.state).streams.insert(id, wire);
        Enrolled {
            farewell: Arc::clone(self),
            id,
        }
    }

    /// Take every session over for this end and return the live MCP streams. From now on a
    /// closing session releases nothing.
    fn take_over(&self) -> Vec<Arc<McpWire>> {
        let mut state = lock(&self.state);
        state.taken_over = true;
        state.streams.values().cloned().collect()
    }

    /// Release `workspace` and close its tabs, unless an end has taken over.
    fn release_on_close(&self, workspace: &WorkspaceId) {
        {
            let mut state = lock(&self.state);
            if state.taken_over {
                return;
            }
            state.releasing += 1;
        }
        let released = self.workspaces.release(workspace);
        cleanup_released_tabs(&released, self.port.as_ref(), &self.workspaces, None);
        lock(&self.state).releasing -= 1;
        self.closed.notify_all();
    }

    /// End every session for a quit with `reason`, in this order and under one [`END_LIMIT`]:
    ///
    /// 1. each MCP stream is handed its last resume token, which carries the session's tabs only
    ///    when its workspace stays bound;
    /// 2. the MCP and shell goodbyes;
    /// 3. the release of every workspace that ends: a command-line workspace at every end, and an
    ///    MCP workspace unless a replacement keeps it bound for its live edge's resume. The work of
    ///    each session that ends is cancelled first, and a workspace a call still holds at the
    ///    deadline is left as it is;
    /// 4. the browser goodbye.
    fn end(&self, reason: QuitReason) {
        let until = Instant::now() + END_LIMIT;
        let goodbye = goodbye_for(reason);
        let streams = self.take_over();
        let kept: HashSet<WorkspaceId> = if reason == QuitReason::Replace {
            streams
                .iter()
                .filter_map(|wire| wire.workspace())
                .filter(|workspace| !self.workspaces.is_owned(workspace))
                .collect()
        } else {
            HashSet::new()
        };
        for wire in &streams {
            let keeps_tabs = wire
                .workspace()
                .is_some_and(|workspace| kept.contains(&workspace));
            wire.hand_last_resume(&self.workspaces, self.port.as_ref(), keeps_tabs);
        }
        for wire in &streams {
            wire.goodbye(goodbye);
        }
        self.shells.say_goodbye(goodbye);
        self.release_all_but(&kept, &streams, until, quit_cause(reason));
        self.browser.say_goodbye(goodbye);
    }

    /// End every session because the session is ending: forget every tab without a browser
    /// command, then say `session_end` on every stream.
    fn end_session(&self) {
        let streams = self.take_over();
        self.workspaces.forget_all();
        for wire in &streams {
            wire.goodbye(GoodbyeReason::SessionEnd);
        }
        self.shells.say_goodbye(GoodbyeReason::SessionEnd);
        self.browser.say_goodbye(GoodbyeReason::SessionEnd);
    }

    /// End every session when no stream is open to say goodbye on: forget every tab without a
    /// browser command.
    fn forget_every_session(&self) {
        self.take_over();
        self.workspaces.forget_all();
    }

    /// Release every workspace but `kept` and close its confirmed tabs before `until`, then wait
    /// until then for the releases that closing sessions began before the end took over.
    ///
    /// The work of each session that ends is cancelled first, for `quit`, so a running call can
    /// finish and give its workspace up. Workspaces no call holds are released before any is
    /// waited for.
    fn release_all_but(
        &self,
        kept: &HashSet<WorkspaceId>,
        streams: &[Arc<McpWire>],
        until: Instant,
        quit: QuitCause,
    ) {
        let ending: Vec<WorkspaceId> = self
            .workspaces
            .ids()
            .into_iter()
            .filter(|workspace| !kept.contains(workspace))
            .collect();
        for live in streams.iter().filter_map(|wire| wire.live.get()) {
            if ending.contains(&live.workspace) {
                live.queue.stop(CancelCause::Quit(quit));
            }
        }
        let mut held = Vec::new();
        for workspace in ending {
            match self.workspaces.release_unleased(&workspace, Instant::now()) {
                Some(released) => self.close_released(&released, until),
                None => held.push(workspace),
            }
        }
        for workspace in held {
            if let Some(released) = self.workspaces.release_unleased(&workspace, until) {
                self.close_released(&released, until);
            }
        }
        let state = lock(&self.state);
        let wait = until.saturating_duration_since(Instant::now());
        let _ = self
            .closed
            .wait_timeout_while(state, wait, |state| state.releasing > 0)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
    }

    /// Close the tabs `released` names before `until`.
    fn close_released(&self, released: &ReleasedTabs, until: Instant) {
        cleanup_released_tabs(released, self.port.as_ref(), &self.workspaces, Some(until));
    }
}

/// One MCP stream recorded for the goodbye. Dropping it forgets the stream.
pub(super) struct Enrolled {
    farewell: Arc<Farewell>,
    id: u64,
}

impl Drop for Enrolled {
    fn drop(&mut self) {
        lock(&self.farewell.state).streams.remove(&self.id);
    }
}
