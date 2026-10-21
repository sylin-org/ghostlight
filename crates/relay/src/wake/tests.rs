//! Tests of the wake endpoints: a ready service wakes each waiting edge, and forgets the ones
//! that have gone.

use std::fs;
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use super::*;
use crate::tests::Scratch;

/// How long a test waits for something that should happen at once.
const PATIENCE: Duration = Duration::from_secs(10);

/// The registrations listed in `home`'s wake directory.
fn registrations(home: &Path) -> Vec<String> {
    fs::read_dir(endpoint::wake_directory(home))
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// Wait on `waiting` from another thread, reporting each wake on the returned channel.
fn watch(waiting: Waiting) -> mpsc::Receiver<()> {
    let (woken, wakes) = mpsc::channel();
    thread::spawn(move || {
        while waiting.next().is_ok() {
            if woken.send(()).is_err() {
                return;
            }
        }
    });
    wakes
}

#[test]
fn a_fresh_id_is_a_wake_id_and_differs_from_the_next() {
    let first = fresh_id();
    let second = fresh_id();
    assert!(is_id(&first), "{first}");
    assert!(is_id(&second), "{second}");
    assert_ne!(first, second);
}

#[test]
fn only_sixteen_lowercase_hexadecimal_digits_name_a_registration() {
    assert!(is_id("0123456789abcdef"));
    assert!(!is_id("0123456789ABCDEF"));
    assert!(!is_id("0123456789abcde"));
    assert!(!is_id("0123456789abcdef0"));
    assert!(!is_id("0123456789abcdeg"));
    assert!(!is_id("../../relay.sock"));
}

#[test]
fn a_home_with_no_waiting_edge_wakes_nothing() {
    let home = Scratch::named("wake-none");
    assert_eq!(wake_all(home.path()), Wakes::default());
}

#[test]
fn a_ready_service_wakes_every_waiting_edge() {
    let home = Scratch::named("wake-every");
    let first = watch(Waiting::register(home.path()).unwrap());
    let second = watch(Waiting::register(home.path()).unwrap());
    assert_eq!(registrations(home.path()).len(), 2);

    let wakes = wake_all(home.path());

    assert_eq!(
        wakes,
        Wakes {
            woken: 2,
            removed: 0
        }
    );
    first
        .recv_timeout(PATIENCE)
        .expect("the first edge was woken");
    second
        .recv_timeout(PATIENCE)
        .expect("the second edge was woken");
}

#[test]
fn an_edge_is_woken_by_each_service_that_becomes_ready() {
    let home = Scratch::named("wake-again");
    let woken = watch(Waiting::register(home.path()).unwrap());

    assert_eq!(wake_all(home.path()).woken, 1);
    woken
        .recv_timeout(PATIENCE)
        .expect("the first wake arrived");
    assert_eq!(wake_all(home.path()).woken, 1);
    woken
        .recv_timeout(PATIENCE)
        .expect("the second wake arrived");
}

#[test]
fn a_registration_whose_edge_has_gone_is_removed() {
    let home = Scratch::named("wake-gone");
    let gone = Waiting::register(home.path()).unwrap();
    let id = registrations(home.path()).pop().expect("one registration");
    // An edge that crashed leaves its file behind: end this one, then put its file back the way
    // the crash would have left it.
    let file = gone.file.clone();
    gone.shutdown();
    drop(gone);
    fs::write(&file, b"").unwrap();
    assert_eq!(registrations(home.path()), [id]);

    let wakes = wake_all(home.path());

    assert_eq!(
        wakes,
        Wakes {
            woken: 0,
            removed: 1
        }
    );
    assert!(registrations(home.path()).is_empty());
}

#[test]
fn a_file_that_is_not_a_registration_is_left_alone() {
    let home = Scratch::named("wake-other");
    let directory = endpoint::wake_directory(home.path());
    crate::private::create_dir_all(&directory).unwrap();
    fs::write(directory.join("notes.txt"), b"kept").unwrap();

    assert_eq!(wake_all(home.path()), Wakes::default());
    assert_eq!(fs::read(directory.join("notes.txt")).unwrap(), b"kept");
}

#[test]
fn a_dropped_registration_takes_its_file_with_it() {
    let home = Scratch::named("wake-drop");
    let waiting = Waiting::register(home.path()).unwrap();
    assert_eq!(registrations(home.path()).len(), 1);
    drop(waiting);
    assert!(registrations(home.path()).is_empty());
}
