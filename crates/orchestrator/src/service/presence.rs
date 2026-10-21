//! Presence: keeping the Ghostlight shell running whenever the desktop can show it (ADR-0199
//! Decision 11).
//!
//! The service serves with no window and links no UI toolkit. The shell, `ghostlight-desktop`,
//! draws the tray and the window. Whatever started the service, presence brings the shell up when
//! a display is reachable and none attaches, and brings it up again when it vanishes without a
//! goodbye. It also answers "show the window" when no shell is attached, by starting one and
//! showing the window once it attaches.
//!
//! [`Presence`] is the rules and holds the state; the world it acts on comes through [`Outside`]
//! (the clock, the display probe, the launcher and whether the session is ending), so every rule
//! can be tested with no process and no wait. The rules:
//!
//! - A shell is launched with `--tray` 2 s after the service is ready and 2 s after the last shell
//!   stream ended with no goodbye. A `--tray` shell shows only the icon (owner answer, 2026-10-15).
//! - A request to show the window starts a shell with no argument, which opens its own window, and
//!   remembers where the window was asked to open. The service sends that when the shell attaches.
//! - After a launch no further launch happens until a shell attaches or [`PENDING_LIMIT`] passes.
//!   A request to show the window meanwhile only updates the remembered address.
//! - At most [`BUDGET_LAUNCHES`] launches happen in [`BUDGET_WINDOW`]. Once they are spent the
//!   reason is latched as `did_not_stay` and nothing launches until a person acts.
//! - A shell that says it cannot show a window (`unavailable`) or that closed with no tray is
//!   recorded as the reason, spends no budget, and nothing launches until a person acts.
//! - Nothing launches while the session is ending or a quit has begun, when the shell file is
//!   missing, or when no display is reachable.
//!
//! A person acts by asking to show the window, or by starting a shell that attaches while no
//! launch of presence's own is pending. Either clears what was latched and the launch budget.

use std::collections::VecDeque;
use std::ffi::OsStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use ghostlight_relay::os;

use crate::install::layout::Layout;
use crate::intake::desktop::ShellView;
use crate::language::workbench::WindowReason;
use crate::sync::lock;
use crate::workbench::PageAddress;

/// How long after the service is ready, or after a shell's stream ended without a goodbye, a
/// shell is launched.
pub const LAUNCH_DELAY: Duration = Duration::from_secs(2);

/// How long after a launch no further launch happens while no shell has attached.
pub const PENDING_LIMIT: Duration = Duration::from_secs(15);

/// How many launches the budget allows in [`BUDGET_WINDOW`].
pub const BUDGET_LAUNCHES: usize = 3;

/// The span in which [`BUDGET_LAUNCHES`] launches may happen.
pub const BUDGET_WINDOW: Duration = Duration::from_secs(120);

/// How long a request to show the window waits for a shell to attach. It is shorter than the
/// 15 s the command line waits for the window, so the service answers first.
pub const SHOW_PATIENCE: Duration = Duration::from_secs(10);

/// The longest the ticking thread sleeps before it looks at the clock again.
const TICK_LIMIT: Duration = Duration::from_millis(500);

/// The name of the thread that launches the shell when it is due.
const THREAD_NAME: &str = "ghostlight-presence";

/// The argument of a shell that shows only the icon. The shell crate owns the word; the service
/// writes it again because it depends on no UI crate.
const TRAY_ARGUMENT: &str = "--tray";

/// Why a launch did not happen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LaunchRefusal {
    /// The shell file is not in the installation.
    NoShellFile,
    /// The operating system would not start the process.
    Failed,
}

/// The world presence acts on.
pub trait Outside: Send + Sync {
    /// The time now.
    fn now(&self) -> Instant;

    /// Whether a window could be seen: `Ok`, or the reason it could not.
    fn display(&self) -> Result<(), WindowReason>;

    /// Start the shell detached: with `--tray` when `tray`, with no argument otherwise.
    fn launch(&self, tray: bool) -> Result<(), LaunchRefusal>;

    /// Whether the session is ending or a quit has begun, so nothing may start.
    fn ending(&self) -> bool;
}

/// Where the window was asked to open, remembered until a shell attaches to show it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Wanted {
    /// The place in the page, or `None` to leave the window where it was.
    pub address: Option<PageAddress>,
}

/// What asking to show the window found.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Shown {
    /// A shell is attached; the caller shows the window on it.
    Attached,
    /// A shell attached and the attach path showed the window.
    Delivered,
}

