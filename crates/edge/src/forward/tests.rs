//! In-process tests of the forwarding loop against a fake relay-4 service over connected pairs.

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use ghostlight_relay::endpoint::pair;
use ghostlight_relay::frame;
use ghostlight_relay::protocol::{McpFrame, Opening, Program, Reply, Role, Templates};
use ghostlight_relay::RELAY;
use ghostlight_relay::{BACKOFF_FIRST, HOLD_LIMIT};
use serde_json::{json, Value};

use super::*;

const TICK: Duration = Duration::from_millis(2);
const PATIENCE: Duration = Duration::from_secs(5);
/// How long `Harness::advance_until` lets the edge react to one step before taking the next.
const STEP_GRACE: Duration = Duration::from_millis(20);

/// The failure templates the golden accepted reply carries.
fn golden_templates() -> Templates {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/relay4/reply.accepted.json");
    let reply: Reply = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    reply.templates.expect("the golden reply carries templates")
}

/// One connection the fake service accepted.
struct Accepted {
    opening: Opening,
    writer: Connection,
    received: Arc<Mutex<Vec<String>>>,
    /// Set once the edge closed its end.
    closed: Arc<AtomicBool>,
}

/// A relay-4 service the test switches on and off.
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

#[test]
fn lines_are_forwarded_unchanged_in_both_directions() {
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
}

#[test]
fn ids_keep_their_json_type_in_every_answer() {
    let harness = Harness::connected(true);
    harness.send(&request(&json!("7"), "ping"));
    harness.send(&request(&json!(7), "ping"));
    assert_eq!(harness.answer(&json!("7"))["id"], json!("7"));
    assert_eq!(harness.answer(&json!(7))["id"], json!(7));

    harness.send(&request(&json!("a"), "tools/call"));
    harness.send(&request(&json!(2), "tools/call"));
    harness.wait(|| harness.fake.received(0).len() == 2);
    harness.fake.go_down();
    assert_eq!(harness.answer(&json!("a"))["id"], json!("a"));
    assert_eq!(harness.answer(&json!(2))["id"], json!(2));
}

#[test]
fn batch_arrays_are_tracked_element_by_element() {
    let harness = Harness::connected(true);
    let batch = json!([
        {"jsonrpc": "2.0", "id": 1, "method": "tools/call"},
        {"jsonrpc": "2.0", "id": "b", "method": "tools/list"},
        {"jsonrpc": "2.0", "method": "notifications/progress"},
    ])
    .to_string();
    harness.send(&batch);
    harness.wait(|| harness.fake.received(0).len() == 1);
    assert_eq!(harness.fake.received(0), [batch]);

    let answered = json!([{"jsonrpc": "2.0", "id": 1, "result": {}}]).to_string();
    harness.fake.send(&McpFrame::Message {
        line: answered.clone(),
    });
    harness.wait(|| harness.lines() == [answered.clone()]);
    harness.fake.go_down();
    let cut_off = harness.answer(&json!("b"));
    assert_eq!(cut_off["error"]["code"], json!(-32603));
    harness.settle();
    let answers_for_one = harness
        .values()
        .iter()
        .filter(|value| value["id"] == json!(1))
        .count();
    assert_eq!(answers_for_one, 0, "an answered element is not cut off");
}

#[test]
fn ping_is_answered_locally_while_connected_and_while_away() {
    let harness = Harness::connected(true);
    harness.send(&request(&json!(1), "ping"));
    assert_eq!(
        harness.answer(&json!(1)),
        json!({"jsonrpc": "2.0", "id": 1, "result": {}})
    );
    harness.go_away(0);
    harness.send(&request(&json!(2), "ping"));
    assert_eq!(
        harness.answer(&json!(2)),
        json!({"jsonrpc": "2.0", "id": 2, "result": {}})
    );
    harness.settle();
    assert_eq!(
        harness.fake.received(0).len(),
        1,
        "pings never reach the service"
    );
    assert_eq!(harness.fake.starts.load(Ordering::SeqCst), 0);
}

