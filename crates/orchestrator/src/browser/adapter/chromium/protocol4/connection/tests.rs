//! Protocol-4 attachment, routing and liveness against a scripted extension.

use std::fmt::Write as _;
use std::sync::mpsc::TryRecvError;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::super::fake::{FakeExtension, Reply, Script};
use super::super::wire;
use super::Receipts;
use crate::browser::adapter::{
    BrowserEvent, BrowserReadiness, RuntimeControlIntent, RuntimeControlState,
};
use crate::browser::{
    lock, AdapterLifecycleObserver, BrowserError, BrowserEventSink, BrowserPort, HeartbeatSettings,
    RelayBrowserPort,
};
use crate::language::adapter::ADAPTER_INCOMPATIBLE;

const FOUR: &str = "browser_four";
const OTHER: &str = "browser_other";
const PATIENCE: Duration = Duration::from_secs(5);

fn assets() -> Arc<wire::Assets> {
    let source = "\"use strict\";\nglobalThis.__ghostlightTestAssets = true;\n".to_owned();
    let mut id = String::new();
    for byte in Sha256::digest(source.as_bytes()) {
        write!(&mut id, "{byte:02x}").unwrap();
    }
    Arc::new(wire::Assets { id, source })
}

fn port() -> RelayBrowserPort {
    RelayBrowserPort::new("service_four".into(), assets())
}

fn quiet() -> Script {
    Box::new(|correlation, _| Reply::Now(FakeExtension::ok(correlation, json!({"tabs": []}))))
}

/// Connect a fake extension with `hello` and let the port attach it.
fn attach(port: &RelayBrowserPort, hello: &Value) -> (FakeExtension, Result<(), BrowserError>) {
    let (service, extension) = ghostlight_relay::endpoint::pair().unwrap();
    let fake = FakeExtension::connect(extension, hello, quiet());
    let attached = port.attach(service);
    (fake, attached)
}

/// The writer and receipts of one attached protocol-4 connection.
fn channel(
    port: &RelayBrowserPort,
    browser: &str,
) -> (Arc<crate::browser::transport::SocketWriter>, Arc<Receipts>) {
    let adapters = lock(&port.adapters);
    let line = &adapters.connections[browser].line;
    (Arc::clone(&line.writer), Arc::clone(&line.receipts))
}

fn eventually(condition: impl Fn() -> bool) {
    let until = Instant::now() + PATIENCE;
    while !condition() {
        assert!(Instant::now() < until, "the condition never held");
        thread::sleep(Duration::from_millis(5));
    }
}

#[derive(Default)]
struct Events(Mutex<Vec<(String, BrowserEvent)>>);

impl BrowserEventSink for Events {
    fn on_event(&self, browser: &str, event: BrowserEvent) {
        self.0.lock().unwrap().push((browser.into(), event));
    }
}

#[derive(Default)]
struct Lifecycle(Mutex<Vec<String>>);

impl AdapterLifecycleObserver for Lifecycle {
    fn adapter_attached(&self, browser_id: &str, _replaced: bool) {
        self.0
            .lock()
            .unwrap()
            .push(format!("attached {browser_id}"));
    }

    fn adapter_detached(&self, browser_id: &str) {
        self.0
            .lock()
            .unwrap()
            .push(format!("detached {browser_id}"));
    }
}

