//! Rate limits for requests that carry no key.
//!
//! Loopback callers cannot be told apart, so each limit is global to its route. A limit never
//! applies to the window, which carries the key (ADR-0199, amendment of 2026-10-15).

use std::collections::VecDeque;
#[cfg(test)]
use std::sync::atomic::AtomicU64;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// Searches a watcher may make per second.
const SEARCHES_PER_SECOND: usize = 5;

/// Policy checks a watcher may make per minute.
const PREVIEWS_PER_MINUTE: usize = 10;

/// Pauses a watcher may make per minute.
const HOLDS_PER_MINUTE: usize = 6;

/// The shortest time between a watcher bringing the window forward.
const WINDOW_SPAN: Duration = Duration::from_secs(5);

/// The shortest time between two scans a watcher can cause.
const RESCAN_SPAN: Duration = Duration::from_secs(5);

const SECOND: Duration = Duration::from_secs(1);
const MINUTE: Duration = Duration::from_secs(60);

/// A sliding window: at most `max` takes in any `span`.
#[derive(Debug)]
struct Rate {
    max: usize,
    span: Duration,
    takes: VecDeque<Instant>,
}

impl Rate {
    const fn new(max: usize, span: Duration) -> Self {
        Self {
            max,
            span,
            takes: VecDeque::new(),
        }
    }

    /// Take one slot at `now`, or say how long until one is free.
    fn take(&mut self, now: Instant) -> Result<(), Duration> {
        while self
            .takes
            .front()
            .is_some_and(|taken| now.saturating_duration_since(*taken) >= self.span)
        {
            self.takes.pop_front();
        }
        if self.takes.len() >= self.max {
            let oldest = *self.takes.front().expect("a full window has a first take");
            return Err(self
                .span
                .saturating_sub(now.saturating_duration_since(oldest)));
        }
        self.takes.push_back(now);
        Ok(())
    }
}

/// How long a refused watcher should wait, in whole seconds and never less than one.
#[must_use]
pub fn retry_after_seconds(wait: Duration) -> u64 {
    let rounded_up = wait.as_secs() + u64::from(wait.subsec_nanos() > 0);
    rounded_up.max(1)
}

/// The limits of one server.
#[derive(Debug)]
pub struct Limits {
    search: Mutex<Rate>,
    preview: Mutex<Rate>,
    preview_in_flight: AtomicBool,
    window: Mutex<Rate>,
    hold: Mutex<Rate>,
    rescan: Mutex<Option<Instant>>,
    /// Scans started, counted for tests.
    #[cfg(test)]
    scans: AtomicU64,
}

/// A policy check in flight. Dropping it lets the next one start.
#[derive(Debug)]
pub struct PreviewPermit<'a> {
    limits: &'a Limits,
}

impl Drop for PreviewPermit<'_> {
    fn drop(&mut self) {
        self.limits
            .preview_in_flight
            .store(false, Ordering::Release);
    }
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            search: Mutex::new(Rate::new(SEARCHES_PER_SECOND, SECOND)),
            preview: Mutex::new(Rate::new(PREVIEWS_PER_MINUTE, MINUTE)),
            preview_in_flight: AtomicBool::new(false),
            window: Mutex::new(Rate::new(1, WINDOW_SPAN)),
            hold: Mutex::new(Rate::new(HOLDS_PER_MINUTE, MINUTE)),
            rescan: Mutex::new(None),
            #[cfg(test)]
            scans: AtomicU64::new(0),
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Limits {
    /// Take a search slot.
    pub fn search(&self) -> Result<(), Duration> {
        lock(&self.search).take(Instant::now())
    }

    /// Take a slot to bring the window forward.
    pub fn window(&self) -> Result<(), Duration> {
        lock(&self.window).take(Instant::now())
    }

    /// Take a slot to pause agents.
    pub fn hold(&self) -> Result<(), Duration> {
        lock(&self.hold).take(Instant::now())
    }

    /// Start a policy check: at most one at a time and ten a minute.
    pub fn preview(&self) -> Result<PreviewPermit<'_>, Duration> {
        if self
            .preview_in_flight
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(SECOND);
        }
        let permit = PreviewPermit { limits: self };
        lock(&self.preview).take(Instant::now())?;
        Ok(permit)
    }

    /// A watcher's rescan: `scan` runs at most once in five seconds, and any other call inside
    /// that time gets `cached`. Concurrent callers wait for the one scan in progress.
    pub fn rescan_for_watcher<R>(&self, scan: impl FnOnce() -> R, cached: impl FnOnce() -> R) -> R {
        let mut last = lock(&self.rescan);
        let now = Instant::now();
        if last.is_some_and(|scanned| now.saturating_duration_since(scanned) < RESCAN_SPAN) {
            return cached();
        }
        let result = self.scan(scan);
        *last = Some(Instant::now());
        result
    }

    /// The window's rescan: it always scans, and its scan counts as the latest one.
    pub fn rescan_for_window<R>(&self, scan: impl FnOnce() -> R) -> R {
        let mut last = lock(&self.rescan);
        let result = self.scan(scan);
        *last = Some(Instant::now());
        result
    }

    fn scan<R>(&self, scan: impl FnOnce() -> R) -> R {
        #[cfg(test)]
        self.scans.fetch_add(1, Ordering::AcqRel);
        scan()
    }

    /// How many scans the server has started.
    #[must_use]
    #[cfg(test)]
    pub fn scans(&self) -> u64 {
        self.scans.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_admits_its_maximum_and_then_says_when() {
        let mut rate = Rate::new(2, SECOND);
        let start = Instant::now();
        assert!(rate.take(start).is_ok());
        assert!(rate.take(start + Duration::from_millis(100)).is_ok());
        let wait = rate.take(start + Duration::from_millis(200)).unwrap_err();
        assert_eq!(wait, Duration::from_millis(800));
        assert!(rate.take(start + SECOND).is_ok());
    }

    #[test]
    fn retry_after_is_whole_seconds_and_never_zero() {
        assert_eq!(retry_after_seconds(Duration::from_millis(1)), 1);
        assert_eq!(retry_after_seconds(Duration::from_millis(1500)), 2);
        assert_eq!(retry_after_seconds(Duration::from_secs(3)), 3);
        assert_eq!(retry_after_seconds(Duration::ZERO), 1);
    }

    #[test]
    fn one_policy_check_runs_at_a_time() {
        let limits = Limits::default();
        let first = limits.preview().unwrap();
        assert!(limits.preview().is_err());
        drop(first);
        assert!(limits.preview().is_ok());
    }

    #[test]
    fn a_watchers_rescans_coalesce_and_the_windows_do_not() {
        let limits = Limits::default();
        let first = limits.rescan_for_watcher(|| "scanned", || "cached");
        let second = limits.rescan_for_watcher(|| "scanned", || "cached");
        assert_eq!((first, second), ("scanned", "cached"));
        assert_eq!(limits.scans(), 1);
        assert_eq!(limits.rescan_for_window(|| "scanned"), "scanned");
        assert_eq!(limits.scans(), 2);
    }
}
