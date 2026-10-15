//! Tests of the relay endpoint: names, the connection type over a connected pair, and a real
//! listener on a temporary home.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::*;
use crate::frame::{read_frame, write_frame};

/// How long a test waits for something that should happen at once.
const PATIENCE: Duration = Duration::from_secs(10);

/// A home under the temporary directory, removed when dropped.
struct TempHome(PathBuf);

impl TempHome {
    fn new() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "ghostlight-endpoint-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn connect(&self) -> Result<Connection, ConnectError> {
        connect(self.path(), Instant::now() + PATIENCE)
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Run `work` on a thread and return its result, failing the test if it takes too long.
fn within_patience<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    let (sent, received) = mpsc::channel();
    thread::spawn(move || {
        let _ = sent.send(work());
    });
    received
        .recv_timeout(PATIENCE)
        .expect("the work finishes in time")
}

#[test]
fn fnv1a32_matches_its_published_vectors() {
    assert_eq!(fnv1a32(b""), 0x811c_9dc5);
    assert_eq!(fnv1a32(b"a"), 0xe40c_292c);
    assert_eq!(fnv1a32(b"foobar"), 0xbf9c_f968);
}

#[test]
fn a_pipe_name_carries_the_user_and_the_hash_of_the_lowercased_home() {
    let endpoint = Endpoint::named_pipe(
        "S-1-5-21-1-2-3-1001",
        Path::new(r"C:\Users\Person\.ghostlight"),
    );
    assert_eq!(
        endpoint,
        Endpoint::NamedPipe(r"\\.\pipe\ghostlight-S-1-5-21-1-2-3-1001-e4c35116".to_owned())
    );
    assert_eq!(
        endpoint.to_string(),
        r"\\.\pipe\ghostlight-S-1-5-21-1-2-3-1001-e4c35116"
    );
}

#[test]
fn homes_that_differ_only_in_case_share_a_name_and_other_homes_do_not() {
    let upper = Endpoint::named_pipe("S-1", Path::new(r"C:\USERS\PERSON\.GHOSTLIGHT"));
    let lower = Endpoint::named_pipe("S-1", Path::new(r"c:\users\person\.ghostlight"));
    let other = Endpoint::named_pipe("S-1", Path::new(r"c:\users\other\.ghostlight"));
    assert_eq!(upper, lower);
    assert_ne!(lower, other);
}

#[test]
fn the_socket_endpoint_is_the_relay_socket_in_the_run_directory() {
    let home = Path::new("/home/person/.ghostlight");
    let expected = home.join("run").join("relay.sock");
    assert_eq!(
        Endpoint::unix_socket(home),
        Endpoint::UnixSocket(expected.clone())
    );
    assert_eq!(
        Endpoint::unix_socket(home).to_string(),
        expected.display().to_string()
    );
}

#[test]
fn frames_cross_a_pair_in_both_directions() {
    let (mut left, mut right) = pair().unwrap();
    write_frame(&mut left, &json!({"from": "left"})).unwrap();
    write_frame(&mut right, &json!({"from": "right"})).unwrap();
    let at_right: Value = read_frame(&mut right).unwrap().unwrap();
    let at_left: Value = read_frame(&mut left).unwrap().unwrap();
    assert_eq!(at_right, json!({"from": "left"}));
    assert_eq!(at_left, json!({"from": "right"}));
}

#[test]
fn cloned_ends_read_and_write_at_the_same_time() {
    const FRAMES: u64 = 200;
    let (left, right) = pair().unwrap();
    let mut left_writer = left.try_clone().unwrap();
    let mut right_writer = right.try_clone().unwrap();
    let (mut left_reader, mut right_reader) = (left, right);

    let to_right = thread::spawn(move || {
        for n in 0..FRAMES {
            write_frame(&mut left_writer, &json!({"to": "right", "n": n})).unwrap();
        }
    });
    let to_left = thread::spawn(move || {
        for n in 0..FRAMES {
            write_frame(&mut right_writer, &json!({"to": "left", "n": n})).unwrap();
        }
    });
    let at_left = thread::spawn(move || {
        for n in 0..FRAMES {
            let frame: Value = read_frame(&mut left_reader).unwrap().unwrap();
            assert_eq!(frame, json!({"to": "left", "n": n}));
        }
    });
    for n in 0..FRAMES {
        let frame: Value = read_frame(&mut right_reader).unwrap().unwrap();
        assert_eq!(frame, json!({"to": "right", "n": n}));
    }
    to_right.join().unwrap();
    to_left.join().unwrap();
    at_left.join().unwrap();
}

#[test]
fn a_frame_larger_than_the_pipe_buffer_crosses_while_the_peer_reads() {
    let (mut left, mut right) = pair().unwrap();
    let payload = "x".repeat(1024 * 1024);
    let expected = payload.clone();
    let writer = thread::spawn(move || write_frame(&mut left, &json!({"data": payload})).unwrap());
    let frame: Value = within_patience(move || read_frame(&mut right).unwrap().unwrap());
    assert_eq!(frame["data"], json!(expected));
    writer.join().unwrap();
}

