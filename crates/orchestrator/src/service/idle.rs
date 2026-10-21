//! Idle exit: the service leaves when no stream of any role has been open for [`IDLE_EXIT`]
//! (ADR-0199 Decision 12).
//!
//! [`Streams`] counts the open streams of every role: `mcp`, `browser`, `control` and `desktop`.
//! An attached shell is a stream, so on a desktop that can show one the service never idles out;
//! the exit is for a headless service, such as one an agent started over SSH or in CI, where a
//! forgotten resident process costs most. The count is event-driven, so a stream that opens and
//! closes between two looks at the clock still restarts the span. [`Streams::verdict`] is the
//! rule, with the clock injected and no thread; [`watch`] is the thread that asks it.
//!
//! The idle stop is the service's own reason and is not relay vocabulary: no stream is open, so
//! there is nobody to say goodbye to, and it never sets the quit latch (see `Quit::idle_exit`).

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::browser::AdapterLifecycleObserver;

use super::lifetime::Clock;
use crate::sync::lock;

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
    /// How many times the rule was asked, so a test can wait for the watch to look.
    #[cfg(test)]
    looks: std::sync::atomic::AtomicUsize,
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
            #[cfg(test)]
            looks: std::sync::atomic::AtomicUsize::new(0),
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
        #[cfg(test)]
        self.looks.fetch_add(1, Ordering::SeqCst);
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

    /// How many times the rule has been asked. A look is counted before it reads the count and
    /// the clock, so a look counted after a test moved its clock sees the move.
    #[cfg(test)]
    pub(crate) fn looks(&self) -> usize {
        self.looks.load(Ordering::SeqCst)
    }

    /// How many streams are open.
    #[cfg(test)]
    pub(crate) fn open(&self) -> usize {
        lock(&self.state).open
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
