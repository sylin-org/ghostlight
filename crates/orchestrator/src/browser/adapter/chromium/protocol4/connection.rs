//! One adapter protocol 4 connection: the hello it opens with, the thread that reads its frames,
//! and the receipts its calls await.
//!
//! Protocol 4 is the only adapter protocol: any other first frame is told to update. A valid
//! hello is accepted with the page assets in `hello_accepted`, and its connection takes work at
//! once. Receipts route by
//! correlation and a receipt nobody awaits is ignored. Only a closed socket fails the calls still
//! waiting: a connection that stops answering heartbeats is hidden from new work and fails
//! nothing.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use ghostlight_relay::endpoint::Connection as RelayConnection;
use serde_json::Value;
use uuid::Uuid;

use super::link::{Line, RecordingIndex};
use super::wire;
use crate::browser::adapter::{BrowserEvent, Family};
use crate::browser::transport::{SocketReader, SocketWriter};
use crate::browser::{
    end_connection, is_current, lock, refuse_incompatible, valid_opaque_id, validated_browser_name,
    AdapterNotifications, AdapterRegistry, BrowserError, Connection, ConnectionTag, Liveness,
    RelayBrowserPort, BROWSER_ID_PREFIX, CONTROL_DELIVERY_TIMEOUT,
};

/// Longest engine version a hello may report.
const ENGINE_VERSION_MAX_CHARS: usize = 32;
/// The receipt frame kind, read from a frame that failed typed decoding.
const RECEIPT_KIND: &str = "receipt";
/// The frame field naming a frame's kind.
const KIND_FIELD: &str = "kind";
/// The kind of the first frame on every connection.
const HELLO_KIND: &str = "hello";
/// The hello field naming the protocol the extension speaks.
const PROTOCOL_FIELD: &str = "protocol";
/// The prefix of every connection identity.
const CONNECTION_PREFIX: &str = "connection_";
/// The receipt field naming the request it answers.
const CORRELATION_FIELD: &str = "correlation";

/// What one awaiting call receives: its receipt, or why none can come.
pub type ReceiptResult = Result<wire::Receipt, BrowserError>;

/// The calls awaiting receipts on one connection, by correlation.
#[derive(Debug, Default)]
pub struct Receipts {
    pending: Mutex<HashMap<String, Sender<ReceiptResult>>>,
}

impl Receipts {
    /// Await the receipt for one correlation.
    pub fn register(&self, correlation: &str) -> Receiver<ReceiptResult> {
        let (sender, receiver) = mpsc::channel();
        lock(&self.pending).insert(correlation.into(), sender);
        receiver
    }

    /// Stop awaiting one correlation. A receipt that arrives for it later is ignored.
    pub fn forget(&self, correlation: &str) {
        lock(&self.pending).remove(correlation);
    }

    /// Hand one result to the call awaiting `correlation`. A correlation nobody awaits is
    /// ignored.
    fn deliver(&self, correlation: &str, result: ReceiptResult) {
        if let Some(sender) = lock(&self.pending).remove(correlation) {
            let _ = sender.send(result);
        }
    }

    /// End every awaiting call with one error.
    pub(crate) fn fail_all(&self, error: &BrowserError) {
        let senders: Vec<_> = lock(&self.pending)
            .drain()
            .map(|(_, sender)| sender)
            .collect();
        for sender in senders {
            let _ = sender.send(Err(error.clone()));
        }
    }
}