#[test]
fn held_lines_flush_in_order_after_an_opening_that_resumes() {
    let harness = Harness::connected(true);
    harness.fake.send(&McpFrame::Resume {
        token: "resume-1".to_owned(),
    });
    harness.go_away(0);

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
    assert_eq!(harness.fake.received(1), held);

    let openings = harness.fake.openings();
    for opening in &openings {
        assert_eq!(opening.relay, RELAY);
        assert_eq!(opening.role, Role::Mcp);
        assert_eq!(opening.channel.as_deref(), Some(MCP_CHANNEL));
        assert_eq!(opening.edge.program, Program::Edge);
        assert_eq!(opening.edge.revision, RELAY);
        assert_eq!(opening.edge.build, env!("CARGO_PKG_VERSION"));
    }
    assert_eq!(openings[0].resume, None);
    assert_eq!(openings[1].resume, Some("resume-1".to_owned()));
}

#[test]
fn an_expired_request_is_answered_with_its_methods_template_or_the_default() {
    let harness = Harness::start(true, false);
    harness.wait(|| harness.fake.connections() == 1);
    harness.go_away(0);
    harness.send(&request(&json!(1), "tools/call"));
    harness.settle();
    harness.advance(Duration::from_secs(10));
    harness.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string());
    harness.send(&request(&json!("x"), "resources/list"));
    harness.settle();
    harness.advance(Duration::from_secs(20));

    let expired_call = harness.answer(&json!(1));
    assert_eq!(expired_call["result"]["isError"], json!(true));
    assert_eq!(
        expired_call["result"]["content"][0]["text"],
        json!("Ghostlight did not come back in time. Try the call again.")
    );
    harness.settle();
    assert!(!harness
        .values()
        .iter()
        .any(|value| value["id"] == json!("x")));

    harness.advance(Duration::from_secs(10));
    let expired_default = harness.answer(&json!("x"));
    assert_eq!(
        expired_default["error"],
        json!({"code": -32603, "message": "Ghostlight did not come back in time."})
    );
    harness.settle();
    let answers = harness
        .values()
        .into_iter()
        .filter(|value| {
            !value["id"]
                .as_str()
                .is_some_and(|id| id.starts_with("settle-"))
        })
        .count();
    assert_eq!(answers, 3, "held notifications expire silently");
    assert_eq!(harness.fake.connections(), 1);
}

#[test]
fn a_cut_off_request_is_answered_with_the_cut_off_template() {
    let harness = Harness::connected(true);
    harness.send(&request(&json!(1), "tools/call"));
    harness.send(&request(&json!(2), "completion/complete"));
    harness.wait(|| harness.fake.received(0).len() == 2);
    harness.fake.go_down();
    assert_eq!(
        harness.answer(&json!(1))["result"]["content"][0]["text"],
        json!("Ghostlight restarted before this call finished. It may have happened; check before repeating.")
    );
    assert_eq!(
        harness.answer(&json!(2))["error"],
        json!({"code": -32603, "message": "Ghostlight restarted before this request finished."})
    );
}

#[test]
fn nothing_written_to_a_service_is_sent_again() {
    let harness = Harness::connected(true);
    harness.send(&request(&json!(1), "tools/call"));
    harness.go_away(0);
    harness.send(&request(&json!(2), "tools/call"));
    harness.settle();
    harness.wait(|| harness.fake.starts.load(Ordering::SeqCst) == 1);
    harness.advance_until(BACKOFF_FIRST, || harness.fake.received(1).len() == 1);
    harness.settle();
    assert_eq!(harness.fake.received(1), [request(&json!(2), "tools/call")]);
    assert_eq!(harness.fake.received(0).len(), 2);
    assert!(harness.answer(&json!(1))["result"]["isError"] == json!(true));
}

