//! Parent-death detection for the connectors.
//!
//! A connector is started by an MCP client or by the browser. When that parent goes away the
//! connector has nothing left to serve and must exit, even when its standard input never
//! reports the end. [`ParentWatchdog`] captures the parent's identity at start (process id plus
//! creation time, so a reused id does not match) and polls it once a second from a thread.

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

#[cfg(target_os = "linux")]
use std::fs;

/// How often the watchdog checks the parent.
pub const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Something that can say whether the process it identifies is still running.
pub trait IdentitySource: Send + 'static {
    /// Whether the identified process is still running.
    fn is_alive(&self) -> bool;
}

/// A watch on the parent process, stopped when dropped.
///
/// The callback runs once, on the watchdog thread, when the parent is gone. Dropping the
/// watchdog stops the thread without running the callback.
#[derive(Debug)]
pub struct ParentWatchdog {
    stop: Option<Sender<()>>,
}

impl ParentWatchdog {
    /// Watch the current process's parent and call `on_parent_gone` once when it is gone.
    ///
    /// Returns `None` when the parent's identity cannot be captured on this platform; the
    /// connector then relies on its streams ending.
    #[must_use]
    pub fn start(on_parent_gone: impl FnOnce() + Send + 'static) -> Option<Self> {
        let parent = ParentProcess::capture()?;
        Some(Self::watch(parent, POLL_INTERVAL, on_parent_gone))
    }

    /// Watch `source` every `interval` and call `on_parent_gone` once when it is not alive.
    fn watch(
        source: impl IdentitySource,
        interval: Duration,
        on_parent_gone: impl FnOnce() + Send + 'static,
    ) -> Self {
        let (stop, stopped) = mpsc::channel::<()>();
        thread::spawn(move || loop {
            match stopped.recv_timeout(interval) {
                Err(RecvTimeoutError::Timeout) => {
                    if !source.is_alive() {
                        on_parent_gone();
                        return;
                    }
                }
                Ok(()) | Err(RecvTimeoutError::Disconnected) => return,
            }
        });
        Self { stop: Some(stop) }
    }
}

impl Drop for ParentWatchdog {
    fn drop(&mut self) {
        // Dropping the sender wakes the thread and tells it to stop.
        self.stop.take();
    }
}

/// The identity of a parent process: its id and when it was created.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParentProcess {
    process_id: u32,
    created: u64,
}

impl ParentProcess {
    /// Capture the current process's parent, or `None` when it cannot be identified.
    #[must_use]
    pub fn capture() -> Option<Self> {
        #[cfg(target_os = "windows")]
        {
            let process = crate::os::parent_process()?;
            Some(Self {
                process_id: process.process_id,
                created: process.created,
            })
        }
        #[cfg(target_os = "linux")]
        {
            let parent_id = read_stat("self")?.parent_process_id;
            identity(parent_id)
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        None
    }
}

impl IdentitySource for ParentProcess {
    fn is_alive(&self) -> bool {
        #[cfg(target_os = "windows")]
        {
            crate::os::process_is_alive(crate::os::ProcessIdentity {
                process_id: self.process_id,
                created: self.created,
            })
        }
        #[cfg(target_os = "linux")]
        {
            let Some(current) = read_stat("self") else {
                return true;
            };
            current.parent_process_id == self.process_id && identity(self.process_id) == Some(*self)
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        true
    }
}

/// The fields of `/proc/<pid>/stat` the watchdog needs.
#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug)]
struct ProcessStat {
    state: char,
    parent_process_id: u32,
    created: u64,
}

/// The identity of a running process, or `None` when it is gone or a zombie.
#[cfg(target_os = "linux")]
fn identity(process_id: u32) -> Option<ParentProcess> {
    let stat = read_stat(&process_id.to_string())?;
    (stat.state != 'Z').then_some(ParentProcess {
        process_id,
        created: stat.created,
    })
}

/// Read `/proc/<process>/stat`, where `process` is a pid or `self`.
#[cfg(target_os = "linux")]
fn read_stat(process: &str) -> Option<ProcessStat> {
    let line = fs::read_to_string(format!("/proc/{process}/stat")).ok()?;
    let fields: Vec<_> = line
        .get(line.rfind(')')? + 1..)?
        .split_whitespace()
        .collect();
    Some(ProcessStat {
        state: fields.first()?.chars().next()?,
        parent_process_id: fields.get(1)?.parse().ok()?,
        created: fields.get(19)?.parse().ok()?,
    })
}

#[cfg(test)]
mod tests {
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
}
