//! Relay and adapter contract tests.

use std::net::{TcpListener, TcpStream};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use ghostlight_bridge::browser::{
    adapter_capability, AdapterCapability, BrowserCommand, BrowserFrame, BrowserOutcome,
    BrowserReadiness, BrowserReceipt, PhysicalTab, ADAPTER_PROTOCOL_MAJOR,
};
use ghostlight_bridge::framing::{read_native, write_native};

use super::super::{
    adapter_error, choose_browser, testing, AdapterRegistry, BrowserError, BrowserPort,
    HeartbeatSettings, RelayBrowserPort,
};
use super::{announce_adapter, announce_browser, capability, short_heartbeat, TEST_BROWSER};

#[test]
fn adapter_local_interlock_is_a_decisive_typed_refusal() {
    assert_eq!(
        adapter_error("local_interlock", "preserved".into(), false),
        BrowserError::LocalInterlock("preserved".into())
    );
    assert_eq!(
        adapter_error("local_interlock", "unknown".into(), true),
        BrowserError::EffectUnknown("unknown".into())
    );
}

#[test]
fn incompatible_browser_bridge_fails_during_hello() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let client = thread::spawn(move || {
        let mut stream = TcpStream::connect(address).unwrap();
        write_native(
            &mut stream,
            &BrowserFrame::Hello {
                major: ADAPTER_PROTOCOL_MAJOR + 1,
                adapter_version: "future".into(),
                browser_id: "browser_test".into(),
                adapter_epoch: "adapter_test".into(),
                browser_name: None,
                attended: false,
                capabilities: vec![],
            },
        )
        .unwrap();
    });
    let (stream, _) = listener.accept().unwrap();
    let port = RelayBrowserPort::new("service_test".into());
    assert_eq!(
        port.attach(stream),
        Err(BrowserError::Incompatible {
            offered: ADAPTER_PROTOCOL_MAJOR + 1,
            required: ADAPTER_PROTOCOL_MAJOR
        })
    );
    client.join().unwrap();
}

#[test]
fn an_older_capability_revision_refuses_before_dispatch() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (release, hold) = mpsc::channel();
    let client = thread::spawn(move || {
        let mut stream = TcpStream::connect(address).unwrap();
        announce_adapter(
            &mut stream,
            vec![
                capability(adapter_capability::TABS),
                AdapterCapability {
                    name: adapter_capability::SCRIPT.into(),
                    revision: 1,
                },
            ],
        );
        hold.recv().unwrap();
    });
    let (stream, _) = listener.accept().unwrap();
    let port = RelayBrowserPort::with_heartbeat_settings("service_test".into(), short_heartbeat());
    port.attach(stream).unwrap();

    assert_eq!(
        port.call(
            TEST_BROWSER,
            "workspace_test",
            BrowserCommand::DescribeDocuments {
                tab_id: 1,
                locators: vec![],
                points: vec![],
                focused: false
            },
            Instant::now() + Duration::from_millis(200),
            &AtomicBool::new(false)
        ),
        Err(BrowserError::CapabilityVersion {
            capability: adapter_capability::DOCUMENT_SCOPE.into(),
            required: 1,
            advertised: 0
        })
    );

    assert_eq!(
        port.call(
            TEST_BROWSER,
            "workspace_test",
            BrowserCommand::EvaluateScript {
                tab_id: 1,
                script: "1+1".into(),
                max_result_chars: 1000,
            },
            Instant::now() + Duration::from_millis(200),
            &AtomicBool::new(false),
        ),
        Err(BrowserError::CapabilityVersion {
            capability: adapter_capability::SCRIPT.into(),
            required: adapter_capability::SCRIPT_REVISION_REPL,
            advertised: 1,
        })
    );

    // A revision-1 command still dispatches against the same connection,
    // so the refusal is per command, not per adapter.
    assert!(matches!(
        port.call(
            TEST_BROWSER,
            "workspace_test",
            BrowserCommand::ListTabs,
            Instant::now() + Duration::from_millis(100),
            &AtomicBool::new(false),
        ),
        Err(BrowserError::DeadlineAfterDispatch)
    ));
    release.send(()).unwrap();
    client.join().unwrap();
}

