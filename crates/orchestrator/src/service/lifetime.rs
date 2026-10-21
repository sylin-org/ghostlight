//! How a service's life begins and ends: what a start reason allows, the quit drain and the
//! readiness limit.
//!
//! A start for an update waits a long time for the previous service to leave; every other start
//! waits a few seconds, so it also outlasts a service that is leaving, and gives way to a service
//! that stays. A quit refuses new work, lets what is in flight finish within
//! a bound, and then asks the desktop to exit. A start that never becomes ready stops by itself.
//! Each is a small state machine with no knowledge of the executor, the sockets or the desktop,
//! so each is tested alone.

use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use ghostlight_relay::protocol::StartReason;

use crate::language::outcome::QuitCause;
use crate::sync::lock;

/// How long a quit lets in-flight invocations finish before the service exits anyway.
pub const QUIT_GRACE: Duration = Duration::from_secs(10);

/// How long a quit may take, once its drain is over, to hand out its last resume tokens, say its
/// goodbyes and release its sessions' tabs before the browser goodbye. An update start waits
/// [`UPDATE_LOCK_WAIT`], so the drain and this end fit inside it.
pub const END_LIMIT: Duration = Duration::from_secs(5);

/// How long an update start waits for the previous service to release `run/service.lock`: the
/// installer's quit lets the old service drain first.
pub const UPDATE_LOCK_WAIT: Duration = Duration::from_secs(30);

/// How long any other start waits for a held `run/service.lock`.
///
/// A service that decided to leave on idle keeps the lock and the endpoint until its teardown
/// ends, and says nothing to a start that arrives meanwhile. A start that gave way at once would
/// leave the caller with no service when the old one finished. Waiting outlasts that teardown and
/// serves in its place. Against a service that stays, the wait only delays a start that nobody
/// awaits, because the caller reaches the running service by its endpoint.
pub const LEAVING_LOCK_WAIT: Duration = Duration::from_secs(5);

/// How long a service may take, from the moment it holds the lock, to accept openings before it
/// stops by itself.
pub const READY_LIMIT: Duration = Duration::from_secs(30);

/// How often a ready watch looks at its clock.
const READY_POLL: Duration = Duration::from_millis(250);

/// How long a start with `reason` asks for a held service lock before it gives way.
///
/// An update waits for the service it replaces, [`UPDATE_LOCK_WAIT`]. Any other start waits
/// [`LEAVING_LOCK_WAIT`], long enough for a leaving service to release the lock, and gives way
/// to one that stays: that service is the answer to the starter's own connection.
#[must_use]
pub fn lock_wait(reason: StartReason) -> Duration {
    match reason {
        StartReason::Update => UPDATE_LOCK_WAIT,
        _ => LEAVING_LOCK_WAIT,
    }
}

/// Whether the quit latch stops a start with `reason`. A browser or a recovering shell must not
/// bring back a Ghostlight the person quit; any other start clears the latch and serves.
#[must_use]
pub fn refused_by_quit_latch(reason: StartReason) -> bool {
    matches!(reason, StartReason::Browser | StartReason::Recover)
}

/// Whether a start with `reason` takes the place of a service that may have ended moments ago:
/// the installer's start after its swap, or the shell's recovery of a service it lost without a
/// goodbye. The browsers that service served are then on their way back to this one.
#[must_use]
pub fn replaces_a_service(reason: StartReason) -> bool {
    matches!(reason, StartReason::Update | StartReason::Recover)
}

/// The source of the current time for the ready watch and the idle watch.
pub trait Clock: Send + Sync + 'static {
    /// The current instant.
    fn now(&self) -> Instant;
}

/// The real clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// A clock a test moves by hand: real time passing never moves it. Clones share one reading.
#[cfg(test)]
#[derive(Clone)]
pub struct HandClock(Arc<Mutex<Instant>>);

#[cfg(test)]
impl Default for HandClock {
    /// A clock that reads the moment it was made until it is moved.
    fn default() -> Self {
        Self(Arc::new(Mutex::new(Instant::now())))
    }
}

#[cfg(test)]
impl HandClock {
    /// Move the clock forward by `by`.
    pub fn advance(&self, by: Duration) {
        *lock(&self.0) += by;
    }
}

#[cfg(test)]
impl Clock for HandClock {
    fn now(&self) -> Instant {
        *lock(&self.0)
    }
}

/// A quit has begun for this cause, so no new invocation is admitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShuttingDown(pub QuitCause);

