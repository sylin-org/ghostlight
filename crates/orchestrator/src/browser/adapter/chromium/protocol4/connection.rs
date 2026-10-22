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
use ghostlight_relay::frame::FrameError;
use serde_json::Value;

use super::link::Line;
use super::wire;
use crate::browser::adapter::BrowserEvent;
use crate::browser::transport::{SocketReader, SocketWriter};
use crate::browser::{
    Admitted, BrowserError, Registration, RelayBrowserPort, CONTROL_DELIVERY_TIMEOUT,
    UNBRANDED_BROWSER_NAME,
};
use crate::sync::lock;

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
// A product name is a label a person reads next to a browser they already recognize, not a
// place for an adapter to write prose.
const BROWSER_NAME_MAX_CHARS: usize = 40;
/// The longest browser identity or extension life a hello may report, prefix included.
const OPAQUE_ID_MAX_BYTES: usize = 80;
/// Every browser identity an extension mints starts with this prefix.
const BROWSER_ID_PREFIX: &str = "browser_";
/// Every extension life an extension mints starts with this prefix.
const LIFE_PREFIX: &str = "life_";
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
    let writer = Arc::new(
        SocketWriter::new(stream).map_err(|error| BrowserError::Protocol(error.to_string()))?,
    );
    let carried = wire::RuntimeControlState::from(*port.control_state());
    writer
        .native(&wire::Outbound::HelloAccepted {
            service_epoch: port.service_epoch().into(),
            service_version: env!("CARGO_PKG_VERSION").into(),
            control_state: carried,
            assets: port.assets().clone(),
        })
        .map_err(|error| {
            writer.close();
            BrowserError::Protocol(error.to_string())
        })?;
    let line = Line {
        writer,
        receipts: Arc::new(Receipts::default()),
    };
    let registration = port.admit(Admitted {
        line: line.clone(),
        adapter_version: hello.adapter_version,
        browser_id: hello.browser_id,
        browser_name,
        life: hello.life,
        attended: hello.attended,
    })?;
    settle_control_state(port, &line, carried);
    let reading = Reading {
        reader,
        line: line.clone(),
        registration: registration.clone(),
    };
    if let Err(error) = thread::Builder::new()
        .name("ghostlight-browser-reader".into())
        .spawn(move || reading.run())
    {
        port.withdraw(&registration);
        return Err(BrowserError::Protocol(error.to_string()));
    }
    port.watch(&registration, line)
}

/// Send a just-registered connection the port's control state when it is no longer the state its
/// `hello_accepted` carried.
///
/// A publish stores the state before it snapshots the registered connections, so a publish that
/// missed this connection stored its state before this read. The state lock is held while the
/// frame is written, so a later publish cannot store a newer state and reach this connection
/// before this older frame does. A writer that cannot deliver the frame closes itself, and the
/// connection's reader then ends it.
fn settle_control_state(port: &RelayBrowserPort, line: &Line, carried: wire::RuntimeControlState) {
    let current = port.control_state();
    if wire::RuntimeControlState::from(*current) != carried {
        let _ = line.control_state(*current, Instant::now() + CONTROL_DELIVERY_TIMEOUT);
    }
}

/// Tell an extension that does not speak adapter protocol 4 to update, then close its socket.
/// Best effort: the extension may already be gone.
fn refuse_incompatible(stream: RelayConnection) {
    let Ok(writer) = SocketWriter::new(stream) else {
        return;
    };
    let _ = writer.native_until(
        &wire::Outbound::Error {
            code: wire::error_code::ADAPTER_INCOMPATIBLE.into(),
            message: crate::language::adapter::ADAPTER_INCOMPATIBLE.into(),
        },
        Instant::now() + CONTROL_DELIVERY_TIMEOUT,
    );
    writer.close();
}

