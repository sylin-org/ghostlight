//! Tests of how a lost parent is noticed.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;

use super::*;

/// An identity whose liveness the test flips by hand.
struct Fake(Arc<AtomicBool>);

impl IdentitySource for Fake {
    fn is_alive(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

const TICK: Duration = Duration::from_millis(5);

fn wait_until(condition: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if condition() {
            return true;
        }
        thread::sleep(TICK);
    }
    condition()
}

#[test]
fn the_callback_fires_once_when_the_parent_is_gone() {
    let alive = Arc::new(AtomicBool::new(true));
    let fired = Arc::new(AtomicU32::new(0));
    let counter = Arc::clone(&fired);
    let _watchdog = ParentWatchdog::watch(Fake(Arc::clone(&alive)), TICK, move || {
        counter.fetch_add(1, Ordering::SeqCst);
    });
    thread::sleep(TICK * 10);
    assert_eq!(fired.load(Ordering::SeqCst), 0);
    alive.store(false, Ordering::SeqCst);
    assert!(wait_until(|| fired.load(Ordering::SeqCst) == 1));
    thread::sleep(TICK * 10);
    assert_eq!(fired.load(Ordering::SeqCst), 1);
}

#[test]
fn dropping_the_watchdog_stops_it_without_the_callback() {
    let alive = Arc::new(AtomicBool::new(true));
    let fired = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&fired);
    let watchdog = ParentWatchdog::watch(Fake(Arc::clone(&alive)), TICK, move || {
        flag.store(true, Ordering::SeqCst);
    });
    drop(watchdog);
    alive.store(false, Ordering::SeqCst);
    thread::sleep(TICK * 20);
    assert!(!fired.load(Ordering::SeqCst));
}

#[test]
fn the_current_parent_has_a_live_stable_identity() {
    let parent = ParentProcess::capture().expect("capture current parent");
    assert!(parent.is_alive());
}

#[test]
#[cfg(target_os = "linux")]
fn a_reaped_process_does_not_match_its_captured_identity() {
    use std::process::Command;

    let mut child = Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("spawn long-lived child");
    let process = identity(child.id()).expect("child process identity");
    assert_eq!(identity(child.id()), Some(process));
    child.kill().expect("terminate child");
    child.wait().expect("reap child status");
    assert_eq!(identity(child.id()), None);
}
