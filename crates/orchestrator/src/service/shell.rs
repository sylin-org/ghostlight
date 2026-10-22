//! The Ghostlight shell as the service's window, notices and native acts (ADR-0199 Decision 7 and
//! its amendment of 2026-10-15, shell milestone 2).
//!
//! A shell of this build attaches by opening a `desktop` stream. [`Shells`] holds the service's one
//! desktop place: a second shell is turned away (it is told `already_open`) and the attached shell
//! is shown instead. [`ShellRoute`] is the one adapter the workbench facade is given for both its
//! presentation port and its native acts: with a shell attached it writes `show`, `notice` and
//! `act` frames to it. Without one, a request to show the window goes to presence, which starts a
//! shell, and every other request is unavailable. The tray model the service describes is written
//! to the attached shell each time it differs from the one the shell last drew. While the shell
//! cannot build its window the service says so in the tooltip, until a person asks for the window
//! again.
//!
//! An act waits for the shell's `act_result`. A stream that ends, a goodbye, and a quit that
//! begins each release every act waiting on the streams they touch, and the waiting act answers
//! that the presentation is unavailable. A waiting act is not work in flight, so it never extends
//! the quit's grace.
//!
//! [`serve_desktop`] serves one shell on an accepted `desktop` stream, and [`TrayRelay`] carries
//! the tray the workbench describes to the attached shell on a thread of its own.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle, Thread};
use std::time::Duration;

use anyhow::Result;
use ghostlight_relay::endpoint::Connection;
use ghostlight_relay::protocol::{GoodbyeReason, QuitReason};

use super::browser_events::reveal_once;
use super::connections::ServiceContext;
use super::hint;
use super::lifetime::QUIT_GRACE;
use super::presence::{Presence, Shown};
use crate::browser::transport::{SocketReader, SocketWriter};
use crate::intake::desktop::{
    ActOutcome, DesktopFrame, NativeAct, ShellView, TrayItemId, TrayModel, UnavailableReason,
};
use crate::intake::listener;
use crate::language::intake::OpeningRefusal;
use crate::language::workbench as words;
use crate::language::workbench::{TRAY_HINT_BODY, TRAY_HINT_TITLE};
use crate::sync::lock;
use crate::workbench::{
    PageAddress, RuntimeSource, WorkbenchNativeActs, WorkbenchNotification,
    WorkbenchPresentationError, WorkbenchPresentationPort, WorkbenchTrayPort,
};

/// How long a link or a folder may take to open before the act counts as failed. A file dialog
/// has no limit: it waits for the person.
const OPEN_PATIENCE: Duration = Duration::from_secs(10);

/// The service's one desktop place.
#[derive(Default)]
pub struct Shells {
    registry: Mutex<Registry>,
}

#[derive(Default)]
struct Registry {
    place: Place,
    /// Set when a quit begins: no new act is carried out, and none is waited for.
    closing: bool,
    next_stream: u64,
}

/// Who holds the desktop place.
#[derive(Default)]
enum Place {
    /// No shell holds it.
    #[default]
    Free,
    /// A shell has been admitted and is being handed the workbench. It takes no request yet.
    Claimed,
    /// A shell is attached.
    Attached(Arc<Stream>),
}

/// Which shell, if any, a request goes to.
enum Current {
    /// The attached shell.
    Shell(Arc<Stream>),
    /// A quit has begun.
    Closing,
    /// No shell is attached.
    None,
}

impl Shells {
    /// Claim the desktop place for a shell that has just been admitted, or learn who holds it.
    ///
    /// The claim is not attached yet: requests keep going elsewhere until [`Claim::attach`], which
    /// the caller does after it has handed the shell the workbench.
    pub fn claim(self: &Arc<Self>) -> Result<Claim, Occupied> {
        let mut registry = lock(&self.registry);
        match &registry.place {
            Place::Free => {
                registry.place = Place::Claimed;
                Ok(Claim {
                    shells: Arc::clone(self),
                    attached: false,
                })
            }
            Place::Claimed => Err(Occupied { holder: None }),
            Place::Attached(stream) => Err(Occupied {
                holder: Some(Arc::clone(stream)),
            }),
        }
    }

