//! Prove runtime admission occurs after queued writer access and before transmission.

use ghostlight_bridge::browser::BrowserPlatform;

use super::super::{lock, BrowserDispatch, Connection};
use super::*;
use crate::governance::ReasonCode;
use std::collections::HashMap;
use std::io::Read;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

#[test]
fn queued_dispatch_rechecks_control_cancellation_and_deadline_without_sending() {
    for case in 0..4 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut peer = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        peer.set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
        let (stream, _) = listener.accept().unwrap();
        let writer = Arc::new(ghostlight_bridge::transport::SocketWriter::new(stream).unwrap());
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let port = RelayBrowserPort::new("test".into());
        lock(&port.adapters).connections.insert(
            TEST_BROWSER.into(),
            Connection {
                id: "connection".into(),
                writer: writer.clone(),
                pending: pending.clone(),
                adapter_version: "test".into(),
                browser_id: TEST_BROWSER.into(),
                browser_name: None,
                platform: BrowserPlatform::Chromium,
                capabilities: HashMap::from([(adapter_capability::TABS.into(), 1)]),
                liveness: None,
                ready: true,
            },
        );
        let cancelled = AtomicBool::new(false);
        let held = AtomicBool::new(false);
        let (admitted, checked) = mpsc::channel();
        let guard = writer
            .until(Instant::now() + Duration::from_secs(3))
            .unwrap();
        let deadline = Instant::now()
            + if case == 2 {
                Duration::from_millis(60)
            } else {
                Duration::from_secs(3)
            };
        thread::scope(|scope| {
            let call = scope.spawn(|| {
                port.call_guarded(
                    TEST_BROWSER,
                    "workspace",
                    BrowserCommand::ListTabs,
                    BrowserDispatch {
                        attention: ghostlight_bridge::browser::BrowserAttention::Foreground,
                        deadline,
                        cancelled: &cancelled,
                        admit: &|| {
                            admitted.send(()).unwrap();
                            if held.load(Ordering::SeqCst) {
                                Err(if case == 3 {
                                    BrowserError::AttentionProtected(ghostlight_bridge::browser::BrowserAttentionReason::PreferenceChanged)
                                } else {
                                    BrowserError::RuntimeControl(ReasonCode::RuntimeHold)
                                })
                            } else {
                                Ok(())
                            }
                        },
                    },
                )
            });
            // A check ahead of the writer would run while this lock is still held and fail this pin.
            assert!(checked.recv_timeout(Duration::from_millis(120)).is_err());
            if case == 1 {
                cancelled.store(true, Ordering::SeqCst);
            } else {
                held.store(true, Ordering::SeqCst);
            }
            drop(guard);
            assert_eq!(
                call.join().unwrap(),
                Err(match case {
                    0 => BrowserError::RuntimeControl(ReasonCode::RuntimeHold),
                    1 => BrowserError::CancelledBeforeDispatch,
                    2 => BrowserError::DeadlineBeforeDispatch,
                    _ => BrowserError::AttentionProtected(
                        ghostlight_bridge::browser::BrowserAttentionReason::PreferenceChanged
                    ),
                })
            );
        });
        assert!(
            lock(&pending).is_empty(),
            "refused work cannot leave a pending receipt"
        );
        let mut byte = [0];
        assert!(
            peer.read(&mut byte).is_err(),
            "no command may reach the adapter"
        );
    }
}

fn connected_pair() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let peer = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let (stream, _) = listener.accept().unwrap();
    (peer, stream)
}

fn register_control_adapter(
    port: &RelayBrowserPort,
    browser: &str,
    stream: TcpStream,
    modern: bool,
) {
    let mut capabilities = HashMap::from([(adapter_capability::TABS.into(), 1)]);
    if modern {
        capabilities.insert(adapter_capability::BROWSER_ATTENTION.into(), 1);
    }
    lock(&port.adapters).connections.insert(
        browser.into(),
        Connection {
            id: format!("connection_{browser}"),
            writer: Arc::new(ghostlight_bridge::transport::SocketWriter::new(stream).unwrap()),
            pending: Arc::new(Mutex::new(HashMap::new())),
            adapter_version: "1.3.12".into(),
            browser_id: browser.into(),
            browser_name: Some("Chrome".into()),
            platform: BrowserPlatform::Chromium,
            capabilities,
            liveness: None,
            ready: true,
        },
    );
}

