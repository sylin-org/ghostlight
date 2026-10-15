//! How a service's life begins and ends: what a start reason allows, the quit drain and the
//! readiness limit.
//!
//! A start for an update waits for the previous service to leave; every other start gives way to
//! a service that is already there. A quit refuses new work, lets what is in flight finish within
//! a bound, and then asks the desktop to exit. A start that never becomes ready stops by itself.
//! Each is a small state machine with no knowledge of the executor, the sockets or the desktop,
//! so each is tested alone.

use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use ghostlight_relay::protocol::StartReason;

/// How long a quit lets in-flight invocations finish before the service exits anyway.
pub const QUIT_GRACE: Duration = Duration::from_secs(10);

/// How long an update start waits for the previous service to release `run/service.lock`: the
/// installer's quit lets the old service drain first.
pub const UPDATE_LOCK_WAIT: Duration = Duration::from_secs(30);

/// How long a service may take, from the moment it holds the lock, to accept openings before it
/// stops by itself.
pub const READY_LIMIT: Duration = Duration::from_secs(30);

/// How often a ready watch looks at its clock.
const READY_POLL: Duration = Duration::from_millis(250);

/// How long a start with `reason` asks for a held service lock before it gives way.
///
/// Only an update waits. Any other start finds another service already serving, and that
/// service is the answer to the starter's own connection.
#[must_use]
pub fn lock_wait(reason: StartReason) -> Duration {
    match reason {
        StartReason::Update => UPDATE_LOCK_WAIT,
        _ => Duration::ZERO,
    }
}

/// Whether the quit latch stops a start with `reason`. A browser or a recovering shell must not
/// bring back a Ghostlight the person quit; any other start clears the latch and serves.
#[must_use]
pub fn refused_by_quit_latch(reason: StartReason) -> bool {
    matches!(reason, StartReason::Browser | StartReason::Recover)
}

/// The source of the current time for the ready watch.
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

/// A quit has begun, so no new invocation is admitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShuttingDown;

/// Counts invocations in flight and refuses new ones once a quit has begun.
#[derive(Default)]
pub struct Drain {
    state: Mutex<DrainState>,
    idle: Condvar,
}

#[derive(Default)]
struct DrainState {
    in_flight: usize,
    quitting: bool,
}

/// One invocation in flight. Dropping it tells the drain the invocation has finished.
pub struct InFlight {
    drain: Arc<Drain>,
}

impl Drain {
    /// Admit one invocation, or refuse it because a quit has begun.
    pub fn admit(self: &Arc<Self>) -> Result<InFlight, ShuttingDown> {
        let mut state = lock(&self.state);
        if state.quitting {
            return Err(ShuttingDown);
        }
        state.in_flight += 1;
        Ok(InFlight {
            drain: Arc::clone(self),
        })
    }

    /// Refuse new invocations from now on. Returns whether this call began the quit.
    pub fn begin_quit(&self) -> bool {
        let mut state = lock(&self.state);
        !std::mem::replace(&mut state.quitting, true)
    }

    /// Whether a quit has begun.
    pub fn is_quitting(&self) -> bool {
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

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    #[test]
    fn a_quit_refuses_new_work_and_waits_for_work_in_flight() {
        let drain = Arc::new(Drain::default());
        let running = drain.admit().unwrap();
        assert!(drain.begin_quit());
        assert!(!drain.begin_quit(), "only the first quit begins the drain");
        assert_eq!(drain.admit().err(), Some(ShuttingDown));

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
        drain.begin_quit();
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

    /// A clock the test moves by hand.
    struct FakeClock(Mutex<Instant>);

    impl FakeClock {
        fn advance(&self, by: Duration) {
            *lock(&self.0) += by;
        }
    }

    impl Clock for Arc<FakeClock> {
        fn now(&self) -> Instant {
            *lock(&self.0)
        }
    }

    #[test]
    fn a_service_that_is_not_ready_within_the_limit_of_taking_the_lock_stops() {
        let taken = Instant::now();
        let clock = Arc::new(FakeClock(Mutex::new(taken)));
        let watch = Arc::new(ReadyWatch::default());
        let (expired, fired) = mpsc::channel();
        watch
            .arm_by(
                Arc::new(Arc::clone(&clock)),
                taken + READY_LIMIT,
                move || {
                    let _ = expired.send(());
                },
            )
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
        let taken = Instant::now();
        let clock = Arc::new(FakeClock(Mutex::new(taken)));
        let watch = Arc::new(ReadyWatch::default());
        let (expired, fired) = mpsc::channel();
        watch
            .arm_by(
                Arc::new(Arc::clone(&clock)),
                taken + READY_LIMIT,
                move || {
                    let _ = expired.send(());
                },
            )
            .unwrap();
        clock.advance(READY_LIMIT - Duration::from_secs(1));
        watch.settle();
        clock.advance(READY_LIMIT);
        assert!(fired.recv_timeout(Duration::from_millis(600)).is_err());
    }

    #[test]
    fn only_an_update_start_waits_for_a_held_lock() {
        assert_eq!(lock_wait(StartReason::Update), UPDATE_LOCK_WAIT);
        for reason in [
            StartReason::Agent,
            StartReason::Browser,
            StartReason::Command,
            StartReason::Person,
            StartReason::Recover,
        ] {
            assert_eq!(lock_wait(reason), Duration::ZERO, "{reason:?}");
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
}
