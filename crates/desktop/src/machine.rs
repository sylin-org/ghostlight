//! The shell's own life as a pure state machine (ADR-0199 Decisions 7, 10, 11 and 14, and its
//! amendment of 2026-10-15, shell milestone 2).
//!
//! The shell must survive a service restart, an update and a missing service, and must never
//! start Ghostlight behind a person's Quit. [`Machine`] decides all of that and does none of it:
//! it takes [`Event`]s with the time they happened and answers with [`Effect`]s for the glue in
//! `app` to carry out. The only things it asks the outside world are the four questions of
//! [`Outside`] (may a process start now, start the service, start the shell again, what is the
//! image in `bin`), so every transition is a unit test with no window, no thread and no clock.
//!
//! How each start behaves before the shell first attaches:
//!
//! - A person's start opens the stream. With no answer it shows Starting, starts the service once
//!   (reason `person`) and retries on the relay's backoff for 30 s. An answer of shutting down is
//!   waited out without starting anything, and the service is started once when nothing answers.
//!   Any other refusal is the Stopped screen.
//! - A tray start (`--tray`) tries once. No answer, shutting down or a refusal ends it, having
//!   started nothing.
//! - An updating start shows Updating, retries for 60 s and starts nothing. Then Stopped.
//!
//! After an attach every shell follows the same rules:
//!
//! - A goodbye of quit, uninstall or session end ends the shell.
//! - A goodbye of replace compares this image with the one in `bin`. A shell whose image was
//!   replaced starts `bin`'s shell again with `--updating` when its window is open, and ends; with
//!   no window it only ends, and presence brings up a shell for the tray. A shell whose image is
//!   the same shows Updating and retries for 60 s.
//! - A stream that ends with no goodbye shows Restarting and starts the service with reason
//!   `recover`, up to three tries. The service refuses that reason after a person's Quit, so no
//!   shell can undo a Quit.
//! - Stopped and a spent Restarting keep trying the opening at the backoff cap and start nothing,
//!   so a service that comes back later picks the shell up. Only the Start button of the Stopped
//!   screen starts the service, and it is a person's act.
//!
//! Nothing starts while the session is ending.

use std::io;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

use ghostlight_relay::protocol::{GoodbyeReason, RefusalCode, StartReason};
use ghostlight_relay::spawn::Backoff;
use ghostlight_relay::BACKOFF_MAX;

use crate::link::{LinkEnd, LinkError};
use crate::states::StateScreen;
use crate::Launch;

/// How long a person's start waits for the service to answer after it has started it.
pub const STARTING_PATIENCE: Duration = Duration::from_secs(30);

/// How long a shell waits for the service to come back during an update.
pub const UPDATING_PATIENCE: Duration = Duration::from_secs(60);

/// How many times a shell starts the service after losing it without a goodbye.
pub const RECOVER_TRIES: u32 = 3;

/// How long each of those tries waits for the service to answer.
pub const RECOVER_PATIENCE: Duration = Duration::from_secs(10);

/// What the window shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    /// One of the shell's own states.
    State(StateScreen),
    /// The service's workbench.
    Workbench,
}

/// How the service answered an attempt to open the stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// The stream is open and the workbench was handed over.
    Attached,
    /// Nothing answered, or what answered went away.
    NoAnswer,
    /// The service is quitting.
    ShuttingDown,
    /// Another shell holds the service's desktop place.
    AlreadyOpen,
    /// The service refused the shell, or answered with something the shell cannot use.
    Refused,
}

impl From<&LinkError> for Answer {
    fn from(error: &LinkError) -> Self {
        use ghostlight_relay::endpoint::ConnectError;
        match error {
            LinkError::Connect(ConnectError::ForeignServer) => Answer::Refused,
            LinkError::Connect(_) | LinkError::Io(_) => Answer::NoAnswer,
            LinkError::Refused {
                code: Some(RefusalCode::ShuttingDown),
                ..
            } => Answer::ShuttingDown,
            LinkError::Refused { .. } | LinkError::Contract { .. } => Answer::Refused,
            LinkError::AlreadyOpen => Answer::AlreadyOpen,
        }
    }
}

/// Something that happened to the shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// The shell has started.
    Started,
    /// An attempt to open the stream ended.
    Opened(Answer),
    /// The stream of an attached shell ended. `window_open` says whether the window is open now.
    Ended {
        /// How it ended.
        end: LinkEnd,
        /// Whether the window is open.
        window_open: bool,
    },
    /// The time reached [`Machine::wake_at`].
    Tick,
    /// The person pressed Start on the Stopped screen.
    StartPressed,
}

