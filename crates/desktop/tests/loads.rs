//! Process tests: Windows loads the built `ghostlight-desktop.exe`, and the arguments it takes.
//!
//! The window's libraries import functions that only Common Controls 6 has, so an image built
//! without its application manifest is refused by the loader before `main` runs. Started with an
//! argument it does not take, a loadable image reaches `main` and exits with the usage code at
//! once, without opening a window or reaching the service.
//!
//! Started with `--tray` in an empty Ghostlight home, where no service exists and no executable
//! could be started, the image tries the service once, finds nothing and ends with status 0 at
//! once. It starts nothing and leaves the home untouched.

#![cfg(windows)]

use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus};
use std::time::{Duration, Instant};

use ghostlight_desktop::{EXIT_USAGE, TRAY_ARGUMENT, UPDATING_ARGUMENT};

/// An argument the window does not take.
const UNKNOWN_ARGUMENT: &str = "--no-such-argument";

/// How long a shell that has nothing to do may take to end. A tray start tries the service once,
/// and one try waits at most five seconds.
const ENDS_WITHIN: Duration = Duration::from_secs(10);

/// The environment variable that names the Ghostlight home.
const HOME_VARIABLE: &str = "GHOSTLIGHT_TEST_HOME";

/// An empty temporary Ghostlight home, removed on drop.
struct EmptyHome(PathBuf);

impl EmptyHome {
    fn new(name: &str) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "ghostlight-desktop-loads-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("the empty home is made");
        Self(directory)
    }
}

impl Drop for EmptyHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Waits for `child` to end, and kills it if it takes longer than `limit`.
fn wait_within(mut child: Child, limit: Duration) -> Option<ExitStatus> {
    let deadline = Instant::now() + limit;
    loop {
        if let Some(status) = child.try_wait().expect("the child can be polled") {
            return Some(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn the_window_image_loads_and_reaches_main() {
    let status = Command::new(env!("CARGO_BIN_EXE_ghostlight-desktop"))
        .arg(UNKNOWN_ARGUMENT)
        .status()
        .expect("start the built window image");
    assert_eq!(
        status.code(),
        Some(i32::from(EXIT_USAGE)),
        "the loader refused the image or main did not run: {status}"
    );
}

#[test]
fn an_argument_pair_the_window_does_not_take_is_refused() {
    let status = Command::new(env!("CARGO_BIN_EXE_ghostlight-desktop"))
        .args([TRAY_ARGUMENT, UPDATING_ARGUMENT])
        .status()
        .expect("start the built window image");
    assert_eq!(status.code(), Some(i32::from(EXIT_USAGE)));
}

#[test]
fn a_tray_start_with_no_service_ends_at_once_and_starts_nothing() {
    let home = EmptyHome::new("tray");
    let started = Instant::now();
    let child = Command::new(env!("CARGO_BIN_EXE_ghostlight-desktop"))
        .arg(TRAY_ARGUMENT)
        .env(HOME_VARIABLE, &home.0)
        .spawn()
        .expect("start the built window image");
    let status = wait_within(child, ENDS_WITHIN).expect("a tray start ends by itself");
    assert_eq!(status.code(), Some(0), "{status}");
    assert!(started.elapsed() < ENDS_WITHIN);
    let left: Vec<_> = std::fs::read_dir(&home.0)
        .expect("the home is readable")
        .collect();
    assert!(left.is_empty(), "nothing was made in the home: {left:?}");
}
