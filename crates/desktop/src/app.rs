//! The Ghostlight window as a program (ADR-0199 Decisions 1 and 7, and its amendment of
//! 2026-10-15).
//!
//! The shell opens the `desktop` stream to the running service, which hands it the workbench
//! address, the bearer for the workbench page and, when the service has no tray of its own, the
//! tray to draw. The window is a webview on that address. The shell decides nothing: a tray choice
//! is reported to the service, and a goodbye or a lost stream ends the shell.
//!
//! With a tray, closing the window keeps the process and the tray opens it again. Without one,
//! closing the window ends the shell, so no invisible process stays behind.
//!
//! The pure parts (the tray action, the exit codes) are unit tested. [`run`] is native glue that
//! opens a window, so it is checked live and never in `cargo test`.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Mutex;

use ghostlight_relay::home;
use tao::event::{Event, StartCause, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget};

use crate::link::{open_link, Link, TrayItemId, Workbench};
use crate::session::{self, SessionSignal, SessionState, FALLBACK_EXIT};
use crate::tray::{Tray, TrayEvent};
use crate::webview::{prepare_profile, profile_directory};
use crate::window::{
    self, LifecycleEvent, ShellView, ShellWindow, WindowAct, WindowError, WindowLifecycle,
};

/// The exit code of a session with no display.
pub const EXIT_NO_DISPLAY: u8 = 3;

/// The exit code of a failure that is not a missing display.
const EXIT_FAILURE: u8 = 1;

/// What a tray choice asks the shell to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    /// Show the window.
    OpenWindow,
    /// Tell the service the person chose this entry.
    Report(TrayItemId),
}

/// What a tray event asks for, if anything. A left click on the icon opens the window.
pub fn tray_action(event: &TrayEvent) -> Option<TrayAction> {
    match event {
        TrayEvent::Activate => Some(TrayAction::OpenWindow),
        TrayEvent::Item(id) => match TrayItemId::parse(id)? {
            TrayItemId::Open => Some(TrayAction::OpenWindow),
            item => Some(TrayAction::Report(item)),
        },
    }
}

/// What reaches the event loop from outside it.
#[derive(Debug)]
enum ShellEvent {
    /// The user used the tray.
    Tray(TrayEvent),
    /// The service said goodbye or the stream ended.
    LinkEnded,
    /// The webview's renderer process ended.
    #[cfg(target_os = "linux")]
    RendererLost,
}

fn post(proxy: &EventLoopProxy<ShellEvent>, event: ShellEvent) {
    // A closed loop has nothing left to tell.
    let _ = proxy.send_event(event);
}

/// Handles a signal about the end of the session: on the final one, arms the fallback exit
/// first and then calls `exit`.
fn end_session(state: &SessionState, signal: SessionSignal, exit: impl FnOnce()) {
    session::handle(
        state,
        signal,
        || session::arm_fallback_exit(FALLBACK_EXIT),
        exit,
    );
}

/// The state the event loop carries.
struct Shell {
    lifecycle: WindowLifecycle,
    window: Option<ShellWindow>,
    // Held so the icon stays in the notification area.
    tray: Option<Tray>,
    workbench: Workbench,
    profile: PathBuf,
    link: Link,
    #[cfg(target_os = "linux")]
    proxy: EventLoopProxy<ShellEvent>,
    session: SessionState,
}

impl Shell {
    /// Builds the tray when the service asked for one, installs the signals and opens the window.
    /// Returns whether the shell goes on.
    fn start(&mut self, target: &EventLoopWindowTarget<ShellEvent>) -> bool {
        if let Some(model) = &self.workbench.tray {
            match Tray::build(model) {
                Ok(tray) => self.tray = Some(tray),
                Err(error) => eprintln!("ghostlight-desktop: {error}"),
            }
        }
        #[cfg(target_os = "linux")]
        {
            let state = self.session.clone();
            session::install_unix_signals(move |signal| {
                end_session(&state, signal, || std::process::exit(0))
            });
        }
        self.apply(LifecycleEvent::OpenRequested, target);
        self.tray.is_some() || self.lifecycle.is_open()
    }

