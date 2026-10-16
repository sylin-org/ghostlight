//! Idle exit: the service leaves when no stream of any role has been open for [`IDLE_EXIT`]
//! (ADR-0199 Decision 12).
//!
//! [`Streams`] counts the open streams of every role: `mcp`, `browser`, `control` and `desktop`.
//! An attached shell is a stream, so on a desktop that can show one the service never idles out;
//! the exit is for a headless service, such as one an agent started over SSH or in CI, where a
//! forgotten resident process costs most. The count is event-driven, so a stream that opens and
//! closes between two looks at the clock still restarts the span. [`Streams::verdict`] is the
//! rule, with the clock injected and no thread, so it is tested alone; [`watch`] is the thread
//! that asks it.
//!
//! The idle stop is the service's own reason and is not relay vocabulary: no stream is open, so
//! there is nobody to say goodbye to, and it never sets the quit latch (see `Quit::idle_exit`).

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::browser::AdapterLifecycleObserver;

use super::lifetime::Clock;

/// How long the service may hold no stream before it exits.
pub const IDLE_EXIT: Duration = Duration::from_secs(10 * 60);

/// The longest the idle watch sleeps before it asks again.
const WATCH_POLL: Duration = Duration::from_secs(1);

/// The shortest the idle watch sleeps, which only a limit shorter than a test's span reaches.
const WATCH_POLL_FLOOR: Duration = Duration::from_millis(5);

/// How many times the idle watch looks inside one limit.
const LOOKS_PER_LIMIT: u32 = 10;

/// The name of the thread that watches for idleness.
const THREAD_NAME: &str = "ghostlight-idle-exit";

/// What the idle rule says about the service now.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Verdict {
    /// At least one stream is open, so nothing is armed.
    Streams,
    /// No stream is open and the limit has not passed; this much of it is left.
    Wait(Duration),
    /// No stream was open for the whole limit and no work is held: the service leaves.
    Leave,
}

struct State {
    /// How many streams are open.
    open: usize,
    /// When the last stream closed, or when the service was born; `None` while a stream is open.
    since: Option<Instant>,
    /// The verdict was [`Verdict::Leave`], so no stream is admitted any more.
    leaving: bool,
}

/// The open streams of every role, and the idle rule over them.
pub struct Streams {
    state: Mutex<State>,
    clock: Arc<dyn Clock>,
    limit: Duration,
}

impl Streams {
    /// A count with no stream open, armed from now: the service leaves `limit` after `clock`
    /// says so unless a stream opens first. [`Streams::published`] arms it again from the moment
    /// the service accepts openings.
    #[must_use]
    pub fn new(clock: Arc<dyn Clock>, limit: Duration) -> Arc<Self> {
        let since = Some(clock.now());
        Arc::new(Self {
            state: Mutex::new(State {
                open: 0,
                since,
                leaving: false,
            }),
            clock,
            limit,
        })
    }

    /// Admit one stream, or `None` once the service has decided to leave. Any admitted stream
    /// disarms the limit; the guard re-arms it when the last one is dropped.
    #[must_use]
    pub fn admit(self: &Arc<Self>) -> Option<Open> {
        self.enter().then(|| Open {
            streams: Arc::clone(self),
        })
    }

    /// Count one stream whose end is reported by [`Streams::depart`] rather than by a guard.
    /// Returns whether the service is still staying.
    fn enter(&self) -> bool {
        let mut state = lock(&self.state);
        if state.leaving {
            return false;
        }
        state.open += 1;
        state.since = None;
        true
    }

    /// The service accepts openings from now. A service with no stream starts its span here,
    /// not at construction, so the time spent starting never counts as idleness.
    pub fn published(&self) {
        let mut state = lock(&self.state);
        if state.open == 0 && !state.leaving {
            state.since = Some(self.clock.now());
        }
    }

    /// One counted stream ended; the last one arms the limit.
    fn depart(&self) {
        let mut state = lock(&self.state);
        state.open = state.open.saturating_sub(1);
        if state.open == 0 {
            state.since = Some(self.clock.now());
        }
    }

    /// What the rule says now. `holds_work` is asked only once the limit has passed: work the
    /// executor still holds, with no stream to deliver it to, re-arms the limit instead of
    /// leaving, so the service looks again a whole limit later. A [`Verdict::Leave`] is final and
    /// closes the count to new streams.
    pub fn verdict(&self, holds_work: impl FnOnce() -> bool) -> Verdict {
        let mut state = lock(&self.state);
        if state.leaving {
            return Verdict::Leave;
        }
        let Some(since) = state.since else {
            return Verdict::Streams;
        };
        let now = self.clock.now();
        let due = since + self.limit;
        if now < due {
            return Verdict::Wait(due - now);
        }
        if holds_work() {
            state.since = Some(now);
            return Verdict::Wait(self.limit);
        }
        state.leaving = true;
        Verdict::Leave
    }