#[test]
fn quiet_legacy_cleanup_retires_only_that_adapter_without_a_physical_close_or_global_stop() {
    use ghostlight_bridge::browser::{BrowserAttention, RuntimeControlState};
    let (mut legacy, legacy_stream) = connected_pair();
    let (mut modern, modern_stream) = connected_pair();
    let port = RelayBrowserPort::new("service_retirement".into());
    register_control_adapter(&port, "browser_legacy", legacy_stream, false);
    register_control_adapter(&port, "browser_modern", modern_stream, true);
    let result = port.call_guarded(
        "browser_legacy",
        "workspace",
        BrowserCommand::CloseTab {
            tab_id: 41,
            released: true,
        },
        BrowserDispatch {
            attention: BrowserAttention::Background,
            deadline: Instant::now() + Duration::from_secs(2),
            cancelled: &AtomicBool::new(false),
            admit: &|| Ok(()),
        },
    );
    assert!(
        matches!(result, Err(BrowserError::CapabilityVersion { ref capability, .. }) if capability == adapter_capability::BROWSER_ATTENTION)
    );
    assert!(matches!(
        read_native::<BrowserFrame>(&mut legacy).unwrap(),
        Some(BrowserFrame::ControlState {
            state: RuntimeControlState::Ended,
            ..
        })
    ));
    assert!(
        matches!(read_native::<BrowserFrame>(&mut legacy).unwrap(), Some(BrowserFrame::Error { correlation: None, code, .. }) if code == super::super::ATTENTION_UPGRADE_CODE)
    );
    assert!(
        read_native::<BrowserFrame>(&mut legacy).unwrap().is_none(),
        "no physical close or Active may follow retirement"
    );
    assert_eq!(*lock(&port.control_state), RuntimeControlState::Active);
    port.publish_control_state(RuntimeControlState::Active)
        .unwrap();
    assert!(matches!(
        read_native::<BrowserFrame>(&mut modern).unwrap(),
        Some(BrowserFrame::ControlState {
            state: RuntimeControlState::Active,
            ..
        })
    ));
    assert!(!port.is_registered("browser_legacy"));
    assert!(port.is_registered("browser_modern"));
    assert!(port
        .browsers()
        .iter()
        .any(|browser| browser.id == "browser_legacy" && browser.attention_incompatible));
    assert_eq!(
        super::super::choose_browser(None, None, &port.browsers()).unwrap(),
        "browser_modern"
    );
    assert!(matches!(
        super::super::choose_browser(None, Some("browser_legacy"), &port.browsers()),
        Err(BrowserError::CapabilityVersion { .. })
    ));
}

fn send_attention_hello(peer: &mut TcpStream, modern: bool) {
    let mut capabilities = vec![capability(adapter_capability::TABS)];
    if modern {
        capabilities.push(capability(adapter_capability::BROWSER_ATTENTION));
    }
    write_native(
        peer,
        &BrowserFrame::Hello {
            major: ADAPTER_PROTOCOL_MAJOR,
            adapter_version: "1.3.12".into(),
            browser_id: "browser_legacy".into(),
            adapter_epoch: "adapter_legacy".into(),
            browser_name: Some("Chrome".into()),
            platform: Some(BrowserPlatform::Chromium),
            attended: false,
            capabilities,
        },
    )
    .unwrap();
}

#[test]
fn legacy_reconnect_and_fresh_background_service_never_restore_active_custody_but_upgrade_recovers()
{
    use ghostlight_bridge::browser::{BrowserAttention, RuntimeControlState};
    for _restart in 0..2 {
        let port = RelayBrowserPort::new("service_background".into());
        port.set_attention_source(Arc::new(|| BrowserAttention::Background));
        for _reconnect in 0..2 {
            let (mut peer, stream) = connected_pair();
            send_attention_hello(&mut peer, false);
            assert!(matches!(
                port.attach(stream),
                Err(BrowserError::CapabilityVersion { .. })
            ));
            assert!(matches!(
                read_native::<BrowserFrame>(&mut peer).unwrap(),
                Some(BrowserFrame::ControlState {
                    state: RuntimeControlState::Ended,
                    ..
                })
            ));
            assert!(matches!(
                read_native::<BrowserFrame>(&mut peer).unwrap(),
                Some(BrowserFrame::Error {
                    correlation: None,
                    ..
                })
            ));
            assert!(read_native::<BrowserFrame>(&mut peer).unwrap().is_none());
            assert!(!port.is_connected());
            assert!(matches!(
                super::super::choose_browser(None, None, &port.browsers()),
                Err(BrowserError::CapabilityVersion { .. })
            ));
            assert_eq!(*lock(&port.control_state), RuntimeControlState::Active);
        }
        let (mut peer, stream) = connected_pair();
        send_attention_hello(&mut peer, true);
        port.attach(stream).unwrap();
        assert!(matches!(
            read_native::<BrowserFrame>(&mut peer).unwrap(),
            Some(BrowserFrame::HelloAccepted {
                control_state: RuntimeControlState::Active,
                ..
            })
        ));
        assert!(port.is_connected());
        assert!(port
            .browsers()
            .iter()
            .all(|browser| !browser.attention_incompatible));
    }
}