/// Counts invocations in flight and refuses new ones once a quit has begun.
#[derive(Default)]
pub struct Drain {
    state: Mutex<DrainState>,
    idle: Condvar,
}

#[derive(Default)]
struct DrainState {
    in_flight: usize,
    /// Why the quit under way began, once one has.
    quitting: Option<QuitCause>,
}

/// One invocation in flight. Dropping it tells the drain the invocation has finished.
pub struct InFlight {
    drain: Arc<Drain>,
}

impl Drain {
    /// Admit one invocation, or refuse it because a quit has begun, naming the quit's cause.
    pub fn admit(self: &Arc<Self>) -> Result<InFlight, ShuttingDown> {
        let mut state = lock(&self.state);
        if let Some(cause) = state.quitting {
            return Err(ShuttingDown(cause));
        }
        state.in_flight += 1;
        Ok(InFlight {
            drain: Arc::clone(self),
        })
    }

    /// Refuse new invocations from now on, for `cause`. Returns whether this call began the
    /// quit; a quit already under way keeps the cause it began with.
    pub fn begin_quit(&self, cause: QuitCause) -> bool {
        let mut state = lock(&self.state);
        if state.quitting.is_some() {
            return false;
        }
        state.quitting = Some(cause);
        true
    }

    /// Whether a quit has begun.
    pub fn is_quitting(&self) -> bool {
        self.cause().is_some()
    }

    /// Why the quit under way began, once one has.
    pub fn cause(&self) -> Option<QuitCause> {
        lock(&self.state).quitting
    }

    /// Wait until nothing is in flight or the grace elapses. Returns whether the drain emptied.
    pub fn wait_idle(&self, grace: Duration) -> bool {
        let state = lock(&self.state);
        let (state, _) = self
            .idle
            .wait_timeout_while(state, grace, |state| state.in_flight > 0)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.in_flight == 0
    }

    /// How many invocations are in flight.
    pub fn in_flight(&self) -> usize {
        lock(&self.state).in_flight
    }
}

impl Drop for InFlight {
    fn drop(&mut self) {
        let mut state = lock(&self.drain.state);
        state.in_flight -= 1;
        if state.in_flight == 0 {
            self.drain.idle.notify_all();
        }
    }
}

/// Watches for the service to accept openings within a deadline.
#[derive(Default)]
pub struct ReadyWatch {
    settled: Mutex<bool>,
    changed: Condvar,
}

impl ReadyWatch {
    /// Stop watching: the service is ready, or it is going away.
    pub fn settle(&self) {
        *lock(&self.settled) = true;
        self.changed.notify_all();
    }

    /// Run `on_expiry` on a watcher thread if nothing settles the watch by `deadline`.
    pub fn arm(
        self: &Arc<Self>,
        deadline: Instant,
        on_expiry: impl FnOnce() + Send + 'static,
    ) -> std::io::Result<()> {
        self.arm_by(Arc::new(SystemClock), deadline, on_expiry)
    }

