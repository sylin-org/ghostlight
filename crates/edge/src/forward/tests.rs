//! In-process tests of the forwarding loop against a fake relay service over connected pairs.
//!
//! Each test proves one rule of the MCP edge that the process journeys do not prove as precisely:
//! what reaches the service, what the edge answers in its place, and when it starts the service.
//! The edge's answers are compared with the whole templates of the accepted-reply golden the
//! service also reads (`tests/golden/relay5/reply.accepted.json`, revision 4's with `too_large`),
//! never with words restated here.

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use ghostlight_relay::endpoint::pair;
use ghostlight_relay::frame;
use ghostlight_relay::protocol::{McpFrame, Opening, Reply, Templates, DEFAULT_TEMPLATE};
use ghostlight_relay::{BACKOFF_FIRST, HOLD_LIMIT, HOLD_LIMIT_BYTES};
use serde_json::{json, Value};

use super::*;

const TICK: Duration = Duration::from_millis(2);
const PATIENCE: Duration = Duration::from_secs(5);
/// How long `Harness::advance_until` lets the edge react to one step before taking the next.
const STEP_GRACE: Duration = Duration::from_millis(20);

/// The failure templates the golden accepted reply carries.
fn golden_templates() -> Templates {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/relay5/reply.accepted.json");
    let reply: Reply = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    reply.templates.expect("the golden reply carries templates")
}

/// The golden reply's `failure` template under `key` (a method, or [`DEFAULT_TEMPLATE`]) with
/// `id` set: the whole answer the edge must give, taken from the golden rather than restated.
fn golden(failure: Failure, key: &str, id: &Value) -> Value {
    let templates = golden_templates();
    let table = match failure {
        Failure::CutOff => &templates.cut_off,
        Failure::Expired => &templates.expired,
        Failure::TooLarge => &templates.too_large,
    };
    let mut answer = table[key].clone();
    answer["id"] = id.clone();
    answer
}

/// One connection the fake service accepted.
struct Accepted {
    opening: Opening,
    writer: Connection,
    received: Arc<Mutex<Vec<String>>>,
    /// Set once the edge closed its end.
    closed: Arc<AtomicBool>,
}

/// A relay service the test switches on and off.
struct FakeState {
    up: AtomicBool,
    start_brings_up: AtomicBool,
    /// When set, the next connection is accepted and then closed at once.
    drop_next: AtomicBool,
    starts: AtomicU32,
    /// The reason of every start, in order.
    reasons: Mutex<Vec<StartReason>>,
    /// Connection attempts, whether or not the service was up.
    attempts: AtomicU32,
    /// The templates the accepted reply carries.
    templates: Mutex<Option<Templates>>,
    accepted: Mutex<Vec<Accepted>>,
    /// How many times the edge asked to be woken.
    watches: AtomicU32,
    /// What the edge asked to have called when a service wakes it.
    ready: Mutex<Option<Box<dyn Fn() + Send>>>,
}

impl FakeState {
    fn new(up: bool, start_brings_up: bool) -> Arc<Self> {
        Arc::new(Self {
            up: AtomicBool::new(up),
            start_brings_up: AtomicBool::new(start_brings_up),
            drop_next: AtomicBool::new(false),
            starts: AtomicU32::new(0),
            reasons: Mutex::new(Vec::new()),
            attempts: AtomicU32::new(0),
            templates: Mutex::new(Some(golden_templates())),
            accepted: Mutex::new(Vec::new()),
            watches: AtomicU32::new(0),
            ready: Mutex::new(None),
        })
    }

    /// Take the service end of a new connection: record the opening and the lines received.
    fn serve(&self, connection: Connection, opening: Opening) {
        let received = Arc::new(Mutex::new(Vec::new()));
        let closed = Arc::new(AtomicBool::new(false));
        if self.drop_next.swap(false, Ordering::SeqCst) {
            connection.shutdown();
        } else {
            let mut reader = connection.try_clone().unwrap();
            let sink = Arc::clone(&received);
            let ended = Arc::clone(&closed);
            thread::spawn(move || {
                while let Ok(Some(frame)) = frame::read_frame::<McpFrame>(&mut reader) {
                    if let McpFrame::Message { line } = frame {
                        sink.lock().unwrap().push(line);
                    }
                }
                ended.store(true, Ordering::SeqCst);
            });
        }
        self.accepted.lock().unwrap().push(Accepted {
            opening,
            writer: connection,
            received,
            closed,
        });
    }

    /// Wake the edge, as a service that became ready does.
    fn wake(&self) {
        if let Some(ready) = self.ready.lock().unwrap().as_ref() {
            ready();
        }
    }

    fn connections(&self) -> usize {
        self.accepted.lock().unwrap().len()
    }

    fn openings(&self) -> Vec<Opening> {
        let accepted = self.accepted.lock().unwrap();
        accepted.iter().map(|one| one.opening.clone()).collect()
    }

    /// Lines received on connection `index`, in order.
    fn received(&self, index: usize) -> Vec<String> {
        let accepted = self.accepted.lock().unwrap();
        accepted
            .get(index)
            .map(|one| one.received.lock().unwrap().clone())
            .unwrap_or_default()
    }