    /// How long the watch sleeps between looks: a tenth of the limit, within a second.
    fn poll(&self) -> Duration {
        (self.limit / LOOKS_PER_LIMIT).clamp(WATCH_POLL_FLOOR, WATCH_POLL)
    }
}

/// One stream open. Dropping it ends the stream, and the last one arms the idle limit.
pub struct Open {
    streams: Arc<Streams>,
}

impl Drop for Open {
    fn drop(&mut self) {
        self.streams.depart();
    }
}

/// Counts browser streams for [`Streams`] from the port's lifecycle events and passes every
/// event on to `inner`.
///
/// A browser stream outlives the opening that created it, so no guard can hold it. A connection
/// that replaces another one for the same browser is still one stream.
pub struct CountBrowsers {
    streams: Arc<Streams>,
    inner: Arc<dyn AdapterLifecycleObserver>,
}

impl CountBrowsers {
    /// An observer that counts into `streams` and forwards to `inner`.
    #[must_use]
    pub fn new(streams: Arc<Streams>, inner: Arc<dyn AdapterLifecycleObserver>) -> Self {
        Self { streams, inner }
    }
}

impl AdapterLifecycleObserver for CountBrowsers {
    fn adapter_attached(&self, browser_id: &str, replaced: bool) {
        if !replaced {
            self.streams.enter();
        }
        self.inner.adapter_attached(browser_id, replaced);
    }

    fn adapter_detached(&self, browser_id: &str) {
        self.streams.depart();
        self.inner.adapter_detached(browser_id);
    }

    fn adapter_unanswered(&self, browser_id: &str) {
        self.inner.adapter_unanswered(browser_id);
    }

    fn adapter_answering(&self, browser_id: &str) {
        self.inner.adapter_answering(browser_id);
    }
}

