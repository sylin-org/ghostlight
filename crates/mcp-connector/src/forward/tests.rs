//! In-process tests of the forwarding loop against a fake relay-3 service on loopback.

use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use ghostlight_relay::frame::{self, FrameError};
use ghostlight_relay::opening::{McpFrame, Opening, Reply, Stream};
use ghostlight_relay::BACKOFF_FIRST;
use serde_json::{json, Value};

use super::*;

const TOKEN: &str = "fake-token";
const TICK: Duration = Duration::from_millis(2);
const PATIENCE: Duration = Duration::from_secs(5);

/// Read the opening a connector sends to the fake service, within `patience`.
fn read_opening(connection: &mut TcpStream, patience: Duration) -> Opening {
    connection.set_read_timeout(Some(patience)).unwrap();
    let opening = frame::read_frame(connection).unwrap().unwrap();
    connection.set_read_timeout(None).unwrap();
    opening
}

/// One connection the fake service accepted.
struct Accepted {
    opening: Opening,
    writer: TcpStream,
    received: Arc<Mutex<Vec<String>>>,
}

/// A relay-3 service on loopback that the test switches on and off.
#[derive(Default)]
struct FakeState {
    port: u16,
    up: AtomicBool,
    start_brings_up: AtomicBool,
    /// When set, the next connection is accepted and then closed at once.
    drop_next: AtomicBool,
    starts: AtomicU32,
    accepted: Mutex<Vec<Accepted>>,
}

impl FakeState {
    fn listen(up: bool, start_brings_up: bool) -> Arc<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let state = Arc::new(Self {
            port: listener.local_addr().unwrap().port(),
            up: AtomicBool::new(up),
            start_brings_up: AtomicBool::new(start_brings_up),
            ..Self::default()
        });
        let weak = Arc::downgrade(&state);
        thread::spawn(move || {
            for connection in listener.incoming() {
                let Some(state) = weak.upgrade() else {
                    return;
                };
                let Ok(mut connection) = connection else {
                    return;
                };
                let opening = read_opening(&mut connection, PATIENCE);
                frame::write_frame(
                    &mut connection,
                    &Reply::Accepted {
                        service_version: "2.0.0".to_owned(),
                    },
                )
                .unwrap();
                let received = Arc::new(Mutex::new(Vec::new()));
                if state.drop_next.swap(false, Ordering::SeqCst) {
                    let _ = connection.shutdown(Shutdown::Both);
                    state.accepted.lock().unwrap().push(Accepted {
                        opening,
                        writer: connection,
                        received,
                    });
                    continue;
                }
                let mut reader = connection.try_clone().unwrap();
                let sink = Arc::clone(&received);
                thread::spawn(move || {
                    while let Ok(Some(frame)) = frame::read_frame::<McpFrame>(&mut reader) {
                        if let McpFrame::Message { line } = frame {
                            sink.lock().unwrap().push(line);
                        }
                    }
                });
                state.accepted.lock().unwrap().push(Accepted {
                    opening,
                    writer: connection,
                    received,
                });
            }
        });
        state
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

    /// Send a frame on the newest connection.
    fn send(&self, frame: &McpFrame) {
        let mut accepted = self.accepted.lock().unwrap();
        let newest = accepted.last_mut().unwrap();
        frame::write_frame(&mut newest.writer, frame).unwrap();
    }

    /// Switch the service off and close every connection.
    fn go_down(&self) {
        self.up.store(false, Ordering::SeqCst);
        for one in self.accepted.lock().unwrap().iter() {
            let _ = one.writer.shutdown(Shutdown::Both);
        }
    }
}

/// The connector's view of the fake: connect while it is up; starting may bring it up.
struct FakeService(Arc<FakeState>);

impl Service for FakeService {
    fn connect(&self, stream: Stream) -> Result<(TcpStream, Reply), ConnectError> {
        if !self.0.up.load(Ordering::SeqCst) {
            return Err(ConnectError::NoService);
        }
        let mut socket = TcpStream::connect(("127.0.0.1", self.0.port))?;
        frame::write_frame(&mut socket, &Opening::new(TOKEN.to_owned(), stream))?;
        match frame::read_frame::<Reply>(&mut socket)? {
            Some(reply) => Ok((socket, reply)),
            None => Err(ConnectError::Frame(FrameError::Truncated)),
        }
    }