/// Validate the bounded product name a hello reports. An extension leaves the generic Chromium
/// brand out, so a browser that reports none is Chromium ([`UNBRANDED_BROWSER_NAME`]), and its
/// connection is named and remembered like any other browser's.
fn validated_browser_name(name: Option<String>) -> Result<String, BrowserError> {
    let Some(name) = name else {
        return Ok(UNBRANDED_BROWSER_NAME.to_owned());
    };
    let acceptable = !name.trim().is_empty()
        && name.chars().count() <= BROWSER_NAME_MAX_CHARS
        && !name.chars().any(char::is_control);
    if !acceptable {
        return Err(BrowserError::Authentication);
    }
    Ok(name)
}

/// Whether `value` is an opaque id an extension mints: `prefix`, then letters, digits and
/// underscores, bounded.
fn valid_opaque_id(value: &str, prefix: &str) -> bool {
    value.starts_with(prefix)
        && value.len() > prefix.len()
        && value.len() <= OPAQUE_ID_MAX_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

/// The protocol a first frame names, or zero when it names none this service can read.
fn offered_protocol(hello: &Value) -> u16 {
    hello
        .get(PROTOCOL_FIELD)
        .and_then(Value::as_u64)
        .and_then(|protocol| u16::try_from(protocol).ok())
        .unwrap_or_default()
}

/// Decode and check a protocol-4 hello by the identity rules: an opaque browser identity, no
/// platform or the Chromium [`wire::PLATFORM`], and a plain engine version.
///
/// A life that is not an opaque life id proves nothing about the browser run, so the hello is
/// taken as one that names no life, and its tabs are confirmed by their URLs.
fn validated_hello(hello: Value) -> Result<wire::Hello, BrowserError> {
    let mut hello: wire::Hello =
        serde_json::from_value(hello).map_err(|_| BrowserError::Authentication)?;
    hello.life = hello.life.filter(|life| valid_opaque_id(life, LIFE_PREFIX));
    if hello
        .platform
        .as_deref()
        .is_some_and(|platform| platform != wire::PLATFORM)
    {
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
        wire::Event::DocumentCommitted { tab_id, url } => {
            Some(BrowserEvent::DocumentCommitted { tab_id, url })
        }
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
        wire::Event::ShowRequested => Some(BrowserEvent::ShowRequested),
        wire::Event::Attended | wire::Event::Unknown => None,
    }
}

/// Everything one connection's reader thread owns.
struct Reading {
    reader: SocketReader,
    line: Line,
    registration: Registration,
}

impl Reading {
    /// Read frames until the stream ends, then fail every call still waiting and forget the
    /// connection. A frame is read whole before its JSON is decoded, so a frame whose JSON this
    /// service cannot decode, such as a 2.0.0 extension's frame that carries half an emoji from a
    /// page, is dropped and the stream stays aligned: one bad frame never ends the connection, and
    /// with it every session's browser.
    fn run(mut self) {
        loop {
            match self.reader.native::<Value>(None) {
                Ok(Some(frame)) => self.take(frame),
                Err(FrameError::Json(_)) => {}
                Ok(None) | Err(_) => break,
            }
        }
        self.line.end();
        self.registration.end();
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
                        self.line
                            .receipts
                            .deliver(correlation, Err(BrowserError::Protocol(error.to_string())));
                    }
                }
                return;
            }
        };
        match inbound {
            wire::Inbound::Receipt(receipt) => {
                let correlation = receipt.correlation.clone();
                self.line.receipts.deliver(&correlation, Ok(receipt));
            }
            wire::Inbound::Event { event } => self.event(event),
            wire::Inbound::HeartbeatAck { sequence } => {
                self.registration.acknowledge(sequence);
            }
            wire::Inbound::Hello(_) | wire::Inbound::Error { .. } | wire::Inbound::Unknown => {}
        }
    }

    /// Apply one event from the connection that currently serves its browser.
    fn event(&self, event: wire::Event) {
        if !self.registration.is_current() {
            return;
        }
        if matches!(event, wire::Event::Attended) {
            self.registration.attended();
            return;
        }
        if let Some(event) = browser_event(event) {
            self.registration.event(event);
        }
    }
}

#[cfg(test)]
mod tests;