/// What the glue does for the machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Try to open the stream, off the event-loop thread, and report [`Event::Opened`].
    Open,
    /// Show `screen`. With `present`, open the window first when it is not open.
    Show {
        /// What to show.
        screen: Screen,
        /// Whether the window must exist.
        present: bool,
    },
    /// End the shell with status 0.
    Exit,
}

/// What the machine asks of the world outside it.
pub trait Outside {
    /// False while the session is ending.
    fn may_start_process(&self) -> bool;

    /// Start the service detached, for `reason`. Success means the process was created.
    fn start_service(&mut self, reason: StartReason) -> io::Result<()>;

    /// Start the shell in `bin` again with `--updating`, detached.
    fn relaunch_updating(&mut self) -> io::Result<()>;

    /// The identity of the shell image in `bin` now, if it can be read.
    fn bin_image(&self) -> Option<Image>;
}

/// The identity of an executable file: its length and last-write time. Two images that differ in
/// either are not the same build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Image {
    length: u64,
    modified: SystemTime,
}

impl Image {
    /// An image of `length` bytes last written at `modified`.
    pub fn new(length: u64, modified: SystemTime) -> Self {
        Self { length, modified }
    }

    /// The identity of the file at `path`, or `None` when its metadata cannot be read.
    pub fn of(path: &Path) -> Option<Self> {
        let metadata = std::fs::metadata(path).ok()?;
        Some(Self::new(metadata.len(), metadata.modified().ok()?))
    }
}

/// Where the shell is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// The one attempt of a tray start.
    Probing,
    /// A person's start, or the Start button.
    Person { started: bool, until: Instant },
    /// An update: only the opening is retried.
    Update { until: Instant },
    /// The service was lost without a goodbye.
    Recover { tried: u32, try_until: Instant },
    /// Stopped: the opening is retried at the backoff cap and nothing is started.
    Watch,
    /// Attached to the service.
    Attached,
    /// The shell is ending.
    Done,
}

/// The pure rules of the shell's life. See the module documentation.
#[derive(Debug)]
pub struct Machine {
    launch: Launch,
    image: Option<Image>,
    stage: Stage,
    screen: Option<Screen>,
    backoff: Backoff,
    next_try: Option<Instant>,
    in_flight: bool,
    attached_before: bool,
    relaunched: bool,
}

impl Machine {
    /// The machine of a shell started as `launch`, whose own image is `image`, read once at start.
    pub fn new(launch: Launch, image: Option<Image>, now: Instant) -> Self {
        let stage = match launch {
            Launch::Person => Stage::Person {
                started: false,
                until: now + STARTING_PATIENCE,
            },
            Launch::Tray => Stage::Probing,
            Launch::Updating => Stage::Update {
                until: now + UPDATING_PATIENCE,
            },
        };
        Self {
            launch,
            image,
            stage,
            screen: None,
            backoff: Backoff::new(),
            next_try: None,
            in_flight: false,
            attached_before: false,
            relaunched: false,
        }
    }

    /// The screen the window shows now, if one was chosen yet.
    pub fn screen(&self) -> Option<Screen> {
        self.screen
    }

    /// Whether the shell has attached to a service at least once.
    pub fn has_attached(&self) -> bool {
        self.attached_before
    }

    /// When the glue must send [`Event::Tick`], if it must.
    pub fn wake_at(&self) -> Option<Instant> {
        if self.in_flight {
            None
        } else {
            self.next_try
        }
    }

    /// Applies one event at `now` and returns what to do about it.
    pub fn on(&mut self, event: Event, now: Instant, outside: &mut dyn Outside) -> Vec<Effect> {
        let mut effects = Vec::new();
        if self.stage == Stage::Done {
            return effects;
        }
        match event {
            Event::Started => {
                if self.launch == Launch::Updating {
                    self.show(Screen::State(StateScreen::Updating), true, &mut effects);
                }
                self.open(&mut effects);
            }
            Event::Opened(answer) => self.opened(answer, now, outside, &mut effects),
            Event::Ended { end, window_open } => {
                if self.stage == Stage::Attached {
                    self.ended(end, window_open, now, outside, &mut effects);
                }
            }
            Event::Tick => {
                if !self.in_flight && self.next_try.is_some_and(|at| at <= now) {
                    self.open(&mut effects);
                }
            }
            Event::StartPressed => self.start_pressed(now, outside, &mut effects),
        }
        effects
    }

    fn open(&mut self, effects: &mut Vec<Effect>) {
        self.in_flight = true;
        self.next_try = None;
        effects.push(Effect::Open);
    }

    fn show(&mut self, screen: Screen, present: bool, effects: &mut Vec<Effect>) {
        if self.screen != Some(screen) {
            self.screen = Some(screen);
            effects.push(Effect::Show { screen, present });
        }
    }

