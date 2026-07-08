// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Parent-death watchdog for the adapter role (ADR-0029; re-scoped from the pre-H6 "mcp-server"
//! role by ADR-0030 Decision 8, PINS.md SS5.5 -- the standalone SERVICE has no client parent and
//! idle-graces instead).
//!
//! The server's intended exit signal is stdin EOF, delivered when the MCP client closes the pipe.
//! On Windows that signal is unreliable: when the client is killed rather than closed cleanly (a
//! window reload, an auto-update that swaps the extension host, a crash), the child's stdin handle
//! is not always closed, so the blocking read behind `tokio::io::stdin()` never returns EOF and the
//! read loop blocks forever -- the process becomes an orphan that serves no one and never releases
//! the IPC endpoint. This module adds a second, reliable exit trigger: poll the parent's liveness
//! and end the process once the parent is gone.
//!
//! The polling loop is generic over the liveness predicate ([`wait_until`]) so it is unit-testable
//! without actually orphaning a process; the caller ([`crate::main`]'s server role) supplies the
//! real check via [`wait_until_orphaned`] and performs the process exit itself, keeping this module
//! free of `process::exit` so it stays testable.

use crate::proc::{self, ProcId};
use std::time::Duration;
use tokio::time::sleep;

/// How often to check that the parent is still alive. A small interval bounds how long an orphaned
/// server lingers before it exits and frees the endpoint; the check itself is a cheap syscall.
pub const POLL_INTERVAL: Duration = Duration::from_millis(1500);

/// Await until `is_orphaned` returns `true`, checking every `poll`. Generic over the predicate so
/// the loop can be tested deterministically without a real parent process to kill.
pub async fn wait_until<F: Fn() -> bool>(is_orphaned: F, poll: Duration) {
    loop {
        sleep(poll).await;
        if is_orphaned() {
            return;
        }
    }
}

/// Await until this process's `parent` has exited (ADR-0029 orphan definition), at [`POLL_INTERVAL`].
pub async fn wait_until_orphaned(parent: ProcId) {
    wait_until(move || proc::orphaned(parent), POLL_INTERVAL).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn returns_once_the_predicate_reports_orphaned() {
        // The predicate reports "not orphaned" twice, then "orphaned"; wait_until must poll past the
        // first two and return on the third, rather than returning early or looping forever.
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = calls.clone();
        wait_until(
            move || seen.fetch_add(1, Ordering::SeqCst) >= 2,
            Duration::from_millis(1),
        )
        .await;
        assert!(
            calls.load(Ordering::SeqCst) >= 3,
            "polled until the predicate flipped to orphaned"
        );
    }

    #[tokio::test]
    async fn does_not_return_while_the_predicate_stays_false() {
        // With a predicate that never reports orphaned, the watchdog must still be pending after
        // several poll intervals (it must not spuriously fire).
        let fut = wait_until(|| false, Duration::from_millis(1));
        tokio::pin!(fut);
        let raced = tokio::time::timeout(Duration::from_millis(30), &mut fut).await;
        assert!(raced.is_err(), "watchdog stayed pending while parent lived");
    }
}
