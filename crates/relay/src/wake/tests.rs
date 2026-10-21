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

/// The registrations listed in `home`'s wake directory, sorted.
fn registrations(home: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(endpoint::wake_directory(home))
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
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

/// Defect S-D3 (b0290246): a service that becomes ready wakes every registered edge, and a
/// registration lasts the edge's life, so the same edges are woken again by each service that
/// becomes ready after them.
#[test]
fn every_waiting_edge_is_woken_by_each_ready_service() {
    let home = Scratch::named("wake-every");
    let first = watch(Waiting::register(home.path()).unwrap());
    let second = watch(Waiting::register(home.path()).unwrap());
    assert_eq!(registrations(home.path()).len(), 2);
    for service in ["the first service", "the next service"] {
        assert_eq!(
            wake_all(home.path()),
            Wakes {
                woken: 2,
                removed: 0
            },
            "{service}"
        );
        first
            .recv_timeout(PATIENCE)
            .unwrap_or_else(|_| panic!("{service} woke the first edge"));
        second
            .recv_timeout(PATIENCE)
            .unwrap_or_else(|_| panic!("{service} woke the second edge"));
    }
}

/// Defect S-D3: an edge that exits leaves its registration behind (its wake endpoint lives in a
/// thread that never ends), so a ready service removes every registration whose edge has gone.
/// It touches nothing else in `run/wake`: a name that is not sixteen lowercase hexadecimal
/// digits is left alone, a near miss included.
#[test]
fn a_wake_removes_only_the_registrations_whose_edge_has_gone() {
    let home = Scratch::named("wake-gone");
    let gone = Waiting::register(home.path()).unwrap();
    let id = registrations(home.path()).pop().expect("one registration");
    // End this edge, then put its file back the way an exit leaves it.
    let file = gone.file.clone();
    gone.shutdown();
    drop(gone);
    fs::write(&file, b"").unwrap();
    assert_eq!(registrations(home.path()), std::slice::from_ref(&id));
    let directory = endpoint::wake_directory(home.path());
    let near_misses = [
        "0123456789ABCDEF",
        "0123456789abcde",
        "0123456789abcdef0",
        "0123456789abcdeg",
        "notes.txt",
    ];
    for name in near_misses {
        fs::write(directory.join(name), b"kept").unwrap();
    }

    assert_eq!(
        wake_all(home.path()),
        Wakes {
            woken: 0,
            removed: 1
        }
    );
    let mut kept: Vec<String> = near_misses.iter().map(|name| (*name).to_owned()).collect();
    kept.sort();
    assert_eq!(
        registrations(home.path()),
        kept,
        "only the registration of {id} is removed"
    );
    for name in near_misses {
        assert_eq!(
            fs::read(directory.join(name)).unwrap(),
            b"kept",
            "{name} is left as it was"
        );
    }
}