#[test]
fn attachment_without_adapter_acknowledgement_becomes_unavailable() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (release, hold) = mpsc::channel();
    let client = thread::spawn(move || {
        let mut stream = TcpStream::connect(address).unwrap();
        announce_adapter(
            &mut stream,
            vec![
                capability(adapter_capability::TABS),
                capability(adapter_capability::ADAPTER_LIVENESS),
            ],
        );
        hold.recv().unwrap();
    });
    let (stream, _) = listener.accept().unwrap();
    let port = RelayBrowserPort::with_heartbeat_settings("service_test".into(), short_heartbeat());
    port.attach(stream).unwrap();
    assert!(port.is_connected());

    let deadline = Instant::now() + Duration::from_millis(500);
    while port.is_connected() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }

    assert!(!port.is_connected());
    assert!(
        port.is_registered(TEST_BROWSER),
        "the relay socket is still attached"
    );
    assert_eq!(
        port.call(
            TEST_BROWSER,
            "workspace_test",
            BrowserCommand::ListTabs,
            Instant::now() + Duration::from_millis(100),
            &AtomicBool::new(false),
        ),
        Err(BrowserError::DisconnectedBeforeDispatch)
    );
    release.send(()).unwrap();
    client.join().unwrap();
}

#[test]
fn an_adapter_without_liveness_keeps_its_compatible_attachment_semantics() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (release, hold) = mpsc::channel();
    let client = thread::spawn(move || {
        let mut stream = TcpStream::connect(address).unwrap();
        announce_adapter(&mut stream, vec![capability(adapter_capability::TABS)]);
        hold.recv().unwrap();
    });
    let (stream, _) = listener.accept().unwrap();
    let port = RelayBrowserPort::with_heartbeat_settings("service_test".into(), short_heartbeat());
    port.attach(stream).unwrap();

    thread::sleep(Duration::from_millis(75));

    assert!(port.is_connected());
    release.send(()).unwrap();
    client.join().unwrap();
}

#[test]
fn an_unanswered_dispatch_probe_quarantines_the_adapter_at_deadline() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (release, hold) = mpsc::channel();
    let client = thread::spawn(move || {
        let mut stream = TcpStream::connect(address).unwrap();
        announce_adapter(
            &mut stream,
            vec![
                capability(adapter_capability::TABS),
                capability(adapter_capability::ADAPTER_LIVENESS),
            ],
        );
        hold.recv().unwrap();
    });
    let (stream, _) = listener.accept().unwrap();
    let port = RelayBrowserPort::with_heartbeat_settings(
        "service_test".into(),
        HeartbeatSettings {
            interval: Duration::from_secs(1),
            timeout: Duration::from_secs(2),
        },
    );
    port.attach(stream).unwrap();

    assert_eq!(
        port.call(
            TEST_BROWSER,
            "workspace_test",
            BrowserCommand::ListTabs,
            Instant::now() + Duration::from_millis(75),
            &AtomicBool::new(false),
        ),
        Err(BrowserError::DeadlineAfterDispatch)
    );
    assert!(!port.is_connected());
    assert!(
        port.is_registered(TEST_BROWSER),
        "the relay socket is still attached"
    );
    release.send(()).unwrap();
    client.join().unwrap();
}