#[derive(Default)]
struct State {
    /// A shell holds the desktop place.
    attached: bool,
    /// The attached shell has its window open, as far as the service knows.
    window_open: bool,
    /// Nothing launches before this, while no shell has attached.
    pending_until: Option<Instant>,
    /// The launch pending was presence's own `--tray` launch.
    pending_is_tray: bool,
    /// The times of the launches in the budget window.
    launches: VecDeque<Instant>,
    /// What stops launching until a person acts.
    latched: Option<WindowReason>,
    /// Why the last launch that was considered did not happen, while it did not.
    refusal: Option<WindowReason>,
    /// When a launch of presence's own is due.
    due: Option<Instant>,
    /// Where the window was asked to open, until a shell attaches.
    wanted: Option<Wanted>,
}

/// The rules of presence and the state they keep.
pub struct Presence {
    outside: Box<dyn Outside>,
    state: Mutex<State>,
    changed: Condvar,
}

impl Presence {
    /// Presence over `outside`, whose requests to show the window wait [`SHOW_PATIENCE`].
    #[must_use]
    pub fn new(outside: Box<dyn Outside>) -> Self {
        Self {
            outside,
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
        }
    }

    /// The service is ready: the first launch is due [`LAUNCH_DELAY`] from now.
    pub fn published(&self) {
        let now = self.outside.now();
        let mut state = lock(&self.state);
        if !state.attached {
            state.due = Some(now + LAUNCH_DELAY);
        }
    }

    /// A shell holds the desktop place now. Returns where the window was asked to open, when a
    /// request to show it was waiting.
    ///
    /// A shell that attaches while no launch of presence's own is pending is a person's start and
    /// clears what was latched. Whether the window is open is what the shell said (`view`); a
    /// shell that said nothing is taken as open unless presence launched it for the tray.
    pub fn attached(&self, view: Option<ShellView>) -> Option<Wanted> {
        let now = self.outside.now();
        let mut state = lock(&self.state);
        let own_launch =
            state.pending_is_tray && state.pending_until.is_some_and(|until| now < until);
        if !own_launch {
            Self::clear_latch(&mut state);
        }
        let wanted = state.wanted.take();
        state.window_open = wanted.is_some()
            || match view {
                Some(ShellView::Window) => true,
                Some(ShellView::Tray) => false,
                None => !own_launch,
            };
        state.attached = true;
        state.pending_until = None;
        state.pending_is_tray = false;
        state.due = None;
        state.refusal = None;
        self.changed.notify_all();
        wanted
    }

    /// The shell's stream ended. With no goodbye, a new shell is due [`LAUNCH_DELAY`] from now,
    /// unless the shell said it cannot stay.
    pub fn detached(&self) {
        let now = self.outside.now();
        let mut state = lock(&self.state);
        state.attached = false;
        state.window_open = false;
        if state.latched.is_none() {
            state.due = Some(now + LAUNCH_DELAY);
        }
        self.changed.notify_all();
    }

    /// The shell cannot show what it was started to show. Nothing launches until a person acts,
    /// and while the shell stays attached its reason is the window's reason.
    pub fn unavailable(&self, reason: WindowReason) {
        let mut state = lock(&self.state);
        state.window_open = false;
        state.latched = Some(reason);
        state.due = None;
    }

    /// The person closed the window. A shell that stays keeps the icon; one that does not leaves,
    /// and nothing launches again until a person acts.
    pub fn closed(&self, stays: bool) {
        let mut state = lock(&self.state);
        state.window_open = false;
        if !stays {
            state.latched = Some(WindowReason::NoTray);
            state.due = None;
        }
    }

    /// The window was shown on the attached shell, or the person opened it from the tray. The
    /// shell sends `unavailable` if it cannot build the window, which replaces this.
    pub fn window_shown(&self) {
        lock(&self.state).window_open = true;
    }

    /// A person asked for the window by another way than a request to show it: a second shell
    /// they started was told the first holds the place. It clears what was latched.
    pub fn person_acted(&self) {
        Self::clear_latch(&mut lock(&self.state));
    }