    fn finish(&mut self, effects: &mut Vec<Effect>) {
        self.stage = Stage::Done;
        self.next_try = None;
        effects.push(Effect::Exit);
    }

    /// Plan the next attempt after the wait `bound` allows.
    fn schedule(&mut self, now: Instant, deadline: Option<Instant>) {
        let mut wait = if self.stage == Stage::Watch {
            BACKOFF_MAX
        } else {
            self.backoff.next_wait()
        };
        if let Some(deadline) = deadline {
            wait = wait.min(deadline.saturating_duration_since(now));
        }
        self.next_try = Some(now + wait);
    }

    /// The Stopped screen: nothing is started from here, the opening is retried at the cap.
    fn stop(&mut self, now: Instant, effects: &mut Vec<Effect>) {
        self.stage = Stage::Watch;
        self.show(Screen::State(StateScreen::Stopped), true, effects);
        self.schedule(now, None);
    }

    fn opened(
        &mut self,
        answer: Answer,
        now: Instant,
        outside: &mut dyn Outside,
        effects: &mut Vec<Effect>,
    ) {
        self.in_flight = false;
        if self.stage == Stage::Attached {
            return;
        }
        match answer {
            Answer::Attached => {
                let present = !self.attached_before && self.launch != Launch::Tray;
                self.stage = Stage::Attached;
                self.attached_before = true;
                self.relaunched = false;
                self.backoff.reset();
                self.next_try = None;
                self.show(Screen::Workbench, present, effects);
            }
            Answer::AlreadyOpen => self.finish(effects),
            Answer::NoAnswer | Answer::ShuttingDown | Answer::Refused => {
                self.unanswered(answer, now, outside, effects);
            }
        }
    }

    fn unanswered(
        &mut self,
        answer: Answer,
        now: Instant,
        outside: &mut dyn Outside,
        effects: &mut Vec<Effect>,
    ) {
        let refused = answer == Answer::Refused;
        match self.stage {
            Stage::Probing => self.finish(effects),
            Stage::Person { started, until } => {
                if refused {
                    self.stop(now, effects);
                    return;
                }
                self.show(Screen::State(StateScreen::Starting), true, effects);
                let (started, until) =
                    if answer == Answer::NoAnswer && !started && outside.may_start_process() {
                        if outside.start_service(StartReason::Person).is_err() {
                            self.stop(now, effects);
                            return;
                        }
                        (true, now + STARTING_PATIENCE)
                    } else {
                        (started, until)
                    };
                self.stage = Stage::Person { started, until };
                if now >= until {
                    self.stop(now, effects);
                } else {
                    self.schedule(now, Some(until));
                }
            }
            Stage::Update { until } => {
                if refused || now >= until {
                    self.stop(now, effects);
                } else {
                    self.schedule(now, Some(until));
                }
            }
            Stage::Recover { tried, try_until } => {
                if refused {
                    self.stop(now, effects);
                    return;
                }
                let mut try_until = try_until;
                if answer == Answer::NoAnswer && now >= try_until {
                    if tried >= RECOVER_TRIES {
                        self.stop(now, effects);
                        return;
                    }
                    try_until = now + RECOVER_PATIENCE;
                    self.stage = Stage::Recover {
                        tried: tried + 1,
                        try_until,
                    };
                    self.try_start(StartReason::Recover, outside);
                }
                // A try window that has already closed bounds nothing: while the service is
                // still shutting down the backoff alone spaces the openings.
                self.schedule(now, (try_until > now).then_some(try_until));
            }
            Stage::Watch => self.schedule(now, None),
            Stage::Attached | Stage::Done => {}
        }
    }

    /// Start the service for `reason` when the session allows it. A start that fails is a try
    /// spent; the opening decides what happens next.
    fn try_start(&self, reason: StartReason, outside: &mut dyn Outside) {
        if outside.may_start_process() {
            let _ = outside.start_service(reason);
        }
    }

    fn ended(
        &mut self,
        end: LinkEnd,
        window_open: bool,
        now: Instant,
        outside: &mut dyn Outside,
        effects: &mut Vec<Effect>,
    ) {
        match end {
            LinkEnd::Goodbye(
                GoodbyeReason::Quit | GoodbyeReason::Uninstall | GoodbyeReason::SessionEnd,
            ) => self.finish(effects),
            LinkEnd::Goodbye(GoodbyeReason::Replace) => {
                self.replaced(window_open, now, outside, effects);
            }
            LinkEnd::Goodbye(GoodbyeReason::Unknown) | LinkEnd::Lost => {
                self.stage = Stage::Recover {
                    tried: 1,
                    try_until: now + RECOVER_PATIENCE,
                };
                self.show(Screen::State(StateScreen::Restarting), false, effects);
                self.try_start(StartReason::Recover, outside);
                self.backoff.reset();
                self.schedule(now, Some(now + RECOVER_PATIENCE));
            }
        }
    }