#[test]
fn an_idle_read_times_out_and_a_later_frame_still_arrives() {
    let (mut left, mut right) = pair().unwrap();
    left.set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    let started = Instant::now();
    let failure = left.read(&mut [0_u8; 8]).unwrap_err();
    assert_eq!(failure.kind(), io::ErrorKind::TimedOut);
    assert!(started.elapsed() >= Duration::from_millis(90));

    write_frame(&mut right, &json!({"later": true})).unwrap();
    let frame: Value = read_frame(&mut left).unwrap().unwrap();
    assert_eq!(frame, json!({"later": true}));
}

#[test]
fn a_timeout_set_on_one_clone_applies_to_the_other() {
    let (left, _right) = pair().unwrap();
    let mut other = left.try_clone().unwrap();
    left.set_read_timeout(Some(Duration::from_millis(50)))
        .unwrap();
    let failure = other.read(&mut [0_u8; 8]).unwrap_err();
    assert_eq!(failure.kind(), io::ErrorKind::TimedOut);
}

#[test]
fn a_zero_timeout_is_refused() {
    let (left, _right) = pair().unwrap();
    let failure = left.set_read_timeout(Some(Duration::ZERO)).unwrap_err();
    assert_eq!(failure.kind(), io::ErrorKind::InvalidInput);
    let failure = left.set_write_timeout(Some(Duration::ZERO)).unwrap_err();
    assert_eq!(failure.kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn shutdown_from_another_thread_ends_a_blocked_read() {
    let (left, _right) = pair().unwrap();
    let stopper = left.try_clone().unwrap();
    let reader = thread::spawn(move || {
        let mut left = left;
        left.read(&mut [0_u8; 8])
    });
    thread::sleep(Duration::from_millis(150));
    stopper.shutdown();
    let outcome = within_patience(move || reader.join().unwrap());
    assert_eq!(outcome.unwrap(), 0);
}

#[test]
fn after_shutdown_reads_end_and_writes_fail_on_every_clone() {
    let (left, _right) = pair().unwrap();
    let mut clone = left.try_clone().unwrap();
    left.shutdown();
    assert_eq!(clone.read(&mut [0_u8; 8]).unwrap(), 0);
    let failure = clone.write(b"late").unwrap_err();
    assert_eq!(failure.kind(), io::ErrorKind::BrokenPipe);
    left.shutdown();
}

#[test]
fn shutdown_ends_the_stream_for_the_peer() {
    let (left, mut right) = pair().unwrap();
    left.shutdown();
    let outcome = within_patience(move || right.read(&mut [0_u8; 8]));
    assert_eq!(outcome.unwrap(), 0);
}

#[test]
fn bytes_written_before_one_end_is_dropped_are_still_read() {
    let (mut left, mut right) = pair().unwrap();
    write_frame(&mut left, &json!({"goodbye": "quit"})).unwrap();
    drop(left);
    let frame: Value = read_frame(&mut right).unwrap().unwrap();
    assert_eq!(frame, json!({"goodbye": "quit"}));
    assert!(read_frame::<Value>(&mut right).unwrap().is_none());
}

#[test]
fn a_write_to_a_dropped_peer_fails() {
    let (mut left, right) = pair().unwrap();
    drop(right);
    let failure = within_patience(move || loop {
        if let Err(failure) = left.write_all(&[0_u8; 4096]) {
            break failure;
        }
    });
    assert!(matches!(
        failure.kind(),
        io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset
    ));
}

#[test]
fn a_second_first_bind_on_the_same_home_is_refused() {
    let home = TempHome::new();
    let _first = Listener::bind_first(home.path()).unwrap();
    let failure = Listener::bind_first(home.path()).unwrap_err();
    assert_eq!(failure.kind(), io::ErrorKind::AddrInUse);
}

#[test]
fn a_connection_reaches_the_listener_and_the_server_end_reads_what_the_client_wrote() {
    let home = TempHome::new();
    let listener = Arc::new(Listener::bind_first(home.path()).unwrap());
    let accepting = Arc::clone(&listener);
    let server = thread::spawn(move || {
        let mut server_end = accepting.accept().unwrap();
        let frame: Value = read_frame(&mut server_end).unwrap().unwrap();
        write_frame(&mut server_end, &json!({"echo": frame})).unwrap();
    });

    let mut client = home.connect().unwrap();
    write_frame(&mut client, &json!({"hello": "service"})).unwrap();
    let reply: Value = within_patience(move || read_frame(&mut client).unwrap().unwrap());
    assert_eq!(reply, json!({"echo": {"hello": "service"}}));
    server.join().unwrap();
}

#[test]
fn a_client_that_connects_before_accept_is_still_served() {
    let home = TempHome::new();
    let listener = Listener::bind_first(home.path()).unwrap();
    let mut client = home.connect().unwrap();
    write_frame(&mut client, &json!({"early": true})).unwrap();
    let mut server_end = listener.accept().unwrap();
    let frame: Value = within_patience(move || read_frame(&mut server_end).unwrap().unwrap());
    assert_eq!(frame, json!({"early": true}));
}

#[test]
fn a_client_that_connects_and_closes_does_not_stop_the_listener_serving_the_next() {
    let home = TempHome::new();
    let listener = Arc::new(Listener::bind_first(home.path()).unwrap());
    let accepting = Arc::clone(&listener);
    let server = thread::spawn(move || loop {
        let mut server_end = accepting.accept().unwrap();
        if let Some(frame) = read_frame::<Value>(&mut server_end).unwrap() {
            break frame;
        }
    });

    drop(home.connect().unwrap());
    let mut client = home.connect().unwrap();
    write_frame(&mut client, &json!({"after": "a probe"})).unwrap();
    let frame = within_patience(move || server.join().unwrap());
    assert_eq!(frame, json!({"after": "a probe"}));
}

#[test]
fn several_clients_each_get_their_own_server_end() {
    let home = TempHome::new();
    let listener = Arc::new(Listener::bind_first(home.path()).unwrap());
    let accepting = Arc::clone(&listener);
    let server = thread::spawn(move || {
        let mut seen = Vec::new();
        for _ in 0..3 {
            let mut server_end = accepting.accept().unwrap();
            let frame: Value = read_frame(&mut server_end).unwrap().unwrap();
            seen.push(frame["n"].as_u64().unwrap());
        }
        seen.sort_unstable();
        seen
    });
    let mut clients = Vec::new();
    for n in 0..3_u64 {
        let mut client = home.connect().unwrap();
        write_frame(&mut client, &json!({"n": n})).unwrap();
        clients.push(client);
    }
    assert_eq!(
        within_patience(move || server.join().unwrap()),
        vec![0, 1, 2]
    );
}

#[test]
fn connecting_with_nothing_bound_finds_no_service() {
    let home = TempHome::new();
    assert!(matches!(home.connect(), Err(ConnectError::NoService)));
}

#[test]
fn connecting_after_the_listener_is_gone_finds_no_service() {
    let home = TempHome::new();
    drop(Listener::bind_first(home.path()).unwrap());
    assert!(matches!(home.connect(), Err(ConnectError::NoService)));
}

#[test]
fn shutdown_wakes_a_blocked_accept_and_later_accepts_fail() {
    let home = TempHome::new();
    let listener = Arc::new(Listener::bind_first(home.path()).unwrap());
    let accepting = Arc::clone(&listener);
    let waiter = thread::spawn(move || accepting.accept().map(|_| ()));
    thread::sleep(Duration::from_millis(150));
    listener.shutdown();
    let outcome = within_patience(move || waiter.join().unwrap());
    assert_eq!(
        outcome.unwrap_err().kind(),
        io::ErrorKind::ConnectionAborted
    );
    assert_eq!(
        listener.accept().map(|_| ()).unwrap_err().kind(),
        io::ErrorKind::ConnectionAborted
    );
}

#[cfg(target_os = "linux")]
mod linux {
    //! The run directory and socket modes, and the refusal of an untrusted run directory.

    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    fn mode(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn the_run_directory_is_private_and_the_socket_is_owner_only() {
        let home = TempHome::new();
        let _listener = Listener::bind_first(home.path()).unwrap();
        let run = home.path().join("run");
        assert_eq!(mode(&run), 0o700);
        assert_eq!(mode(&run.join("relay.sock")), 0o600);
    }

    #[test]
    fn an_open_run_directory_is_tightened_when_binding() {
        let home = TempHome::new();
        let run = home.path().join("run");
        fs::create_dir(&run).unwrap();
        fs::set_permissions(&run, fs::Permissions::from_mode(0o755)).unwrap();
        let _listener = Listener::bind_first(home.path()).unwrap();
        assert_eq!(mode(&run), 0o700);
    }

    #[test]
    fn a_socket_file_nobody_listens_on_is_replaced() {
        let home = TempHome::new();
        drop(Listener::bind_first(home.path()).unwrap());
        assert!(home.path().join("run").join("relay.sock").exists());
        let _listener = Listener::bind_first(home.path()).unwrap();
    }

    #[test]
    fn a_run_directory_open_to_group_or_other_makes_connect_refuse() {
        let home = TempHome::new();
        let _listener = Listener::bind_first(home.path()).unwrap();
        let run = home.path().join("run");
        fs::set_permissions(&run, fs::Permissions::from_mode(0o750)).unwrap();
        assert!(matches!(home.connect(), Err(ConnectError::ForeignServer)));
        fs::set_permissions(&run, fs::Permissions::from_mode(0o705)).unwrap();
        assert!(matches!(home.connect(), Err(ConnectError::ForeignServer)));
    }
}