    /// Whether the edge has closed its end of connection `index`.
    fn closed(&self, index: usize) -> bool {
        let accepted = self.accepted.lock().unwrap();
        accepted
            .get(index)
            .is_some_and(|one| one.closed.load(Ordering::SeqCst))
    }

    /// Send a frame on the newest connection.
    fn send(&self, frame: &McpFrame) {
        let mut accepted = self.accepted.lock().unwrap();
        let newest = accepted.last_mut().unwrap();
        frame::write_frame(&mut newest.writer, frame).unwrap();
    }

    /// Switch the service on again.
    fn bring_up(&self) {
        self.up.store(true, Ordering::SeqCst);
    }

    /// Switch the service off and close every connection.
    fn go_down(&self) {
        self.up.store(false, Ordering::SeqCst);
        for one in self.accepted.lock().unwrap().iter() {
            one.writer.shutdown();
        }
    }
}

/// The edge's view of the fake: connect while it is up; starting may bring it up.
struct FakeService(Arc<FakeState>);

impl Service for FakeService {
    fn connect(&self, opening: &Opening) -> Result<(Connection, Reply), ConnectError> {
        self.0.attempts.fetch_add(1, Ordering::SeqCst);
        if !self.0.up.load(Ordering::SeqCst) {
            return Err(ConnectError::NoService);
        }
        let (client, server) = pair()?;
        let templates = self.0.templates.lock().unwrap().clone();
        let reply = Reply::accepted("2.0.0".to_owned(), "instance_fake".to_owned(), templates);
        self.0.serve(server, opening.clone());
        Ok((client, reply))
    }

    fn start(&self, reason: StartReason) -> io::Result<()> {
        self.0.reasons.lock().unwrap().push(reason);
        self.0.starts.fetch_add(1, Ordering::SeqCst);
        if self.0.start_brings_up.load(Ordering::SeqCst) {
            self.0.up.store(true, Ordering::SeqCst);
        }
        Ok(())
    }

    fn watch(&self, ready: Box<dyn Fn() + Send>) -> io::Result<()> {
        self.0.watches.fetch_add(1, Ordering::SeqCst);
        *self.0.ready.lock().unwrap() = Some(ready);
        Ok(())
    }
}

/// A clock the test moves by hand.
struct FakeClock(Mutex<Instant>);

impl Clock for FakeClock {
    fn now(&self) -> Instant {
        *self.0.lock().unwrap()
    }
}

/// Stdin as a channel of byte chunks; dropping the sender closes it.
struct ChannelInput {
    chunks: Receiver<Vec<u8>>,
    pending: Vec<u8>,
    /// How much of `pending` has been read, so a chunk of many megabytes is never shifted.
    taken: usize,
}

impl Read for ChannelInput {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.taken == self.pending.len() {
            match self.chunks.recv() {
                Ok(chunk) => {
                    self.pending = chunk;
                    self.taken = 0;
                }
                Err(_) => return Ok(0),
            }
        }
        let rest = &self.pending[self.taken..];
        let count = buffer.len().min(rest.len());
        buffer[..count].copy_from_slice(&rest[..count]);
        self.taken += count;
        Ok(count)
    }
}

/// Stdout as a shared buffer.
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

/// A running edge wired to a fake service, fake stdio and a fake clock.
struct Harness {
    fake: Arc<FakeState>,
    clock: Arc<FakeClock>,
    events: mpsc::Sender<Event>,
    input: Option<mpsc::Sender<Vec<u8>>>,
    output: SharedOutput,
    done: Option<JoinHandle<io::Result<()>>>,
}

fn no_log(_: LogEvent, _: &str) {}

impl Harness {
    fn start(up: bool, start_brings_up: bool) -> Self {
        let fake = FakeState::new(up, start_brings_up);
        let clock = Arc::new(FakeClock(Mutex::new(Instant::now())));
        let (input, chunks) = mpsc::channel();
        let output = SharedOutput::default();
        let environment = Environment {
            service: Arc::new(FakeService(Arc::clone(&fake))),
            clock: Arc::clone(&clock) as Arc<dyn Clock>,
            log: no_log,
            poll: Some(TICK),
        };
        let stdout = output.clone();
        let (events, inbox) = mpsc::channel();
        let sender = events.clone();
        let done = thread::spawn(move || {
            let stdin = ChannelInput {
                chunks,
                pending: Vec::new(),
                taken: 0,
            };
            run(environment, stdin, stdout, sender, inbox)
        });
        Self {
            fake,
            clock,
            events,
            input: Some(input),
            output,
            done: Some(done),
        }
    }

    fn connected(up: bool) -> Self {
        let harness = Self::start(up, true);
        harness.wait(|| harness.fake.connections() == 1);
        harness
    }

    fn send(&self, line: &str) {
        let mut bytes = line.as_bytes().to_vec();
        bytes.push(b'\n');
        self.input.as_ref().unwrap().send(bytes).unwrap();
    }

    fn advance(&self, by: Duration) {
        *self.clock.0.lock().unwrap() += by;
    }