    /// Like [`ReadyWatch::arm`], with the deadline judged by `clock`.
    pub fn arm_by(
        self: &Arc<Self>,
        clock: Arc<dyn Clock>,
        deadline: Instant,
        on_expiry: impl FnOnce() + Send + 'static,
    ) -> std::io::Result<()> {
        let watch = Arc::clone(self);
        thread::Builder::new()
            .name("ghostlight-ready-limit".into())
            .spawn(move || {
                let mut settled = lock(&watch.settled);
                while !*settled && clock.now() < deadline {
                    let wait = deadline
                        .saturating_duration_since(clock.now())
                        .min(READY_POLL);
                    settled = watch
                        .changed
                        .wait_timeout_while(settled, wait, |settled| !*settled)
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .0;
                }
                if !*settled {
                    drop(settled);
                    on_expiry();
                }
            })
            .map(drop)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    #[test]
    fn a_quit_refuses_new_work_and_waits_for_work_in_flight() {
        let drain = Arc::new(Drain::default());
        let running = drain.admit().unwrap();
        assert!(drain.begin_quit(QuitCause::Replace));
        assert!(
            !drain.begin_quit(QuitCause::Uninstall),
            "only the first quit begins the drain"
        );
        assert_eq!(
            drain.admit().err(),
            Some(ShuttingDown(QuitCause::Replace)),
            "the drain keeps the cause its quit began with"
        );
        assert_eq!(drain.cause(), Some(QuitCause::Replace));

        let started = Instant::now();
        assert!(!drain.wait_idle(Duration::from_millis(100)));
        assert!(started.elapsed() >= Duration::from_millis(100));

        let finisher = thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            drop(running);
        });
        assert!(drain.wait_idle(Duration::from_secs(5)));
        finisher.join().unwrap();
        assert_eq!(drain.in_flight(), 0);
    }

    #[test]
    fn an_idle_drain_is_empty_at_once() {
        let drain = Arc::new(Drain::default());
        drain.begin_quit(QuitCause::Quit);
        let started = Instant::now();
        assert!(drain.wait_idle(Duration::from_secs(5)));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn the_deadline_fires_when_the_service_is_never_ready() {
        let watch = Arc::new(ReadyWatch::default());
        let (expired, fired) = mpsc::channel();
        watch
            .arm(Instant::now() + Duration::from_millis(50), move || {
                expired.send(()).unwrap();
            })
            .unwrap();
        assert!(fired.recv_timeout(Duration::from_secs(5)).is_ok());
    }

    #[test]
    fn the_deadline_does_not_fire_after_the_service_is_ready() {
        let watch = Arc::new(ReadyWatch::default());
        let (expired, fired) = mpsc::channel();
        watch
            .arm(Instant::now() + Duration::from_millis(150), move || {
                let _ = expired.send(());
            })
            .unwrap();
        watch.settle();
        assert!(fired.recv_timeout(Duration::from_millis(500)).is_err());
    }

    #[test]
    fn a_watch_settled_before_it_is_armed_never_fires() {
        let watch = Arc::new(ReadyWatch::default());
        watch.settle();
        let (expired, fired) = mpsc::channel();
        watch
            .arm(Instant::now(), move || {
                let _ = expired.send(());
            })
            .unwrap();
        assert!(fired.recv_timeout(Duration::from_millis(300)).is_err());
    }

    #[test]
    fn a_service_that_is_not_ready_within_the_limit_of_taking_the_lock_stops() {
        let clock = HandClock::default();
        let taken = clock.now();
        let watch = Arc::new(ReadyWatch::default());
        let (expired, fired) = mpsc::channel();
        watch
            .arm_by(Arc::new(clock.clone()), taken + READY_LIMIT, move || {
                let _ = expired.send(());
            })
            .unwrap();

        clock.advance(READY_LIMIT - Duration::from_secs(1));
        assert!(
            fired.recv_timeout(Duration::from_millis(600)).is_err(),
            "one second inside the limit"
        );
        clock.advance(Duration::from_secs(1));
        assert!(
            fired.recv_timeout(Duration::from_secs(5)).is_ok(),
            "the limit is reached"
        );
    }

    #[test]
    fn a_service_that_is_ready_inside_the_limit_is_left_alone() {
        let clock = HandClock::default();
        let taken = clock.now();
        let watch = Arc::new(ReadyWatch::default());
        let (expired, fired) = mpsc::channel();
        watch
            .arm_by(Arc::new(clock.clone()), taken + READY_LIMIT, move || {
                let _ = expired.send(());
            })
            .unwrap();
        clock.advance(READY_LIMIT - Duration::from_secs(1));
        watch.settle();
        clock.advance(READY_LIMIT);
        assert!(fired.recv_timeout(Duration::from_millis(600)).is_err());
    }

    #[test]
    fn an_update_start_waits_longest_for_a_held_lock_and_every_other_start_a_few_seconds() {
        assert_eq!(lock_wait(StartReason::Update), UPDATE_LOCK_WAIT);
        for reason in [
            StartReason::Agent,
            StartReason::Browser,
            StartReason::Command,
            StartReason::Person,
            StartReason::Recover,
        ] {
            assert_eq!(lock_wait(reason), LEAVING_LOCK_WAIT, "{reason:?}");
        }
    }

    #[test]
    fn only_a_browser_or_recover_start_is_refused_by_the_quit_latch() {
        for reason in [StartReason::Browser, StartReason::Recover] {
            assert!(refused_by_quit_latch(reason), "{reason:?}");
        }
        for reason in [
            StartReason::Agent,
            StartReason::Command,
            StartReason::Person,
            StartReason::Update,
        ] {
            assert!(!refused_by_quit_latch(reason), "{reason:?}");
        }
    }

    #[test]
    fn only_an_update_or_recover_start_replaces_a_service() {
        for reason in [StartReason::Update, StartReason::Recover] {
            assert!(replaces_a_service(reason), "{reason:?}");
        }
        for reason in [
            StartReason::Agent,
            StartReason::Browser,
            StartReason::Command,
            StartReason::Person,
            StartReason::Unknown,
        ] {
            assert!(!replaces_a_service(reason), "{reason:?}");
        }
    }
}