    /// Ask for the window to be shown at `address`, starting a shell when none is attached, and
    /// wait for one to attach.
    ///
    /// This is a person's act: it clears what was latched and the launch budget. Another request
    /// while a launch is pending only updates where the window opens. The reason, when there is
    /// one, is returned when nothing could be started or no shell attached within the patience.
    pub fn request_show(
        &self,
        address: Option<PageAddress>,
    ) -> Result<Shown, Option<WindowReason>> {
        let begun = self.outside.now();
        let mut state = lock(&self.state);
        Self::clear_latch(&mut state);
        if state.attached {
            return Ok(Shown::Attached);
        }
        state.wanted = Some(Wanted { address });
        let pending = state.pending_until.is_some_and(|until| begun < until);
        if !pending {
            if let Err(reason) = self.launch(&mut state, begun, false) {
                state.wanted = None;
                return Err(reason);
            }
        }
        let deadline = Instant::now() + SHOW_PATIENCE;
        while !state.attached {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                state.wanted = None;
                return Err(state.latched.or(state.refusal));
            }
            state = self
                .changed
                .wait_timeout(state, remaining)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
        Ok(Shown::Delivered)
    }

    /// Launch the shell when one is due.
    pub fn tick(&self) {
        let now = self.outside.now();
        let mut state = lock(&self.state);
        if state.due.is_none_or(|due| due > now) {
            return;
        }
        state.due = None;
        if state.attached || state.latched.is_some() {
            return;
        }
        if let Some(until) = state.pending_until.filter(|until| now < *until) {
            state.due = Some(until);
            return;
        }
        let _ = self.launch(&mut state, now, true);
    }

    /// When the next launch is due, if one is.
    #[must_use]
    pub fn due(&self) -> Option<Instant> {
        lock(&self.state).due
    }

    /// Why there is, or is not, a window: from the shell that is attached, or from what keeps
    /// one from being brought up. A reason the attached shell gave for having no window comes
    /// first. `None` while presence has nothing to say yet.
    #[must_use]
    pub fn window_reason(&self) -> Option<WindowReason> {
        let state = lock(&self.state);
        if state.attached {
            return Some(match state.latched {
                Some(reason) => reason,
                None if state.window_open => WindowReason::Shown,
                None => WindowReason::Tray,
            });
        }
        state.latched.or(state.refusal)
    }

    /// Clear what a person's act clears: the latch, the reason, and the launch budget.
    fn clear_latch(state: &mut State) {
        state.latched = None;
        state.refusal = None;
        state.launches.clear();
    }

    /// Try one launch, `tray` for a shell that shows only the icon. The caller holds the state.
    fn launch(
        &self,
        state: &mut State,
        now: Instant,
        tray: bool,
    ) -> Result<(), Option<WindowReason>> {
        if self.outside.ending() {
            return Err(None);
        }
        if let Err(reason) = self.outside.display() {
            state.refusal = Some(reason);
            return Err(Some(reason));
        }
        while state
            .launches
            .front()
            .is_some_and(|at| now.saturating_duration_since(*at) >= BUDGET_WINDOW)
        {
            state.launches.pop_front();
        }
        if state.launches.len() >= BUDGET_LAUNCHES {
            state.latched = Some(WindowReason::DidNotStay);
            state.pending_until = None;
            return Err(Some(WindowReason::DidNotStay));
        }
        state.launches.push_back(now);
        state.pending_until = Some(now + PENDING_LIMIT);
        state.pending_is_tray = tray;
        state.due = Some(now + PENDING_LIMIT);
        match self.outside.launch(tray) {
            Ok(()) => {
                state.refusal = None;
                Ok(())
            }
            Err(LaunchRefusal::NoShellFile) => {
                state.launches.pop_back();
                state.pending_until = None;
                state.due = None;
                state.refusal = Some(WindowReason::NoShellFile);
                Err(Some(WindowReason::NoShellFile))
            }
            Err(LaunchRefusal::Failed) => Ok(()),
        }
    }

    /// Start the thread that launches the shell when it is due. It ends when `stop` is set.
    pub fn spawn(self: &Arc<Self>, stop: Arc<AtomicBool>) -> std::io::Result<JoinHandle<()>> {
        let presence = Arc::clone(self);
        thread::Builder::new()
            .name(THREAD_NAME.into())
            .spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    presence.tick();
                    let wait = presence
                        .due()
                        .map_or(TICK_LIMIT, |due| {
                            due.saturating_duration_since(presence.outside.now())
                        })
                        .min(TICK_LIMIT);
                    thread::park_timeout(wait.max(Duration::from_millis(10)));
                }
            })
    }
}