    /// Move the clock on by `by` a step at a time until `condition` holds.
    ///
    /// The edge schedules its next attempt from the clock at the moment it handles a failed
    /// one, and a test cannot see when that is. A single jump made just before the handling
    /// lands short of the new deadline, so the clock keeps moving, one step per round trip
    /// through the edge. The steps stay inside half the hold limit so that no held line
    /// expires on the way.
    fn advance_until(&self, by: Duration, condition: impl Fn() -> bool) {
        let steps = (HOLD_LIMIT / 2).as_millis() / by.as_millis().max(1);
        for _ in 0..steps {
            if condition() {
                return;
            }
            self.advance(by);
            self.settle();
            let deadline = Instant::now() + STEP_GRACE;
            while Instant::now() < deadline && !condition() {
                thread::sleep(TICK);
            }
        }
        self.wait(condition);
    }

    fn lines(&self) -> Vec<String> {
        let bytes = self.output.0.lock().unwrap().clone();
        String::from_utf8(bytes)
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn values(&self) -> Vec<Value> {
        self.lines()
            .iter()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    /// The answer written for `id`, waiting for it to appear.
    fn answer(&self, id: &Value) -> Value {
        self.wait(|| self.values().iter().any(|value| &value["id"] == id));
        self.values()
            .into_iter()
            .find(|value| &value["id"] == id)
            .unwrap()
    }

    /// Send a ping and wait for its answer: every line sent before it has been handled.
    fn settle(&self) {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let id = format!("settle-{}", NEXT.fetch_add(1, Ordering::SeqCst));
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": "ping"}).to_string());
        self.answer(&json!(id));
    }

    /// Take the service away while a request is outstanding, and wait until the edge has
    /// answered it as cut off.
    fn go_away(&self, connection: usize) {
        let probe = format!("probe-{connection}");
        self.send(&request(&json!(probe), "tools/call"));
        self.wait(|| {
            self.fake
                .received(connection)
                .last()
                .is_some_and(|line| line.contains(&probe))
        });
        self.fake.go_down();
        self.answer(&json!(probe));
    }

    fn wait(&self, condition: impl Fn() -> bool) {
        let deadline = Instant::now() + PATIENCE;
        while Instant::now() < deadline {
            if condition() {
                return;
            }
            thread::sleep(TICK);
        }
        assert!(condition(), "condition not met; output: {:?}", self.lines());
    }

    /// Close stdin and wait for the edge to end.
    fn close(&mut self) -> io::Result<()> {
        self.input.take();
        self.finish()
    }

    /// Wait for the edge to end without closing stdin.
    fn finish(&mut self) -> io::Result<()> {
        let done = self.done.take().unwrap();
        self.wait(|| done.is_finished());
        done.join().unwrap()
    }
}

fn request(id: &Value, method: &str) -> String {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": {}}).to_string()
}

/// Send the service's goodbye on the newest connection and wait until the edge has acted
/// on it by closing its end.
fn say_goodbye(harness: &Harness, reason: GoodbyeReason) {
    let connection = harness.fake.connections() - 1;
    harness.fake.send(&McpFrame::Goodbye { reason });
    harness.wait(|| harness.fake.closed(connection));
}

/// ACCEPTANCE C3: the MCP edge forwards every line unchanged in both directions, with no
/// per-tool code. Its one exception is `ping`, which it answers itself with the id exactly as
/// the client wrote it, while connected and while the service is away; no ping reaches the
/// service, and answering one starts nothing.
#[test]
fn lines_pass_unchanged_and_the_edge_answers_only_ping_itself() {
    let harness = Harness::connected(true);
    let lines = [
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}"#,
        "not json at all",
        r#"{ "jsonrpc" : "2.0", "method" : "notifications/initialized" }"#,
    ];
    for line in lines {
        harness.send(line);
    }
    harness.wait(|| harness.fake.received(0).len() == lines.len());
    assert_eq!(harness.fake.received(0), lines);

    let from_service = r#"{"jsonrpc":"2.0","id":1,"result":{ "spaced" : true }}"#;
    harness.fake.send(&McpFrame::Message {
        line: from_service.to_owned(),
    });
    harness.wait(|| harness.lines().len() == 1);
    assert_eq!(harness.lines(), [from_service]);

    harness.send(&request(&json!("7"), "ping"));
    harness.send(&request(&json!(7), "ping"));
    assert_eq!(
        harness.answer(&json!("7")),
        json!({"jsonrpc": "2.0", "id": "7", "result": {}})
    );
    assert_eq!(
        harness.answer(&json!(7)),
        json!({"jsonrpc": "2.0", "id": 7, "result": {}})
    );
    harness.go_away(0);
    harness.send(&request(&json!(8), "ping"));
    assert_eq!(
        harness.answer(&json!(8)),
        json!({"jsonrpc": "2.0", "id": 8, "result": {}})
    );
    harness.settle();
    assert_eq!(
        harness.fake.received(0).len(),
        lines.len() + 1,
        "only the lines and the probe reached the service, never a ping"
    );
    assert_eq!(harness.fake.starts.load(Ordering::SeqCst), 0);
}

/// ACCEPTANCE C10 and relay-protocol-4 section 8: when the service goes away the edge answers
/// every request still outstanding, and only those, from the `cut_off` templates of the last
/// accepted reply that carried any: the method's own template or the default, with the id
/// exactly as sent. A batch is tracked element by element: an element the service answered is
/// not answered again, and a batch is never sent again (defect eaf7948d).
#[test]
fn a_cut_off_answers_each_outstanding_request_with_its_template_and_its_id() {
    let harness = Harness::connected(true);
    harness.send(&request(&json!("a"), "tools/call"));
    harness.send(&request(&json!(2), "completion/complete"));
    let batch = json!([
        {"jsonrpc": "2.0", "id": 1, "method": "tools/call"},
        {"jsonrpc": "2.0", "id": "b", "method": "tools/list"},
        {"jsonrpc": "2.0", "method": "notifications/progress"},
    ])
    .to_string();
    harness.send(&batch);
    harness.wait(|| harness.fake.received(0).len() == 3);
    let answered = json!([{"jsonrpc": "2.0", "id": 1, "result": {}}]).to_string();
    harness.fake.send(&McpFrame::Message {
        line: answered.clone(),
    });
    harness.wait(|| harness.lines() == [answered.clone()]);
    harness.fake.go_down();
    assert_eq!(
        harness.answer(&json!("a")),
        golden(Failure::CutOff, "tools/call", &json!("a"))
    );
    assert_eq!(
        harness.answer(&json!(2)),
        golden(Failure::CutOff, DEFAULT_TEMPLATE, &json!(2))
    );
    assert_eq!(
        harness.answer(&json!("b")),
        golden(Failure::CutOff, DEFAULT_TEMPLATE, &json!("b")),
        "a listing inside a batch is answered, never sent again"
    );
    harness.settle();
    assert!(
        !harness.values().iter().any(|value| value["id"] == json!(1)),
        "an element the service answered is not answered again"
    );

    // The next service's reply carries no templates, so the last ones stay.
    *harness.fake.templates.lock().unwrap() = None;
    harness.send(&request(&json!(5), "tools/call"));
    harness.settle();
    harness.wait(|| harness.fake.starts.load(Ordering::SeqCst) == 1);
    harness.advance_until(BACKOFF_FIRST, || harness.fake.received(1).len() == 1);
    assert_eq!(
        harness.fake.received(1),
        [request(&json!(5), "tools/call")],
        "nothing of the lost connection is sent again"
    );
    harness.fake.go_down();
    assert_eq!(
        harness.answer(&json!(5)),
        golden(Failure::CutOff, "tools/call", &json!(5))
    );
}

/// Relay-protocol-4 sections 8 and 10: while the service is away the edge holds at most 4 MiB,
/// each line for at most 30 s. A held request that outlives the limit, oldest first, is answered
/// from the `expired` templates of the last accepted reply, its method's own or the default; one
/// that finds the hold full is answered at once; a held notification expires silently; and
/// before any reply the answer is a bare internal error with an empty message.
#[test]
fn a_request_the_hold_cannot_keep_is_answered_as_expired() {
    let harness = Harness::start(true, false);
    harness.wait(|| harness.fake.connections() == 1);
    harness.go_away(0);

    harness.send(&request(&json!(1), "tools/call"));
    harness.settle();
    harness.advance(Duration::from_secs(10));
    let notification = json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string();
    let younger = request(&json!("x"), "resources/list");
    harness.send(&notification);
    harness.send(&younger);
    harness.settle();
    harness.advance(Duration::from_secs(20));
    assert_eq!(
        harness.answer(&json!(1)),
        golden(Failure::Expired, "tools/call", &json!(1))
    );
    harness.settle();
    assert!(
        !harness
            .values()
            .iter()
            .any(|value| value["id"] == json!("x")),
        "a line held for less than the limit is still held"
    );

    // Fill the hold to its last byte: a request that does not fit is answered at once.
    let padding = |length: usize| {
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/message",
            "params": {"padding": "x".repeat(length)},
        })
        .to_string()
    };
    let room = HOLD_LIMIT_BYTES - notification.len() - younger.len();
    let filler = padding(room - padding(0).len());
    assert_eq!(filler.len(), room);
    harness.send(&filler);
    harness.send(&request(&json!("full"), "tools/call"));
    assert_eq!(
        harness.answer(&json!("full")),
        golden(Failure::Expired, "tools/call", &json!("full"))
    );

    harness.advance(Duration::from_secs(10));
    assert_eq!(
        harness.answer(&json!("x")),
        golden(Failure::Expired, DEFAULT_TEMPLATE, &json!("x"))
    );
    harness.settle();
    let answered: Vec<Value> = harness
        .values()
        .into_iter()
        .map(|value| value["id"].clone())
        .filter(|id| !id.as_str().is_some_and(|id| id.starts_with("settle-")))
        .collect();
    assert_eq!(
        answered,
        [json!("probe-0"), json!(1), json!("full"), json!("x")],
        "held notifications expire silently"
    );
    assert_eq!(harness.fake.connections(), 1);

    let unreplied = Harness::start(false, false);
    unreplied.send(&request(&json!(5), "tools/call"));
    unreplied.settle();
    unreplied.advance(Duration::from_secs(60));
    assert_eq!(
        unreplied.answer(&json!(5)),
        json!({"jsonrpc": "2.0", "id": 5, "error": {"code": -32603, "message": ""}}),
        "before any reply the edge has no words of its own"
    );
}