    /// Whether a shell is attached.
    #[cfg(test)]
    pub(crate) fn attached(&self) -> usize {
        usize::from(matches!(lock(&self.registry).place, Place::Attached(_)))
    }

    fn current(&self) -> Current {
        let registry = lock(&self.registry);
        if registry.closing {
            return Current::Closing;
        }
        match &registry.place {
            Place::Attached(stream) => Current::Shell(Arc::clone(stream)),
            Place::Free | Place::Claimed => Current::None,
        }
    }

    fn attached_stream(&self) -> Option<Arc<Stream>> {
        match &lock(&self.registry).place {
            Place::Attached(stream) => Some(Arc::clone(stream)),
            Place::Free | Place::Claimed => None,
        }
    }

    /// A quit has begun: carry out no more acts and release every act that is waiting.
    pub fn close(&self) {
        let stream = {
            let mut registry = lock(&self.registry);
            registry.closing = true;
            match &registry.place {
                Place::Attached(stream) => Some(Arc::clone(stream)),
                Place::Free | Place::Claimed => None,
            }
        };
        if let Some(stream) = stream {
            stream.release();
        }
    }

    /// Describe the tray to the attached shell, when it differs from the one the shell last drew.
    pub fn send_tray(&self, model: TrayModel) {
        if let Some(stream) = self.attached_stream() {
            stream.draw(model);
        }
    }

    /// Say goodbye with `reason` to the attached shell and release the acts waiting on it.
    pub fn say_goodbye(&self, reason: GoodbyeReason) {
        if let Some(stream) = self.attached_stream() {
            let _ = stream.writer.native(&DesktopFrame::Goodbye { reason });
            stream.release();
        }
    }
}

/// The desktop place, claimed by a shell that is being handed the workbench. Dropping it before
/// [`Claim::attach`] frees the place.
pub struct Claim {
    shells: Arc<Shells>,
    attached: bool,
}

impl Claim {
    /// Attach the shell on `writer`, which has been sent `tray`: it is the one requests go to
    /// until the returned guard is dropped.
    pub fn attach(mut self, writer: Arc<SocketWriter>, tray: TrayModel) -> Attached {
        let mut registry = lock(&self.shells.registry);
        registry.next_stream += 1;
        let stream = Arc::new(Stream {
            id: registry.next_stream,
            writer,
            next_act: AtomicU64::new(0),
            waiting: Mutex::default(),
            tray: Mutex::new(Tray {
                described: tray.clone(),
                drawn: tray,
                window_fails: false,
            }),
        });
        registry.place = Place::Attached(Arc::clone(&stream));
        self.attached = true;
        Attached {
            shells: Arc::clone(&self.shells),
            stream,
        }
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        if !self.attached {
            lock(&self.shells.registry).place = Place::Free;
        }
    }
}

/// The shell that holds the desktop place, as a second shell learns it.
pub struct Occupied {
    holder: Option<Arc<Stream>>,
}

impl Occupied {
    /// Bring the holder's window forward, best effort, and say whether there was a holder to ask.
    /// A shell that is still being handed the workbench is about to open its window anyway.
    pub fn show(&self) -> bool {
        match &self.holder {
            Some(stream) => stream.show(None).is_ok(),
            None => false,
        }
    }
}

/// One attached shell. Dropping it detaches the shell and releases the acts waiting on it.
pub struct Attached {
    shells: Arc<Shells>,
    stream: Arc<Stream>,
}

impl Attached {
    /// Describe the tray to this shell, when it differs from the one the shell last drew.
    pub fn draw_tray(&self, model: TrayModel) {
        self.stream.draw(model);
    }

    /// Ask this shell to bring its window forward at `address`.
    pub fn show(&self, address: Option<PageAddress>) {
        let _ = self.stream.show(address);
    }