/// The service answers a first frame by whether it is a protocol-4 hello it can trust: a valid
/// hello, with the Chromium platform or none (the frozen extension names none), hears
/// `hello_accepted` with the page assets and is listed and reported attached; a hello of another
/// protocol, or no hello, hears why and is closed (ACCEPTANCE, contracts 5); and a malformed hello
/// or one naming another platform is closed without an answer.
#[test]
fn the_service_answers_a_first_frame_by_whether_it_is_a_protocol_four_hello() {
    let port = port();
    let lifecycle = Arc::new(Lifecycle::default());
    port.set_lifecycle_observer(lifecycle.clone());
    let (fake, attached) = attach(&port, &FakeExtension::hello(FOUR));
    attached.unwrap();
    let seen = fake.wait(|seen| !seen.frames.is_empty());
    let accepted = &seen.frames[0];
    assert_eq!(accepted["kind"], "hello_accepted");
    assert_eq!(accepted["service_epoch"], "service_four");
    assert_eq!(accepted["control_state"], "active");
    assert_eq!(accepted["assets"]["id"], assets().id.as_str());
    assert_eq!(accepted["assets"]["source"], assets().source.as_str());
    drop(seen);
    let browsers = port.browsers();
    assert_eq!(
        (
            browsers.len(),
            browsers[0].id.as_str(),
            browsers[0].name.as_str()
        ),
        (1, FOUR, "Chrome")
    );
    assert_eq!(*lifecycle.0.lock().unwrap(), [format!("attached {FOUR}")]);

    let mut without_platform = FakeExtension::hello(FOUR);
    without_platform.as_object_mut().unwrap().remove("platform");
    let mut chromium = FakeExtension::hello(FOUR);
    chromium["platform"] = json!(wire::PLATFORM);
    for hello in [without_platform, chromium] {
        let port = self::port();
        let (fake, attached) = attach(&port, &hello);
        assert_eq!(attached, Ok(()), "{hello}");
        assert_eq!(
            fake.wait(|seen| !seen.frames.is_empty()).frames[0]["kind"],
            "hello_accepted"
        );
        assert_eq!(port.browsers().len(), 1, "{hello}");
    }

    for (hello, offered) in [
        (
            json!({"kind": "hello", "protocol": 5, "browser_id": FOUR}),
            5,
        ),
        (json!({"kind": "hello", "major": 3, "browser_id": FOUR}), 0),
        (json!({"kind": "receipt", "protocol": 4}), 4),
        (json!([]), 0),
    ] {
        let port = self::port();
        let (fake, attached) = attach(&port, &hello);
        assert_eq!(
            attached,
            Err(BrowserError::Incompatible {
                offered,
                required: wire::PROTOCOL
            })
        );
        assert_eq!(
            fake.wait(|seen| seen.closed).frames,
            [json!({
                "kind": "error",
                "code": "adapter_incompatible",
                "message": ADAPTER_INCOMPATIBLE
            })]
        );
        assert!(port.browsers().is_empty());
    }

    let mut silent = Vec::new();
    for (field, value) in [
        ("browser_id", json!("browser-four")),
        ("browser_id", json!("browser_")),
        ("browser_name", json!("")),
        ("engine_version", json!("131.0-beta")),
        ("engine_version", json!("1".repeat(33))),
        ("adapter_version", Value::Null),
        ("platform", json!("ghostlight/gecko")),
    ] {
        let mut hello = FakeExtension::hello(FOUR);
        hello[field] = value;
        silent.push(hello);
    }
    let mut without_identity = FakeExtension::hello(FOUR);
    without_identity
        .as_object_mut()
        .unwrap()
        .remove("browser_id");
    silent.push(without_identity);
    for hello in silent {
        let port = self::port();
        let (fake, attached) = attach(&port, &hello);
        assert_eq!(attached, Err(BrowserError::Authentication), "{hello}");
        assert!(fake.wait(|seen| seen.closed).frames.is_empty(), "{hello}");
        assert!(port.browsers().is_empty());
    }
}

/// The fake extension's hello for [`FOUR`], naming `life`.
fn hello_in(life: &str) -> Value {
    let mut hello = FakeExtension::hello(FOUR);
    hello["life"] = json!(life);
    hello
}

/// The service reads the extension life each hello names, and each departure, by replacement or
/// by the end of the stream, names the life that left (ADR-0203 Decision 1); a hello without a
/// readable life is accepted and names none.
#[test]
fn the_port_reads_the_life_each_hello_names_and_each_departure_carries_it() {
    let port = port();
    let events = Arc::new(Events::default());
    port.set_event_sink(events.clone());
    assert_eq!(port.life_of(FOUR), None);

    let (_first, attached) = attach(&port, &hello_in("life_first"));
    attached.unwrap();
    assert_eq!(port.life_of(FOUR).as_deref(), Some("life_first"));

    let (second, attached) = attach(&port, &hello_in("life_second"));
    attached.unwrap();
    assert_eq!(port.life_of(FOUR).as_deref(), Some("life_second"));
    assert_eq!(
        *events.0.lock().unwrap(),
        [(
            FOUR.to_owned(),
            BrowserEvent::Disconnected {
                life: Some("life_first".into())
            }
        )]
    );

    second.close();
    eventually(|| events.0.lock().unwrap().len() == 2);
    assert_eq!(
        events.0.lock().unwrap()[1],
        (
            FOUR.to_owned(),
            BrowserEvent::Disconnected {
                life: Some("life_second".into())
            }
        )
    );
    assert_eq!(port.life_of(FOUR), None);

    for life in [
        "life_",
        "life-first",
        "browser_first",
        &format!("life_{}", "a".repeat(80)),
    ] {
        let port = self::port();
        let (_fake, attached) = attach(&port, &hello_in(life));
        assert_eq!(attached, Ok(()), "{life}");
        assert_eq!(port.life_of(FOUR), None, "{life}");
    }
}

