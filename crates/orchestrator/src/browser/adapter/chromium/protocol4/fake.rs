//! A scripted protocol-4 extension over a relay connection, for tests.
//!
//! It sends its hello at once, acknowledges heartbeats, answers each request through a scripted
//! closure (at once, when the request is cancelled, or never), reassembles chunked requests, and
//! records every frame the service sends.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use base64::Engine as _;
use ghostlight_relay::endpoint::Connection;
use ghostlight_relay::frame::{read_frame, write_frame, write_frame_raw};
use serde_json::{json, Value};

use super::wire;
use crate::sync::lock;

/// How long a test waits for the fake to see what it expects.
const PATIENCE: Duration = Duration::from_secs(5);

/// When the fake answers one request.
pub(crate) enum Reply {
    /// Answer with this receipt at once.
    Now(wire::Receipt),
    /// Hold this receipt until the service cancels the request, then answer with it.
    OnCancel(wire::Receipt),
    /// Never answer.
    Never,
}

/// The scripted reply to one request, given its correlation and primitive.
pub(crate) type Script = Box<dyn Fn(&str, &wire::Primitive) -> Reply + Send>;

/// Everything the fake has seen from the service.
#[derive(Debug, Default)]
pub(crate) struct Seen {
    /// Every frame the service sent, in order, chunks as they arrived.
    pub(crate) frames: Vec<Value>,
    /// Every request, chunked ones reassembled, with its correlation.
    pub(crate) requests: Vec<(String, wire::Primitive)>,
    /// The correlation of every cancel frame.
    pub(crate) cancels: Vec<String>,
    /// Every notice frame.
    pub(crate) notices: Vec<wire::Notice>,
    /// Whether the service closed the socket.
    pub(crate) closed: bool,
}

impl Seen {
    /// The frames of one kind, in order.
    pub(crate) fn of_kind(&self, kind: &str) -> Vec<&Value> {
        self.frames
            .iter()
            .filter(|frame| frame["kind"] == kind)
            .collect()
    }
}

/// A protocol-4 extension on the far side of one connection.
pub(crate) struct FakeExtension {
    writer: Arc<Mutex<Connection>>,
    seen: Arc<(Mutex<Seen>, Condvar)>,
    acknowledging: Arc<AtomicBool>,
    reader: Option<JoinHandle<()>>,
}

impl FakeExtension {
    /// A valid protocol-4 hello for one browser identity.
    pub(crate) fn hello(browser_id: &str) -> Value {
        json!({
            "kind": "hello",
            "protocol": wire::PROTOCOL,
            "browser_id": browser_id,
            "browser_name": "Chrome",
            "platform": "ghostlight/chromium",
            "engine_version": "131.0.6778.86",
            "adapter_version": "2.0.0",
            "attended": false
        })
    }

    /// A receipt that answers `correlation` with a failure code and no effect.
    pub(crate) fn failed(correlation: &str, failure: wire::Failure) -> wire::Receipt {
        wire::Receipt {
            ok: None,
            failure: Some(failure),
            ..Self::ok(correlation, Value::Null)
        }
    }

    /// A receipt that answers `correlation` with an outcome and no effect.
    pub(crate) fn ok(correlation: &str, outcome: Value) -> wire::Receipt {
        wire::Receipt {
            correlation: correlation.into(),
            ok: Some(outcome),
            failure: None,
            message: None,
            dispatched: false,
            committed_urls: vec![],
            tab: None,
        }
    }

    /// Speak for the extension on `stream`, the far end of a connected pair: send `hello`, and
    /// answer what follows with `script`.
    pub(crate) fn connect(mut stream: Connection, hello: &Value, script: Script) -> Self {
        write_frame(&mut stream, hello).expect("the fake sends its hello");
        let writer = Arc::new(Mutex::new(stream.try_clone().expect("a writer handle")));
        let seen = Arc::new((Mutex::new(Seen::default()), Condvar::new()));
        let acknowledging = Arc::new(AtomicBool::new(true));
        let reading = Reading {
            stream,
            writer: Arc::clone(&writer),
            seen: Arc::clone(&seen),
            acknowledging: Arc::clone(&acknowledging),
            script,
            transfers: HashMap::new(),
            held: HashMap::new(),
        };
        let reader = thread::spawn(move || reading.run());
        Self {
            writer,
            seen,
            acknowledging,
            reader: Some(reader),
        }
    }

    /// Send one frame to the service.
    pub(crate) fn send(&self, frame: &impl serde::Serialize) {
        write_frame(&mut *lock(&self.writer), frame).expect("the fake sends a frame");
    }

    /// Send one frame of exactly these bytes, which need not be JSON this service can decode.
    pub(crate) fn send_raw(&self, payload: &[u8]) {
        write_frame_raw(&mut *lock(&self.writer), payload).expect("the fake sends a frame");
    }