    /// The service is being replaced. The deploy has already put the new shell in `bin`, so a
    /// shell whose own image differs from it is out of date.
    fn replaced(
        &mut self,
        window_open: bool,
        now: Instant,
        outside: &mut dyn Outside,
        effects: &mut Vec<Effect>,
    ) {
        let out_of_date = match (self.image, outside.bin_image()) {
            (Some(own), Some(installed)) => own != installed,
            _ => false,
        };
        if out_of_date {
            if !window_open || self.relaunched || !outside.may_start_process() {
                self.finish(effects);
                return;
            }
            self.relaunched = true;
            match outside.relaunch_updating() {
                Ok(()) => self.finish(effects),
                Err(_) => self.stop(now, effects),
            }
            return;
        }
        self.stage = Stage::Update {
            until: now + UPDATING_PATIENCE,
        };
        self.show(Screen::State(StateScreen::Updating), false, effects);
        self.backoff.reset();
        self.schedule(now, Some(now + UPDATING_PATIENCE));
    }

    fn start_pressed(
        &mut self,
        now: Instant,
        outside: &mut dyn Outside,
        effects: &mut Vec<Effect>,
    ) {
        if self.stage != Stage::Watch || !outside.may_start_process() {
            return;
        }
        if outside.start_service(StartReason::Person).is_err() {
            return;
        }
        let until = now + STARTING_PATIENCE;
        self.stage = Stage::Person {
            started: true,
            until,
        };
        self.show(Screen::State(StateScreen::Starting), true, effects);
        self.backoff.reset();
        self.schedule(now, Some(until));
    }
}

#[cfg(test)]
mod tests {
    use std::time::UNIX_EPOCH;

    use super::*;

    /// A world the machine asks questions of, and what it was asked to do.
    struct World {
        may_start: bool,
        started: Vec<StartReason>,
        relaunches: u32,
        bin: Option<Image>,
        start_fails: bool,
        relaunch_fails: bool,
    }

    impl World {
        fn new() -> Self {
            Self {
                may_start: true,
                started: Vec::new(),
                relaunches: 0,
                bin: Some(old_image()),
                start_fails: false,
                relaunch_fails: false,
            }
        }
    }

    impl Outside for World {
        fn may_start_process(&self) -> bool {
            self.may_start
        }

        fn start_service(&mut self, reason: StartReason) -> io::Result<()> {
            self.started.push(reason);
            if self.start_fails {
                Err(io::Error::other("no service image"))
            } else {
                Ok(())
            }
        }

        fn relaunch_updating(&mut self) -> io::Result<()> {
            self.relaunches += 1;
            if self.relaunch_fails {
                Err(io::Error::other("no shell image"))
            } else {
                Ok(())
            }
        }

        fn bin_image(&self) -> Option<Image> {
            self.bin
        }
    }

    fn old_image() -> Image {
        Image::new(100, UNIX_EPOCH + Duration::from_secs(10))
    }

    fn new_image() -> Image {
        Image::new(120, UNIX_EPOCH + Duration::from_secs(20))
    }

    /// A machine, a world and a clock that the tests move.
    struct Rig {
        machine: Machine,
        world: World,
        now: Instant,
    }

    impl Rig {
        fn new(launch: Launch) -> Self {
            let now = Instant::now();
            Self {
                machine: Machine::new(launch, Some(old_image()), now),
                world: World::new(),
                now,
            }
        }

        fn send(&mut self, event: Event) -> Vec<Effect> {
            self.machine.on(event, self.now, &mut self.world)
        }

        fn after(&mut self, seconds: u64) {
            self.now += Duration::from_secs(seconds);
        }

        /// Start, and answer the first attempt with `answer`.
        fn start_and_answer(&mut self, answer: Answer) -> Vec<Effect> {
            let started = self.send(Event::Started);
            assert_eq!(started.last(), Some(&Effect::Open));
            self.send(Event::Opened(answer))
        }

        /// Let the wait pass and answer the next attempt with `answer`.
        fn retry_and_answer(&mut self, answer: Answer) -> Vec<Effect> {
            let wake = self.machine.wake_at().expect("an attempt is planned");
            self.now = self.now.max(wake);
            assert_eq!(self.send(Event::Tick), [Effect::Open]);
            self.send(Event::Opened(answer))
        }

        /// Attach and drop the stream with `end`, the window open.
        fn attached_then(&mut self, end: LinkEnd, window_open: bool) -> Vec<Effect> {
            self.start_and_answer(Answer::Attached);
            self.send(Event::Ended { end, window_open })
        }
    }

