//! Prove runtime admission occurs after queued writer access and before transmission.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use super::super::{lock, BrowserDispatch, BrowserError, BrowserPort};
use super::{attach, navigated, port, TEST_BROWSER};
use crate::browser::adapter::chromium::protocol4::fake::Reply;
use crate::browser::adapter::chromium::protocol4::wire;
use crate::browser::adapter::BrowserCommand;
use crate::governance::ReasonCode;

/// How long the test watches for an admission check that runs while another holder still has
/// the writer. The window only bounds how long that failure has to show: a correct port is never
/// admitted inside it, however slowly the test runs.
const EARLY_ADMISSION_WINDOW: Duration = Duration::from_millis(120);

/// The address of the one command the adapter receives after each refused call.
const FOLLOW_UP_URL: &str = "https://example.com/after";

/// ACCEPTANCE EC4 and E3 (ADR-0185): a hold, a cancellation or a deadline that arrives while a
/// call waits for the browser's writer stops it before anything is written. Admission runs only
/// once the call holds the writer, the refusal names what stopped it, and the next command the
/// adapter receives is the first it ever sees.
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
        let released = AtomicBool::new(false);
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
                    BrowserCommand::Navigate {
                        tab_id: 5,
                        url: "https://example.com/".into(),
                    },
                    BrowserDispatch {
                        deadline,
                        cancelled: &cancelled,
                        admit: &|| {
                            admitted.send(released.load(Ordering::SeqCst)).unwrap();
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
            assert!(
                checked.recv_timeout(EARLY_ADMISSION_WINDOW).is_err(),
                "admission ran while another holder had the writer"
            );
            if case == 1 {
                cancelled.store(true, Ordering::SeqCst);
            } else {
                held.store(true, Ordering::SeqCst);
            }
            if case == 2 {
                // The writer comes free only once the call's deadline has passed.
                while Instant::now() < deadline {
                    thread::sleep(deadline.saturating_duration_since(Instant::now()));
                }
            }
            released.store(true, Ordering::SeqCst);
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
            checked.try_iter().all(|after_release| after_release),
            "admission ran before the writer was free"
        );
        assert!(!written.load(Ordering::SeqCst));

        // The adapter's stream is ordered, so the refused command, had it been written, would
        // come before the follow-up.
        let _ = port.call(
            TEST_BROWSER,
            BrowserCommand::Navigate {
                tab_id: 5,
                url: FOLLOW_UP_URL.into(),
            },
            Instant::now() + Duration::from_secs(5),
            &AtomicBool::new(false),
        );
        let follow_up = wire::NavigationTarget::Url {
            url: FOLLOW_UP_URL.into(),
        };
        let seen = fake.wait(|seen| {
            seen.requests.iter().any(|(_, primitive)| {
                matches!(primitive, wire::Primitive::Navigate(navigate) if navigate.to == follow_up)
            })
        });
        assert_eq!(
            seen.requests.len(),
            1,
            "no refused command may reach the adapter: {:?}",
            seen.requests
        );
    }
}
