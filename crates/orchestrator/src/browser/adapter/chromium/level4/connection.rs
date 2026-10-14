//! One adapter protocol 4 connection: the hello it opens with, the thread that reads its frames,
//! and the receipts its calls await.
//!
//! The connection is ready as soon as its hello is valid. The page assets travel in
//! `hello_accepted`, so there is no install round trip and no capability gate. Receipts route by
//! correlation and a receipt nobody awaits is ignored. Only a closed socket fails the calls still
//! waiting: a connection that stops answering heartbeats is hidden from new work and fails
//! nothing.

use std::collections::HashMap;
use std::net::TcpStream;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use serde_json::Value;
use uuid::Uuid;

use super::link::{Line, RecordingIndex};
use super::wire;
use crate::browser::adapter::{Adapter, BrowserEvent, Level};
use crate::browser::transport::{SocketReader, SocketWriter};
use crate::browser::{
    end_connection, is_current, lock, valid_opaque_id, validated_browser_name,
    AdapterNotifications, AdapterRegistry, BrowserError, Channel, Connection, ConnectionTag,
    Liveness, RelayBrowserPort, BROWSER_ID_PREFIX,
};

/// Longest engine version a hello may report.
const ENGINE_VERSION_MAX_CHARS: usize = 32;
/// The receipt frame kind, read from a frame that failed typed decoding.
const RECEIPT_KIND: &str = "receipt";
/// The frame field naming a frame's kind.
const KIND_FIELD: &str = "kind";
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

/// Validate a hello that chose protocol 4 and attach its connection to `port`.
///
/// The connection registers, hears `hello_accepted` with the port's page assets, and is
/// published to callers at once.
///
/// # Errors
///
/// Returns [`BrowserError::Authentication`] for a hello that breaks the identity rules, and the
/// transport failure when the connection cannot be set up.
pub(crate) fn attach(
    port: &RelayBrowserPort,
    stream: TcpStream,
    reader: SocketReader,
    hello: Value,
    adapter: Adapter,
) -> Result<(), BrowserError> {
    let hello = validated_hello(hello)?;
    let browser_name = validated_browser_name(hello.browser_name)?;
    let lifecycle = lock(&port.lifecycle).clone();
    let liveness = Arc::new(Liveness::new(
        hello.browser_id.clone(),
        Level::Four,
        lifecycle,
        Instant::now(),
    ));
    let writer = Arc::new(
        SocketWriter::new(stream).map_err(|error| BrowserError::Protocol(error.to_string()))?,
    );
    let receipts = Arc::new(Receipts::default());
    let tag = ConnectionTag {
        browser_id: hello.browser_id,
        connection_id: format!("connection_{}", Uuid::new_v4().simple()),
    };
    let replaced = port.register(
        Connection {
            id: tag.connection_id.clone(),
            writer: Arc::clone(&writer),
            channel: Channel::Four {
                line: Line {
                    writer: Arc::clone(&writer),
                    receipts: Arc::clone(&receipts),
                    recordings: Arc::new(RecordingIndex::default()),
                },
            },
            adapter_version: hello.adapter_version,
            adapter,
            browser_id: tag.browser_id.clone(),
            browser_name,
            liveness: Some(Arc::clone(&liveness)),
            ready: false,
        },
        hello.attended,
    )?;
    if let Err(error) = writer.native(&wire::Outbound::HelloAccepted {
        service_epoch: port.service_epoch.clone(),
        service_version: env!("CARGO_PKG_VERSION").into(),
        control_state: (*lock(&port.control_state)).into(),
        assets: (*port.assets).clone(),
    }) {
        port.detach_registered(&tag.browser_id, &tag.connection_id);
        return Err(BrowserError::Protocol(error.to_string()));
    }
    let announced = Arc::new(AtomicBool::new(false));
    let reading = Reading {
        reader,
        writer: Arc::clone(&writer),
        receipts,
        notifications: port.notifications(&announced),
        adapters: Arc::clone(&port.adapters),
        tag: tag.clone(),
        liveness: Arc::clone(&liveness),
    };
    if let Err(error) = thread::Builder::new()
        .name("ghostlight-browser-reader".into())
        .spawn(move || reading.run())
    {
        port.detach_registered(&tag.browser_id, &tag.connection_id);
        return Err(BrowserError::Protocol(error.to_string()));
    }
    if !port.publish_ready(&tag, &announced, replaced) {
        return Err(BrowserError::DisconnectedBeforeDispatch);
    }
    port.spawn_heartbeat(&tag, writer, liveness, None)
}

/// Decode and check a protocol-4 hello by the identity rules every level shares.
fn validated_hello(hello: Value) -> Result<wire::Hello, BrowserError> {
    let hello: wire::Hello =
        serde_json::from_value(hello).map_err(|_| BrowserError::Authentication)?;
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