    /// The shell could not build its window (`true`), or a person asked it for the window again
    /// (`false`): the tooltip says so until then.
    pub fn window_fails(&self, fails: bool) {
        self.stream.window_fails(fails);
    }

    /// Hand this shell one notice to show.
    pub fn notice(&self, title: &str, body: &str) {
        let _ = self.stream.tell(&DesktopFrame::Notice {
            title: title.to_owned(),
            body: body.to_owned(),
        });
    }

    /// The shell reported how act `id` ended.
    pub fn answer(&self, id: u64, outcome: ActOutcome) {
        let waiting = lock(&self.stream.waiting).acts.remove(&id);
        if let Some(waiting) = waiting {
            // A caller that gave up has dropped its end; nothing is left to tell.
            let _ = waiting.send(outcome);
        }
    }
}

impl Drop for Attached {
    fn drop(&mut self) {
        let mut registry = lock(&self.shells.registry);
        if matches!(&registry.place, Place::Attached(held) if held.id == self.stream.id) {
            registry.place = Place::Free;
        }
        drop(registry);
        self.stream.release();
    }
}

/// One shell's stream and the acts waiting on it.
struct Stream {
    id: u64,
    writer: Arc<SocketWriter>,
    next_act: AtomicU64,
    waiting: Mutex<Waiting>,
    tray: Mutex<Tray>,
}

/// The tray of one stream.
struct Tray {
    /// The tray as the service last described it.
    described: TrayModel,
    /// The tray the shell last drew, as far as the service has told it.
    drawn: TrayModel,
    /// Set while the shell cannot build its window.
    window_fails: bool,
}

#[derive(Default)]
struct Waiting {
    /// Set once the stream is over: no act waits on it any more.
    released: bool,
    acts: HashMap<u64, Sender<ActOutcome>>,
}

impl Stream {
    /// Write one frame, best effort.
    fn tell(&self, frame: &DesktopFrame) -> Result<(), WorkbenchPresentationError> {
        self.writer
            .native(frame)
            .map_err(|_| WorkbenchPresentationError::Unavailable)
    }

    /// Tell the shell to draw `model` when it is not the tray it already draws.
    fn draw(&self, model: TrayModel) {
        let mut tray = lock(&self.tray);
        tray.described = model;
        self.redraw(&mut tray);
    }

    /// Say in the tooltip whether the shell can build its window.
    fn window_fails(&self, fails: bool) {
        let mut tray = lock(&self.tray);
        tray.window_fails = fails;
        self.redraw(&mut tray);
    }

    /// Ask the shell for its window at `address`. The window it could not build before is tried
    /// again, so the tooltip stops saying it fails; the shell says so again if it does.
    fn show(&self, address: Option<PageAddress>) -> Result<(), WorkbenchPresentationError> {
        self.window_fails(false);
        self.tell(&DesktopFrame::Show { address })
    }

    fn redraw(&self, tray: &mut Tray) {
        let model = if tray.window_fails {
            tray.described.while_window_fails()
        } else {
            tray.described.clone()
        };
        if tray.drawn == model {
            return;
        }
        if self
            .tell(&DesktopFrame::TrayModel {
                model: model.clone(),
            })
            .is_ok()
        {
            tray.drawn = model;
        }
    }