/// Start the thread that asks the idle rule until `stop` is set, and runs `leave` once when the
/// service is to leave. `holds_work` says whether the executor still holds work. The thread is
/// unparked by whoever sets `stop`.
pub fn watch(
    streams: Arc<Streams>,
    stop: Arc<AtomicBool>,
    holds_work: impl Fn() -> bool + Send + 'static,
    leave: impl FnOnce() + Send + 'static,
) -> io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name(THREAD_NAME.into())
        .spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                if streams.verdict(&holds_work) == Verdict::Leave {
                    leave();
                    return;
                }
                thread::park_timeout(streams.poll());
            }
        })
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

    const MINUTE: Duration = Duration::from_secs(60);

    fn streams() -> (Arc<FakeClock>, Arc<Streams>) {
        let clock = Arc::new(FakeClock(Mutex::new(Instant::now())));
        let streams = Streams::new(Arc::new(Arc::clone(&clock)), IDLE_EXIT);
        (clock, streams)
    }

    fn idle(streams: &Streams) -> Verdict {
        streams.verdict(|| false)
    }

    #[test]
    fn no_stream_for_ten_minutes_leaves() {
        let (clock, streams) = streams();
        assert_eq!(idle(&streams), Verdict::Wait(IDLE_EXIT));
        clock.advance(IDLE_EXIT - Duration::from_secs(1));
        assert_eq!(idle(&streams), Verdict::Wait(Duration::from_secs(1)));
        clock.advance(Duration::from_secs(1));
        assert_eq!(idle(&streams), Verdict::Leave);
    }

    #[test]
    fn the_span_of_a_service_that_was_not_yet_serving_starts_when_it_publishes() {
        let (clock, streams) = streams();
        clock.advance(IDLE_EXIT - Duration::from_secs(1));
        streams.published();
        assert_eq!(idle(&streams), Verdict::Wait(IDLE_EXIT));

        let stream = streams.admit().expect("the service is staying");
        clock.advance(IDLE_EXIT * 2);
        streams.published();
        assert_eq!(
            idle(&streams),
            Verdict::Streams,
            "an open stream stays armed off"
        );
        drop(stream);
    }

    #[test]
    fn an_opening_just_inside_the_limit_disarms_it_and_the_last_close_arms_it_again() {
        let (clock, streams) = streams();
        clock.advance(IDLE_EXIT - Duration::from_secs(1));
        let first = streams.admit().expect("the service is staying");
        let second = streams.admit().expect("the service is staying");
        assert_eq!(idle(&streams), Verdict::Streams);

        clock.advance(IDLE_EXIT * 3);
        assert_eq!(idle(&streams), Verdict::Streams, "open streams never idle");

        drop(first);
        assert_eq!(idle(&streams), Verdict::Streams, "one stream is still open");
        drop(second);
        assert_eq!(idle(&streams), Verdict::Wait(IDLE_EXIT));
        clock.advance(IDLE_EXIT);
        assert_eq!(idle(&streams), Verdict::Leave);
    }

    #[test]
    fn a_stream_that_opens_and_closes_between_two_looks_restarts_the_span() {
        let (clock, streams) = streams();
        clock.advance(IDLE_EXIT - Duration::from_secs(1));
        drop(streams.admit());
        clock.advance(Duration::from_secs(2));
        assert_eq!(
            idle(&streams),
            Verdict::Wait(IDLE_EXIT - Duration::from_secs(2))
        );
    }

    #[test]
    fn an_attached_shell_keeps_the_service_alive_indefinitely() {
        let (clock, streams) = streams();
        let shell = streams.admit();
        for _ in 0..200 {
            clock.advance(IDLE_EXIT);
            assert_eq!(idle(&streams), Verdict::Streams);
        }
        drop(shell);
        assert_eq!(idle(&streams), Verdict::Wait(IDLE_EXIT));
    }

    #[test]
    fn work_the_executor_still_holds_re_arms_the_limit() {
        let (clock, streams) = streams();
        clock.advance(IDLE_EXIT);
        assert_eq!(streams.verdict(|| true), Verdict::Wait(IDLE_EXIT));
        clock.advance(IDLE_EXIT - Duration::from_secs(1));
        assert_eq!(
            streams.verdict(|| panic!("asked before the limit")),
            Verdict::Wait(Duration::from_secs(1))
        );
        clock.advance(Duration::from_secs(1));
        assert_eq!(idle(&streams), Verdict::Leave);
    }

    #[test]
    fn work_is_not_asked_about_while_a_stream_is_open() {
        let (clock, streams) = streams();
        let _stream = streams.admit();
        clock.advance(IDLE_EXIT);
        assert_eq!(
            streams.verdict(|| panic!("asked while a stream is open")),
            Verdict::Streams
        );
    }

    #[test]
    fn a_service_that_decided_to_leave_admits_no_stream() {
        let (clock, streams) = streams();
        clock.advance(IDLE_EXIT);
        assert_eq!(idle(&streams), Verdict::Leave);
        assert!(streams.admit().is_none());
        assert_eq!(idle(&streams), Verdict::Leave);
    }

    #[test]
    fn a_browser_stream_counts_once_however_many_times_it_is_replaced() {
        struct Silent;
        impl AdapterLifecycleObserver for Silent {
            fn adapter_attached(&self, _: &str, _: bool) {}
            fn adapter_detached(&self, _: &str) {}
        }
        let (clock, streams) = streams();
        let observer = CountBrowsers::new(Arc::clone(&streams), Arc::new(Silent));
        observer.adapter_attached("chrome", false);
        observer.adapter_attached("chrome", true);
        clock.advance(IDLE_EXIT * 2);
        assert_eq!(idle(&streams), Verdict::Streams);
        observer.adapter_detached("chrome");
        assert_eq!(idle(&streams), Verdict::Wait(IDLE_EXIT));
        clock.advance(MINUTE);
        assert_eq!(idle(&streams), Verdict::Wait(IDLE_EXIT - MINUTE));
    }

    #[test]
    fn the_watch_runs_leave_once_when_the_service_idles_out() {
        let clock = Arc::new(FakeClock(Mutex::new(Instant::now())));
        let streams = Streams::new(Arc::new(Arc::clone(&clock)), IDLE_EXIT);
        let stop = Arc::new(AtomicBool::new(false));
        let (left, heard) = mpsc::channel();
        let watcher = watch(
            Arc::clone(&streams),
            Arc::clone(&stop),
            || false,
            move || left.send(()).unwrap(),
        )
        .unwrap();
        assert!(heard.recv_timeout(Duration::from_millis(300)).is_err());
        clock.advance(IDLE_EXIT);
        heard
            .recv_timeout(Duration::from_secs(5))
            .expect("the watch leaves once the limit has passed");
        watcher.join().unwrap();
    }

    #[test]
    fn a_stopped_watch_never_leaves() {
        let clock = Arc::new(FakeClock(Mutex::new(Instant::now())));
        let streams = Streams::new(Arc::new(Arc::clone(&clock)), IDLE_EXIT);
        let stop = Arc::new(AtomicBool::new(true));
        let (left, heard) = mpsc::channel();
        clock.advance(IDLE_EXIT);
        let watcher = watch(
            streams,
            stop,
            || false,
            move || {
                let _ = left.send(());
            },
        )
        .unwrap();
        watcher.join().unwrap();
        assert!(heard.try_recv().is_err());
    }
}