    fn show(screen: StateScreen, present: bool) -> Effect {
        Effect::Show {
            screen: Screen::State(screen),
            present,
        }
    }

    fn workbench(present: bool) -> Effect {
        Effect::Show {
            screen: Screen::Workbench,
            present,
        }
    }

    // A person's start.

    #[test]
    fn a_person_with_a_service_running_gets_the_workbench_and_no_start() {
        let mut rig = Rig::new(Launch::Person);
        assert_eq!(rig.start_and_answer(Answer::Attached), [workbench(true)]);
        assert!(rig.world.started.is_empty());
    }

    #[test]
    fn a_person_with_no_service_sees_starting_and_the_service_is_started_once() {
        let mut rig = Rig::new(Launch::Person);
        assert_eq!(
            rig.start_and_answer(Answer::NoAnswer),
            [show(StateScreen::Starting, true)]
        );
        assert_eq!(rig.world.started, [StartReason::Person]);
        assert!(rig.retry_and_answer(Answer::NoAnswer).is_empty());
        assert!(rig.retry_and_answer(Answer::NoAnswer).is_empty());
        assert_eq!(rig.world.started, [StartReason::Person], "started once");
        assert_eq!(rig.retry_and_answer(Answer::Attached), [workbench(true)]);
    }

    #[test]
    fn a_person_waits_out_a_service_that_is_shutting_down_and_then_starts_it_once() {
        let mut rig = Rig::new(Launch::Person);
        assert_eq!(
            rig.start_and_answer(Answer::ShuttingDown),
            [show(StateScreen::Starting, true)]
        );
        assert!(rig.world.started.is_empty());
        rig.retry_and_answer(Answer::ShuttingDown);
        assert!(
            rig.world.started.is_empty(),
            "nothing starts while it quits"
        );
        rig.retry_and_answer(Answer::NoAnswer);
        assert_eq!(rig.world.started, [StartReason::Person]);
        rig.retry_and_answer(Answer::NoAnswer);
        assert_eq!(rig.world.started, [StartReason::Person]);
    }

    #[test]
    fn a_person_whose_service_never_answers_ends_on_the_stopped_screen_after_30_seconds() {
        let mut rig = Rig::new(Launch::Person);
        rig.start_and_answer(Answer::NoAnswer);
        rig.after(29);
        assert!(rig.retry_and_answer(Answer::NoAnswer).is_empty());
        rig.after(2);
        assert_eq!(
            rig.retry_and_answer(Answer::NoAnswer),
            [show(StateScreen::Stopped, true)]
        );
        assert_eq!(rig.world.started, [StartReason::Person]);
    }

    #[test]
    fn a_person_who_is_refused_gets_the_stopped_screen() {
        let mut rig = Rig::new(Launch::Person);
        assert_eq!(
            rig.start_and_answer(Answer::Refused),
            [show(StateScreen::Stopped, true)]
        );
        assert!(rig.world.started.is_empty());
    }

    #[test]
    fn a_person_whose_start_fails_gets_the_stopped_screen_at_once() {
        let mut rig = Rig::new(Launch::Person);
        rig.world.start_fails = true;
        assert_eq!(
            rig.start_and_answer(Answer::NoAnswer),
            [
                show(StateScreen::Starting, true),
                show(StateScreen::Stopped, true)
            ]
        );
    }

    #[test]
    fn a_shell_that_finds_another_open_ends_whoever_started_it() {
        for launch in [Launch::Person, Launch::Tray, Launch::Updating] {
            let mut rig = Rig::new(launch);
            assert_eq!(
                rig.start_and_answer(Answer::AlreadyOpen),
                [Effect::Exit],
                "{launch:?}"
            );
            assert!(rig.world.started.is_empty());
        }
    }

    // A tray start.

    #[test]
    fn a_tray_start_with_no_service_ends_having_started_nothing() {
        for answer in [Answer::NoAnswer, Answer::ShuttingDown, Answer::Refused] {
            let mut rig = Rig::new(Launch::Tray);
            assert_eq!(rig.start_and_answer(answer), [Effect::Exit], "{answer:?}");
            assert!(rig.world.started.is_empty());
            assert_eq!(rig.world.relaunches, 0);
            assert_eq!(rig.machine.wake_at(), None);
        }
    }

    #[test]
    fn a_tray_start_that_attaches_shows_no_window() {
        let mut rig = Rig::new(Launch::Tray);
        assert_eq!(rig.start_and_answer(Answer::Attached), [workbench(false)]);
    }

    // An updating start.

