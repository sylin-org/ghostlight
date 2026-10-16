//! The Ghostlight window as a program (ADR-0199 Decisions 1, 7, 10, 11 and 14, and its amendment
//! of 2026-10-15).
//!
//! The shell opens the `desktop` stream to the running service, which hands it the workbench
//! address, the bearer for the workbench page and, when the service has no tray of its own, the
//! tray to draw. The window is a webview on that address. The shell decides nothing about
//! Ghostlight: a tray choice is reported to the service, and what the service asks afterwards is
//! carried out here and on the event-loop thread: show the window (at a section of the page), show
//! a notice, open a link or a folder, pick a file. Each act is answered with its result.
//!
//! The shell's own life is decided by [`Machine`](crate::machine::Machine), a pure state machine.
//! This module is its glue: it carries out the machine's effects (open the stream on a worker
//! thread, show a screen, exit), feeds it what happens (the stream opened or ended, the time came,
//! a button was pressed) and answers its questions from the real world. A screen is either the
//! workbench or one of the shell's four states (`states`). A new workbench frame, after a service
//! restart, rebuilds the window's webview with the new origin and bearer.
//!
//! With a tray, closing the window keeps the process and the tray opens it again; the tray's
//! tooltip says the state while there is no workbench, and only Open can be chosen. Without one,
//! losing the window ends the shell once it has attached, whether the person closed it or the
//! renderer failed, so no invisible process stays behind.
//!
//! One listener owns the end of the session (`session`). Its callback arms the fallback exit
//! first and posts one event, and the shell then stops. Nothing starts once the session is ending.
//!
//! The pure parts (the tray action, the exit rule) are unit tested. [`run`] is native glue that
//! opens a window, so it is checked live and never in `cargo test`.

use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::Mutex;
use std::thread;
use std::time::Instant;

use ghostlight_relay::os::session::SessionListener;
use ghostlight_relay::protocol::StartReason;
use ghostlight_relay::{home, os, spawn};
use tao::event::{Event, StartCause, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget};

use crate::link::{open_link, ActOutcome, DesktopFrame, Link, LinkEnd, LinkError, NativeAct};
use crate::link::{TrayItemId, Workbench};
use crate::machine::{self, Answer, Effect, Image, Machine, Outside, Screen};
use crate::native::{self, ThisComputer};
use crate::session::{self, Session, ThisSession};
use crate::states::{StateButton, StateScreen};
use crate::tray::{Tray, TrayEvent, TrayModel};
use crate::webview::{prepare_profile, profile_directory};
use crate::window::{
    self, Callbacks, Content, LifecycleEvent, ShellWindow, WindowAct, WindowError, WindowLifecycle,
    WindowState,
};
use crate::{Launch, UPDATING_ARGUMENT};

/// The exit code of a session with no display.
pub const EXIT_NO_DISPLAY: u8 = 3;

/// The exit code of a failure that is not a missing display.
const EXIT_FAILURE: u8 = 1;

/// The file name of the shell in `bin`.
#[cfg(windows)]
const SHELL_EXECUTABLE_NAME: &str = "ghostlight-desktop.exe";

/// The file name of the shell in `bin`.
#[cfg(not(windows))]
const SHELL_EXECUTABLE_NAME: &str = "ghostlight-desktop";

/// The name of the thread that opens the stream to the service.
const OPEN_THREAD_NAME: &str = "ghostlight-desktop-open";

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

/// Whether the shell has a reason to keep running. A tray can always open the window again, and a
/// shell that has not attached yet has not had its first chance to show anything. Without a tray,
/// a shell whose window is gone has nothing left for a person to use, and it would hold the
/// service's desktop place while invisible, so it ends.
fn goes_on(has_tray: bool, window: WindowState, attached_before: bool) -> bool {
    has_tray || window != WindowState::Absent || !attached_before
}

