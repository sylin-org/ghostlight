//! The Ghostlight shell as the service's window, notices and native acts (ADR-0199 Decision 7 and
//! its amendment of 2026-10-15, shell milestone 2).
//!
//! A shell of this build attaches by opening a `desktop` stream. [`Shells`] holds the service's one
//! desktop place: a second shell is turned away (it is told `already_open`) and the attached shell
//! is shown instead. [`ShellRoute`] is the one adapter the workbench facade is given for both its
//! presentation port and its native acts: with a shell attached it writes `show`, `notice` and
//! `act` frames to it, and without one it hands the request to the fallback adapter (the Tauri
//! window, while it is still in this process).
//!
//! An act waits for the shell's `act_result`. A stream that ends, a goodbye, and a quit that
//! begins each release every act waiting on the streams they touch, and the waiting act answers
//! that the presentation is unavailable. A waiting act is not work in flight, so it never extends
//! the quit's grace.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use ghostlight_relay::protocol::GoodbyeReason;

use crate::browser::transport::SocketWriter;
use crate::intake::desktop::{ActOutcome, DesktopFrame, NativeAct};
use crate::language::workbench as words;
use crate::workbench::{
    PageAddress, WorkbenchNativeActs, WorkbenchNotification, WorkbenchPresentationError,
    WorkbenchPresentationPort,
};

/// How long a link or a folder may take to open before the act counts as failed. A file dialog
/// has no limit: it waits for the person.
const OPEN_PATIENCE: Duration = Duration::from_secs(10);

/// Lock `mutex`, taking the data back from a poisoned one.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

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
    /// Attach the shell on `writer`: it is the one requests go to until the returned guard is
    /// dropped.
    pub fn attach(mut self, writer: Arc<SocketWriter>) -> Attached {
        let mut registry = lock(&self.shells.registry);
        registry.next_stream += 1;
        let stream = Arc::new(Stream {
            id: registry.next_stream,
            writer,
            next_act: AtomicU64::new(0),
            waiting: Mutex::default(),
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
    /// Bring the holder's window forward, best effort. A shell that is still being handed the
    /// workbench is about to open its window anyway.
    pub fn show(&self) {
        if let Some(stream) = &self.holder {
            let _ = stream.tell(&DesktopFrame::Show { address: None });
        }
    }
}

/// One attached shell. Dropping it detaches the shell and releases the acts waiting on it.
pub struct Attached {
    shells: Arc<Shells>,
    stream: Arc<Stream>,
}

impl Attached {
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
/// With a shell attached, a request goes to it. With none, it goes to the fallback
/// when one is installed, and is unavailable otherwise. A quit that has begun makes every request
/// unavailable. The fallback is the Tauri window and goes when Tauri leaves the service.
pub struct ShellRoute {
    shells: Arc<Shells>,
    fallback: OnceLock<Fallback>,
}

struct Fallback {
    presentation: Arc<dyn WorkbenchPresentationPort>,
    acts: Arc<dyn WorkbenchNativeActs>,
}

/// Where one request goes.
enum Target<'a> {
    Shell(Arc<Stream>),
    Fallback(&'a Fallback),
    Nowhere,
}

impl ShellRoute {
    /// A route over `shells`, with no fallback.
    #[must_use]
    pub fn new(shells: Arc<Shells>) -> Self {
        Self {
            shells,
            fallback: OnceLock::new(),
        }
    }

    /// Install what serves a request while no shell is attached. The first install stands.
    pub fn install_fallback(
        &self,
        presentation: Arc<dyn WorkbenchPresentationPort>,
        acts: Arc<dyn WorkbenchNativeActs>,
    ) {
        let _ = self.fallback.set(Fallback { presentation, acts });
    }

    fn target(&self) -> Target<'_> {
        match self.shells.current() {
            Current::Shell(stream) => Target::Shell(stream),
            Current::Closing => Target::Nowhere,
            Current::None => self
                .fallback
                .get()
                .map_or(Target::Nowhere, Target::Fallback),
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
        match self.target() {
            Target::Shell(stream) => stream.tell(&DesktopFrame::Show { address }),
            Target::Fallback(fallback) => fallback.presentation.reveal(address),
            Target::Nowhere => Err(WorkbenchPresentationError::Unavailable),
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
            Target::Fallback(fallback) => fallback.presentation.notify(notification),
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
            Target::Fallback(fallback) => fallback.acts.open_url(url),
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
            Target::Fallback(fallback) => fallback.acts.open_folder(path),
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
            Target::Fallback(fallback) => fallback.acts.pick_file(title),
            Target::Nowhere => Err(WorkbenchPresentationError::Unavailable),
        }
    }
}