    /// Ask the shell to carry out `act` and wait for its answer: for `patience` when one is
    /// given, otherwise for as long as the stream lives.
    fn ask(
        &self,
        act: NativeAct,
        patience: Option<Duration>,
    ) -> Result<ActOutcome, WorkbenchPresentationError> {
        let id = self.next_act.fetch_add(1, Ordering::Relaxed) + 1;
        let (answer, answered): (Sender<ActOutcome>, Receiver<ActOutcome>) = mpsc::channel();
        {
            let mut waiting = lock(&self.waiting);
            if waiting.released {
                return Err(WorkbenchPresentationError::Unavailable);
            }
            waiting.acts.insert(id, answer);
        }
        if let Err(error) = self.tell(&DesktopFrame::Act { id, act }) {
            lock(&self.waiting).acts.remove(&id);
            return Err(error);
        }
        let outcome = match patience {
            Some(patience) => answered.recv_timeout(patience),
            None => answered.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        match outcome {
            Ok(outcome) => Ok(outcome),
            Err(RecvTimeoutError::Timeout) => {
                lock(&self.waiting).acts.remove(&id);
                Err(WorkbenchPresentationError::ActFailed(
                    words::NATIVE_ACT_FAILED.to_owned(),
                ))
            }
            Err(RecvTimeoutError::Disconnected) => Err(WorkbenchPresentationError::Unavailable),
        }
    }

    /// The stream is over, or a quit is: every waiting act is told it will not be answered.
    fn release(&self) {
        let mut waiting = lock(&self.waiting);
        waiting.released = true;
        waiting.acts.clear();
    }
}

/// What the workbench facade is given for its window, notices and native acts.
///
/// With a shell attached, a request goes to it. With none, a request to show the window goes to
/// presence, which starts a shell and shows the window when it attaches, and every other request
/// is unavailable. A quit that has begun makes every request unavailable.
pub struct ShellRoute {
    shells: Arc<Shells>,
    presence: Arc<Presence>,
}

/// Where one request goes.
enum Target {
    Shell(Arc<Stream>),
    Nowhere,
}

impl ShellRoute {
    /// A route over `shells`, which asks `presence` for a shell when there is none.
    #[must_use]
    pub fn new(shells: Arc<Shells>, presence: Arc<Presence>) -> Self {
        Self { shells, presence }
    }

    fn target(&self) -> Target {
        match self.shells.current() {
            Current::Shell(stream) => Target::Shell(stream),
            Current::Closing | Current::None => Target::Nowhere,
        }
    }

    /// Ask the shell for an act that opens something, and read how it ended.
    fn open(stream: &Stream, act: NativeAct) -> Result<(), WorkbenchPresentationError> {
        match stream.ask(act, Some(OPEN_PATIENCE))? {
            ActOutcome::Done => Ok(()),
            ActOutcome::Picked { .. } | ActOutcome::Cancelled | ActOutcome::Failed => Err(
                WorkbenchPresentationError::ActFailed(words::NATIVE_ACT_FAILED.to_owned()),
            ),
        }
    }
}

impl WorkbenchPresentationPort for ShellRoute {
    fn reveal(&self, address: Option<PageAddress>) -> Result<(), WorkbenchPresentationError> {
        loop {
            match self.shells.current() {
                Current::Shell(stream) => {
                    // Both callers are a person. A reason the shell gave for a window it could
                    // not build is over: it gives one again if this window fails too.
                    self.presence.person_acted();
                    stream.show(address)?;
                    self.presence.window_shown();
                    return Ok(());
                }
                Current::Closing => return Err(WorkbenchPresentationError::Unavailable),
                Current::None => match self.presence.request_show(address) {
                    Ok(Shown::Delivered) => return Ok(()),
                    Ok(Shown::Attached) => {}
                    Err(_) => return Err(WorkbenchPresentationError::Unavailable),
                },
            }
        }
    }

    fn notify(
        &self,
        notification: WorkbenchNotification,
    ) -> Result<(), WorkbenchPresentationError> {
        match self.target() {
            Target::Shell(stream) => stream.tell(&DesktopFrame::Notice {
                title: notification.title,
                body: notification.body,
            }),
            Target::Nowhere => Err(WorkbenchPresentationError::Unavailable),
        }
    }
}

impl WorkbenchNativeActs for ShellRoute {
    fn open_url(&self, url: &str) -> Result<(), WorkbenchPresentationError> {
        match self.target() {
            Target::Shell(stream) => Self::open(
                &stream,
                NativeAct::OpenUrl {
                    url: url.to_owned(),
                },
            ),
            Target::Nowhere => Err(WorkbenchPresentationError::Unavailable),
        }
    }

