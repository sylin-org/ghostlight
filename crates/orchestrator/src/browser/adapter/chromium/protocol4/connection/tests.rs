//! Protocol-4 attachment, routing and liveness against a scripted extension.

use std::fmt::Write as _;
use std::net::TcpListener;
use std::sync::mpsc::{RecvTimeoutError, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::super::fake::{FakeExtension, Reply, Script};
use super::super::wire;
use super::Receipts;
use crate::browser::adapter::{
    BrowserEvent, BrowserReadiness, PresentationActivity, PresentationKind, RuntimeControlIntent,
    RuntimeControlState,
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
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let fake = FakeExtension::connect(listener.local_addr().unwrap(), hello, quiet());
    let attached = port.attach(listener.accept().unwrap().0);
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

#[test]
fn a_protocol_four_hello_is_accepted_with_the_assets_and_listed_ready() {
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
    assert_eq!(browsers.len(), 1);
    assert_eq!(browsers[0].id, FOUR);
    assert_eq!(browsers[0].name.as_deref(), Some("Chrome"));
    assert_eq!(browsers[0].adapter_version, "2.0.0");
    assert_eq!(*lifecycle.0.lock().unwrap(), [format!("attached {FOUR}")]);
}

#[test]
fn a_first_frame_that_is_not_a_protocol_four_hello_hears_why_and_is_closed() {
    for (hello, offered) in [
        (
            json!({"kind": "hello", "protocol": 5, "browser_id": FOUR}),
            5,
        ),
        (json!({"kind": "hello", "major": 3, "browser_id": FOUR}), 0),
        (json!({"kind": "receipt", "protocol": 4}), 4),
        (json!([]), 0),
    ] {
        let port = port();
        let (fake, attached) = attach(&port, &hello);
        assert_eq!(
            attached,
            Err(BrowserError::Incompatible {
                offered,
                required: wire::PROTOCOL
            })
        );
        let seen = fake.wait(|seen| seen.closed);
        assert_eq!(
            seen.frames,
            [json!({
                "kind": "error",
                "code": "adapter_incompatible",
                "message": ADAPTER_INCOMPATIBLE
            })]
        );
        assert!(port.browsers().is_empty());
    }
}

#[test]
fn a_malformed_protocol_four_hello_is_closed_without_an_answer() {
    let mut cases = Vec::new();
    for (field, value) in [
        ("browser_id", json!("browser-four")),
        ("browser_id", json!("browser_")),
        ("browser_name", json!("")),
        ("engine_version", json!("131.0-beta")),
        ("engine_version", json!("1".repeat(33))),
        ("adapter_version", Value::Null),
    ] {
        let mut hello = FakeExtension::hello(FOUR);
        hello[field] = value;
        cases.push(hello);
    }
    let mut without_identity = FakeExtension::hello(FOUR);
    without_identity
        .as_object_mut()
        .unwrap()
        .remove("browser_id");
    cases.push(without_identity);

    for hello in cases {
        let port = port();
        let (fake, attached) = attach(&port, &hello);
        assert_eq!(attached, Err(BrowserError::Authentication), "{hello}");
        let seen = fake.wait(|seen| seen.closed);
        assert!(seen.frames.is_empty(), "{hello}");
        assert!(port.browsers().is_empty());
    }
}

#[test]
fn events_reach_the_sink_and_unknown_frames_are_ignored() {
    let port = port();
    let events = Arc::new(Events::default());
    port.set_event_sink(events.clone());
    let (fake, attached) = attach(&port, &FakeExtension::hello(FOUR));
    attached.unwrap();

    for frame in [
        json!({"kind": "event", "event": {"event": "document_committed", "tab_id": 7, "url": "https://example.com/"}}),
        json!({"kind": "future_frame", "anything": 1}),
        json!({"kind": "event", "event": {"event": "readiness_changed", "tab_id": 7, "readiness": "complete"}}),
        json!({"kind": "event", "event": {"event": "future_event", "tab_id": 7}}),
        json!({"kind": "event", "event": {"event": "dialog_changed", "tab_id": 7, "present": true, "dialog_type": "alert"}}),
        json!({"kind": "event", "event": {"event": "runtime_control_requested", "intent": "future_intent"}}),
        json!({"kind": "event", "event": {"event": "runtime_control_requested", "intent": "hold"}}),
        json!({"kind": "error", "code": "anything", "message": "ignored"}),
        json!({"kind": "event", "event": {"event": "tab_closed", "tab_id": 7}}),
    ] {
        fake.send(&frame);
    }
    eventually(|| events.0.lock().unwrap().len() == 5);
    let seen: Vec<_> = events.0.lock().unwrap().clone();
    assert!(seen.iter().all(|(browser, _)| browser == FOUR));
    assert_eq!(
        seen.into_iter().map(|(_, event)| event).collect::<Vec<_>>(),
        [
            BrowserEvent::DocumentCommitted {
                tab_id: 7,
                url: "https://example.com/".into(),
                correlation: None,
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
            BrowserEvent::TabClosed { tab_id: 7 },
        ]
    );
    assert!(port.is_registered(FOUR));
}

#[test]
fn attention_moves_the_browser_to_the_front() {
    let port = port();
    let mut attended = FakeExtension::hello(OTHER);
    attended["attended"] = json!(true);
    let (_other, attached) = attach(&port, &attended);
    attached.unwrap();
    let (fake, attached) = attach(&port, &FakeExtension::hello(FOUR));
    attached.unwrap();
    assert_eq!(port.browsers()[0].id, OTHER);
    fake.send(&json!({"kind": "event", "event": {"event": "attended"}}));
    eventually(|| port.browsers()[0].id == FOUR);
    assert!(port.browsers()[0].attended);
}

#[test]
fn heartbeat_acknowledgements_keep_liveness_and_silence_fails_nothing() {
    let port = RelayBrowserPort::with_heartbeat_settings(
        "service_four".into(),
        assets(),
        HeartbeatSettings {
            interval: Duration::from_millis(10),
            timeout: Duration::from_millis(60),
        },
    );
    let (fake, attached) = attach(&port, &FakeExtension::hello(FOUR));
    attached.unwrap();

    thread::sleep(Duration::from_millis(250));
    assert_eq!(port.browsers().len(), 1, "acknowledged heartbeats keep it");
    assert!(!fake.wait(|seen| seen.of_kind("heartbeat").len() > 3).closed);

    let (_, receipts) = channel(&port, FOUR);
    let waiting = receipts.register("p4_waiting");
    fake.acknowledge_heartbeats(false);
    eventually(|| port.browsers().is_empty());
    assert!(
        port.is_registered(FOUR),
        "a silent connection stays attached"
    );
    thread::sleep(Duration::from_millis(150));
    assert_eq!(
        waiting.try_recv(),
        Err(TryRecvError::Empty),
        "staleness fails no waiting call"
    );

    fake.acknowledge_heartbeats(true);
    eventually(|| port.browsers().len() == 1);
}

#[test]
fn unknown_and_unreadable_receipts_end_nothing() {
    let port = port();
    let (fake, attached) = attach(&port, &FakeExtension::hello(FOUR));
    attached.unwrap();
    let (_, receipts) = channel(&port, FOUR);

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
    assert!(
        port.is_registered(FOUR),
        "unknown and unreadable receipts end nothing"
    );
}

#[test]
fn cancel_and_notice_frames_reach_the_extension() {
    let port = port();
    let (fake, attached) = attach(&port, &FakeExtension::hello(FOUR));
    attached.unwrap();
    let (writer, _) = channel(&port, FOUR);
    writer
        .native(&wire::Outbound::Cancel {
            correlation: "p4_cancelled".into(),
        })
        .unwrap();
    let notice = wire::Notice {
        tab_id: 7,
        frame: None,
        signal: wire::NoticeSignal {
            invocation: "invocation_four".into(),
            signal: PresentationKind::Progress.into(),
            activity: PresentationActivity::Quiet.into(),
            phase: "Working".into(),
            label: None,
            detail: None,
            locator: None,
            click: None,
        },
        defer_until_visible: false,
    };
    writer
        .native(&wire::Outbound::Notice(notice.clone()))
        .unwrap();
    let seen = fake.wait(|seen| !seen.notices.is_empty());
    assert_eq!(seen.cancels, ["p4_cancelled"]);
    assert_eq!(seen.notices, [notice]);
}

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
        [(FOUR.to_owned(), BrowserEvent::Disconnected)]
    );
    assert_eq!(
        waiting.recv_timeout(Duration::from_millis(10)),
        Err(RecvTimeoutError::Disconnected)
    );
}

#[test]
fn control_state_reaches_every_connection() {
    let port = port();
    let (fake, attached) = attach(&port, &FakeExtension::hello(FOUR));
    attached.unwrap();
    port.publish_control_state(RuntimeControlState::Held)
        .unwrap();
    let seen = fake.wait(|seen| !seen.of_kind("control_state").is_empty());
    assert_eq!(
        seen.of_kind("control_state"),
        [&json!({"kind": "control_state", "state": "held"})]
    );
}

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