    /// Answer heartbeats from now on, or stop answering them.
    pub(crate) fn acknowledge_heartbeats(&self, acknowledging: bool) {
        self.acknowledging.store(acknowledging, Ordering::SeqCst);
    }

    /// Close the fake's side of the connection.
    pub(crate) fn close(&self) {
        lock(&self.writer).shutdown();
    }

    /// Wait until what the fake has seen satisfies `condition`, and return it.
    ///
    /// # Panics
    ///
    /// Panics when the condition does not hold within the test patience.
    pub(crate) fn wait(&self, condition: impl Fn(&Seen) -> bool) -> MutexGuard<'_, Seen> {
        let (seen, changed) = &*self.seen;
        let until = Instant::now() + PATIENCE;
        let mut guard = lock(seen);
        while !condition(&guard) {
            let left = until.saturating_duration_since(Instant::now());
            assert!(!left.is_zero(), "the fake never saw it: {guard:?}");
            guard = changed
                .wait_timeout(guard, left)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
        guard
    }
}

impl Drop for FakeExtension {
    fn drop(&mut self) {
        self.close();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

/// The fake's reader thread.
struct Reading {
    stream: Connection,
    writer: Arc<Mutex<Connection>>,
    seen: Arc<(Mutex<Seen>, Condvar)>,
    acknowledging: Arc<AtomicBool>,
    script: Script,
    transfers: HashMap<String, BTreeMap<u64, Vec<u8>>>,
    held: HashMap<String, wire::Receipt>,
}

impl Reading {
    fn run(mut self) {
        while let Ok(Some(frame)) = read_frame::<Value>(&mut self.stream) {
            self.record(|seen| seen.frames.push(frame.clone()));
            self.take(&frame);
        }
        self.record(|seen| seen.closed = true);
    }

    fn take(&mut self, frame: &Value) {
        match frame["kind"].as_str() {
            Some("heartbeat") if self.acknowledging.load(Ordering::SeqCst) => {
                self.send(&json!({"kind": "heartbeat_ack", "sequence": frame["sequence"]}));
            }
            Some("request") => self.request(frame),
            Some("command_chunk") => self.chunk(frame),
            Some("cancel") => {
                let correlation = frame["correlation"].as_str().unwrap_or_default().to_owned();
                self.record(|seen| seen.cancels.push(correlation.clone()));
                if let Some(receipt) = self.held.remove(&correlation) {
                    self.send(&wire::Inbound::Receipt(receipt));
                }
            }
            Some("notice") => {
                let notice: wire::Notice =
                    serde_json::from_value(frame.clone()).expect("a notice frame decodes");
                self.record(|seen| seen.notices.push(notice.clone()));
            }
            _ => {}
        }
    }

    /// Collect one chunk, and run the request once its transfer is complete.
    fn chunk(&mut self, frame: &Value) {
        let transfer = frame["transfer_id"].as_str().unwrap_or_default().to_owned();
        let count = frame["count"].as_u64().unwrap_or_default();
        let data = base64::engine::general_purpose::STANDARD
            .decode(frame["data"].as_str().unwrap_or_default())
            .expect("chunk data is base64");
        let parts = self.transfers.entry(transfer.clone()).or_default();
        parts.insert(frame["index"].as_u64().unwrap_or_default(), data);
        if parts.len() as u64 == count {
            let bytes: Vec<u8> = self
                .transfers
                .remove(&transfer)
                .unwrap_or_default()
                .into_values()
                .flatten()
                .collect();
            let request: Value =
                serde_json::from_slice(&bytes).expect("a reassembled transfer is a request");
            self.request(&request);
        }
    }

    /// Record one request and answer it as the script says, or as unsupported when its
    /// primitive is not one this fake knows.
    fn request(&mut self, frame: &Value) {
        let correlation = frame["correlation"].as_str().unwrap_or_default().to_owned();
        let reply = match serde_json::from_value::<wire::Primitive>(frame["primitive"].clone()) {
            Ok(primitive) => {
                let reply = (self.script)(&correlation, &primitive);
                self.record(|seen| {
                    seen.requests.push((correlation.clone(), primitive.clone()));
                });
                reply
            }
            Err(_) => Reply::Now(FakeExtension::failed(
                &correlation,
                wire::Failure::Unsupported,
            )),
        };
        match reply {
            Reply::Now(receipt) => self.send(&wire::Inbound::Receipt(receipt)),
            Reply::OnCancel(receipt) => {
                self.held.insert(correlation, receipt);
            }
            Reply::Never => {}
        }
    }

    fn send(&self, frame: &impl serde::Serialize) {
        let _ = write_frame(&mut *lock(&self.writer), frame);
    }

    fn record(&self, change: impl FnOnce(&mut Seen)) {
        let (seen, changed) = &*self.seen;
        change(&mut lock(seen));
        changed.notify_all();
    }
}