/// What reaches the event loop from outside it.
enum ShellEvent {
    /// The user used the tray.
    Tray(TrayEvent),
    /// An attempt to open the stream ended.
    Opened(Result<(Link, Workbench), LinkError>),
    /// The service asked for something: to show the window, a notice, or an act.
    Service {
        /// The stream it came on.
        generation: u64,
        /// What it asked.
        request: DesktopFrame,
    },
    /// A stream ended.
    LinkEnded {
        /// The stream.
        generation: u64,
        /// How it ended.
        end: LinkEnd,
    },
    /// An act the service asked for has ended, possibly long after it was carried out: a file
    /// dialog ends when the person decides.
    ActDone {
        /// The stream it was asked on.
        generation: u64,
        /// The act, as the service numbered it.
        id: u64,
        /// How it ended.
        outcome: ActOutcome,
    },
    /// A button of a state screen was pressed.
    Pressed(StateButton),
    /// The session is ending.
    SessionEnding,
    /// The webview's renderer process ended.
    #[cfg(target_os = "linux")]
    RendererLost,
}

fn post(proxy: &EventLoopProxy<ShellEvent>, event: ShellEvent) {
    // A closed loop has nothing left to tell.
    let _ = proxy.send_event(event);
}

/// The questions the machine asks of the real world.
struct ThisWorld {
    session: ThisSession,
}

impl Outside for ThisWorld {
    fn may_start_process(&self) -> bool {
        self.session.may_start_process()
    }

    fn start_service(&mut self, reason: StartReason) -> std::io::Result<()> {
        spawn::start_service(reason)
    }

    fn relaunch_updating(&mut self) -> std::io::Result<()> {
        // The path comes from the Ghostlight home, never from this process's own image: this image
        // may be the one the deploy moved aside.
        os::spawn_detached(
            &home::bin()?.join(SHELL_EXECUTABLE_NAME),
            &[OsStr::new(UPDATING_ARGUMENT)],
            &home::home()?,
        )
        .map(|_| ())
    }

    fn bin_image(&self) -> Option<Image> {
        Image::of(&home::bin().ok()?.join(SHELL_EXECUTABLE_NAME))
    }
}

/// The state the event loop carries.
struct Shell {
    machine: Machine,
    world: ThisWorld,
    lifecycle: WindowLifecycle,
    window: Option<ShellWindow>,
    // Held so the icon stays in the notification area.
    tray: Option<Tray>,
    /// The tray the current service described, whatever the screen.
    tray_model: Option<TrayModel>,
    workbench: Option<Workbench>,
    link: Option<Link>,
    /// Names the stream in `link`, so a late event of an older stream is ignored.
    generation: u64,
    home: PathBuf,
    profile: PathBuf,
    /// The section the service asked the window to open at, until a window shows it.
    requested: Option<String>,
    /// Reaches the event loop from callbacks that end after the call that started them.
    proxy: EventLoopProxy<ShellEvent>,
    session: ThisSession,
    /// Set when the shell is to end.
    exiting: bool,
    // Held so the shell keeps hearing the end of the session.
    _listener: Option<SessionListener>,
}

impl Shell {
    /// Whether the shell still has a tray or a window, or has yet to attach.
    fn goes_on(&self) -> bool {
        goes_on(
            self.tray.is_some(),
            self.lifecycle.state(),
            self.machine.has_attached(),
        )
    }

    /// Feeds the machine one event and carries out what it answers.
    fn feed(&mut self, event: machine::Event, target: &EventLoopWindowTarget<ShellEvent>) {
        let effects = self.machine.on(event, Instant::now(), &mut self.world);
        self.dispatch(effects, target);
    }

    fn dispatch(&mut self, effects: Vec<Effect>, target: &EventLoopWindowTarget<ShellEvent>) {
        for effect in effects {
            match effect {
                Effect::Open => self.open_stream(),
                Effect::Show { screen, present } => self.show_screen(screen, present, target),
                Effect::Exit => self.exiting = true,
            }
        }
    }

    /// Tries to open the stream on a thread of its own, since the attempt may wait for the
    /// service for several seconds.
    fn open_stream(&self) {
        let home = self.home.clone();
        let proxy = self.proxy.clone();
        let spawned = thread::Builder::new()
            .name(OPEN_THREAD_NAME.to_owned())
            .spawn(move || post(&proxy, ShellEvent::Opened(open_link(&home))));
        if let Err(error) = spawned {
            eprintln!("ghostlight-desktop: {error}");
            post(&self.proxy, ShellEvent::Opened(Err(LinkError::Io(error))));
        }
    }