/// ACCEPTANCE C10: lines held while the service was away reach the next service once and in
/// order, on an opening that carries the session's last resume token, and a call already written
/// to the lost service is answered as cut off, never sent again.
#[test]
fn held_lines_reach_the_next_service_once_in_order_under_the_sessions_token() {
    let harness = Harness::connected(true);
    harness.fake.send(&McpFrame::Resume {
        token: "resume-1".to_owned(),
    });
    harness.send(&request(&json!(1), "tools/call"));
    harness.go_away(0);
    assert_eq!(
        harness.answer(&json!(1)),
        golden(Failure::CutOff, "tools/call", &json!(1))
    );

    let held = [
        request(&json!(10), "tools/call"),
        json!({"jsonrpc": "2.0", "method": "notifications/cancelled"}).to_string(),
        request(&json!(11), "tools/list"),
    ];
    for line in &held {
        harness.send(line);
    }
    harness.settle();
    harness.wait(|| harness.fake.starts.load(Ordering::SeqCst) == 1);
    harness.advance_until(BACKOFF_FIRST, || {
        harness.fake.received(1).len() == held.len()
    });
    harness.settle();
    assert_eq!(
        harness.fake.received(1),
        held,
        "each held line once, in order, and nothing the lost service was sent"
    );
    let openings = harness.fake.openings();
    assert_eq!(openings[0].resume, None);
    assert_eq!(openings[1].resume.as_deref(), Some("resume-1"));
}