    fn open_folder(&self, path: &Path) -> Result<(), WorkbenchPresentationError> {
        match self.target() {
            Target::Shell(stream) => {
                let path = path.to_str().ok_or_else(|| {
                    WorkbenchPresentationError::ActFailed(words::NATIVE_ACT_FAILED.to_owned())
                })?;
                Self::open(
                    &stream,
                    NativeAct::OpenFolder {
                        path: path.to_owned(),
                    },
                )
            }
            Target::Nowhere => Err(WorkbenchPresentationError::Unavailable),
        }
    }

    fn pick_file(&self, title: &str) -> Result<Option<PathBuf>, WorkbenchPresentationError> {
        match self.target() {
            Target::Shell(stream) => {
                let act = NativeAct::PickFile {
                    title: title.to_owned(),
                };
                match stream.ask(act, None)? {
                    ActOutcome::Picked { path } => Ok(Some(PathBuf::from(path))),
                    ActOutcome::Cancelled => Ok(None),
                    ActOutcome::Done | ActOutcome::Failed => Err(
                        WorkbenchPresentationError::ActFailed(words::NATIVE_ACT_FAILED.to_owned()),
                    ),
                }
            }
            Target::Nowhere => Err(WorkbenchPresentationError::Unavailable),
        }
    }
}

/// Serve one shell of this build on an accepted `desktop` stream.
///
/// The service has one desktop place. A shell that finds it taken is told `already_open` instead of
/// the workbench and the stream ends. The shell that holds the place is asked to show its window,
/// unless the newcomer said it shows only the tray: that is presence's own launch, made while the
/// shell it would have replaced was coming back, and nothing is there to show.
/// Otherwise the shell is handed the workbench address and its secret and the tray to draw. It is
/// then attached, so the window, notices and native acts of the workbench go to it, a quit says
/// goodbye to it, and presence knows a shell is there. If a request to show the window was waiting
/// for a shell, the shell is asked to show it. What the shell sends is a click on a tray entry, the
/// result of each act, that it cannot show a window, and that the person closed the window; the
/// first close that stays is explained once. The shell opens its own window, so an `open` click
/// only tells presence that a person has it open; a frame kind this build does not know is
/// skipped. A frame that does not decode ends the stream, and the shell is detached.
pub(super) fn serve_desktop(
    connection: Connection,
    context: &Arc<ServiceContext>,
    view: Option<ShellView>,
) -> Result<()> {
    let Some(server) = context.workbench_server.get() else {
        listener::refuse(&connection, OpeningRefusal::Unsupported);
        return Ok(());
    };
    let Some(_place) = context.desktops.acquire(1) else {
        listener::refuse(&connection, OpeningRefusal::Capacity);
        return Ok(());
    };
    listener::accept(&connection, &context.accepted())?;
    let mut reader =
        SocketReader::new(connection.try_clone()?).interrupted_by(Arc::clone(&context.stop));
    let writer = Arc::new(SocketWriter::new(connection)?);
    let claim = match context.quit.shells().claim() {
        Ok(claim) => claim,
        Err(occupied) => {
            if view != Some(ShellView::Tray) && occupied.show() {
                context.presence.person_acted();
                context.presence.window_shown();
            }
            writer.native(&DesktopFrame::AlreadyOpen)?;
            return Ok(());
        }
    };
    let tray = context.workbench.tray_model();
    writer.native(&DesktopFrame::workbench(
        server.address().url(),
        server.bearer(),
        tray.clone(),
    ))?;
    let attached = claim.attach(writer, tray);
    let _present = ShellPresent::begin(&context.presence);
    // Whether a reveal for the tray's Open is in flight: clicks that arrive meanwhile join it.
    let revealing = Arc::new(AtomicBool::new(false));
    if let Some(wanted) = context.presence.attached(view) {
        attached.show(wanted.address);
    }
    // A change between the tray handed over and the shell being attached would have gone nowhere.
    attached.draw_tray(context.workbench.tray_model());
    while let Some(frame) = reader.native::<DesktopFrame>(None)? {
        let item = match frame {
            DesktopFrame::Tray { item } => item,
            DesktopFrame::ActResult { id, outcome } => {
                attached.answer(id, outcome);
                continue;
            }
            DesktopFrame::Unavailable { reason } => {
                context.presence.unavailable(reason.window_reason());
                attached.window_fails(reason == UnavailableReason::WindowFailed);
                continue;
            }
            DesktopFrame::Closed { stays } => {
                context.presence.closed(stays);
                if stays {
                    explain_close(context, &attached);
                }
                continue;
            }
            _ => continue,
        };
        match item {
            TrayItemId::Hold | TrayItemId::Resume => {
                if let Some(intent) = item.intent(context.workbench.runtime_state()) {
                    context
                        .workbench
                        .apply_runtime_control(intent, RuntimeSource::Tray);
                }
            }
            TrayItemId::Quit => context.quit.begin(QuitReason::Quit, QUIT_GRACE),
            TrayItemId::Open => {
                context.presence.person_acted();
                context.presence.window_shown();
                attached.window_fails(false);
                // The shell shows its own window, where it last was. While setup is incomplete
                // the person is brought back to it, as a window opened fresh would be.
                if let Some(address) = context.workbench.show_address() {
                    reveal_once(&context.workbench, Some(address), &revealing);
                }
            }
        }
    }
    Ok(())
}

/// Tells presence that a shell is gone when it is dropped, however the stream ended.
struct ShellPresent<'a>(&'a Presence);