    /// The stream opened and handed over a workbench.
    fn attach(
        &mut self,
        link: Link,
        workbench: Workbench,
        target: &EventLoopWindowTarget<ShellEvent>,
    ) {
        self.generation += 1;
        let generation = self.generation;
        let ended = self.proxy.clone();
        let asked = self.proxy.clone();
        let reading = link.spawn_reader(
            move |request| {
                post(
                    &asked,
                    ShellEvent::Service {
                        generation,
                        request,
                    },
                )
            },
            move |end| post(&ended, ShellEvent::LinkEnded { generation, end }),
        );
        if let Err(error) = reading {
            eprintln!("ghostlight-desktop: {error}");
            self.feed(machine::Event::Opened(Answer::NoAnswer), target);
            return;
        }
        self.set_tray_model(workbench.tray.clone());
        self.workbench = Some(workbench);
        self.link = Some(link);
        self.feed(machine::Event::Opened(Answer::Attached), target);
    }

    /// Adopts the tray the service described: builds the icon when there is none, and drops it
    /// when the service no longer asks for one.
    fn set_tray_model(&mut self, model: Option<TrayModel>) {
        match &model {
            Some(model) if self.tray.is_none() => match Tray::build(model) {
                Ok(tray) => self.tray = Some(tray),
                Err(error) => eprintln!("ghostlight-desktop: {error}"),
            },
            Some(_) => {}
            None => self.tray = None,
        }
        self.tray_model = model;
    }

    /// Makes the tray say what the shell shows.
    fn sync_tray(&self) {
        let (Some(tray), Some(model)) = (&self.tray, &self.tray_model) else {
            return;
        };
        let shown = match self.machine.screen() {
            Some(Screen::State(state)) => model.while_without_service(state.words()),
            Some(Screen::Workbench) | None => model.clone(),
        };
        if let Err(error) = tray.show(&shown) {
            eprintln!("ghostlight-desktop: {error}");
        }
    }

    /// What the window shows for `screen`, or `None` for a workbench the service has not handed
    /// over.
    fn content_for(&self, screen: Screen) -> Option<Content> {
        match (screen, &self.workbench) {
            (Screen::State(state), _) => Some(Content::State(state)),
            (Screen::Workbench, Some(workbench)) => Some(Content::Workbench {
                start_url: native::start_url(&workbench.url, self.requested.as_deref()),
                origin: workbench.origin().to_owned(),
                bearer: workbench.bearer.clone(),
            }),
            (Screen::Workbench, None) => None,
        }
    }

    /// Shows `screen`, opening the window first when `present` and it is not open.
    fn show_screen(
        &mut self,
        screen: Screen,
        present: bool,
        target: &EventLoopWindowTarget<ShellEvent>,
    ) {
        self.sync_tray();
        if self.window.is_some() {
            let Some(content) = self.content_for(screen) else {
                return;
            };
            let is_workbench = matches!(content, Content::Workbench { .. });
            let shown = self.window.as_mut().map(|window| window.show(content));
            match shown {
                Some(Ok(())) if is_workbench => self.requested = None,
                Some(Err(error)) => {
                    eprintln!("ghostlight-desktop: {error}");
                    self.window = None;
                    self.apply(LifecycleEvent::Destroyed, target);
                }
                Some(Ok(())) | None => {}
            }
        } else if present && self.lifecycle.state() == WindowState::Absent {
            self.apply(LifecycleEvent::OpenRequested, target);
        }
    }

    fn apply(&mut self, event: LifecycleEvent, target: &EventLoopWindowTarget<ShellEvent>) {
        match self.lifecycle.on(event) {
            Some(WindowAct::Create) => self.create_window(target),
            Some(WindowAct::Focus) => {
                if let Some(window) = &self.window {
                    window.focus();
                    if let Some(address) = self.requested.take() {
                        window.go_to(&address);
                    }
                }
            }
            Some(WindowAct::Destroy) => self.window = None,
            None => {}
        }
    }