    #[test]
    fn an_updating_start_shows_updating_and_starts_nothing() {
        let mut rig = Rig::new(Launch::Updating);
        assert_eq!(
            rig.send(Event::Started),
            [show(StateScreen::Updating, true), Effect::Open]
        );
        assert!(rig.send(Event::Opened(Answer::NoAnswer)).is_empty());
        assert!(rig.retry_and_answer(Answer::ShuttingDown).is_empty());
        assert_eq!(rig.retry_and_answer(Answer::Attached), [workbench(true)]);
        assert!(rig.world.started.is_empty());
    }

    #[test]
    fn an_updating_start_gives_up_after_60_seconds() {
        let mut rig = Rig::new(Launch::Updating);
        rig.start_and_answer(Answer::NoAnswer);
        rig.after(59);
        assert!(rig.retry_and_answer(Answer::NoAnswer).is_empty());
        rig.after(2);
        assert_eq!(
            rig.retry_and_answer(Answer::NoAnswer),
            [show(StateScreen::Stopped, true)]
        );
        assert!(rig.world.started.is_empty());
    }

    // Goodbyes.

    #[test]
    fn a_goodbye_of_quit_uninstall_or_session_end_ends_the_shell_and_starts_nothing() {
        for reason in [
            GoodbyeReason::Quit,
            GoodbyeReason::Uninstall,
            GoodbyeReason::SessionEnd,
        ] {
            let mut rig = Rig::new(Launch::Person);
            assert_eq!(
                rig.attached_then(LinkEnd::Goodbye(reason), true),
                [Effect::Exit],
                "{reason:?}"
            );
            assert!(rig.world.started.is_empty());
            assert_eq!(rig.world.relaunches, 0);
        }
    }

    #[test]
    fn a_replaced_image_with_the_window_open_relaunches_once_and_ends() {
        let mut rig = Rig::new(Launch::Person);
        rig.world.bin = Some(new_image());
        assert_eq!(
            rig.attached_then(LinkEnd::Goodbye(GoodbyeReason::Replace), true),
            [Effect::Exit]
        );
        assert_eq!(rig.world.relaunches, 1);
        assert!(rig.send(Event::Tick).is_empty());
        assert!(rig
            .send(Event::Ended {
                end: LinkEnd::Goodbye(GoodbyeReason::Replace),
                window_open: true
            })
            .is_empty());
        assert_eq!(rig.world.relaunches, 1, "only once");
    }

    #[test]
    fn a_replaced_image_with_the_window_closed_only_ends() {
        let mut rig = Rig::new(Launch::Tray);
        rig.world.bin = Some(new_image());
        assert_eq!(
            rig.attached_then(LinkEnd::Goodbye(GoodbyeReason::Replace), false),
            [Effect::Exit]
        );
        assert_eq!(rig.world.relaunches, 0);
    }

    #[test]
    fn a_replaced_image_whose_relaunch_fails_shows_the_stopped_screen() {
        let mut rig = Rig::new(Launch::Person);
        rig.world.bin = Some(new_image());
        rig.world.relaunch_fails = true;
        assert_eq!(
            rig.attached_then(LinkEnd::Goodbye(GoodbyeReason::Replace), true),
            [show(StateScreen::Stopped, true)]
        );
    }

    #[test]
    fn nothing_is_relaunched_while_the_session_is_ending() {
        let mut rig = Rig::new(Launch::Person);
        rig.world.bin = Some(new_image());
        rig.world.may_start = false;
        assert_eq!(
            rig.attached_then(LinkEnd::Goodbye(GoodbyeReason::Replace), true),
            [Effect::Exit]
        );
        assert_eq!(rig.world.relaunches, 0);
    }

    #[test]
    fn the_same_image_shows_updating_and_never_relaunches() {
        let mut rig = Rig::new(Launch::Person);
        assert_eq!(
            rig.attached_then(LinkEnd::Goodbye(GoodbyeReason::Replace), true),
            [show(StateScreen::Updating, false)]
        );
        assert!(rig.retry_and_answer(Answer::NoAnswer).is_empty());
        assert_eq!(rig.retry_and_answer(Answer::Attached), [workbench(false)]);
        assert_eq!(rig.world.relaunches, 0);
        assert!(rig.world.started.is_empty());
    }

    #[test]
    fn the_same_image_goes_to_stopped_on_any_refusal_and_does_not_relaunch() {
        let mut rig = Rig::new(Launch::Person);
        rig.attached_then(LinkEnd::Goodbye(GoodbyeReason::Replace), true);
        assert_eq!(
            rig.retry_and_answer(Answer::Refused),
            [show(StateScreen::Stopped, true)]
        );
        assert_eq!(rig.world.relaunches, 0);
    }

