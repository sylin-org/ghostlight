//! Tests of the relay endpoint: the connection type over a connected pair, and a real listener
//! on a temporary home. On Windows these drive the audited named-pipe calls of `crate::os`.

use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::*;
use crate::frame::{read_frame, write_frame};
use crate::tests::Scratch;

/// How long a test waits for something that should happen at once.
const PATIENCE: Duration = Duration::from_secs(10);

/// A home in a scratch directory, removed when dropped.
struct TempHome(Scratch);

impl TempHome {
    fn new() -> Self {
        Self(Scratch::named("endpoint"))
    }

    fn path(&self) -> &Path {
        self.0.path()
    }

    fn connect(&self) -> Result<Connection, ConnectError> {
        connect(self.path(), Instant::now() + PATIENCE)
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

/// Defect 5917fbd4 and the endpoint's audited pipe and peer calls: the first bind owns the
/// endpoint of a home, so a second is refused. The listener admits each client to an end of its
/// own, a client that connected before the accept included, and keeps serving after a client
/// that connected and left before it was accepted (on Windows the pipe then reports
/// ERROR_NO_DATA, which ended the listener before the fix). The accepted end, and each clone of
/// it, knows the process the operating system says connected; a connecting end and a pair know
/// none.
#[test]
fn the_first_bind_owns_the_endpoint_and_admits_each_client_to_an_end_of_its_own() {
    let home = TempHome::new();
    let listener = Arc::new(Listener::bind_first(home.path()).unwrap());

    let mut early = home.connect().unwrap();
    write_frame(&mut early, &json!({"n": 0})).unwrap();
    let server_end = listener.accept().unwrap();
    assert_eq!(server_end.peer_process_id(), Some(std::process::id()));
    assert_eq!(
        server_end.try_clone().unwrap().peer_process_id(),
        Some(std::process::id())
    );
    assert_eq!(early.peer_process_id(), None);
    let (first, second) = pair().unwrap();
    assert_eq!(
        (first.peer_process_id(), second.peer_process_id()),
        (None, None)
    );
    let frame: Value = within_patience(move || {
        let mut server_end = server_end;
        read_frame(&mut server_end).unwrap().unwrap()
    });
    assert_eq!(
        frame,
        json!({"n": 0}),
        "a client that connected before the accept is served"
    );

    // Connect and leave before the listener takes the waiting instance, then serve the next.
    drop(home.connect().unwrap());
    let accepting = Arc::clone(&listener);
    let server = thread::spawn(move || {
        let mut seen = Vec::new();
        while seen.len() < 2 {
            let mut end = accepting.accept().unwrap();
            if let Some(frame) = read_frame::<Value>(&mut end).unwrap() {
                seen.push(frame["n"].as_u64().unwrap());
            }
        }
        seen.sort_unstable();
        seen
    });
    let mut clients = Vec::new();
    for n in 1..=2_u64 {
        let mut client = home.connect().unwrap();
        write_frame(&mut client, &json!({"n": n})).unwrap();
        clients.push(client);
    }
    assert_eq!(
        within_patience(move || server.join().unwrap()),
        [1, 2],
        "each later client gets an end of its own"
    );
    // Last, because on Linux a second bind probes the socket with a connection of its own.
    assert_eq!(
        Listener::bind_first(home.path()).unwrap_err().kind(),
        io::ErrorKind::AddrInUse,
        "the first bind owns the endpoint"
    );
}

/// The endpoint's audited cancel and stop calls (on Windows `CancelIoEx` and the listener's
/// stop event): a shutdown from any thread ends a blocked read or accept, however far it got,
/// and holds for every clone. Later reads end, writes fail, a second shutdown is harmless, later
/// accepts fail, and the peer sees the end of the stream.
#[test]
fn a_shutdown_from_any_thread_ends_the_stream_for_every_clone_and_the_peer() {
    let (left, mut right) = pair().unwrap();
    let stopper = left.try_clone().unwrap();
    let (sent, read) = mpsc::channel();
    thread::spawn(move || {
        let mut left = left;
        let _ = sent.send(left.read(&mut [0_u8; 8]));
    });
    stopper.shutdown();
    assert_eq!(
        read.recv_timeout(PATIENCE).expect("the read ends").unwrap(),
        0,
        "a read on another thread ends, before or during its wait"
    );
    let mut clone = stopper.try_clone().unwrap();
    assert_eq!(clone.read(&mut [0_u8; 8]).unwrap(), 0);
    assert_eq!(
        clone.write(b"late").unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    stopper.shutdown();
    assert_eq!(
        within_patience(move || right.read(&mut [0_u8; 8])).unwrap(),
        0,
        "the peer sees the end of the stream"
    );

    let home = TempHome::new();
    let listener = Arc::new(Listener::bind_first(home.path()).unwrap());
    let accepting = Arc::clone(&listener);
    let (sent, accepted) = mpsc::channel();
    thread::spawn(move || {
        let _ = sent.send(accepting.accept().map(|_| ()));
    });
    listener.shutdown();
    assert_eq!(
        accepted
            .recv_timeout(PATIENCE)
            .expect("the accept ends")
            .unwrap_err()
            .kind(),
        io::ErrorKind::ConnectionAborted,
        "an accept on another thread ends, before or during its wait"
    );
    assert_eq!(
        listener.accept().map(|_| ()).unwrap_err().kind(),
        io::ErrorKind::ConnectionAborted
    );
}

/// Relay-protocol-4 section 6: the service writes its goodbye and then closes the stream, so a
/// frame written just before an end is dropped or shut down still reaches the peer, and only
/// then does the peer see the end of the stream.
#[test]
fn a_frame_written_before_the_close_still_arrives() {
    for shut_down in [false, true] {
        let (mut left, mut right) = pair().unwrap();
        write_frame(&mut left, &json!({"goodbye": "quit"})).unwrap();
        if shut_down {
            left.shutdown();
        } else {
            drop(left);
        }
        let (frame, then) = within_patience(move || {
            let frame: Value = read_frame(&mut right).unwrap().unwrap();
            (frame, read_frame::<Value>(&mut right).unwrap())
        });
        assert_eq!(frame, json!({"goodbye": "quit"}), "shut down: {shut_down}");
        assert!(then.is_none(), "shut down: {shut_down}");
    }
}

#[cfg(target_os = "linux")]
mod linux {
    //! The run and wake directories and their sockets, which only the user may reach.

    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use super::*;
    use crate::wake::{wake_all, Waiting, Wakes};

    fn mode(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    /// Relay-protocol-4 sections 1, 2 and 11: on Linux the run directory and the wake directory
    /// below it are the user's alone (0700, and an existing run directory that was open is
    /// tightened), and each socket is the owner's (0600). A client refuses an endpoint whose
    /// directory is open to group or other, since another account could have placed the socket,
    /// and a ready service leaves such a wake endpoint alone.
    #[test]
    fn the_run_and_wake_directories_are_private_and_a_client_refuses_an_open_one() {
        let home = TempHome::new();
        let run = home.path().join(RUN_DIRECTORY_NAME);
        fs::create_dir(&run).unwrap();
        fs::set_permissions(&run, fs::Permissions::from_mode(0o755)).unwrap();
        let _listener = Listener::bind_first(home.path()).unwrap();
        assert_eq!(mode(&run), 0o700, "an open run directory is tightened");
        assert_eq!(mode(&run.join(RELAY_SOCKET_NAME)), 0o600);
        let _waiting = Waiting::register(home.path()).unwrap();
        let wake = wake_directory(home.path());
        assert_eq!(mode(&wake), 0o700);
        let socket = fs::read_dir(&wake)
            .unwrap()
            .flatten()
            .next()
            .expect("one registration")
            .path();
        assert_eq!(mode(&socket), 0o600);

        for open in [0o750, 0o705] {
            fs::set_permissions(&run, fs::Permissions::from_mode(open)).unwrap();
            assert!(
                matches!(home.connect(), Err(ConnectError::ForeignServer)),
                "{open:o}"
            );
        }
        fs::set_permissions(&run, fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(&wake, fs::Permissions::from_mode(0o750)).unwrap();
        let id = socket.file_name().unwrap().to_str().unwrap().to_owned();
        assert!(matches!(
            connect_wake(home.path(), &id, Instant::now() + PATIENCE),
            Err(ConnectError::ForeignServer)
        ));
        assert_eq!(wake_all(home.path()), Wakes::default());
        assert!(
            socket.exists(),
            "a ready service removes no endpoint it cannot trust"
        );
    }
}