#[test]
fn a_listing_cut_off_by_a_lost_service_is_sent_again_to_the_next_one() {
    let harness = Harness::connected(true);
    harness.send(&request(&json!(1), "tools/call"));
    harness.send(&request(&json!(2), "tools/list"));
    harness.wait(|| harness.fake.received(0).len() == 2);
    harness.fake.go_down();
    assert_eq!(harness.answer(&json!(1))["result"]["isError"], json!(true));
    harness.wait(|| harness.fake.starts.load(Ordering::SeqCst) == 1);
    harness.advance_until(BACKOFF_FIRST, || harness.fake.received(1).len() == 1);
    assert_eq!(harness.fake.received(1), [request(&json!(2), "tools/list")]);
    harness.settle();
    assert!(
        !harness.values().iter().any(|value| value["id"] == json!(2)),
        "the next service answers the listing, not the edge"
    );
}

#[test]
fn a_listing_is_sent_again_once_and_then_answered_as_cut_off() {
    let harness = Harness::connected(true);
    harness.send(&request(&json!(1), "initialize"));
    harness.wait(|| harness.fake.received(0).len() == 1);
    harness.fake.go_down();
    harness.wait(|| harness.fake.starts.load(Ordering::SeqCst) == 1);
    harness.advance_until(BACKOFF_FIRST, || harness.fake.received(1).len() == 1);
    harness.fake.go_down();
    assert_eq!(
        harness.answer(&json!(1))["error"]["code"],
        json!(-32603),
        "a request that brings each service down is not sent a third time"
    );
    harness.settle();
    assert_eq!(
        harness.fake.starts.load(Ordering::SeqCst),
        1,
        "nothing is held"
    );
}

#[test]
fn after_a_quit_goodbye_a_listing_is_answered_and_nothing_starts() {
    let harness = Harness::connected(true);
    harness.send(&request(&json!(1), "tools/list"));
    harness.wait(|| harness.fake.received(0).len() == 1);
    say_goodbye(&harness, GoodbyeReason::Quit);
    assert_eq!(harness.answer(&json!(1))["error"]["code"], json!(-32603));
    harness.advance(Duration::from_secs(60));
    harness.settle();
    assert_eq!(harness.fake.starts.load(Ordering::SeqCst), 0, "Quit holds");
}