    fn start(&self) -> io::Result<()> {
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

/// A running connector wired to a fake service, fake stdio and a fake clock.
struct Harness {
    fake: Arc<FakeState>,
    clock: Arc<FakeClock>,
    input: Option<mpsc::Sender<Vec<u8>>>,
    output: SharedOutput,
    done: Option<JoinHandle<io::Result<()>>>,
}

fn no_log(_: LogEvent, _: &str) {}

fn golden_templates() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/relay3/templates.json")
}

impl Harness {
    fn start(up: bool, start_brings_up: bool, templates: PathBuf) -> Self {
        let fake = FakeState::listen(up, start_brings_up);
        let clock = Arc::new(FakeClock(Mutex::new(Instant::now())));
        let (input, chunks) = mpsc::channel();
        let output = SharedOutput::default();
        let environment = Environment {
            service: Arc::new(FakeService(Arc::clone(&fake))),
            clock: Arc::clone(&clock) as Arc<dyn Clock>,
            templates: Some(templates),
            log: no_log,
            poll: Some(TICK),
        };
        let stdout = output.clone();
        let done = thread::spawn(move || {
            let (events, inbox) = mpsc::channel();
            let stdin = ChannelInput {
                chunks,
                pending: Vec::new(),
            };
            run(environment, stdin, stdout, events, inbox)
        });
        Self {
            fake,
            clock,
            input: Some(input),
            output,
            done: Some(done),
        }
    }

    fn connected(up: bool) -> Self {
        let harness = Self::start(up, true, golden_templates());
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

    /// Take the service away while a request is outstanding, and wait until the connector has
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

    /// Close stdin and wait for the connector to end.
    fn close(&mut self) -> io::Result<()> {
        self.input.take();
        self.done.take().unwrap().join().unwrap()
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
    harness.advance(BACKOFF_FIRST);
    harness.wait(|| harness.fake.received(1).len() == held.len());
    assert_eq!(harness.fake.received(1), held);

    let openings = harness.fake.openings();
    let resume_of = |opening: &Opening| match &opening.stream {
        Stream::Mcp {
            channel, resume, ..
        } => {
            assert_eq!(channel, CHANNEL);
            resume.clone()
        }
        other => panic!("not an mcp stream: {other:?}"),
    };
    assert_eq!(resume_of(&openings[0]), None);
    assert_eq!(resume_of(&openings[1]), Some("resume-1".to_owned()));
}

#[test]
fn an_expired_request_is_answered_with_its_methods_template_or_the_default() {
    let harness = Harness::start(false, false, golden_templates());
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
    assert_eq!(answers, 2, "held notifications expire silently");
    assert_eq!(harness.fake.connections(), 0);
}

#[test]
fn a_cut_off_request_is_answered_with_the_cut_off_template() {
    let harness = Harness::connected(true);
    harness.send(&request(&json!(1), "tools/call"));
    harness.send(&request(&json!(2), "resources/list"));
    harness.wait(|| harness.fake.received(0).len() == 2);
    harness.fake.go_down();
    assert_eq!(
        harness.answer(&json!(1))["result"]["content"][0]["text"],
        json!("Ghostlight restarted before this call finished. Try it again.")
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
    harness.advance(BACKOFF_FIRST);
    harness.wait(|| harness.fake.received(1).len() == 1);
    harness.settle();
    assert_eq!(harness.fake.received(1), [request(&json!(2), "tools/call")]);
    assert_eq!(harness.fake.received(0).len(), 2);
    assert!(harness.answer(&json!(1))["result"]["isError"] == json!(true));
}

#[test]
fn the_service_is_started_at_start_and_for_a_held_request_never_while_idle() {
    let harness = Harness::start(false, true, golden_templates());
    harness.wait(|| harness.fake.starts.load(Ordering::SeqCst) == 1);
    harness.advance(BACKOFF_FIRST);
    harness.wait(|| harness.fake.connections() == 1);

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
    harness.advance(BACKOFF_FIRST);
    harness.wait(|| harness.fake.received(1).len() == 1);
    assert_eq!(harness.fake.received(1), [request(&json!(3), "tools/call")]);
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
    harness.advance(BACKOFF_FIRST);

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
    harness.wait(|| ids.iter().all(settled));
    for answer in harness
        .values()
        .iter()
        .filter(|value| ids.contains(&value["id"]))
    {
        assert_eq!(
            answer["result"]["content"][0]["text"],
            json!("Ghostlight restarted before this call finished. Try it again."),
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
    harness.advance(BACKOFF_FIRST);
    harness.wait(|| harness.fake.received(1).len() == 1);
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
fn a_missing_template_file_answers_a_bare_internal_error() {
    let missing = std::env::temp_dir().join(format!(
        "ghostlight-mcp-connector-no-templates-{}.json",
        std::process::id()
    ));
    let harness = Harness::start(true, true, missing);
    harness.wait(|| harness.fake.connections() == 1);
    harness.send(&request(&json!(5), "tools/call"));
    harness.wait(|| harness.fake.received(0).len() == 1);
    harness.fake.go_down();
    assert_eq!(
        harness.answer(&json!(5)),
        json!({"jsonrpc": "2.0", "id": 5, "error": {"code": -32603, "message": ""}})
    );
}

#[test]
fn closing_stdin_answers_held_requests_and_ends() {
    let mut harness = Harness::start(false, false, golden_templates());
    harness.send(&request(&json!(9), "tools/call"));
    harness.settle();
    harness.close().unwrap();
    assert_eq!(harness.answer(&json!(9))["result"]["isError"], json!(true));
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
