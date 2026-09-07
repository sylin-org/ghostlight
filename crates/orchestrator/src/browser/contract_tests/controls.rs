//! Prove runtime admission occurs after queued writer access and before transmission.

use super::super::{lock, BrowserDispatch, Connection};
use super::*;
use crate::governance::ReasonCode;
use std::collections::HashMap;
use std::io::Read;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

#[test]
fn queued_dispatch_rechecks_control_cancellation_and_deadline_without_sending() {
    for case in 0..3 {
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
                capabilities: HashMap::from([(adapter_capability::TABS.into(), 1)]),
                liveness: None,
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
                        deadline,
                        cancelled: &cancelled,
                        admit: &|| {
                            admitted.send(()).unwrap();
                            if held.load(Ordering::SeqCst) {
                                Err(BrowserError::RuntimeControl(ReasonCode::RuntimeHold))
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
                    _ => BrowserError::DeadlineBeforeDispatch,
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