#[test]
fn a_line_whose_write_failed_is_held_and_delivered_in_order_to_the_next_service() {
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

#[test]
fn the_service_is_started_at_start_and_for_a_held_request_never_while_idle() {
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
    assert_eq!(harness.fake.starts.load(Ordering::SeqCst), 1, "idle");
    assert_eq!(harness.fake.connections(), 1);

    harness.send(&request(&json!(3), "tools/call"));
    harness.wait(|| harness.fake.starts.load(Ordering::SeqCst) == 2);
    harness.advance_until(BACKOFF_FIRST, || harness.fake.received(1).len() == 1);
    assert_eq!(harness.fake.received(1), [request(&json!(3), "tools/call")]);
    assert_eq!(
        *harness.fake.reasons.lock().unwrap(),
        [StartReason::Agent, StartReason::Agent],
        "an agent start, at the edge's own start and for the held request"
    );
}

/// Send the service's goodbye on the newest connection and wait until the edge has acted
/// on it by closing its end.
fn say_goodbye(harness: &Harness, reason: GoodbyeReason) {
    let connection = harness.fake.connections() - 1;
    harness.fake.send(&McpFrame::Goodbye { reason });
    harness.wait(|| harness.fake.closed(connection));
}

#[test]
fn after_a_quit_goodbye_requests_are_answered_nothing_is_held_and_the_next_request_starts_it() {
    let harness = Harness::connected(true);
    harness.send(&request(&json!(1), "tools/call"));
    harness.wait(|| harness.fake.received(0).len() == 1);
    harness.fake.up.store(false, Ordering::SeqCst);
    say_goodbye(&harness, GoodbyeReason::Quit);
    assert_eq!(
        harness.answer(&json!(1))["result"]["content"][0]["text"],
        json!("Ghostlight restarted before this call finished. It may have happened; check before repeating."),
        "the outstanding request is answered from the cut_off template"
    );

    for _ in 0..4 {
        harness.advance(Duration::from_secs(60));
        harness.settle();
    }
    assert_eq!(harness.fake.starts.load(Ordering::SeqCst), 0, "idle");
    assert_eq!(harness.fake.connections(), 1);

    harness.fake.start_brings_up.store(true, Ordering::SeqCst);
    harness.send(&request(&json!(2), "tools/call"));
    harness.wait(|| harness.fake.starts.load(Ordering::SeqCst) == 1);
    assert_eq!(*harness.fake.reasons.lock().unwrap(), [StartReason::Agent]);
    harness.advance_until(BACKOFF_FIRST, || harness.fake.received(1).len() == 1);
    assert_eq!(harness.fake.received(1), [request(&json!(2), "tools/call")]);
}

#[test]
fn after_a_replace_goodbye_lines_are_held_and_the_edge_reconnects_without_starting() {
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
}

#[test]
fn a_replacement_that_does_not_come_back_within_the_hold_limit_is_a_lost_service() {
    let harness = Harness::connected(true);
    harness.fake.up.store(false, Ordering::SeqCst);
    say_goodbye(&harness, GoodbyeReason::Replace);
    harness.settle();
    harness.advance(HOLD_LIMIT + Duration::from_secs(1));
    harness.settle();
    assert_eq!(harness.fake.starts.load(Ordering::SeqCst), 0, "idle");

    harness.send(&request(&json!(3), "tools/call"));
    harness.advance_until(BACKOFF_FIRST, || {
        harness.fake.starts.load(Ordering::SeqCst) == 1
    });
    assert_eq!(*harness.fake.reasons.lock().unwrap(), [StartReason::Agent]);
}

#[test]
fn after_an_uninstall_goodbye_requests_are_answered_and_the_edge_exits() {
    let mut harness = Harness::connected(true);
    harness.send(&request(&json!(1), "tools/call"));
    harness.wait(|| harness.fake.received(0).len() == 1);
    say_goodbye(&harness, GoodbyeReason::Uninstall);
    harness.finish().unwrap();
    assert_eq!(
        harness.answer(&json!(1))["result"]["isError"],
        json!(true),
        "the outstanding request is answered before the edge ends"
    );
    assert_eq!(harness.fake.starts.load(Ordering::SeqCst), 0);
}

#[test]
fn after_a_session_end_goodbye_requests_are_answered_and_the_edge_exits_without_a_start() {
    let mut harness = Harness::connected(true);
    harness.send(&request(&json!(1), "tools/call"));
    harness.wait(|| harness.fake.received(0).len() == 1);
    say_goodbye(&harness, GoodbyeReason::SessionEnd);
    harness.finish().unwrap();
    assert_eq!(
        harness.answer(&json!(1))["result"]["isError"],
        json!(true),
        "the outstanding request is answered before the edge ends"
    );
    assert_eq!(harness.fake.starts.load(Ordering::SeqCst), 0);
    assert_eq!(harness.fake.connections(), 1, "no new connection is tried");
}

#[test]
fn a_session_that_ends_makes_the_edge_exit_without_trying_the_service_again() {
    let mut harness = Harness::connected(true);
    harness.send(&request(&json!(1), "tools/call"));
    harness.wait(|| harness.fake.received(0).len() == 1);
    let attempts = harness.fake.attempts.load(Ordering::SeqCst);

    harness.events.send(Event::SessionEnded).unwrap();
    harness.finish().unwrap();

    assert_eq!(harness.fake.starts.load(Ordering::SeqCst), 0);
    assert_eq!(harness.fake.attempts.load(Ordering::SeqCst), attempts);
}

#[test]
fn a_held_request_left_by_a_service_that_died_mid_flush_still_starts_one() {
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
    let small: Vec<Value> = (0..8).map(|n| json!(n)).collect();
    for id in &small {
        harness.send(&request(id, "tools/call"));
    }
    harness.settle();
    harness.wait(|| harness.fake.starts.load(Ordering::SeqCst) == 1);

    let ids: Vec<Value> = std::iter::once(json!("large")).chain(small).collect();
    let settled = |id: &Value| {
        let delivered = harness
            .fake
            .received(2)
            .iter()
            .any(|line| serde_json::from_str::<Value>(line).unwrap()["id"] == *id);
        let answered = harness.values().iter().any(|value| value["id"] == *id);
        delivered || answered
    };
    harness.advance_until(BACKOFF_FIRST, || ids.iter().all(settled));
    for answer in harness
        .values()
        .iter()
        .filter(|value| ids.contains(&value["id"]))
    {
        assert_eq!(
            answer["result"]["content"][0]["text"],
            json!("Ghostlight restarted before this call finished. It may have happened; check before repeating."),
            "a held request is cut off or delivered, never left to expire"
        );
    }
}

#[test]
fn a_late_answer_to_a_service_that_went_away_never_reaches_the_next_one() {
    let harness = Harness::connected(true);
    let asked = request(&json!(1), "sampling/createMessage");
    harness.fake.send(&McpFrame::Message {
        line: asked.clone(),
    });
    harness.wait(|| harness.lines() == [asked.clone()]);
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

#[test]
fn only_answers_to_outstanding_service_requests_reach_the_service() {
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
    assert_eq!(harness.fake.received(0), [answer, kept]);
}

#[test]
fn a_reply_without_templates_keeps_the_ones_already_held() {
    let harness = Harness::connected(true);
    *harness.fake.templates.lock().unwrap() = None;
    harness.go_away(0);
    harness.send(&request(&json!(5), "tools/call"));
    harness.settle();
    harness.wait(|| harness.fake.starts.load(Ordering::SeqCst) == 1);
    harness.advance_until(BACKOFF_FIRST, || harness.fake.received(1).len() == 1);
    harness.fake.go_down();
    assert_eq!(
        harness.answer(&json!(5))["result"]["content"][0]["text"],
        json!("Ghostlight restarted before this call finished. It may have happened; check before repeating.")
    );
}

#[test]
fn before_any_reply_a_failed_request_is_answered_the_bare_internal_error() {
    let harness = Harness::start(false, false);
    harness.send(&request(&json!(5), "tools/call"));
    harness.settle();
    harness.advance(Duration::from_secs(60));
    assert_eq!(
        harness.answer(&json!(5)),
        json!({"jsonrpc": "2.0", "id": 5, "error": {"code": -32603, "message": ""}})
    );
}

#[test]
fn closing_stdin_answers_held_requests_and_ends() {
    let mut harness = Harness::start(false, false);
    harness.send(&request(&json!(9), "tools/call"));
    harness.settle();
    harness.close().unwrap();
    assert_eq!(harness.answer(&json!(9))["error"]["code"], json!(-32603));
}

#[test]
fn a_line_over_the_frame_bound_is_skipped_up_to_its_newline() {
    let mut input = io::Cursor::new(b"abcdef\nxy\nlast".to_vec());
    assert!(matches!(
        read_line(&mut input, 4).unwrap(),
        Some(InputLine::TooLong)
    ));
    assert!(matches!(
        read_line(&mut input, 4).unwrap(),
        Some(InputLine::Line(line)) if line == b"xy"
    ));
    assert!(matches!(
        read_line(&mut input, 4).unwrap(),
        Some(InputLine::Line(line)) if line == b"last"
    ));
    assert!(read_line(&mut input, 4).unwrap().is_none());
}

#[test]
fn the_edge_sends_the_opening_bytes_it_sent_before() {
    let build = env!("CARGO_PKG_VERSION");
    let fresh = serde_json::to_string(&opening(None)).unwrap();
    assert_eq!(
        fresh,
        format!(
            r#"{{"relay":4,"role":"mcp","edge":{{"program":"edge","revision":4,"build":"{build}"}},"channel":"mcp"}}"#
        )
    );
    let resumed = serde_json::to_string(&opening(Some("rs1.token".to_owned()))).unwrap();
    assert_eq!(
        resumed,
        format!(
            r#"{{"relay":4,"role":"mcp","edge":{{"program":"edge","revision":4,"build":"{build}"}},"resume":"rs1.token","channel":"mcp"}}"#
        )
    );
}