/// Whether a window could be seen on a Linux session with these display variables: the display
/// server a window is drawn on is named by `DISPLAY` or `WAYLAND_DISPLAY`.
pub fn display_named(display: Option<&OsStr>, wayland: Option<&OsStr>) -> Result<(), WindowReason> {
    let named = |value: Option<&OsStr>| value.is_some_and(|value| !value.is_empty());
    if named(display) || named(wayland) {
        Ok(())
    } else {
        Err(WindowReason::NoDisplay)
    }
}

/// Says whether a quit has begun.
pub type Ending = Arc<dyn Fn() -> bool + Send + Sync>;

/// The real world: the installation's shell, this session's display and the system clock.
pub struct System {
    layout: Layout,
    ending: Ending,
}

impl System {
    /// The world of the installation `layout` names. `ending` says whether a quit has begun.
    #[must_use]
    pub fn new(layout: Layout, ending: Ending) -> Self {
        Self { layout, ending }
    }

    /// The same world, boxed as what [`Presence`] acts on.
    #[must_use]
    pub fn boxed(layout: Layout, ending: Ending) -> Box<dyn Outside> {
        Box::new(Self::new(layout, ending))
    }
}

/// Whether a window this process starts could be seen: on Windows when it runs on the interactive
/// desktop, on Linux when its session names a display. The service asks it of itself through
/// presence, and `ghostlight open` of the person's own session (`cli::desktop`).
///
/// # Errors
///
/// The reason no window could be seen.
pub fn display_here() -> Result<(), WindowReason> {
    #[cfg(windows)]
    {
        if ghostlight_win_peer::interactive_window_station() {
            Ok(())
        } else {
            Err(WindowReason::NoInteractiveDesktop)
        }
    }
    #[cfg(target_os = "linux")]
    {
        display_named(
            std::env::var_os("DISPLAY").as_deref(),
            std::env::var_os("WAYLAND_DISPLAY").as_deref(),
        )
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Ok(())
    }
}

/// Start the shell of the installation `layout` names, detached, in the calling process's
/// session: with `tray` it shows only the icon, without it it opens its own window. The service
/// starts it through presence, and `ghostlight open` starts it when the service reaches no
/// display (`cli::desktop`).
///
/// # Errors
///
/// [`LaunchRefusal::NoShellFile`] when the shell is not in the installation, and
/// [`LaunchRefusal::Failed`] when the operating system would not start it.
pub fn start_shell(layout: &Layout, tray: bool) -> Result<(), LaunchRefusal> {
    let shell = layout.desktop_executable();
    if !shell.is_file() {
        return Err(LaunchRefusal::NoShellFile);
    }
    let arguments: &[&OsStr] = if tray {
        &[OsStr::new(TRAY_ARGUMENT)]
    } else {
        &[]
    };
    os::spawn_detached(&shell, arguments, layout.home())
        .map(|_| ())
        .map_err(|_| LaunchRefusal::Failed)
}

impl Outside for System {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn display(&self) -> Result<(), WindowReason> {
        display_here()
    }

    fn launch(&self, tray: bool) -> Result<(), LaunchRefusal> {
        start_shell(&self.layout, tray)
    }

    fn ending(&self) -> bool {
        os::session::session_ending() || (self.ending)()
    }
}

/// A world for tests: a clock the test moves and a launcher that records what it was asked.
#[cfg(test)]
pub(crate) mod testing {
    use super::*;

    /// What a test sets and reads of the world presence acts on.
    #[derive(Default)]
    pub(crate) struct Fake {
        offset: Mutex<Duration>,
        display: Mutex<Option<WindowReason>>,
        refusal: Mutex<Option<LaunchRefusal>>,
        ending: AtomicBool,
        launched: Mutex<Vec<bool>>,
    }

    struct World(Arc<Fake>, Instant);

    impl Outside for World {
        fn now(&self) -> Instant {
            self.1 + *lock(&self.0.offset)
        }

        fn display(&self) -> Result<(), WindowReason> {
            lock(&self.0.display).map_or(Ok(()), Err)
        }

        fn launch(&self, tray: bool) -> Result<(), LaunchRefusal> {
            if let Some(refusal) = *lock(&self.0.refusal) {
                return Err(refusal);
            }
            lock(&self.0.launched).push(tray);
            Ok(())
        }

        fn ending(&self) -> bool {
            self.0.ending.load(Ordering::SeqCst)
        }
    }