/// Attach one connection whose first frame is `hello` to `port`.
///
/// A first frame that is not a protocol-4 hello hears `adapter_incompatible` with the sentence
/// that tells the person to update the extension, and its socket closes. A valid hello hears
/// `hello_accepted` with the port's page assets, and its connection is registered and can take
/// work at once.
///
/// `hello_accepted` is written before the connection is registered, so no request or
/// `control_state` frame can precede it. Once registered, the connection's control state is
/// settled against the state `hello_accepted` carried: a publish that ran in between missed this
/// connection in its snapshot, and the settle sends what it published.
///
/// # Errors
///
/// Returns [`BrowserError::Incompatible`] for a first frame that is not a protocol-4 hello,
/// [`BrowserError::Authentication`] for a hello that breaks the identity rules, and the
/// transport failure when the connection cannot be set up.
pub(crate) fn attach(
    port: &RelayBrowserPort,
    stream: RelayConnection,
    reader: SocketReader,
    hello: Value,
) -> Result<(), BrowserError> {
    let offered = offered_protocol(&hello);
    if hello.get(KIND_FIELD).and_then(Value::as_str) != Some(HELLO_KIND)
        || offered != wire::PROTOCOL
    {
        refuse_incompatible(stream);
        return Err(BrowserError::Incompatible {
            offered,
            required: wire::PROTOCOL,
        });
    }
    let hello = validated_hello(hello)?;
    let browser_name = validated_browser_name(hello.browser_name)?;
    let liveness = Arc::new(Liveness::new(
        hello.browser_id.clone(),
        lock(&port.lifecycle).clone(),
        Instant::now(),
    ));
    let writer = Arc::new(
        SocketWriter::new(stream).map_err(|error| BrowserError::Protocol(error.to_string()))?,
    );
    let carried: wire::RuntimeControlState = (*lock(&port.control_state)).into();
    writer
        .native(&wire::Outbound::HelloAccepted {
            service_epoch: port.service_epoch.clone(),
            service_version: env!("CARGO_PKG_VERSION").into(),
            control_state: carried,
            assets: (*port.assets).clone(),
        })
        .map_err(|error| {
            writer.close();
            BrowserError::Protocol(error.to_string())
        })?;
    let receipts = Arc::new(Receipts::default());
    let tag = ConnectionTag {
        browser_id: hello.browser_id,
        connection_id: format!("{CONNECTION_PREFIX}{}", Uuid::new_v4().simple()),
    };
    let reading = Reading {
        reader,
        writer: Arc::clone(&writer),
        receipts: Arc::clone(&receipts),
        notifications: port.notifications(),
        adapters: Arc::clone(&port.adapters),
        tag: tag.clone(),
        liveness: Arc::clone(&liveness),
    };
    port.register(
        Connection {
            id: tag.connection_id.clone(),
            line: Line {
                writer: Arc::clone(&writer),
                receipts,
                recordings: Arc::new(RecordingIndex::default()),
            },
            adapter_version: hello.adapter_version,
            browser_id: tag.browser_id.clone(),
            browser_name,
            liveness: Arc::clone(&liveness),
        },
        hello.attended,
    )?;
    settle_control_state(port, &writer, carried);
    if let Err(error) = thread::Builder::new()
        .name("ghostlight-browser-reader".into())
        .spawn(move || reading.run())
    {
        port.detach_registered(&tag.browser_id, &tag.connection_id);
        return Err(BrowserError::Protocol(error.to_string()));
    }
    port.spawn_heartbeat(&tag, writer, liveness)
}

/// Send a just-registered connection the port's control state when it is no longer the state its
/// `hello_accepted` carried.
///
/// A publish stores the state before it snapshots the registered connections, so a publish that
/// missed this connection stored its state before this read. The state lock is held while the
/// frame is written, so a later publish cannot store a newer state and reach this connection
/// before this older frame does. A writer that cannot deliver the frame closes itself, and the
/// connection's reader then ends it.
fn settle_control_state(
    port: &RelayBrowserPort,
    writer: &SocketWriter,
    carried: wire::RuntimeControlState,
) {
    let current = lock(&port.control_state);
    let state = wire::RuntimeControlState::from(*current);
    if state != carried {
        let _ = writer.native_until(
            &wire::Outbound::ControlState { state },
            Instant::now() + CONTROL_DELIVERY_TIMEOUT,
        );
    }
}

/// The protocol a first frame names, or zero when it names none this service can read.
fn offered_protocol(hello: &Value) -> u16 {
    hello
        .get(PROTOCOL_FIELD)
        .and_then(Value::as_u64)
        .and_then(|protocol| u16::try_from(protocol).ok())
        .unwrap_or_default()
}