    fn apply(&mut self, event: LifecycleEvent, target: &EventLoopWindowTarget<ShellEvent>) {
        match self.lifecycle.on(event) {
            Some(WindowAct::Create) => self.create_window(target),
            Some(WindowAct::Focus) => {
                if let Some(window) = &self.window {
                    window.focus();
                }
            }
            Some(WindowAct::Destroy) => self.window = None,
            None => {}
        }
    }

    fn create_window(&mut self, target: &EventLoopWindowTarget<ShellEvent>) {
        let view = ShellView {
            start_url: self.workbench.url.clone(),
            origin: self.workbench.origin().to_owned(),
            bearer: self.workbench.bearer.clone(),
            profile: self.profile.clone(),
        };
        match ShellWindow::create(target, view) {
            Ok(window) => {
                #[cfg(target_os = "linux")]
                {
                    let proxy = self.proxy.clone();
                    window.on_renderer_lost(move || post(&proxy, ShellEvent::RendererLost));
                }
                window.focus();
                self.window = Some(window);
            }
            Err(error) => {
                eprintln!("ghostlight-desktop: {error}");
                self.apply(LifecycleEvent::Destroyed, target);
            }
        }
    }

    fn report(&self, item: TrayItemId) {
        if let Err(error) = self.link.send_tray(item) {
            eprintln!("ghostlight-desktop: {error}");
        }
    }

    fn handle(
        &mut self,
        event: Event<'_, ShellEvent>,
        target: &EventLoopWindowTarget<ShellEvent>,
        flow: &mut ControlFlow,
    ) {
        *flow = ControlFlow::Wait;
        match event {
            Event::NewEvents(StartCause::Init) => {
                *flow = if self.start(target) {
                    ControlFlow::Wait
                } else {
                    ControlFlow::Exit
                };
            }
            Event::WindowEvent {
                window_id,
                event: WindowEvent::CloseRequested,
                ..
            } if self.window.as_ref().is_some_and(|w| w.id() == window_id) => {
                if self.tray.is_some() {
                    self.apply(LifecycleEvent::CloseRequested, target);
                } else {
                    *flow = ControlFlow::Exit;
                }
            }
            Event::WindowEvent {
                event: WindowEvent::Destroyed,
                ..
            } => self.apply(LifecycleEvent::Destroyed, target),
            Event::UserEvent(ShellEvent::Tray(event)) => match tray_action(&event) {
                Some(TrayAction::OpenWindow) => self.apply(LifecycleEvent::OpenRequested, target),
                Some(TrayAction::Report(item)) => self.report(item),
                None => {}
            },
            Event::UserEvent(ShellEvent::LinkEnded) => *flow = ControlFlow::Exit,
            #[cfg(target_os = "linux")]
            Event::UserEvent(ShellEvent::RendererLost) => {
                self.apply(LifecycleEvent::RendererLost, target);
            }
            Event::LoopDestroyed => {
                // The loop's own teardown would drop the tray and the window, but the exit
                // comes first. Drop them here so Windows removes the notification icon.
                let state = self.session.clone();
                end_session(&state, SessionSignal::Final, || {
                    self.window = None;
                    self.tray = None;
                    std::process::exit(0)
                });
            }
            _ => {}
        }
    }
}

/// Runs the shell: prepares the display, opens the stream to the service, installs the tray when
/// the service asked for one and opens the window. Returns only when the shell cannot start;
/// otherwise the process ends from the event loop.
///
/// With no service, a refused opening or a workbench the shell cannot use, it writes one line to
/// standard error and returns a failure.
pub fn run() -> ExitCode {
    window::apply_renderer_policy();
    window::prepare_identity();
    if let Err(error) = window::init_display() {
        eprintln!("ghostlight-desktop: {error}");
        return ExitCode::from(exit_code_of(&error));
    }
    match prepare_and_open() {
        Ok((link, workbench, profile)) => run_loop(link, workbench, profile),
        Err(failure) => {
            eprintln!("ghostlight-desktop: {failure}");
            ExitCode::from(EXIT_FAILURE)
        }
    }
}