/// The browser's events reach the service through the real reader, a browser's attention moves
/// it to the front, and a frame, event or intent this service does not know ends nothing
/// (ACCEPTANCE, executor and truth gates H8: the browser's Pause reaches the service).
#[test]
fn events_reach_the_sink_and_unknown_frames_are_ignored() {
    let port = port();
    let events = Arc::new(Events::default());
    port.set_event_sink(events.clone());
    let mut attended = FakeExtension::hello(OTHER);
    attended["attended"] = json!(true);
    let (_other, attached) = attach(&port, &attended);
    attached.unwrap();
    let (fake, attached) = attach(&port, &FakeExtension::hello(FOUR));
    attached.unwrap();
    assert_eq!(port.browsers()[0].id, OTHER);

    for frame in [
        json!({"kind": "event", "event": {"event": "document_committed", "tab_id": 7, "url": "https://example.com/"}}),
        json!({"kind": "future_frame", "anything": 1}),
        json!({"kind": "event", "event": {"event": "readiness_changed", "tab_id": 7, "readiness": "complete"}}),
        json!({"kind": "event", "event": {"event": "future_event", "tab_id": 7}}),
        json!({"kind": "event", "event": {"event": "dialog_changed", "tab_id": 7, "present": true, "dialog_type": "alert"}}),
        json!({"kind": "event", "event": {"event": "runtime_control_requested", "intent": "future_intent"}}),
        json!({"kind": "event", "event": {"event": "runtime_control_requested", "intent": "hold"}}),
        json!({"kind": "error", "code": "anything", "message": "ignored"}),
        json!({"kind": "event", "event": {"event": "show_requested"}}),
        json!({"kind": "event", "event": {"event": "tab_closed", "tab_id": 7}}),
        json!({"kind": "event", "event": {"event": "attended"}}),
    ] {
        fake.send(&frame);
    }
    eventually(|| events.0.lock().unwrap().len() == 6);
    let seen: Vec<_> = events.0.lock().unwrap().clone();
    assert!(seen.iter().all(|(browser, _)| browser == FOUR));
    assert_eq!(
        seen.into_iter().map(|(_, event)| event).collect::<Vec<_>>(),
        [
            BrowserEvent::DocumentCommitted {
                tab_id: 7,
                url: "https://example.com/".into(),
            },
            BrowserEvent::ReadinessChanged {
                tab_id: 7,
                readiness: BrowserReadiness::Complete,
            },
            BrowserEvent::DialogChanged {
                tab_id: 7,
                present: true,
                dialog_type: "alert".into(),
            },
            BrowserEvent::RuntimeControlRequested {
                intent: RuntimeControlIntent::Hold,
            },
            BrowserEvent::ShowRequested,
            BrowserEvent::TabClosed { tab_id: 7 },
        ]
    );
    eventually(|| port.browsers()[0].id == FOUR);
    assert!(port.browsers()[0].attended);
    assert!(port.is_registered(FOUR));
}

/// Acknowledged heartbeats keep a browser listed; silence unlists it but leaves it attached and
/// fails no waiting call; and an acknowledgement lists it again (ACCEPTANCE, contracts 17).
#[test]
fn heartbeat_acknowledgements_keep_liveness_and_silence_fails_nothing() {
    let port = RelayBrowserPort::with_heartbeat_settings(
        "service_four".into(),
        assets(),
        HeartbeatSettings {
            interval: Duration::from_millis(10),
            timeout: Duration::from_millis(500),
        },
    );
    let (fake, attached) = attach(&port, &FakeExtension::hello(FOUR));
    attached.unwrap();
    let heartbeats = || fake.wait(|_| true).of_kind("heartbeat").len();

    let first = heartbeats();
    drop(fake.wait(|seen| seen.of_kind("heartbeat").len() > first + 3));
    assert_eq!(port.browsers().len(), 1, "acknowledged heartbeats keep it");

    let (_, receipts) = channel(&port, FOUR);
    let waiting = receipts.register("p4_waiting");
    fake.acknowledge_heartbeats(false);
    eventually(|| port.browsers().is_empty());
    assert!(
        port.is_registered(FOUR),
        "a silent connection stays attached"
    );
    let unanswered = heartbeats();
    drop(fake.wait(|seen| seen.of_kind("heartbeat").len() > unanswered + 3));
    assert_eq!(
        waiting.try_recv(),
        Err(TryRecvError::Empty),
        "staleness fails no waiting call"
    );

    fake.acknowledge_heartbeats(true);
    eventually(|| port.browsers().len() == 1);
}