#[test]
fn human_foreground_reveal_cannot_reacquire_legacy_custody_after_stop() {
    use ghostlight_bridge::browser::{BrowserAttention, RuntimeControlState};
    let (mut legacy, stream) = connected_pair();
    legacy
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    let port = RelayBrowserPort::new("service_legacy_reveal".into());
    register_control_adapter(&port, "browser_legacy", stream, false);
    *lock(&port.control_state) = RuntimeControlState::Ended;
    let result = port.call_guarded(
        "browser_legacy",
        "workspace",
        BrowserCommand::FocusTab { tab_id: 41 },
        BrowserDispatch {
            attention: BrowserAttention::Foreground,
            deadline: Instant::now() + Duration::from_secs(2),
            cancelled: &AtomicBool::new(false),
            admit: &|| Ok(()),
        },
    );
    assert!(
        matches!(result, Err(BrowserError::CapabilityVersion { ref capability, .. }) if capability == adapter_capability::BROWSER_ATTENTION)
    );
    let mut byte = [0];
    assert!(
        legacy.read(&mut byte).is_err(),
        "legacy reveal cannot dispatch focus or reacquire debugger custody"
    );
    assert_eq!(*lock(&port.control_state), RuntimeControlState::Ended);
}

#[test]
fn stale_legacy_retirement_cannot_stop_a_replacement_capable_adapter() {
    use ghostlight_bridge::browser::RuntimeControlState;
    let (_legacy, old_stream) = connected_pair();
    let (mut modern, updated_stream) = connected_pair();
    let port = RelayBrowserPort::new("service_retirement_replacement".into());
    register_control_adapter(&port, "browser_legacy", old_stream, false);
    let old_id = lock(&port.adapters).connections["browser_legacy"]
        .id
        .clone();
    register_control_adapter(&port, "browser_legacy", updated_stream, true);
    lock(&port.adapters)
        .connections
        .get_mut("browser_legacy")
        .unwrap()
        .id = "connection_updated".into();
    assert!(!port
        .retire_attention_incompatible_if_current("browser_legacy", &old_id)
        .unwrap());
    assert!(port.is_registered("browser_legacy"));
    assert!(port
        .browsers()
        .iter()
        .all(|browser| !browser.attention_incompatible));
    port.publish_control_state(RuntimeControlState::Active)
        .unwrap();
    assert!(
        matches!(
            read_native::<BrowserFrame>(&mut modern).unwrap(),
            Some(BrowserFrame::ControlState {
                state: RuntimeControlState::Active,
                ..
            })
        ),
        "a stale observation cannot send Ended to the upgraded replacement"
    );
}

#[test]
fn incoming_legacy_hello_cannot_quarantine_a_healthy_adapter_with_the_same_identity() {
    use ghostlight_bridge::browser::{BrowserAttention, RuntimeControlState};
    let (mut modern, modern_stream) = connected_pair();
    let port = RelayBrowserPort::new("service_hello_replacement".into());
    port.set_attention_source(Arc::new(|| BrowserAttention::Background));
    register_control_adapter(&port, "browser_legacy", modern_stream, true);
    let (mut old_peer, old_stream) = connected_pair();
    send_attention_hello(&mut old_peer, false);
    assert!(matches!(
        port.attach(old_stream),
        Err(BrowserError::CapabilityVersion { .. })
    ));
    assert!(matches!(
        read_native::<BrowserFrame>(&mut old_peer).unwrap(),
        Some(BrowserFrame::ControlState {
            state: RuntimeControlState::Ended,
            ..
        })
    ));
    assert!(matches!(
        read_native::<BrowserFrame>(&mut old_peer).unwrap(),
        Some(BrowserFrame::Error {
            correlation: None,
            ..
        })
    ));
    assert!(read_native::<BrowserFrame>(&mut old_peer)
        .unwrap()
        .is_none());
    assert!(port.is_registered("browser_legacy"));
    assert_eq!(port.browsers().len(), 1);
    assert!(port
        .browsers()
        .iter()
        .all(|browser| !browser.attention_incompatible));
    port.publish_control_state(RuntimeControlState::Active)
        .unwrap();
    assert!(matches!(
        read_native::<BrowserFrame>(&mut modern).unwrap(),
        Some(BrowserFrame::ControlState {
            state: RuntimeControlState::Active,
            ..
        })
    ));
}