/// Why the shell could not get as far as its window.
#[derive(Debug, thiserror::Error)]
enum StartError {
    /// The Ghostlight home cannot be found.
    #[error("the Ghostlight folder cannot be found: {0}")]
    Home(std::io::Error),
    /// The webview's storage folder cannot be made.
    #[error("the window's storage folder cannot be made: {0}")]
    Profile(std::io::Error),
    /// The service could not be used.
    #[error(transparent)]
    Link(#[from] crate::link::LinkError),
}

fn prepare_and_open() -> Result<(Link, Workbench, PathBuf), StartError> {
    let home = home::home().map_err(StartError::Home)?;
    let profile = profile_directory(&home);
    prepare_profile(&profile).map_err(StartError::Profile)?;
    let (link, workbench) = open_link(&home)?;
    Ok((link, workbench, profile))
}

fn run_loop(link: Link, workbench: Workbench, profile: PathBuf) -> ExitCode {
    let event_loop = EventLoopBuilder::<ShellEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let ended = proxy.clone();
    if let Err(error) = link.spawn_reader(move || post(&ended, ShellEvent::LinkEnded)) {
        eprintln!("ghostlight-desktop: {error}");
        return ExitCode::from(EXIT_FAILURE);
    }
    let tray_proxy = Mutex::new(proxy.clone());
    crate::tray::install_events(move |event| {
        if let Ok(proxy) = tray_proxy.lock() {
            post(&proxy, ShellEvent::Tray(event));
        }
    });
    let mut shell = Shell {
        lifecycle: WindowLifecycle::for_platform(),
        window: None,
        tray: None,
        workbench,
        profile,
        link,
        #[cfg(target_os = "linux")]
        proxy,
        session: SessionState::new(),
    };
    event_loop.run(move |event, target, flow| shell.handle(event, target, flow))
}

fn exit_code_of(error: &WindowError) -> u8 {
    match error {
        WindowError::NoDisplay { .. } => EXIT_NO_DISPLAY,
        WindowError::Native { .. } => EXIT_FAILURE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str) -> TrayEvent {
        TrayEvent::Item(id.to_owned())
    }

    #[test]
    fn open_and_a_left_click_open_the_window() {
        assert_eq!(tray_action(&item("open")), Some(TrayAction::OpenWindow));
        assert_eq!(
            tray_action(&TrayEvent::Activate),
            Some(TrayAction::OpenWindow)
        );
    }

    #[test]
    fn hold_resume_and_quit_are_reported_to_the_service() {
        assert_eq!(
            tray_action(&item("hold")),
            Some(TrayAction::Report(TrayItemId::Hold))
        );
        assert_eq!(
            tray_action(&item("resume")),
            Some(TrayAction::Report(TrayItemId::Resume))
        );
        assert_eq!(
            tray_action(&item("quit")),
            Some(TrayAction::Report(TrayItemId::Quit))
        );
    }

    #[test]
    fn an_entry_this_build_does_not_have_does_nothing() {
        assert_eq!(tray_action(&item("settings")), None);
        assert_eq!(tray_action(&item("")), None);
    }

    #[test]
    fn no_display_is_exit_code_three_and_other_failures_are_one() {
        let no_display = WindowError::NoDisplay {
            detail: String::new(),
        };
        let native = WindowError::Native {
            detail: String::new(),
        };
        assert_eq!(exit_code_of(&no_display), EXIT_NO_DISPLAY);
        assert_eq!(exit_code_of(&native), EXIT_FAILURE);
    }
}