/// Defect eaf7948d: a lone handshake or catalog listing changes nothing, so when a lost service
/// cuts it off it is sent to the next service once instead of being answered. Cut off again it
/// is answered, so a request that brings each service down cannot keep doing so. After a quit
/// goodbye it is answered at once and nothing is held, so Quit still holds.
#[test]
fn a_lone_listing_cut_off_by_a_lost_service_is_sent_to_the_next_one_once() {
    let harness = Harness::connected(true);
    harness.send(&request(&json!(1), "tools/call"));
    harness.send(&request(&json!(2), "tools/list"));
    harness.wait(|| harness.fake.received(0).len() == 2);
    harness.fake.go_down();
    assert_eq!(
        harness.answer(&json!(1)),
        golden(Failure::CutOff, "tools/call", &json!(1)),
        "a call may have taken effect, so it is answered"
    );
    harness.wait(|| harness.fake.starts.load(Ordering::SeqCst) == 1);
    harness.advance_until(BACKOFF_FIRST, || harness.fake.received(1).len() == 1);
    assert_eq!(harness.fake.received(1), [request(&json!(2), "tools/list")]);
    harness.settle();
    assert!(
        !harness.values().iter().any(|value| value["id"] == json!(2)),
        "the next service answers the listing, not the edge"
    );
    harness.fake.go_down();
    assert_eq!(
        harness.answer(&json!(2)),
        golden(Failure::CutOff, DEFAULT_TEMPLATE, &json!(2)),
        "a listing that brings each service down is not sent a third time"
    );
    harness.settle();
    assert_eq!(
        harness.fake.starts.load(Ordering::SeqCst),
        1,
        "nothing is held"
    );

    let quitting = Harness::connected(true);
    quitting.send(&request(&json!(1), "tools/list"));
    quitting.wait(|| quitting.fake.received(0).len() == 1);
    say_goodbye(&quitting, GoodbyeReason::Quit);
    assert_eq!(
        quitting.answer(&json!(1)),
        golden(Failure::CutOff, DEFAULT_TEMPLATE, &json!(1))
    );
    quitting.advance(Duration::from_secs(60));
    quitting.settle();
    assert_eq!(quitting.fake.starts.load(Ordering::SeqCst), 0, "Quit holds");
    assert_eq!(
        quitting.fake.connections(),
        1,
        "nothing is held for a next service"
    );
}

/// Relay-protocol-4 section 6 and ACCEPTANCE C13: after a `replace` goodbye the edge holds lines
/// and reconnects without starting the service, since the installer starts the replacement. A
/// replacement that has not come back within the hold limit is a lost service, and the next
/// request starts one as an `agent` start.
#[test]
fn a_replace_holds_lines_without_starting_the_service_for_the_hold_limit() {
    let harness = Harness::connected(true);
    harness.fake.up.store(false, Ordering::SeqCst);
    say_goodbye(&harness, GoodbyeReason::Replace);
    harness.send(&request(&json!(7), "tools/call"));
    harness.settle();
    for _ in 0..3 {
        let before = harness.fake.attempts.load(Ordering::SeqCst);
        harness.advance_until(BACKOFF_FIRST * 4, || {
            harness.fake.attempts.load(Ordering::SeqCst) > before
        });
    }
    assert_eq!(
        harness.fake.starts.load(Ordering::SeqCst),
        0,
        "the installer starts the replacement, not the edge"
    );
    harness.fake.bring_up();
    harness.advance_until(BACKOFF_FIRST * 10, || harness.fake.received(1).len() == 1);
    assert_eq!(harness.fake.received(1), [request(&json!(7), "tools/call")]);
    assert_eq!(harness.fake.starts.load(Ordering::SeqCst), 0);

    let late = Harness::connected(true);
    late.fake.up.store(false, Ordering::SeqCst);
    say_goodbye(&late, GoodbyeReason::Replace);
    late.settle();
    late.advance(HOLD_LIMIT + Duration::from_secs(1));
    late.settle();
    assert_eq!(
        late.fake.starts.load(Ordering::SeqCst),
        0,
        "idle once the replacement is no longer expected"
    );
    late.send(&request(&json!(3), "tools/call"));
    late.advance_until(BACKOFF_FIRST, || {
        late.fake.starts.load(Ordering::SeqCst) == 1
    });
    assert_eq!(*late.fake.reasons.lock().unwrap(), [StartReason::Agent]);
}