/// Decode and check a protocol-4 hello by the identity rules: an opaque browser identity, a
/// platform this service serves, and a plain engine version.
fn validated_hello(hello: Value) -> Result<wire::Hello, BrowserError> {
    let hello: wire::Hello =
        serde_json::from_value(hello).map_err(|_| BrowserError::Authentication)?;
    if Family::from_platform(hello.platform.as_deref()).is_none() {
        return Err(BrowserError::Authentication);
    }
    let engine_version_valid = hello.engine_version.as_deref().is_none_or(|version| {
        !version.is_empty()
            && version.len() <= ENGINE_VERSION_MAX_CHARS
            && version
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'.')
    });
    if !valid_opaque_id(&hello.browser_id, BROWSER_ID_PREFIX) || !engine_version_valid {
        return Err(BrowserError::Authentication);
    }
    Ok(hello)
}

/// The physical event a protocol-4 event reports, when the service acts on it.
///
/// `Attended` is not here: it changes the port's attention order, not a workspace. An event or
/// a control intent this service does not know is ignored.
fn browser_event(event: wire::Event) -> Option<BrowserEvent> {
    let control = event.control_intent();
    match event {
        wire::Event::DocumentCommitted { tab_id, url } => Some(BrowserEvent::DocumentCommitted {
            tab_id,
            url,
            correlation: None,
        }),
        wire::Event::ReadinessChanged { tab_id, readiness } => {
            Some(BrowserEvent::ReadinessChanged {
                tab_id,
                readiness: readiness.into(),
            })
        }
        wire::Event::DialogChanged {
            tab_id,
            present,
            dialog_type,
        } => Some(BrowserEvent::DialogChanged {
            tab_id,
            present,
            dialog_type,
        }),
        wire::Event::TabClosed { tab_id } => Some(BrowserEvent::TabClosed { tab_id }),
        wire::Event::RuntimeControlRequested { .. } => {
            control.map(|intent| BrowserEvent::RuntimeControlRequested { intent })
        }
        wire::Event::Attended | wire::Event::Unknown => None,
    }
}

/// Everything one connection's reader thread owns.
struct Reading {
    reader: SocketReader,
    writer: Arc<SocketWriter>,
    receipts: Arc<Receipts>,
    notifications: AdapterNotifications,
    adapters: Arc<Mutex<AdapterRegistry>>,
    tag: ConnectionTag,
    liveness: Arc<Liveness>,
}

impl Reading {
    /// Read frames until the stream ends, then fail every call still waiting and forget the
    /// connection.
    fn run(mut self) {
        while let Ok(Some(frame)) = self.reader.native::<Value>(None) {
            self.take(frame);
        }
        self.writer.close();
        self.receipts
            .fail_all(&BrowserError::DisconnectedAfterDispatch);
        end_connection(&self.adapters, &self.tag, &self.notifications);
    }

    /// Act on one frame. A frame this service cannot read is ignored, except a receipt, whose
    /// call learns that its answer was unreadable.
    fn take(&self, frame: Value) {
        let inbound = match serde_json::from_value::<wire::Inbound>(frame.clone()) {
            Ok(inbound) => inbound,
            Err(error) => {
                if frame.get(KIND_FIELD).and_then(Value::as_str) == Some(RECEIPT_KIND) {
                    if let Some(correlation) = frame.get(CORRELATION_FIELD).and_then(Value::as_str)
                    {
                        self.receipts
                            .deliver(correlation, Err(BrowserError::Protocol(error.to_string())));
                    }
                }
                return;
            }
        };
        match inbound {
            wire::Inbound::Receipt(receipt) => {
                let correlation = receipt.correlation.clone();
                self.receipts.deliver(&correlation, Ok(receipt));
            }
            wire::Inbound::Event { event } => self.event(event),
            wire::Inbound::HeartbeatAck { sequence } => {
                self.liveness.acknowledge(sequence, Instant::now());
            }
            wire::Inbound::Hello(_) | wire::Inbound::Error { .. } | wire::Inbound::Unknown => {}
        }
    }

    /// Apply one event from the connection that currently serves its browser.
    fn event(&self, event: wire::Event) {
        if !is_current(&self.adapters, &self.tag) {
            return;
        }
        if matches!(event, wire::Event::Attended) {
            lock(&self.adapters).attend(&self.tag.browser_id);
            return;
        }
        if let (Some(sink), Some(event)) = (&self.notifications.sink, browser_event(event)) {
            sink.on_event(&self.tag.browser_id, event);
        }
    }
}

#[cfg(test)]
mod tests;
