//! In-process tests of the port against a fake relay-4 service over connected pairs and in-memory
//! Chrome pipes.

use std::io::Cursor;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Mutex;
use std::thread::JoinHandle;
use std::time::Duration;

use ghostlight_relay::endpoint::pair;
use ghostlight_relay::frame;
use ghostlight_relay::protocol::{goodbye_frame, RefusalCode, Role};
use ghostlight_relay::{MAX_FRAME_BYTES, RELAY};

use super::*;

const PATIENCE: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_millis(5);
const CONTROL_RESULT: &[u8] = br#"{"kind":"result","value":null}"#;
const ORIGIN: &str = "chrome-extension://abcdefghijklmnopabcdefghijklmnop/";

/// One browser stream the fake service accepted.
struct Browser {
    writer: Connection,
    received: Arc<Mutex<Vec<Vec<u8>>>>,
}

/// A relay-4 service the test switches on and off.
#[derive(Default)]
struct FakeState {
    up: AtomicBool,
    start_brings_up: AtomicBool,
    /// How many openings to refuse before accepting.
    refusals: AtomicU32,
    starts: AtomicU32,
    /// The reason of every start, in order.
    reasons: Mutex<Vec<StartReason>>,
    /// Browser openings the connector tried, whether or not the service was up.
    attempts: AtomicU32,
    /// Control openings the connector tried, whether or not the service was up.
    probes: AtomicU32,
    openings: Mutex<Vec<Opening>>,
    browsers: Mutex<Vec<Browser>>,
}

impl FakeState {
    fn new(up: bool, start_brings_up: bool) -> Arc<Self> {
        Arc::new(Self {
            up: AtomicBool::new(up),
            start_brings_up: AtomicBool::new(start_brings_up),
            ..Self::default()
        })
    }

    /// Answer one opening the way a service would, on the far end of a new pair.
    fn answer(&self, opening: &Opening) -> io::Result<(Connection, Reply)> {
        let (client, mut server) = pair()?;
        self.openings.lock().unwrap().push(opening.clone());
        let refused = self
            .refusals
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                left.checked_sub(1)
            })
            .is_ok();
        if refused {
            let reply = Reply::refused(RefusalCode::ShuttingDown, String::new());
            return Ok((client, reply));
        }
        let accepted = Reply::accepted("2.0.0".to_owned(), "instance_fake".to_owned(), None);
        match opening.role {
            Role::Browser => {
                let received = Arc::new(Mutex::new(Vec::new()));
                let sink = Arc::clone(&received);
                let mut reader = server.try_clone().unwrap();
                thread::spawn(move || {
                    while let Ok(Some(payload)) = frame::read_frame_raw(&mut reader) {
                        sink.lock().unwrap().push(payload);
                    }
                });
                self.browsers.lock().unwrap().push(Browser {
                    writer: server,
                    received,
                });
            }
            Role::Control => {
                frame::write_frame_raw(&mut server, CONTROL_RESULT).unwrap();
            }
            _ => {}
        }
        Ok((client, accepted))
    }

    fn bring_up(&self) {
        self.up.store(true, Ordering::SeqCst);
    }

    /// Switch the service off and close every browser stream.
    fn go_down(&self) {
        self.up.store(false, Ordering::SeqCst);
        for browser in self.browsers.lock().unwrap().iter() {
            browser.writer.shutdown();
        }
    }

    fn browser_streams(&self) -> usize {
        self.browsers.lock().unwrap().len()
    }

    fn openings(&self) -> Vec<Opening> {
        self.openings.lock().unwrap().clone()
    }

    /// The role of every opening the service saw, in order.
    fn roles(&self) -> Vec<Role> {
        self.openings().iter().map(|one| one.role).collect()
    }

    /// Frames received on browser stream `index`, in order.
    fn received(&self, index: usize) -> Vec<Vec<u8>> {
        let browsers = self.browsers.lock().unwrap();
        browsers
            .get(index)
            .map(|one| one.received.lock().unwrap().clone())
            .unwrap_or_default()
    }

    /// Send a raw frame on the newest browser stream.
    fn send(&self, payload: &[u8]) {
        let mut browsers = self.browsers.lock().unwrap();
        let newest = browsers.last_mut().unwrap();
        frame::write_frame_raw(&mut newest.writer, payload).unwrap();
    }
}