/// ACCEPTANCE C13 and ADR-0199 Decision 13: the MCP edge ends for good when the service says
/// goodbye with `uninstall` or `session_end`, answering what is outstanding as cut off first;
/// when its own session listener hears the session end, leaving at once; and when its client
/// closes stdin, answering what it holds as expired. After a goodbye or the session's end it
/// starts nothing and tries no service again.
#[test]
fn the_edge_ends_for_good_on_a_final_goodbye_the_sessions_end_or_a_closed_stdin() {
    for reason in [GoodbyeReason::Uninstall, GoodbyeReason::SessionEnd] {
        let mut harness = Harness::connected(true);
        harness.send(&request(&json!(1), "tools/call"));
        harness.wait(|| harness.fake.received(0).len() == 1);
        say_goodbye(&harness, reason);
        harness.finish().unwrap();
        assert_eq!(
            harness.answer(&json!(1)),
            golden(Failure::CutOff, "tools/call", &json!(1)),
            "{reason:?}"
        );
        assert_eq!(harness.fake.starts.load(Ordering::SeqCst), 0, "{reason:?}");
        assert_eq!(
            harness.fake.connections(),
            1,
            "{reason:?}: no new connection is tried"
        );
    }

    let mut ending = Harness::connected(true);
    ending.send(&request(&json!(1), "tools/call"));
    ending.wait(|| ending.fake.received(0).len() == 1);
    let attempts = ending.fake.attempts.load(Ordering::SeqCst);
    ending.events.send(Event::SessionEnded).unwrap();
    ending.finish().unwrap();
    assert_eq!(ending.fake.starts.load(Ordering::SeqCst), 0);
    assert_eq!(ending.fake.attempts.load(Ordering::SeqCst), attempts);

    let mut closed = Harness::start(true, false);
    closed.wait(|| closed.fake.connections() == 1);
    closed.go_away(0);
    closed.send(&request(&json!(9), "tools/call"));
    closed.settle();
    closed.close().unwrap();
    assert_eq!(
        closed.answer(&json!(9)),
        golden(Failure::Expired, "tools/call", &json!(9))
    );
}

/// Defects eaf7948d and 514c8429: a line whose write failed never reached the service, which
/// acts only on a whole frame, so it goes back to the hold in its place and reaches the next
/// service in order, never answered. A service that dies while the hold flushes leaves the next
/// attempt due at once, so the held requests start a service instead of waiting to expire.
#[test]
fn a_line_that_never_reached_a_service_is_delivered_in_order_to_the_next_one() {
    let harness = Harness::connected(true);
    harness.go_away(0);
    harness.fake.drop_next.store(true, Ordering::SeqCst);
    let large = json!({
        "jsonrpc": "2.0",
        "id": "large",
        "method": "tools/call",
        "params": {"padding": "x".repeat(2 * 1024 * 1024)},
    })
    .to_string();
    harness.send(&large);
    let small: Vec<String> = (0..4).map(|n| request(&json!(n), "tools/call")).collect();
    for line in &small {
        harness.send(line);
    }
    harness.settle();
    harness.wait(|| harness.fake.starts.load(Ordering::SeqCst) == 1);
    let expected: Vec<String> = std::iter::once(large).chain(small).collect();
    harness.advance_until(BACKOFF_FIRST, || {
        harness.fake.received(2).len() == expected.len()
    });
    assert_eq!(
        harness.fake.received(2),
        expected,
        "nothing lost, nothing reordered"
    );
    assert!(
        !harness
            .values()
            .iter()
            .any(|value| value["id"] == json!("large") || value["id"].is_number()),
        "a line that never reached a service is delivered, not answered"
    );
}

