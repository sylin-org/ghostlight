//! Prove runtime admission occurs after queued writer access and before transmission.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use super::super::{lock, BrowserDispatch, BrowserError, BrowserPort};
use super::{attach, navigated, port, TEST_BROWSER};
use crate::browser::adapter::chromium::protocol4::fake::Reply;
use crate::browser::adapter::BrowserCommand;
use crate::governance::ReasonCode;

#[test]
fn queued_dispatch_rechecks_control_cancellation_and_deadline_without_sending() {
    for case in 0..3 {
        let port = port();
        let fake = attach(
            &port,
            TEST_BROWSER,
            Box::new(|correlation, _| Reply::Now(navigated(correlation))),
        );
        let writer =
            std::sync::Arc::clone(&lock(&port.adapters).connections[TEST_BROWSER].line.writer);
        let cancelled = AtomicBool::new(false);
        let written = AtomicBool::new(false);
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
                    BrowserCommand::Navigate {
                        tab_id: 5,
                        url: "https://example.com/".into(),
                    },
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
                        written: &written,
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
        thread::sleep(Duration::from_millis(50));
        assert!(
            fake.wait(|_| true).requests.is_empty(),
            "no command may reach the adapter"
        );
        assert!(!written.load(Ordering::SeqCst));
    }
}
