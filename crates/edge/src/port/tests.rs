//! In-process tests of the port against a fake relay service over connected pairs and in-memory
//! Chrome pipes.
//!
//! Each test proves one rule of the browser edge that the process journeys do not prove as
//! precisely: what reaches the service and Chrome, what the edge says when no service is there,
//! and when it starts or looks for one.

use std::io::Cursor;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Mutex;
use std::thread::JoinHandle;
use std::time::Duration;

use ghostlight_relay::endpoint::pair;
use ghostlight_relay::frame;
use ghostlight_relay::protocol::{goodbye_frame, RefusalCode, Role};
use ghostlight_relay::MAX_FRAME_BYTES;
use serde_json::Value;

use super::*;

const PATIENCE: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_millis(5);
const CONTROL_RESULT: &[u8] = br#"{"kind":"result","value":null}"#;
/// A wait for an absence that no test waits out.
const NEVER_ABSENT: Duration = Duration::from_secs(600);
const ORIGIN: &str = "chrome-extension://abcdefghijklmnopabcdefghijklmnop/";

/// One browser stream the fake service accepted.
struct Browser {
    writer: Connection,
    received: Arc<Mutex<Vec<Vec<u8>>>>,
}

/// A relay service the test switches on and off.
#[derive(Default)]
struct FakeState {
    up: AtomicBool,
    start_brings_up: AtomicBool,
    /// How many openings to refuse before accepting.
    refusals: AtomicU32,
    starts: AtomicU32,
    /// The reason of every start, in order.
    reasons: Mutex<Vec<StartReason>>,
    /// Browser openings the edge tried, whether or not the service was up.
    attempts: AtomicU32,
    /// Control openings the edge tried, whether or not the service was up.
    probes: AtomicU32,
    openings: Mutex<Vec<Opening>>,
    browsers: Mutex<Vec<Browser>>,
    /// How many times the edge asked to be woken.
    watches: AtomicU32,
    /// How many times it had asked when it first started the service.
    watched_at_start: AtomicU32,
    /// Set to refuse every request to be woken.
    watch_fails: AtomicBool,
    /// What the edge asked to have called when a service wakes it.
    ready: Mutex<Option<Box<dyn Fn() + Send>>>,
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

    /// Wake the edge, as a service that became ready does.
    fn wake(&self) {
        if let Some(ready) = self.ready.lock().unwrap().as_ref() {
            ready();
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

/// The edge's view of the fake: connect while it is up; starting may bring it up.
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
        if self.0.starts.fetch_add(1, Ordering::SeqCst) == 0 {
            let watches = self.0.watches.load(Ordering::SeqCst);
            self.0.watched_at_start.store(watches, Ordering::SeqCst);
        }
        if self.0.start_brings_up.load(Ordering::SeqCst) {
            self.0.up.store(true, Ordering::SeqCst);
        }
        Ok(())
    }

    fn watch(&self, ready: Box<dyn Fn() + Send>) -> io::Result<()> {
        self.0.watches.fetch_add(1, Ordering::SeqCst);
        if self.0.watch_fails.load(Ordering::SeqCst) {
            return Err(io::Error::other("no wake endpoint"));
        }
        *self.0.ready.lock().unwrap() = Some(ready);
        Ok(())
    }
}

/// Chrome's end of the port as a channel of byte chunks; dropping the sender closes it.
struct ChannelInput {
    chunks: Receiver<Vec<u8>>,
    pending: Vec<u8>,
    /// Bytes the edge has read so far.
    delivered: Arc<AtomicUsize>,
    /// Set while the edge waits for bytes Chrome has not sent yet.
    waiting: Arc<AtomicBool>,
}

impl Read for ChannelInput {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.pending.is_empty() {
            self.waiting.store(true, Ordering::SeqCst);
            let chunk = self.chunks.recv();
            self.waiting.store(false, Ordering::SeqCst);
            match chunk {
                Ok(chunk) => self.pending = chunk,
                Err(_) => return Ok(0),
            }
        }
        let count = buffer.len().min(self.pending.len());
        buffer[..count].copy_from_slice(&self.pending[..count]);
        self.pending.drain(..count);
        self.delivered.fetch_add(count, Ordering::SeqCst);
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

/// A running edge wired to a fake service and in-memory Chrome pipes.
struct Harness {
    fake: Arc<FakeState>,
    input: Option<mpsc::Sender<Vec<u8>>>,
    /// Bytes Chrome has sent so far.
    sent: AtomicUsize,
    delivered: Arc<AtomicUsize>,
    waiting: Arc<AtomicBool>,
    output: SharedOutput,
    events: Sender<Event>,
    done: Option<JoinHandle<io::Result<Ended>>>,
}

impl Harness {
    fn start(up: bool, start_brings_up: bool) -> Self {
        Self::start_with(up, start_brings_up, NEVER_ABSENT)
    }