    fn create_window(&mut self, target: &EventLoopWindowTarget<ShellEvent>) {
        if let Err(error) = prepare_profile(&self.profile) {
            eprintln!("ghostlight-desktop: the window's storage folder cannot be made: {error}");
            self.apply(LifecycleEvent::Destroyed, target);
            return;
        }
        let content = self
            .machine
            .screen()
            .and_then(|screen| self.content_for(screen))
            .unwrap_or(Content::State(StateScreen::Starting));
        let is_workbench = matches!(content, Content::Workbench { .. });
        let pressed = self.proxy.clone();
        #[cfg(target_os = "linux")]
        let lost = self.proxy.clone();
        let callbacks = Callbacks {
            pressed: Rc::new(move |button| post(&pressed, ShellEvent::Pressed(button))),
            #[cfg(target_os = "linux")]
            renderer_lost: Rc::new(move || post(&lost, ShellEvent::RendererLost)),
        };
        match ShellWindow::create(target, self.profile.clone(), content, callbacks) {
            Ok(window) => {
                if is_workbench {
                    self.requested = None;
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

    /// Brings the window forward, opening it first when it is not open, at `address` when the
    /// service gave a safe one.
    fn show(&mut self, address: Option<String>, target: &EventLoopWindowTarget<ShellEvent>) {
        if let Some(address) = address.filter(|address| native::is_safe_address(address)) {
            self.requested = Some(address);
        }
        self.apply(LifecycleEvent::OpenRequested, target);
    }

    /// Carries out what the service asked for.
    fn serve(&mut self, request: DesktopFrame, target: &EventLoopWindowTarget<ShellEvent>) {
        match request {
            DesktopFrame::Show { address } => self.show(address, target),
            DesktopFrame::Notice { title, body } => native::show_notice(title, body),
            DesktopFrame::Act { id, act } => {
                let computer = ThisComputer {
                    parent: self.window.as_ref().map(ShellWindow::native_window),
                };
                let proxy = self.proxy.clone();
                let generation = self.generation;
                native::carry_out(
                    &act,
                    &self.session,
                    &computer,
                    Box::new(move |outcome| {
                        post(
                            &proxy,
                            ShellEvent::ActDone {
                                generation,
                                id,
                                outcome,
                            },
                        );
                    }),
                );
            }
            _ => {}
        }
    }

    fn report(&self, item: TrayItemId) {
        let Some(link) = &self.link else { return };
        if let Err(error) = link.send_tray(item) {
            eprintln!("ghostlight-desktop: {error}");
        }
    }

    /// A button of a state screen was pressed.
    fn pressed(&mut self, button: StateButton, target: &EventLoopWindowTarget<ShellEvent>) {
        match button {
            StateButton::Start => self.feed(machine::Event::StartPressed, target),
            StateButton::Diagnostics => self.show_diagnostics(),
        }
    }

    /// Opens the folder of diagnostics, or the Ghostlight folder when diagnostics never ran.
    fn show_diagnostics(&self) {
        let folder = home::diagnostics_dir()
            .ok()
            .filter(|folder| folder.is_dir())
            .unwrap_or_else(|| self.home.clone());
        let Some(path) = folder.to_str() else { return };
        let computer = ThisComputer {
            parent: self.window.as_ref().map(ShellWindow::native_window),
        };
        native::carry_out(
            &NativeAct::OpenFolder {
                path: path.to_owned(),
            },
            &self.session,
            &computer,
            Box::new(|_| {}),
        );
    }

    fn handle(
        &mut self,
        event: Event<'_, ShellEvent>,
        target: &EventLoopWindowTarget<ShellEvent>,
        flow: &mut ControlFlow,
    ) {
        match event {
            Event::NewEvents(StartCause::Init) => self.feed(machine::Event::Started, target),
            Event::WindowEvent {
                window_id,
                event: WindowEvent::CloseRequested,
                ..
            } if self.window.as_ref().is_some_and(|w| w.id() == window_id) => {
                if self.tray.is_some() {
                    self.apply(LifecycleEvent::CloseRequested, target);
                } else {
                    self.exiting = true;
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
            Event::UserEvent(ShellEvent::Opened(opened)) => match opened {
                Ok((link, workbench)) => self.attach(link, workbench, target),
                Err(error) => self.feed(machine::Event::Opened(Answer::from(&error)), target),
            },
            Event::UserEvent(ShellEvent::Service {
                generation,
                request,
            }) if generation == self.generation => self.serve(request, target),
            Event::UserEvent(ShellEvent::LinkEnded { generation, end })
                if generation == self.generation =>
            {
                self.link = None;
                let window_open = self.lifecycle.is_open();
                self.feed(machine::Event::Ended { end, window_open }, target);
            }
            Event::UserEvent(ShellEvent::ActDone {
                generation,
                id,
                outcome,
            }) if generation == self.generation => {
                if let Some(link) = &self.link {
                    if let Err(error) = link.send_act_result(id, outcome) {
                        eprintln!("ghostlight-desktop: {error}");
                    }
                }
            }
            Event::UserEvent(ShellEvent::Pressed(button)) => self.pressed(button, target),
            Event::UserEvent(ShellEvent::SessionEnding) => self.exiting = true,
            #[cfg(target_os = "linux")]
            Event::UserEvent(ShellEvent::RendererLost) => {
                self.apply(LifecycleEvent::RendererLost, target);
            }
            Event::LoopDestroyed => {
                // The loop's own teardown would drop the tray and the window, but the process
                // ends first. Drop them here so Windows removes the notification icon.
                self.window = None;
                self.tray = None;
            }
            _ => {}
        }
        if self
            .machine
            .wake_at()
            .is_some_and(|at| at <= Instant::now())
        {
            self.feed(machine::Event::Tick, target);
        }
        *flow = if self.exiting || !self.goes_on() {
            ControlFlow::Exit
        } else {
            self.machine
                .wake_at()
                .map_or(ControlFlow::Wait, ControlFlow::WaitUntil)
        };
    }
}

/// Runs the shell as `launch` says: prepares the display, hears the end of the session, and lets
/// the machine open the stream to the service and show what comes of it. Returns only when the
/// shell cannot start; otherwise the process ends from the event loop.
///
/// With no display or no Ghostlight folder it writes one line to standard error and returns a
/// failure.
pub fn run(launch: Launch) -> ExitCode {
    window::apply_renderer_policy();
    window::prepare_identity();
    native::prepare_identity();
    if let Err(error) = window::init_display() {
        eprintln!("ghostlight-desktop: {error}");
        return ExitCode::from(exit_code_of(&error));
    }
    let home = match home::home() {
        Ok(home) => home,
        Err(error) => {
            eprintln!("ghostlight-desktop: the Ghostlight folder cannot be found: {error}");
            return ExitCode::from(EXIT_FAILURE);
        }
    };
    let event_loop = EventLoopBuilder::<ShellEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let ending = Mutex::new(proxy.clone());
    let listener = match session::listen(move || {
        if let Ok(proxy) = ending.lock() {
            post(&proxy, ShellEvent::SessionEnding);
        }
    }) {
        Ok(listener) => Some(listener),
        Err(error) => {
            eprintln!("ghostlight-desktop: {error}");
            None
        }
    };
    let tray_proxy = Mutex::new(proxy.clone());
    crate::tray::install_events(move |event| {
        if let Ok(proxy) = tray_proxy.lock() {
            post(&proxy, ShellEvent::Tray(event));
        }
    });
    // Read once, before anything can replace the file: a deploy moves the running image aside.
    let image = std::env::current_exe()
        .ok()
        .and_then(|path| Image::of(&path));
    let mut shell = Shell {
        machine: Machine::new(launch, image, Instant::now()),
        world: ThisWorld {
            session: ThisSession,
        },
        lifecycle: WindowLifecycle::for_platform(),
        window: None,
        tray: None,
        tray_model: None,
        workbench: None,
        link: None,
        generation: 0,
        profile: profile_directory(&home),
        home,
        requested: None,
        proxy,
        session: ThisSession,
        exiting: false,
        _listener: listener,
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
    fn a_shell_with_a_tray_goes_on_without_a_window() {
        assert!(goes_on(true, WindowState::Absent, true));
        assert!(goes_on(true, WindowState::Open, true));
    }

    #[test]
    fn a_shell_without_a_tray_ends_when_its_window_is_gone_once_it_has_attached() {
        assert!(!goes_on(false, WindowState::Absent, true));
        assert!(goes_on(false, WindowState::Open, true));
        assert!(goes_on(false, WindowState::Replacing, true));
    }

    #[test]
    fn a_shell_that_has_not_attached_yet_goes_on_without_a_window_or_a_tray() {
        assert!(goes_on(false, WindowState::Absent, false));
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