impl<'a> ShellPresent<'a> {
    fn begin(presence: &'a Presence) -> Self {
        Self(presence)
    }
}

impl Drop for ShellPresent<'_> {
    fn drop(&mut self) {
        self.0.detached();
    }
}

/// Say, once per home, that closing the window left Ghostlight running and where its icon is.
fn explain_close(context: &ServiceContext, shell: &Attached) {
    match hint::first_close(&context.layout.tray_hint()) {
        Ok(true) => shell.notice(TRAY_HINT_TITLE, TRAY_HINT_BODY),
        Ok(false) => {}
        Err(error) => context.diagnostics.sink().emit(
            ghostlight_bridge::diagnostics::event::PROCESS_FAILED,
            ghostlight_bridge::diagnostics::Level::Warn,
            None,
            &format!("the tray hint marker could not be written: {error}"),
        ),
    }
}

/// Carries the tray the workbench describes to the attached shell on a thread of its own, so a
/// refresh of the live parts never waits on a stream.
pub(super) struct TrayRelay {
    latest: Mutex<Option<TrayModel>>,
    thread: OnceLock<Thread>,
}

impl TrayRelay {
    /// Start the thread that writes each tray the workbench describes to the attached shell, until
    /// `stop` is set. The thread is unparked by whoever sets `stop`.
    pub(super) fn spawn(
        shells: Arc<Shells>,
        stop: Arc<AtomicBool>,
    ) -> io::Result<(Arc<Self>, JoinHandle<()>)> {
        let relay = Arc::new(Self {
            latest: Mutex::new(None),
            thread: OnceLock::new(),
        });
        let carrying = Arc::clone(&relay);
        let handle = thread::Builder::new()
            .name("ghostlight-tray".into())
            .spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    let model = lock(&carrying.latest).take();
                    match model {
                        Some(model) => shells.send_tray(model),
                        None => thread::park(),
                    }
                }
            })?;
        let _ = relay.thread.set(handle.thread().clone());
        Ok((relay, handle))
    }
}

impl WorkbenchTrayPort for TrayRelay {
    fn tray_changed(&self, model: TrayModel) {
        *lock(&self.latest) = Some(model);
        if let Some(thread) = self.thread.get() {
            thread.unpark();
        }
    }
}