    /// An edge that tells Chrome no service is there once `absent_after` passes.
    fn start_absent_after(up: bool, absent_after: Duration) -> Self {
        Self::start_with(up, false, absent_after)
    }

    fn start_with(up: bool, start_brings_up: bool, absent_after: Duration) -> Self {
        let fake = FakeState::new(up, start_brings_up);
        let (input, chunks) = mpsc::channel();
        let delivered = Arc::new(AtomicUsize::new(0));
        let waiting = Arc::new(AtomicBool::new(false));
        let output = SharedOutput::default();
        let environment = Environment {
            service: Arc::new(FakeService(Arc::clone(&fake))),
            origin: ORIGIN.to_owned(),
            absent_after,
            log: no_log,
        };
        let (events, inbox) = mpsc::channel();
        let sender = events.clone();
        let stdout = output.clone();
        let stdin = ChannelInput {
            chunks,
            pending: Vec::new(),
            delivered: Arc::clone(&delivered),
            waiting: Arc::clone(&waiting),
        };
        let done = thread::spawn(move || run(environment, stdin, stdout, events, inbox));
        Self {
            fake,
            input: Some(input),
            sent: AtomicUsize::new(0),
            delivered,
            waiting,
            output,
            events: sender,
            done: Some(done),
        }
    }

    /// An edge whose first hello has been delivered to a service that is already up.
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
        self.sent.fetch_add(bytes.len(), Ordering::SeqCst);
        self.input.as_ref().unwrap().send(bytes).unwrap();
    }

    fn chrome_closes(&mut self) {
        self.input.take();
    }