/// The connector's view of the fake: connect while it is up; starting may bring it up.
struct FakeService(Arc<FakeState>);

impl Service for FakeService {
    fn connect(&self, opening: &Opening) -> Result<(Connection, Reply), ConnectError> {
        if opening.role == Role::Control {
            self.0.probes.fetch_add(1, Ordering::SeqCst);
        } else {
            self.0.attempts.fetch_add(1, Ordering::SeqCst);
        }
        if !self.0.up.load(Ordering::SeqCst) {
            return Err(ConnectError::NoService);
        }
        Ok(self.0.answer(opening)?)
    }

    fn start(&self, reason: StartReason) -> io::Result<()> {
        self.0.reasons.lock().unwrap().push(reason);
        self.0.starts.fetch_add(1, Ordering::SeqCst);
        if self.0.start_brings_up.load(Ordering::SeqCst) {
            self.0.up.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
}

/// Chrome's end of the port as a channel of byte chunks; dropping the sender closes it.
struct ChannelInput {
    chunks: Receiver<Vec<u8>>,
    pending: Vec<u8>,
}

impl Read for ChannelInput {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.pending.is_empty() {
            match self.chunks.recv() {
                Ok(chunk) => self.pending = chunk,
                Err(_) => return Ok(0),
            }
        }
        let count = buffer.len().min(self.pending.len());
        buffer[..count].copy_from_slice(&self.pending[..count]);
        self.pending.drain(..count);
        Ok(count)
    }
}

/// Chrome's stdout as a shared buffer.
#[derive(Clone, Default)]
struct SharedOutput(Arc<Mutex<Vec<u8>>>);

impl Write for SharedOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn no_log(_: LogEvent, _: &str) {}

/// A running connector wired to a fake service and in-memory Chrome pipes.
struct Harness {
    fake: Arc<FakeState>,
    input: Option<mpsc::Sender<Vec<u8>>>,
    output: SharedOutput,
    events: Sender<Event>,
    done: Option<JoinHandle<io::Result<Ended>>>,
}

impl Harness {
    fn start(up: bool, start_brings_up: bool) -> Self {
        Self::start_from(up, start_brings_up, Some(ORIGIN.to_owned()))
    }

    /// A connector started by Chrome with `origin` as its first argument, if it had one.
    fn start_from(up: bool, start_brings_up: bool, origin: Option<String>) -> Self {
        let fake = FakeState::new(up, start_brings_up);
        let (input, chunks) = mpsc::channel();
        let output = SharedOutput::default();
        let environment = Environment {
            service: Arc::new(FakeService(Arc::clone(&fake))),
            origin,
            log: no_log,
        };
        let (events, inbox) = mpsc::channel();
        let sender = events.clone();
        let stdout = output.clone();
        let done = thread::spawn(move || {
            let stdin = ChannelInput {
                chunks,
                pending: Vec::new(),
            };
            run(environment, stdin, stdout, events, inbox)
        });
        Self {
            fake,
            input: Some(input),
            output,
            events: sender,
            done: Some(done),
        }
    }

    /// A connector whose first hello has been delivered to a service that is already up.
    fn connected(hello: &[u8]) -> Self {
        let harness = Self::start(true, false);
        harness.chrome_sends(hello);
        harness.wait(|| harness.fake.received(0).len() == 1);
        harness
    }

    /// Write one native frame from Chrome.
    fn chrome_sends(&self, payload: &[u8]) {
        let mut bytes = Vec::new();
        frame::write_frame_raw(&mut bytes, payload).unwrap();
        self.input.as_ref().unwrap().send(bytes).unwrap();
    }