#[test]
fn heartbeat_acknowledgements_keep_a_silent_operation_available() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (release, hold) = mpsc::channel();
    let client = thread::spawn(move || {
        let mut stream = TcpStream::connect(address).unwrap();
        announce_adapter(
            &mut stream,
            vec![
                capability(adapter_capability::TABS),
                capability(adapter_capability::ADAPTER_LIVENESS),
            ],
        );
        let mut pending = None;
        loop {
            let frame = read_native::<BrowserFrame>(&mut stream).unwrap().unwrap();
            match frame {
                BrowserFrame::Request { request }
                    if matches!(request.command, BrowserCommand::ListTabs) =>
                {
                    pending = Some((request.correlation, Instant::now()));
                }
                BrowserFrame::Heartbeat { sequence } => {
                    write_native(&mut stream, &BrowserFrame::HeartbeatAck { sequence }).unwrap();
                }
                _ => {}
            }
            if pending
                .as_ref()
                .is_some_and(|(_, started)| started.elapsed() >= Duration::from_millis(1_500))
            {
                let (correlation, _) = pending.take().unwrap();
                write_native(
                    &mut stream,
                    &BrowserFrame::Receipt {
                        receipt: BrowserReceipt {
                            correlation,
                            result: BrowserOutcome::Tabs { tabs: vec![] },
                        },
                    },
                )
                .unwrap();
                break;
            }
        }
        hold.recv().unwrap();
    });
    let (stream, _) = listener.accept().unwrap();
    let port = RelayBrowserPort::with_heartbeat_settings(
        "service_test".into(),
        HeartbeatSettings {
            interval: Duration::from_millis(100),
            timeout: Duration::from_secs(1),
        },
    );
    port.attach(stream).unwrap();

    assert_eq!(
        port.call(
            TEST_BROWSER,
            "workspace_test",
            BrowserCommand::ListTabs,
            Instant::now() + Duration::from_secs(4),
            &AtomicBool::new(false),
        ),
        Ok(BrowserOutcome::Tabs { tabs: vec![] })
    );
    assert!(port.is_connected());
    release.send(()).unwrap();
    client.join().unwrap();
}

#[test]
fn two_browsers_are_two_adapters_and_each_keeps_its_own_work() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let port = RelayBrowserPort::new("service_test".into());

    let mut adapters = Vec::new();
    for browser in ["browser_chrome", "browser_edge"] {
        let (ready, wait) = mpsc::channel();
        let client = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            announce_browser(
                &mut stream,
                browser,
                false,
                vec![capability(adapter_capability::TABS)],
            );
            ready.send(()).unwrap();
            // Answer exactly one request, naming which browser answered it.
            let Some(BrowserFrame::Request { request }) =
                read_native::<BrowserFrame>(&mut stream).unwrap()
            else {
                panic!("the adapter is asked for one primitive");
            };
            write_native(
                &mut stream,
                &BrowserFrame::Receipt {
                    receipt: BrowserReceipt {
                        correlation: request.correlation,
                        result: BrowserOutcome::Tabs {
                            tabs: vec![PhysicalTab {
                                tab_id: 5,
                                title: browser.into(),
                                url: "about:blank".into(),
                                active: true,
                                readiness: BrowserReadiness::Complete,
                            }],
                        },
                    },
                },
            )
            .unwrap();
        });
        let (stream, _) = listener.accept().unwrap();
        port.attach(stream).unwrap();
        wait.recv().unwrap();
        adapters.push(client);
    }

    // Both are connected at once. Neither replaced the other, because they are two browsers.
    let connected: Vec<_> = port
        .connected_browsers()
        .into_iter()
        .map(|browser| browser.id)
        .collect();
    assert_eq!(connected, vec!["browser_chrome", "browser_edge"]);

    // A request reaches the browser it named, and each browser's tab 5 is its own.
    for browser in ["browser_chrome", "browser_edge"] {
        let Ok(BrowserOutcome::Tabs { tabs }) = port.call(
            browser,
            "workspace_test",
            BrowserCommand::ListTabs,
            Instant::now() + Duration::from_millis(500),
            &AtomicBool::new(false),
        ) else {
            panic!("each browser answers its own request");
        };
        assert_eq!(tabs[0].title, browser);
    }
    for adapter in adapters {
        adapter.join().unwrap();
    }
}