/// ACCEPTANCE C13 and relay-protocol-4 section 7: the MCP edge starts the service, as an
/// `agent` start, at its own start and for each held request, and never while idle: not after a
/// lost service with nothing held, and not after a quit goodbye, which answers what is
/// outstanding and holds nothing, until the next request.
#[test]
fn the_service_is_started_as_an_agent_only_at_start_and_for_a_held_request() {
    let harness = Harness::start(false, true);
    harness.wait(|| harness.fake.starts.load(Ordering::SeqCst) == 1);
    harness.advance_until(BACKOFF_FIRST, || harness.fake.connections() == 1);

    harness.go_away(0);
    for _ in 0..5 {
        harness.advance(Duration::from_secs(60));
        harness.settle();
    }
    harness.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string());
    harness.settle();
    harness.advance(Duration::from_secs(60));
    harness.settle();
    assert_eq!(
        harness.fake.starts.load(Ordering::SeqCst),
        1,
        "idle, and a held notification starts nothing"
    );
    assert_eq!(harness.fake.connections(), 1);
    harness.send(&request(&json!(3), "tools/call"));
    harness.wait(|| harness.fake.starts.load(Ordering::SeqCst) == 2);
    harness.advance_until(BACKOFF_FIRST, || harness.fake.received(1).len() == 1);
    assert_eq!(harness.fake.received(1), [request(&json!(3), "tools/call")]);

    harness.send(&request(&json!(4), "tools/call"));
    harness.wait(|| harness.fake.received(1).len() == 2);
    harness.fake.up.store(false, Ordering::SeqCst);
    say_goodbye(&harness, GoodbyeReason::Quit);
    assert_eq!(
        harness.answer(&json!(4)),
        golden(Failure::CutOff, "tools/call", &json!(4))
    );
    for _ in 0..4 {
        harness.advance(Duration::from_secs(60));
        harness.settle();
    }
    assert_eq!(harness.fake.starts.load(Ordering::SeqCst), 2, "Quit holds");
    assert_eq!(harness.fake.connections(), 2);
    harness.send(&request(&json!(5), "tools/call"));
    harness.wait(|| harness.fake.starts.load(Ordering::SeqCst) == 3);
    harness.advance_until(BACKOFF_FIRST, || harness.fake.received(2).len() == 1);
    assert_eq!(harness.fake.received(2), [request(&json!(5), "tools/call")]);
    assert_eq!(
        *harness.fake.reasons.lock().unwrap(),
        [StartReason::Agent; 3],
        "an agent start every time"
    );
}

/// Defect 514c8429: a client's answer reaches the service only when it answers a request the
/// live connection made, the service's own ping included. A lone answer to nothing outstanding
/// is dropped and a batch loses such elements, so a late answer to a service that went away
/// never reaches the next one, which may have reused the id.
#[test]
fn only_answers_to_the_live_connections_own_requests_reach_the_service() {
    let harness = Harness::connected(true);
    let ping = json!({"jsonrpc": "2.0", "id": "p", "method": "ping"}).to_string();
    harness.fake.send(&McpFrame::Message { line: ping.clone() });
    harness.wait(|| harness.lines() == [ping.clone()]);
    let answer = json!({"jsonrpc": "2.0", "id": "p", "result": {}}).to_string();
    harness.send(&answer);
    let stray = json!({"jsonrpc": "2.0", "id": "q", "result": {}}).to_string();
    harness.send(&stray);
    let mixed = json!([
        {"jsonrpc": "2.0", "id": "r", "result": {}},
        {"jsonrpc": "2.0", "id": 3, "method": "tools/call"},
    ]);
    harness.send(&mixed.to_string());
    harness.send(&answer);
    harness.wait(|| harness.fake.received(0).len() == 2);
    harness.settle();
    let kept = json!([{"jsonrpc": "2.0", "id": 3, "method": "tools/call"}]).to_string();
    assert_eq!(
        harness.fake.received(0),
        [answer, kept],
        "the ping is answered once; the stray and the batch's stray element are dropped"
    );

    let asked = request(&json!(1), "sampling/createMessage");
    harness.fake.send(&McpFrame::Message {
        line: asked.clone(),
    });
    harness.wait(|| harness.lines().last() == Some(&asked));
    harness.go_away(0);
    harness.send(&json!({"jsonrpc": "2.0", "id": 1, "result": {}}).to_string());
    harness.send(&json!([{"jsonrpc": "2.0", "id": 1, "result": {}}]).to_string());
    harness.send(&request(&json!(2), "tools/call"));
    harness.settle();
    harness.wait(|| harness.fake.starts.load(Ordering::SeqCst) == 1);
    harness.advance_until(BACKOFF_FIRST, || harness.fake.received(1).len() == 1);
    harness.settle();
    assert_eq!(harness.fake.received(1), [request(&json!(2), "tools/call")]);
}

/// Every line `reader` gives under a bound of `bound` bytes: its bytes, or for a line that was
/// skipped, the requests it held.
fn lines_within(bound: usize, mut reader: impl BufRead) -> Vec<Result<Vec<u8>, Vec<Request>>> {
    let mut lines = Vec::new();
    while let Some(line) = read_line(&mut reader, bound).unwrap() {
        lines.push(match line {
            InputLine::Line(bytes) => Ok(bytes),
            InputLine::TooLong(requests) => Err(requests),
        });
    }
    lines
}

/// A lone `tools/call` whose id `big` comes after its params, as the TypeScript SDK writes it,
/// with an id, a method, an escaped quote and a brace inside the params that are not the
/// request's, and `data` in its arguments.
fn call_with_id_last(data: &str) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","method":"tools/call","params":{{"name":"browser_upload","arguments":{{"id":7,"method":"ping","note":"a \"}}\" brace","data":"{data}"}}}},"id":"big"}}"#
    )
}