    fn chrome_closes(&mut self) {
        self.input.take();
    }

    /// The frames written to Chrome so far.
    fn chrome_receives(&self) -> Vec<Vec<u8>> {
        let bytes = self.output.0.lock().unwrap().clone();
        let mut reader = Cursor::new(bytes);
        let mut frames = Vec::new();
        while let Ok(Some(payload)) = frame::read_frame_raw(&mut reader) {
            frames.push(payload);
        }
        frames
    }

    fn wait(&self, condition: impl Fn() -> bool) {
        let mut waited = Duration::ZERO;
        while !condition() {
            assert!(waited < PATIENCE, "gave up waiting");
            thread::sleep(POLL);
            waited += POLL;
        }
    }

    /// Wait for the run function to return.
    fn finish(&mut self) -> io::Result<Ended> {
        self.done.take().unwrap().join().unwrap()
    }
}

#[test]
fn the_held_hello_is_the_first_frame_the_service_sees() {
    let mut harness = Harness::start(false, false);
    harness.chrome_sends(b"hello");
    harness.chrome_sends(b"early one");
    harness.chrome_sends(b"early two");
    harness.wait(|| harness.fake.starts.load(Ordering::SeqCst) == 1);
    // The attempt that started the service has failed; the later frames were read and dropped
    // while the hello stayed held.
    thread::sleep(Duration::from_millis(100));
    assert_eq!(harness.fake.browser_streams(), 0);
    harness.fake.bring_up();
    harness.wait(|| harness.fake.browser_streams() == 1);
    harness.chrome_sends(b"later");
    harness.wait(|| harness.fake.received(0).len() == 2);
    assert_eq!(
        harness.fake.received(0),
        [b"hello".to_vec(), b"later".to_vec()]
    );
    harness.chrome_closes();
    assert_eq!(harness.finish().unwrap(), Ended::BrowserGone);
}

#[test]
fn the_service_is_started_before_the_first_connection_and_not_after_it_is_found() {
    let mut harness = Harness::start(false, true);
    harness.chrome_sends(b"hello");
    harness.wait(|| harness.fake.received(0).len() == 1);
    assert_eq!(harness.fake.starts.load(Ordering::SeqCst), 1);
    assert_eq!(harness.fake.roles(), [Role::Browser]);
    assert_eq!(
        *harness.fake.reasons.lock().unwrap(),
        [StartReason::Browser]
    );
    harness.chrome_closes();
    harness.finish().unwrap();
}

#[test]
fn a_service_that_stays_down_is_started_once_and_connected_to_when_it_appears() {
    let mut harness = Harness::start(false, false);
    harness.chrome_sends(b"hello");
    harness.wait(|| harness.fake.attempts.load(Ordering::SeqCst) >= 3);
    assert_eq!(
        harness.fake.starts.load(Ordering::SeqCst),
        1,
        "a browser start is made once, at the connector's own start"
    );
    harness.fake.bring_up();
    harness.wait(|| harness.fake.received(0).len() == 1);
    assert_eq!(harness.fake.received(0), [b"hello".to_vec()]);
    assert_eq!(harness.fake.starts.load(Ordering::SeqCst), 1);
    harness.chrome_closes();
    harness.finish().unwrap();
}

#[test]
fn a_refused_opening_keeps_the_hello_held_and_starts_the_service() {
    let mut harness = Harness::start(true, false);
    harness.fake.refusals.store(1, Ordering::SeqCst);
    harness.chrome_sends(b"hello");
    harness.wait(|| harness.fake.received(0).len() == 1);
    assert_eq!(harness.fake.starts.load(Ordering::SeqCst), 1);
    assert_eq!(harness.fake.roles(), [Role::Browser, Role::Browser]);
    assert_eq!(harness.fake.received(0), [b"hello".to_vec()]);
    harness.chrome_closes();
    harness.finish().unwrap();
}

#[test]
fn frames_cross_both_ways_byte_for_byte() {
    let mut harness = Harness::connected(b"hello");
    let not_json = vec![0_u8, 255, 1, 254, b'{', b'"'];
    let large = vec![b'x'; 300 * 1024];
    let empty: Vec<u8> = Vec::new();
    let to_service = [
        not_json,
        large,
        empty,
        br#"{"id":"7","kind":"command"}"#.to_vec(),
    ];
    for payload in &to_service {
        harness.chrome_sends(payload);
    }
    harness.wait(|| harness.fake.received(0).len() == 1 + to_service.len());
    let mut expected = vec![b"hello".to_vec()];
    expected.extend(to_service.iter().cloned());
    assert_eq!(harness.fake.received(0), expected);

    let to_chrome = [
        br#"{"kind":"hello_accepted"}"#.to_vec(),
        vec![9_u8, 8, 7, 0],
        vec![b'y'; 200 * 1024],
    ];
    for payload in &to_chrome {
        harness.fake.send(payload);
    }
    harness.wait(|| harness.chrome_receives().len() == to_chrome.len());
    assert_eq!(harness.chrome_receives(), to_chrome);
    harness.chrome_closes();
    harness.finish().unwrap();
}

#[test]
fn losing_the_service_tells_chrome_once_and_never_starts_it() {
    let mut harness = Harness::connected(b"hello");
    harness.fake.send(b"before the loss");
    harness.wait(|| harness.chrome_receives().len() == 1);
    harness.fake.go_down();
    harness.wait(|| harness.chrome_receives().len() == 2);
    harness.chrome_sends(b"while away");
    // Two probe intervals pass: the connector keeps looking and keeps its hands off the service.
    harness.wait(|| harness.fake.probes.load(Ordering::SeqCst) >= 2);
    assert_eq!(harness.fake.starts.load(Ordering::SeqCst), 0);
    assert_eq!(harness.fake.browser_streams(), 1);
    assert_eq!(harness.fake.received(0), [b"hello".to_vec()]);
    assert_eq!(
        harness.chrome_receives(),
        [b"before the loss".to_vec(), BACKEND_UNAVAILABLE.to_vec()]
    );
    harness.chrome_closes();
    assert_eq!(harness.finish().unwrap(), Ended::BrowserGone);
}

#[test]
fn a_returning_service_ends_the_run_without_a_new_stream() {
    let mut harness = Harness::connected(b"hello");
    harness.fake.go_down();
    harness.wait(|| harness.chrome_receives().len() == 1);
    harness.wait(|| harness.fake.probes.load(Ordering::SeqCst) >= 1);
    harness.fake.bring_up();
    assert_eq!(harness.finish().unwrap(), Ended::ServiceReturned);
    assert_eq!(harness.fake.starts.load(Ordering::SeqCst), 0);
    assert_eq!(harness.fake.roles(), [Role::Browser, Role::Control]);
    assert_eq!(
        harness.fake.openings()[1].control,
        Some(Control::Readiness),
        "the probe changes nothing"
    );
    assert_eq!(harness.chrome_receives(), [BACKEND_UNAVAILABLE.to_vec()]);
}

#[test]
fn chrome_closing_its_pipe_ends_the_run_cleanly() {
    let mut harness = Harness::connected(b"hello");
    harness.chrome_closes();
    assert_eq!(harness.finish().unwrap(), Ended::BrowserGone);
    // The close was ours, not the service's, so Chrome is told nothing.
    thread::sleep(Duration::from_millis(100));
    assert!(harness.chrome_receives().is_empty());
}

#[test]
fn chrome_closing_before_any_hello_ends_the_run_without_a_connection() {
    let mut harness = Harness::start(true, false);
    harness.chrome_closes();
    assert_eq!(harness.finish().unwrap(), Ended::BrowserGone);
    assert!(harness.fake.openings().is_empty());
    assert_eq!(harness.fake.starts.load(Ordering::SeqCst), 0);
}

#[test]
fn chrome_closing_while_the_service_is_away_ends_the_run() {
    let mut harness = Harness::start(false, false);
    harness.chrome_sends(b"hello");
    harness.wait(|| harness.fake.starts.load(Ordering::SeqCst) == 1);
    harness.chrome_closes();
    assert_eq!(harness.finish().unwrap(), Ended::BrowserGone);
}

#[test]
fn the_parent_going_away_ends_the_run() {
    let mut harness = Harness::connected(b"hello");
    harness.events.send(Event::ParentGone).unwrap();
    assert_eq!(harness.finish().unwrap(), Ended::ParentGone);
}

#[test]
fn an_unreadable_chrome_frame_is_an_error() {
    let mut harness = Harness::connected(b"hello");
    let declared = u32::try_from(MAX_FRAME_BYTES + 1).unwrap();
    harness
        .input
        .as_ref()
        .unwrap()
        .send(declared.to_le_bytes().to_vec())
        .unwrap();
    assert!(harness.finish().is_err());
}

#[test]
fn the_opening_names_the_edge_and_carries_the_extension_origin() {
    let mut harness = Harness::connected(b"hello");
    let opening = &harness.fake.openings()[0];
    assert_eq!(opening.relay, RELAY);
    assert_eq!(opening.role, Role::Browser);
    assert_eq!(opening.edge.program, Program::Edge);
    assert_eq!(opening.edge.revision, RELAY);
    assert_eq!(opening.edge.build, env!("CARGO_PKG_VERSION"));
    assert_eq!(opening.origin.as_deref(), Some(ORIGIN));
    harness.chrome_closes();
    harness.finish().unwrap();
}

#[test]
fn an_opening_without_an_origin_when_chrome_named_none() {
    let mut harness = Harness::start_from(true, false, None);
    harness.chrome_sends(b"hello");
    harness.wait(|| harness.fake.received(0).len() == 1);
    assert_eq!(harness.fake.openings()[0].origin, None);
    harness.chrome_closes();
    harness.finish().unwrap();
}

#[test]
fn only_a_first_argument_that_names_an_extension_is_an_origin() {
    assert_eq!(
        extension_origin(Some(ORIGIN.to_owned())).as_deref(),
        Some(ORIGIN)
    );
    assert_eq!(
        extension_origin(Some("--parent-window=4242".to_owned())),
        None
    );
    assert_eq!(
        extension_origin(Some("https://example.com/".to_owned())),
        None
    );
    assert_eq!(extension_origin(None), None);
}

#[test]
fn quit_and_replace_goodbyes_are_not_forwarded_and_are_a_lost_service() {
    for reason in [GoodbyeReason::Quit, GoodbyeReason::Replace] {
        let mut harness = Harness::connected(b"hello");
        harness.fake.send(&goodbye_frame(reason));
        harness.fake.go_down();
        harness.wait(|| harness.chrome_receives().len() == 1);
        harness.wait(|| harness.fake.probes.load(Ordering::SeqCst) >= 2);
        assert_eq!(
            harness.chrome_receives(),
            [BACKEND_UNAVAILABLE.to_vec()],
            "{reason:?}: Chrome is told the service is away and sees no goodbye"
        );
        assert_eq!(
            harness.fake.starts.load(Ordering::SeqCst),
            0,
            "{reason:?}: nothing is started while waiting"
        );
        harness.fake.bring_up();
        assert_eq!(
            harness.finish().unwrap(),
            Ended::ServiceReturned,
            "{reason:?}"
        );
    }
}

#[test]
fn an_uninstall_goodbye_ends_the_run_at_once_without_telling_chrome() {
    let mut harness = Harness::connected(b"hello");
    harness.fake.send(&goodbye_frame(GoodbyeReason::Uninstall));
    assert_eq!(harness.finish().unwrap(), Ended::ServiceUninstalled);
    assert!(harness.chrome_receives().is_empty());
    assert_eq!(harness.fake.starts.load(Ordering::SeqCst), 0);
    assert_eq!(harness.fake.roles(), [Role::Browser], "no probe follows");
}