    impl Fake {
        /// The world, as presence acts on it.
        pub(crate) fn world(self: &Arc<Self>) -> Box<dyn Outside> {
            Box::new(World(Arc::clone(self), Instant::now()))
        }

        /// Move the clock forward.
        pub(crate) fn advance(&self, by: Duration) {
            *lock(&self.offset) += by;
        }

        /// Make the display probe fail with `reason`, or pass.
        pub(crate) fn set_display(&self, reason: Option<WindowReason>) {
            *lock(&self.display) = reason;
        }

        /// Make the launcher refuse, or start.
        pub(crate) fn set_refusal(&self, refusal: Option<LaunchRefusal>) {
            *lock(&self.refusal) = refusal;
        }

        /// Say whether the session is ending.
        pub(crate) fn set_ending(&self, ending: bool) {
            self.ending.store(ending, Ordering::SeqCst);
        }

        /// Whether each launch so far was a `--tray` one, in order.
        pub(crate) fn launches(&self) -> Vec<bool> {
            lock(&self.launched).clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::Fake;
    use super::*;
    use crate::scratch::Scratch;

    fn presence() -> (Presence, Arc<Fake>) {
        let fake = Arc::new(Fake::default());
        let presence = Presence::new(fake.world());
        (presence, fake)
    }

    const SECOND: Duration = Duration::from_secs(1);

    /// How long a test waits for what it expects. Only a failure waits it out.
    const PATIENCE: Duration = Duration::from_secs(10);

    /// Run presence for `seconds` seconds, ticking each second.
    fn run(presence: &Presence, fake: &Fake, seconds: u64) {
        for _ in 0..seconds {
            fake.advance(SECOND);
            presence.tick();
        }
    }

    /// Wait until `condition` holds, and fail once the patience is spent.
    fn eventually(condition: impl Fn() -> bool) {
        let until = Instant::now() + PATIENCE;
        while !condition() {
            assert!(Instant::now() < until, "the condition never held");
            thread::sleep(Duration::from_millis(2));
        }
    }

    /// Defect 17ec7571: the window is what the shell says it shows when it attaches, whoever
    /// launched it, so status after a crash or an update names the window as it is.
    #[test]
    fn the_window_is_what_the_shell_says_it_shows_whoever_launched_it() {
        // The shell a crash left comes back with its window open while presence's own tray
        // launch is pending: its window is open.
        let (crashed, fake) = presence();
        crashed.published();
        run(&crashed, &fake, 2);
        assert_eq!(
            fake.launches(),
            [true],
            "a tray launch of presence's own is pending"
        );
        crashed.attached(Some(ShellView::Window));
        assert_eq!(crashed.window_reason(), Some(WindowReason::Shown));

        // A shell with only its icon that comes back with no launch pending has no window open.
        let (iconic, _) = presence();
        iconic.attached(Some(ShellView::Tray));
        assert_eq!(iconic.window_reason(), Some(WindowReason::Tray));
    }

    /// ACCEPTANCE desktop gates 13 and 1 and ADR-0199 Decision 11: a shell that vanished without
    /// a goodbye is brought back 2 s later, and again each time a launch does not attach, at most
    /// three times in two minutes; then nothing launches until a person acts. A person's request
    /// for the window, inside those two minutes, starts a shell with no argument, which is handed
    /// the request, and from then on a shell that vanishes is brought back again.
    #[test]
    fn a_vanished_shell_is_relaunched_at_most_three_times_in_two_minutes_until_a_person_acts() {
        let (presence, fake) = presence();
        let presence = Arc::new(presence);
        presence.published();
        run(&presence, &fake, 2);
        assert_eq!(fake.launches(), [true]);
        presence.attached(None);
        presence.detached();
        run(&presence, &fake, 1);
        assert_eq!(fake.launches(), [true], "nothing before 2 s");
        run(&presence, &fake, 1);
        assert_eq!(
            fake.launches(),
            [true, true],
            "brought back 2 s after it vanished"
        );

        // The shells it starts never attach, so it starts another each time it gives one up,
        // until three launches inside two minutes have spent the budget; then nothing more.
        run(&presence, &fake, 60);
        assert_eq!(fake.launches(), [true; BUDGET_LAUNCHES]);
        assert_eq!(presence.window_reason(), Some(WindowReason::DidNotStay));

        let asked = {
            let presence = Arc::clone(&presence);
            thread::spawn(move || presence.request_show(None))
        };
        eventually(|| fake.launches().len() > BUDGET_LAUNCHES);
        assert_eq!(fake.launches()[BUDGET_LAUNCHES..], [false]);
        assert_eq!(presence.attached(None), Some(Wanted { address: None }));
        assert_eq!(asked.join().unwrap(), Ok(Shown::Delivered));
        assert_eq!(presence.window_reason(), Some(WindowReason::Shown));
        presence.detached();
        run(&presence, &fake, 2);
        assert_eq!(fake.launches()[BUDGET_LAUNCHES..], [false, true]);
    }

    /// ACCEPTANCE desktop gate 13 and ADR-0199 Decision 11: a shell that said it cannot show a
    /// window, or that closed with no tray to stay in, is not brought back when it goes and
    /// spends no launch budget, and its reason stays the window's reason, so status says why,
    /// until a person acts.
    #[test]
    fn unavailable_and_a_close_with_no_tray_spend_no_budget_and_block_until_a_person_acts() {
        for (blocked, expected) in [
            (WindowReason::NoDisplay, WindowReason::NoDisplay),
            (WindowReason::NoWebview, WindowReason::NoWebview),
            (WindowReason::NoTray, WindowReason::NoTray),
        ] {
            let (presence, fake) = presence();
            presence.published();
            run(&presence, &fake, 2);
            assert_eq!(fake.launches().len(), 1);
            presence.attached(None);
            presence.unavailable(blocked);
            presence.detached();
            run(&presence, &fake, 60);
            assert_eq!(fake.launches().len(), 1, "{expected:?} launches nothing");
            assert_eq!(presence.window_reason(), Some(expected));
        }
        let (presence, fake) = presence();
        presence.attached(None);
        presence.closed(false);
        presence.detached();
        run(&presence, &fake, 60);
        assert!(fake.launches().is_empty());
        assert_eq!(presence.window_reason(), Some(WindowReason::NoTray));
    }

    /// ACCEPTANCE desktop gates 13, 20 and 1: with no display, no desktop a person can see or no
    /// shell file, presence launches nothing and names the reason, and once the session is ending
    /// it launches nothing at all.
    #[test]
    fn no_display_no_desktop_no_file_and_an_ending_session_launch_nothing() {
        for (display, refusal, ending, expected) in [
            (
                Some(WindowReason::NoDisplay),
                None,
                false,
                Some(WindowReason::NoDisplay),
            ),
            (
                Some(WindowReason::NoInteractiveDesktop),
                None,
                false,
                Some(WindowReason::NoInteractiveDesktop),
            ),
            (
                None,
                Some(LaunchRefusal::NoShellFile),
                false,
                Some(WindowReason::NoShellFile),
            ),
            (None, None, true, None),
        ] {
            let (presence, fake) = presence();
            fake.set_display(display);
            fake.set_refusal(refusal);
            fake.set_ending(ending);
            presence.published();
            run(&presence, &fake, 60);
            assert!(fake.launches().is_empty(), "{expected:?}");
            assert_eq!(presence.window_reason(), expected);
        }
    }

    /// ACCEPTANCE desktop gate 20 and defect 56c49dc4: on Linux a window can be seen only where
    /// `DISPLAY` or `WAYLAND_DISPLAY` names a display server, so with both unset or empty the
    /// service says `no_display` and `ghostlight open` over SSH says why at once.
    #[test]
    fn the_display_variables_name_a_display_server() {
        assert_eq!(display_named(Some(OsStr::new(":0")), None), Ok(()));
        assert_eq!(display_named(None, Some(OsStr::new("wayland-0"))), Ok(()));
        assert_eq!(display_named(None, None), Err(WindowReason::NoDisplay));
        assert_eq!(
            display_named(Some(OsStr::new("")), Some(OsStr::new(""))),
            Err(WindowReason::NoDisplay)
        );
    }

    /// ACCEPTANCE desktop gate 13: in an installation without the window program the launcher
    /// that presence and `ghostlight open` share says so, rather than reporting a start that
    /// failed, and starts nothing.
    #[test]
    fn an_installation_without_the_window_program_says_so_and_starts_nothing() {
        let home = Scratch::new();
        let layout = Layout::new(home.path());
        for tray in [true, false] {
            assert_eq!(
                start_shell(&layout, tray),
                Err(LaunchRefusal::NoShellFile),
                "tray {tray}"
            );
        }
    }
}
