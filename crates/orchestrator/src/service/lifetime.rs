//! How a service's life ends: the quit drain and the discovery deadline.
//!
//! A quit refuses new work, lets what is in flight finish within a bound, and then asks the
//! desktop to exit. A `serve` start that never becomes discoverable stops by itself. Both are
//! small state machines with no knowledge of the executor, the sockets or the desktop, so each is
//! tested alone.

use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

/// How long a quit lets in-flight invocations finish before the service exits anyway.
pub const QUIT_GRACE: Duration = Duration::from_secs(10);

/// How long a `serve` start may take to publish discovery before it stops by itself.
pub const DISCOVERY_WINDOW: Duration = Duration::from_secs(30);

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

/// Watches for discovery to be published within a deadline.
#[derive(Default)]
pub struct DiscoveryWatch {
    settled: Mutex<bool>,
    changed: Condvar,
}

impl DiscoveryWatch {
    /// Stop watching: discovery was published, or the service is going away.
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
        let watch = Arc::clone(self);
        thread::Builder::new()
            .name("ghostlight-discovery-deadline".into())
            .spawn(move || {
                let settled = lock(&watch.settled);
                let (settled, _) = watch
                    .changed
                    .wait_timeout_while(
                        settled,
                        deadline.saturating_duration_since(Instant::now()),
                        |settled| !*settled,
                    )
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
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
    fn the_deadline_fires_when_discovery_is_never_published() {
        let watch = Arc::new(DiscoveryWatch::default());
        let (expired, fired) = mpsc::channel();
        watch
            .arm(Instant::now() + Duration::from_millis(50), move || {
                expired.send(()).unwrap();
            })
            .unwrap();
        assert!(fired.recv_timeout(Duration::from_secs(5)).is_ok());
    }

    #[test]
    fn the_deadline_does_not_fire_after_discovery_is_published() {
        let watch = Arc::new(DiscoveryWatch::default());
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
        let watch = Arc::new(DiscoveryWatch::default());
        watch.settle();
        let (expired, fired) = mpsc::channel();
        watch
            .arm(Instant::now(), move || {
                let _ = expired.send(());
            })
            .unwrap();
        assert!(fired.recv_timeout(Duration::from_millis(300)).is_err());
    }
}