/// Defect S-D9 and ACCEPTANCE C6: a line over the frame bound is skipped up to its newline
/// without being kept, and each request it holds is answered from the `too_large` template of the
/// last accepted reply, by its method and with its id as sent, whatever the order of its members:
/// a lone call whose id follows its params, and each request of a batch but not its
/// notification. A shorter line whose frame would pass the bound is answered the same way.
/// Nothing of either reaches the service, the next line is still read, and before any reply the
/// answer is the bare internal error. Read one byte at a time or all at once, the input gives the
/// same lines, and a last line with no newline is still a line.
#[test]
fn a_line_over_the_bound_is_answered_as_too_large_for_each_request_it_holds() {
    let input = format!("{}\nxy\nlast", call_with_id_last("x")).into_bytes();
    let big = Request {
        key: json!("big").to_string(),
        id: json!("big"),
        method: "tools/call".to_owned(),
    };
    let expected = [Err(vec![big]), Ok(b"xy".to_vec()), Ok(b"last".to_vec())];
    assert_eq!(
        lines_within(16, io::Cursor::new(input.clone())),
        expected,
        "coalesced"
    );
    let one_byte_at_a_time = BufReader::with_capacity(1, io::Cursor::new(input));
    assert_eq!(lines_within(16, one_byte_at_a_time), expected, "split");

    let harness = Harness::connected(true);
    let padding = "x".repeat(MAX_FRAME_BYTES);
    let lone = call_with_id_last(&padding);
    harness.send(&lone);
    assert_eq!(
        harness.answer(&json!("big")),
        golden(Failure::TooLarge, "tools/call", &json!("big"))
    );
    let batch = format!(
        r#"[{{"jsonrpc":"2.0","method":"notifications/progress","params":{{"data":"{padding}"}}}},{{"jsonrpc":"2.0","id":3,"method":"resources/list"}},{{"params":{{}},"id":4,"method":"tools/call","jsonrpc":"2.0"}}]"#
    );
    harness.send(&batch);
    assert_eq!(
        harness.answer(&json!(3)),
        golden(Failure::TooLarge, DEFAULT_TEMPLATE, &json!(3))
    );
    assert_eq!(
        harness.answer(&json!(4)),
        golden(Failure::TooLarge, "tools/call", &json!(4))
    );
    // Within the line bound, but each escaped quote doubles in the frame.
    let quotes = r#"\""#.repeat(MAX_FRAME_BYTES * 3 / 8);
    let framed = format!(
        r#"{{"jsonrpc":"2.0","id":"framed","method":"tools/call","params":{{"q":"{quotes}"}}}}"#
    );
    assert!(framed.len() < MAX_FRAME_BYTES);
    harness.send(&framed);
    assert_eq!(
        harness.answer(&json!("framed")),
        golden(Failure::TooLarge, "tools/call", &json!("framed"))
    );
    let next = request(&json!(9), "tools/call");
    harness.send(&next);
    harness.wait(|| !harness.fake.received(0).is_empty());
    harness.settle();
    assert_eq!(
        harness.fake.received(0),
        [next],
        "nothing over the bound reaches the service, and the next line does"
    );
    assert_eq!(
        harness
            .values()
            .iter()
            .filter(|value| !value["id"]
                .as_str()
                .is_some_and(|id| id.starts_with("settle-")))
            .count(),
        4,
        "one answer for each request, none for the notification"
    );

    let unreplied = Harness::start(false, false);
    unreplied.send(&lone);
    assert_eq!(
        unreplied.answer(&json!("big")),
        json!({"jsonrpc": "2.0", "id": "big", "error": {"code": -32603, "message": ""}}),
        "before any reply the edge has no words of its own"
    );
}

/// Defect S-D3 (b0290246): an edge left idle by a quit has asked once to be woken. A wake that
/// finds no service makes it look once and starts nothing, and the idle edge tries no more; a
/// wake from a service that became ready makes it connect at once, still without a start.
#[test]
fn a_wake_makes_an_idle_edge_look_once_and_never_start_the_service() {
    let harness = Harness::connected(true);
    harness.fake.up.store(false, Ordering::SeqCst);
    say_goodbye(&harness, GoodbyeReason::Quit);
    harness.advance(Duration::from_secs(60));
    harness.settle();
    assert_eq!(
        harness.fake.connections(),
        1,
        "an idle edge waits after a quit"
    );
    assert_eq!(harness.fake.watches.load(Ordering::SeqCst), 1);

    let attempts = harness.fake.attempts.load(Ordering::SeqCst);
    harness.fake.wake();
    harness.wait(|| harness.fake.attempts.load(Ordering::SeqCst) == attempts + 1);
    harness.advance(Duration::from_secs(60));
    harness.settle();
    assert_eq!(
        harness.fake.attempts.load(Ordering::SeqCst),
        attempts + 1,
        "an idle edge does not try again"
    );

    harness.fake.bring_up();
    harness.fake.wake();
    harness.wait(|| harness.fake.connections() == 2);
    assert_eq!(harness.fake.starts.load(Ordering::SeqCst), 0, "Quit holds");
    assert_eq!(harness.fake.watches.load(Ordering::SeqCst), 1);
}