    #[test]
    fn the_same_image_gives_up_after_60_seconds() {
        let mut rig = Rig::new(Launch::Person);
        rig.attached_then(LinkEnd::Goodbye(GoodbyeReason::Replace), true);
        rig.after(61);
        assert_eq!(
            rig.retry_and_answer(Answer::NoAnswer),
            [show(StateScreen::Stopped, true)]
        );
    }

    #[test]
    fn an_image_that_cannot_be_read_counts_as_the_same() {
        let mut rig = Rig::new(Launch::Person);
        rig.world.bin = None;
        assert_eq!(
            rig.attached_then(LinkEnd::Goodbye(GoodbyeReason::Replace), true),
            [show(StateScreen::Updating, false)]
        );
        assert_eq!(rig.world.relaunches, 0);
    }

    // A loss without goodbye.

    #[test]
    fn a_loss_without_goodbye_shows_restarting_and_starts_the_service_for_recover() {
        let mut rig = Rig::new(Launch::Person);
        assert_eq!(
            rig.attached_then(LinkEnd::Lost, true),
            [show(StateScreen::Restarting, false)]
        );
        assert_eq!(rig.world.started, [StartReason::Recover]);
        assert_eq!(rig.retry_and_answer(Answer::Attached), [workbench(false)]);
    }

    #[test]
    fn an_unknown_goodbye_is_a_loss() {
        let mut rig = Rig::new(Launch::Person);
        assert_eq!(
            rig.attached_then(LinkEnd::Goodbye(GoodbyeReason::Unknown), true),
            [show(StateScreen::Restarting, false)]
        );
    }

    #[test]
    fn three_recover_tries_and_then_the_stopped_screen() {
        let mut rig = Rig::new(Launch::Person);
        rig.attached_then(LinkEnd::Lost, true);
        let mut effects = Vec::new();
        for _ in 0..40 {
            rig.after(RECOVER_PATIENCE.as_secs() + 1);
            effects = rig.retry_and_answer(Answer::NoAnswer);
            if !effects.is_empty() {
                break;
            }
        }
        assert_eq!(effects, [show(StateScreen::Stopped, true)]);
        assert_eq!(rig.world.started, [StartReason::Recover; 3]);
    }

    #[test]
    fn a_refusal_while_recovering_is_the_stopped_screen() {
        let mut rig = Rig::new(Launch::Person);
        rig.attached_then(LinkEnd::Lost, true);
        assert_eq!(
            rig.retry_and_answer(Answer::Refused),
            [show(StateScreen::Stopped, true)]
        );
        assert_eq!(rig.world.started, [StartReason::Recover]);
    }

    #[test]
    fn a_service_that_is_shutting_down_spends_no_recover_try() {
        let mut rig = Rig::new(Launch::Person);
        rig.attached_then(LinkEnd::Lost, true);
        for _ in 0..5 {
            rig.after(11);
            assert!(rig.retry_and_answer(Answer::ShuttingDown).is_empty());
            let wake = rig.machine.wake_at().expect("the opening is retried");
            assert!(wake > rig.now, "no busy loop while the service quits");
        }
        assert_eq!(rig.world.started, [StartReason::Recover]);
    }

    // Stopped.

    fn stopped_rig() -> Rig {
        let mut rig = Rig::new(Launch::Person);
        rig.start_and_answer(Answer::Refused);
        rig
    }

    #[test]
    fn stopped_keeps_retrying_at_the_cap_and_starts_nothing() {
        let mut rig = stopped_rig();
        for _ in 0..4 {
            let before = rig.now;
            let wake = rig.machine.wake_at().expect("the opening is retried");
            assert_eq!(wake - before, BACKOFF_MAX);
            assert!(rig.retry_and_answer(Answer::NoAnswer).is_empty());
            assert!(rig.retry_and_answer(Answer::Refused).is_empty());
        }
        assert!(rig.world.started.is_empty());
    }

    #[test]
    fn a_service_that_comes_back_picks_the_stopped_shell_up() {
        let mut rig = stopped_rig();
        assert_eq!(rig.retry_and_answer(Answer::Attached), [workbench(true)]);
    }

    #[test]
    fn the_start_button_is_a_persons_start_and_shows_starting() {
        let mut rig = stopped_rig();
        assert_eq!(
            rig.send(Event::StartPressed),
            [show(StateScreen::Starting, true)]
        );
        assert_eq!(rig.world.started, [StartReason::Person]);
        assert_eq!(rig.retry_and_answer(Answer::Attached), [workbench(true)]);
    }

    #[test]
    fn the_start_button_does_nothing_when_the_screen_is_not_stopped() {
        let mut rig = Rig::new(Launch::Person);
        rig.start_and_answer(Answer::NoAnswer);
        assert!(rig.send(Event::StartPressed).is_empty());
        assert_eq!(
            rig.world.started,
            [StartReason::Person],
            "only the first start"
        );
    }