/// A frame the service cannot use ends nothing: half an emoji at the end of a cut text, which
/// V8 writes as a lone surrogate serde_json refuses, once closed every browser's connection
/// (P-D5, commit 583b637a); a receipt for no waiting call is dropped, and an unreadable one fails
/// only its own call.
#[test]
fn a_frame_whose_json_cannot_be_decoded_ends_nothing() {
    let port = port();
    let (fake, attached) = attach(&port, &FakeExtension::hello(FOUR));
    attached.unwrap();
    let (_, receipts) = channel(&port, FOUR);

    fake.send_raw(br#"{"kind":"receipt","correlation":"p4_half","ok":{"title":"a\ud83d"}}"#);
    fake.send(&wire::Inbound::Receipt(FakeExtension::ok(
        "p4_nobody",
        json!({}),
    )));
    let unreadable = receipts.register("p4_unreadable");
    fake.send(&json!({"kind": "receipt", "correlation": "p4_unreadable", "dispatched": "yes"}));
    assert!(matches!(
        unreadable.recv_timeout(PATIENCE).unwrap(),
        Err(BrowserError::Protocol(_))
    ));
    let after = receipts.register("p4_after");
    fake.send(&wire::Inbound::Receipt(FakeExtension::ok(
        "p4_after",
        json!({}),
    )));
    assert!(after.recv_timeout(PATIENCE).unwrap().is_ok());
    assert!(port.is_registered(FOUR), "the connection survives");
}

/// The end of the stream fails each waiting call as possibly dispatched, detaches the browser and
/// reports its departure (ACCEPTANCE, executor and truth gates 4 and contracts 17).
#[test]
fn a_closed_socket_fails_waiting_calls_and_detaches() {
    let port = port();
    let lifecycle = Arc::new(Lifecycle::default());
    let events = Arc::new(Events::default());
    port.set_lifecycle_observer(lifecycle.clone());
    port.set_event_sink(events.clone());
    let (fake, attached) = attach(&port, &FakeExtension::hello(FOUR));
    attached.unwrap();
    let (_, receipts) = channel(&port, FOUR);
    let waiting = receipts.register("p4_waiting");
    fake.close();
    assert_eq!(
        waiting.recv_timeout(PATIENCE),
        Ok(Err(BrowserError::DisconnectedAfterDispatch))
    );
    eventually(|| !port.is_registered(FOUR));
    eventually(|| lifecycle.0.lock().unwrap().len() == 2);
    assert_eq!(
        *lifecycle.0.lock().unwrap(),
        [format!("attached {FOUR}"), format!("detached {FOUR}")]
    );
    assert_eq!(
        *events.0.lock().unwrap(),
        [(FOUR.to_owned(), BrowserEvent::Disconnected { life: None })]
    );
    assert_eq!(waiting.try_recv(), Err(TryRecvError::Disconnected));
}

/// A hold published while a connection attached reaches it, as does every later publish (commit
/// 4f7b3b69): the race between `hello_accepted` and the registration leaves no connection on a
/// stale state.
#[test]
fn a_connection_registered_after_a_publish_missed_it_ends_on_the_published_state() {
    let port = port();
    let (fake, attached) = attach(&port, &FakeExtension::hello(FOUR));
    attached.unwrap();
    let (writer, _) = channel(&port, FOUR);
    // A hold stored after `hello_accepted` carried `active`, whose snapshot predates the
    // registration and so never reached this connection.
    *lock(&port.control_state) = RuntimeControlState::Held;
    super::settle_control_state(&port, &writer, wire::RuntimeControlState::Active);
    // A connection whose `hello_accepted` already carried the state hears nothing more.
    super::settle_control_state(&port, &writer, wire::RuntimeControlState::Held);
    port.publish_control_state(RuntimeControlState::Ended)
        .unwrap();

    let seen = fake.wait(|seen| seen.of_kind("control_state").len() == 2);
    assert_eq!(seen.frames[0]["control_state"], "active");
    assert_eq!(
        seen.of_kind("control_state"),
        [
            &json!({"kind": "control_state", "state": "held"}),
            &json!({"kind": "control_state", "state": "ended"})
        ]
    );
}