#[test]
fn a_second_connection_from_one_browser_collapses_onto_the_first() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let port = RelayBrowserPort::new("service_test".into());

    // One browser opening two native ports is the failure this design exists to make
    // impossible: the browser mints its identity once, so the second connection is the same
    // adapter arriving twice, not a second browser.
    let (closed, retired) = mpsc::channel();
    let first = thread::spawn(move || {
        let mut stream = TcpStream::connect(address).unwrap();
        announce_browser(
            &mut stream,
            "browser_one",
            false,
            vec![capability(adapter_capability::TABS)],
        );
        let mut drained = Vec::new();
        // Returns only when the service closes this connection.
        let _ = std::io::Read::read_to_end(&mut stream, &mut drained);
        closed.send(()).unwrap();
    });
    let (stream, _) = listener.accept().unwrap();
    port.attach(stream).unwrap();

    let (ready, holding) = mpsc::channel();
    let (release, hold) = mpsc::channel();
    let second = thread::spawn(move || {
        let mut stream = TcpStream::connect(address).unwrap();
        announce_browser(
            &mut stream,
            "browser_one",
            false,
            vec![capability(adapter_capability::TABS)],
        );
        ready.send(()).unwrap();
        hold.recv().unwrap();
    });
    let (stream, _) = listener.accept().unwrap();
    port.attach(stream).unwrap();
    holding.recv().unwrap();

    // One browser is still one browser, served by its newest connection.
    assert_eq!(port.connected_browsers().len(), 1);

    // The replaced connection is closed rather than abandoned. Left open, it would keep its
    // relay process alive and the browser's stale native port with it, and every request
    // written into it would be dropped in silence instead of refused.
    assert!(
        retired.recv_timeout(Duration::from_secs(5)).is_ok(),
        "the replaced connection reads end-of-stream instead of hanging open"
    );
    first.join().unwrap();
    release.send(()).unwrap();
    second.join().unwrap();
}

#[test]
fn attention_is_reported_move_to_front_and_never_routes_to_an_absent_browser() {
    let mut registry = AdapterRegistry::default();
    assert_eq!(registry.attended(), None);

    registry.attend("browser_chrome");
    registry.attend("browser_edge");
    registry.attend("browser_chrome");
    assert_eq!(registry.attention, ["browser_chrome", "browser_edge"]);

    // Attention outlives connections, so a browser that reconnects keeps the place it earned.
    // Until one of them is actually connected, it routes nothing.
    assert_eq!(registry.attended(), None);
}

#[test]
fn routing_prefers_selection_then_binding_then_attention() {
    let chrome = testing::summary("browser_chrome", false);
    let edge = testing::summary("browser_edge", true);
    let connected = vec![chrome.clone(), edge.clone()];

    // An explicit selection wins over the attended default.
    assert_eq!(
        choose_browser(Some("browser_chrome"), None, &connected).unwrap(),
        "browser_chrome"
    );
    // An established binding wins over the attended default, so work stays where it started.
    assert_eq!(
        choose_browser(None, Some("browser_chrome"), &connected).unwrap(),
        "browser_chrome"
    );
    // With nothing else to go on, the browser the person last attended gets the work.
    assert_eq!(
        choose_browser(None, None, &connected).unwrap(),
        "browser_edge"
    );
    // A sole browser needs no evidence at all.
    assert_eq!(
        choose_browser(None, None, &[testing::summary("browser_only", false)]).unwrap(),
        "browser_only"
    );
}

#[test]
fn routing_refuses_rather_than_guessing_or_failing_over() {
    let unattended = vec![
        testing::summary("browser_chrome", false),
        testing::summary("browser_edge", false),
    ];
    // Two browsers and no evidence: name the candidates, choose nothing.
    assert_eq!(
        choose_browser(None, None, &unattended),
        Err(BrowserError::AmbiguousBrowser(vec![
            "browser_chrome".into(),
            "browser_edge".into()
        ]))
    );
    // A bound browser that stopped never fails over to the other one.
    assert_eq!(
        choose_browser(None, Some("browser_gone"), &unattended),
        Err(BrowserError::DisconnectedBeforeDispatch)
    );
    // A workspace cannot be told to continue somewhere its tabs are not.
    assert_eq!(
        choose_browser(Some("browser_edge"), Some("browser_chrome"), &unattended),
        Err(BrowserError::BrowserPinned)
    );
    // A selection nobody is serving is a refusal, not a substitution.
    assert_eq!(
        choose_browser(Some("browser_absent"), None, &unattended),
        Err(BrowserError::UnknownBrowser("browser_absent".into()))
    );
    assert_eq!(
        choose_browser(None, None, &[]),
        Err(BrowserError::DisconnectedBeforeDispatch)
    );
}