    #[test]
    fn a_service_that_is_not_answering_after_the_start_button_ends_stopped_again() {
        let mut rig = stopped_rig();
        rig.send(Event::StartPressed);
        rig.after(31);
        assert_eq!(
            rig.retry_and_answer(Answer::NoAnswer),
            [show(StateScreen::Stopped, true)]
        );
    }

    // The session.

    #[test]
    fn nothing_starts_while_the_session_is_ending() {
        let mut rig = Rig::new(Launch::Person);
        rig.world.may_start = false;
        rig.start_and_answer(Answer::NoAnswer);
        rig.retry_and_answer(Answer::NoAnswer);
        rig.attached_then(LinkEnd::Lost, true);
        for _ in 0..8 {
            rig.after(11);
            rig.retry_and_answer(Answer::NoAnswer);
        }
        rig.send(Event::StartPressed);
        assert!(rig.world.started.is_empty());
        assert_eq!(rig.world.relaunches, 0);
    }

    // The loop's side.

    #[test]
    fn a_tick_before_the_planned_time_or_during_an_attempt_does_nothing() {
        let mut rig = Rig::new(Launch::Person);
        rig.start_and_answer(Answer::NoAnswer);
        assert!(rig.send(Event::Tick).is_empty(), "too early");
        let wake = rig.machine.wake_at().unwrap();
        rig.now = wake;
        assert_eq!(rig.send(Event::Tick), [Effect::Open]);
        assert_eq!(rig.machine.wake_at(), None, "an attempt is in flight");
        assert!(rig.send(Event::Tick).is_empty());
    }

    #[test]
    fn an_ended_stream_before_the_first_attach_is_ignored() {
        let mut rig = Rig::new(Launch::Person);
        rig.send(Event::Started);
        assert!(rig
            .send(Event::Ended {
                end: LinkEnd::Lost,
                window_open: false
            })
            .is_empty());
    }

    #[test]
    fn the_polls_follow_the_relay_backoff() {
        let mut rig = Rig::new(Launch::Person);
        rig.start_and_answer(Answer::NoAnswer);
        let first = rig.machine.wake_at().unwrap() - rig.now;
        rig.retry_and_answer(Answer::NoAnswer);
        let second = rig.machine.wake_at().unwrap() - rig.now;
        assert_eq!(first, ghostlight_relay::BACKOFF_FIRST);
        assert_eq!(second, ghostlight_relay::BACKOFF_FIRST * 2);
    }

    #[test]
    fn a_link_error_is_an_answer() {
        use ghostlight_relay::endpoint::ConnectError;
        let refused = |code| LinkError::Refused {
            code,
            say: String::new(),
        };
        assert_eq!(
            Answer::from(&LinkError::Connect(ConnectError::NoService)),
            Answer::NoAnswer
        );
        assert_eq!(
            Answer::from(&LinkError::Connect(ConnectError::Closed)),
            Answer::NoAnswer
        );
        assert_eq!(
            Answer::from(&LinkError::Io(io::Error::other("x"))),
            Answer::NoAnswer
        );
        assert_eq!(
            Answer::from(&LinkError::Connect(ConnectError::ForeignServer)),
            Answer::Refused
        );
        assert_eq!(
            Answer::from(&refused(Some(RefusalCode::ShuttingDown))),
            Answer::ShuttingDown
        );
        assert_eq!(
            Answer::from(&refused(Some(RefusalCode::Capacity))),
            Answer::Refused
        );
        assert_eq!(Answer::from(&refused(None)), Answer::Refused);
        assert_eq!(
            Answer::from(&LinkError::Contract { reason: "x" }),
            Answer::Refused
        );
        assert_eq!(Answer::from(&LinkError::AlreadyOpen), Answer::AlreadyOpen);
    }

    #[test]
    fn an_image_is_the_same_only_with_the_same_length_and_time() {
        let at = UNIX_EPOCH + Duration::from_secs(5);
        assert_eq!(Image::new(1, at), Image::new(1, at));
        assert_ne!(Image::new(1, at), Image::new(2, at));
        assert_ne!(
            Image::new(1, at),
            Image::new(1, at + Duration::from_secs(1))
        );
    }

    #[test]
    fn the_image_of_a_file_reads_its_metadata_and_a_missing_file_has_none() {
        let scratch = crate::scratch::Scratch::named("image");
        let path = scratch.join("image");
        std::fs::write(&path, b"abc").unwrap();
        let image = Image::of(&path).expect("the file has metadata");
        assert_eq!(image.length, 3);
        std::fs::remove_file(&path).unwrap();
        assert_eq!(Image::of(&path), None);
    }
}