    /// Whether the edge has read every byte Chrome sent, done with each frame, and waits for
    /// more.
    fn chrome_input_drained(&self) -> bool {
        self.waiting.load(Ordering::SeqCst)
            && self.delivered.load(Ordering::SeqCst) == self.sent.load(Ordering::SeqCst)
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

/// Relay-protocol-4 section 5 and ACCEPTANCE C13: the browser edge's first connection. Before
/// the extension's hello it opens nothing and starts nothing. It holds the hello, drops every
/// frame Chrome sends after it until a service accepts the stream, and sends the hello first.
/// It starts the service once, as a `browser` start, after its first failed attempt (a refused
/// opening is one), and never again while it keeps connecting.
#[test]
fn the_first_connection_carries_the_held_hello_after_one_browser_start() {
    let mut silent = Harness::start(true, false);
    silent.chrome_closes();
    assert_eq!(silent.finish().unwrap(), Ended::BrowserGone);
    assert!(
        silent.fake.openings().is_empty(),
        "nothing before the hello"
    );
    assert_eq!(silent.fake.starts.load(Ordering::SeqCst), 0);

    let mut harness = Harness::start(false, false);
    harness.chrome_sends(b"hello");
    harness.chrome_sends(b"early one");
    harness.chrome_sends(b"early two");
    // The scheduled attempts come 0.5 s and 1.5 s after the first.
    harness.wait(|| harness.fake.attempts.load(Ordering::SeqCst) >= 3);
    assert_eq!(
        harness.fake.starts.load(Ordering::SeqCst),
        1,
        "a browser start is made once, after the first failed attempt"
    );
    harness.wait(|| harness.chrome_input_drained());
    assert_eq!(harness.fake.browser_streams(), 0);
    harness.fake.bring_up();
    harness.wait(|| harness.fake.browser_streams() == 1);
    harness.chrome_sends(b"later");
    harness.wait(|| harness.fake.received(0).len() == 2);
    assert_eq!(
        harness.fake.received(0),
        [b"hello".to_vec(), b"later".to_vec()],
        "the hello goes first and the early frames were dropped"
    );
    assert_eq!(harness.fake.roles(), [Role::Browser]);
    assert_eq!(
        *harness.fake.reasons.lock().unwrap(),
        [StartReason::Browser]
    );
    harness.chrome_closes();
    assert_eq!(harness.finish().unwrap(), Ended::BrowserGone);

    let mut refused = Harness::start(true, false);
    refused.fake.refusals.store(1, Ordering::SeqCst);
    refused.chrome_sends(b"hello");
    refused.wait(|| refused.fake.received(0).len() == 1);
    assert_eq!(refused.fake.starts.load(Ordering::SeqCst), 1);
    assert_eq!(refused.fake.roles(), [Role::Browser, Role::Browser]);
    assert_eq!(refused.fake.received(0), [b"hello".to_vec()]);
    refused.chrome_closes();
    assert_eq!(refused.finish().unwrap(), Ended::BrowserGone);
}

/// ACCEPTANCE C4 and C8: the browser edge carries every frame both ways byte for byte, whatever
/// its bytes, without reading them: not JSON, empty, or larger than a pipe's buffer. A frame from
/// Chrome that declares more than the relay's largest frame ends the port as an error before
/// anything is misread.
#[test]
fn frames_cross_both_ways_byte_for_byte_up_to_the_relays_largest_frame() {
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

    let declared = u32::try_from(MAX_FRAME_BYTES + 1).unwrap();
    harness
        .input
        .as_ref()
        .unwrap()
        .send(declared.to_le_bytes().to_vec())
        .unwrap();
    assert!(harness.finish().is_err());
}

/// ACCEPTANCE C10 and relay-protocol-4 section 6: a lost service, whether its stream just ends
/// or it said `quit` or `replace` first, is told to Chrome once with the absent frame, and the
/// goodbye itself is never passed on. While it is away the browser edge drops what Chrome sends,
/// never starts the service and only probes with a read-only `readiness` opening. When a service
/// answers, the edge ends without opening a new stream, so the extension reconnects with a fresh
/// hello.
#[test]
fn a_lost_service_is_told_once_probed_read_only_and_never_started() {
    for goodbye in [
        None,
        Some(GoodbyeReason::Quit),
        Some(GoodbyeReason::Replace),
    ] {
        let mut harness = Harness::connected(b"hello");
        harness.fake.send(b"before the loss");
        harness.wait(|| harness.chrome_receives().len() == 1);
        if let Some(reason) = goodbye {
            harness.fake.send(&goodbye_frame(reason));
        }
        harness.fake.go_down();
        harness.wait(|| harness.chrome_receives().len() == 2);
        harness.chrome_sends(b"while away");
        // Two probe intervals pass: the edge keeps looking and keeps its hands off the service.
        harness.wait(|| harness.fake.probes.load(Ordering::SeqCst) >= 2);
        assert_eq!(
            harness.chrome_receives(),
            [b"before the loss".to_vec(), BACKEND_UNAVAILABLE.to_vec()],
            "{goodbye:?}: Chrome is told once and sees no goodbye"
        );
        assert_eq!(
            harness.fake.received(0),
            [b"hello".to_vec()],
            "{goodbye:?}: what Chrome sent while the service was away was dropped"
        );
        assert_eq!(harness.fake.starts.load(Ordering::SeqCst), 0, "{goodbye:?}");
        harness.fake.bring_up();
        assert_eq!(
            harness.finish().unwrap(),
            Ended::ServiceReturned,
            "{goodbye:?}"
        );
        assert_eq!(
            harness.fake.roles(),
            [Role::Browser, Role::Control],
            "{goodbye:?}: no new stream"
        );
        assert_eq!(
            harness.fake.openings()[1].control,
            Some(Control::Readiness),
            "{goodbye:?}: the probe changes nothing"
        );
        assert_eq!(harness.fake.starts.load(Ordering::SeqCst), 0, "{goodbye:?}");
    }
}

/// ACCEPTANCE C13 and ADR-0199 Decision 13: an `uninstall` or `session_end` goodbye, or the end
/// of the session that the edge's own listener hears, ends the port at once. Chrome is told
/// nothing more, nothing is started and no probe follows, whether the service was connected,
/// away, or never found.
#[test]
fn a_final_goodbye_or_the_sessions_end_ends_the_port_at_once() {
    for (reason, ended) in [
        (GoodbyeReason::Uninstall, Ended::ServiceUninstalled),
        (GoodbyeReason::SessionEnd, Ended::SessionOver),
    ] {
        let mut harness = Harness::connected(b"hello");
        harness.fake.send(&goodbye_frame(reason));
        assert_eq!(harness.finish().unwrap(), ended, "{reason:?}");
        assert!(harness.chrome_receives().is_empty(), "{reason:?}");
        assert_eq!(harness.fake.starts.load(Ordering::SeqCst), 0, "{reason:?}");
        assert_eq!(
            harness.fake.roles(),
            [Role::Browser],
            "{reason:?}: no probe follows"
        );
    }

    let mut away = Harness::connected(b"hello");
    away.fake.go_down();
    away.wait(|| away.chrome_receives().len() == 1);
    away.events.send(Event::SessionEnded).unwrap();
    assert_eq!(away.finish().unwrap(), Ended::SessionOver);
    assert_eq!(away.chrome_receives(), [BACKEND_UNAVAILABLE.to_vec()]);
    assert_eq!(away.fake.starts.load(Ordering::SeqCst), 0);

    let mut never_found = Harness::start(false, false);
    never_found.events.send(Event::SessionEnded).unwrap();
    assert_eq!(never_found.finish().unwrap(), Ended::SessionOver);
    assert!(never_found.fake.openings().is_empty());
    assert_eq!(never_found.fake.starts.load(Ordering::SeqCst), 0);
}

/// Defect 4f3d67d6: when no service has accepted within the absent wait of the edge's own
/// start, it tells Chrome so once with the absent frame, and not before, so the extension reads
/// not connected rather than an outdated service. It starts nothing more, drops what Chrome
/// sends and probes read-only, and a service that accepts after that ends the run without a new
/// stream.
#[test]
fn no_service_within_the_wait_is_told_to_chrome_once_and_not_before() {
    let wait = Duration::from_millis(1500);
    let began = Instant::now();
    let mut harness = Harness::start_absent_after(false, wait);
    harness.chrome_sends(b"hello");
    harness.wait(|| !harness.chrome_receives().is_empty());
    assert!(began.elapsed() >= wait, "the frame is not written early");
    harness.chrome_sends(b"while absent");
    harness.wait(|| harness.fake.probes.load(Ordering::SeqCst) >= 2);
    assert_eq!(harness.chrome_receives(), [BACKEND_UNAVAILABLE.to_vec()]);
    assert_eq!(harness.fake.starts.load(Ordering::SeqCst), 1);
    assert_eq!(harness.fake.browser_streams(), 0);
    assert!(harness
        .fake
        .roles()
        .iter()
        .skip_while(|role| **role == Role::Browser)
        .all(|role| *role == Role::Control));

    harness.fake.bring_up();
    assert_eq!(harness.finish().unwrap(), Ended::ServiceReturned);
    assert_eq!(harness.chrome_receives(), [BACKEND_UNAVAILABLE.to_vec()]);
    assert_eq!(harness.fake.starts.load(Ordering::SeqCst), 1);
    assert_eq!(
        harness.fake.browser_streams(),
        0,
        "the held hello is not sent; the extension sends a fresh one"
    );
}

/// Defect 4f3d67d6 and the adapter protocol 4 golden both sides read: the frame the edge writes
/// when no service is there is the golden `backend_unavailable` frame with `service` `absent`,
/// the one the extension's golden test reads, and the edge writes it before the extension's
/// hello wait (`limits.hello_timeout_ms`) runs out, or the extension would read the silence as
/// an outdated service.
#[test]
fn the_absence_is_the_golden_frame_and_comes_before_the_extensions_hello_wait() {
    let golden =
        include_str!("../../../../tests/golden/protocol4/frames/backend_unavailable.absent.json");
    assert_eq!(golden.trim().as_bytes(), BACKEND_UNAVAILABLE);
    let vocabulary: Value = serde_json::from_str(include_str!(
        "../../../../tests/golden/protocol4/vocabulary.json"
    ))
    .unwrap();
    let hello_wait = Duration::from_millis(
        vocabulary["limits"]["hello_timeout_ms"]
            .as_u64()
            .expect("the vocabulary names the extension's hello wait"),
    );
    assert!(
        ABSENT_AFTER < hello_wait,
        "the edge reports absence after {ABSENT_AFTER:?}, the extension stops waiting after {hello_wait:?}"
    );
}

/// Defect S-D3 (b0290246): the browser edge registers its wake endpoint once, before it first
/// starts the service, so a service that starts at once still finds it. Each wake makes it look
/// at once instead of at its next scheduled look, before its first connection and after a loss.
#[test]
fn a_wake_makes_the_browser_edge_look_at_once() {
    let mut harness = Harness::start(false, false);
    harness.chrome_sends(b"hello");
    harness.wait(|| harness.fake.starts.load(Ordering::SeqCst) == 1);
    assert_eq!(
        harness.fake.watched_at_start.load(Ordering::SeqCst),
        1,
        "registered before the start"
    );
    // The scheduled attempts come 0.5 s, 1.5 s and 3.5 s after the first.
    harness.wait(|| harness.fake.attempts.load(Ordering::SeqCst) >= 3);
    harness.fake.bring_up();
    let woken = Instant::now();
    harness.fake.wake();
    harness.wait(|| harness.fake.browser_streams() == 1);
    assert!(
        woken.elapsed() < Duration::from_secs(1),
        "the edge connected without waiting for its next scheduled attempt"
    );
    assert_eq!(harness.fake.received(0), [b"hello".to_vec()]);

    harness.fake.go_down();
    harness.wait(|| harness.chrome_receives().len() == 1);
    // The scheduled looks come 0.5 s, 1.5 s and 3.5 s after the loss.
    harness.wait(|| harness.fake.probes.load(Ordering::SeqCst) >= 2);
    harness.fake.bring_up();
    let woken = Instant::now();
    harness.fake.wake();
    assert_eq!(harness.finish().unwrap(), Ended::ServiceReturned);
    assert!(
        woken.elapsed() < Duration::from_secs(1),
        "the edge looked without waiting for its next scheduled look"
    );
    assert_eq!(
        harness.fake.watches.load(Ordering::SeqCst),
        1,
        "once for the life of the port"
    );
    assert_eq!(harness.fake.starts.load(Ordering::SeqCst), 1);
}

/// ACCEPTANCE C10: an edge that cannot register its wake endpoint keeps its scheduled looks and
/// still finds a service that comes back.
#[test]
fn an_edge_that_cannot_be_woken_still_finds_the_service_by_its_looks() {
    let mut harness = Harness::connected(b"hello");
    harness.fake.watch_fails.store(true, Ordering::SeqCst);
    harness.fake.go_down();
    harness.wait(|| harness.chrome_receives().len() == 1);
    harness.wait(|| harness.fake.watches.load(Ordering::SeqCst) == 1);
    harness.fake.bring_up();
    assert_eq!(harness.finish().unwrap(), Ended::ServiceReturned);
    assert_eq!(harness.fake.starts.load(Ordering::SeqCst), 0);
}
