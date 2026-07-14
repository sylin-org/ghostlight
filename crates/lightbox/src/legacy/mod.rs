// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial
//! Named parity scenarios for the 27 legacy spawn tests (ADR-0056 Decision 3). Each scenario moves
//! one process-boundary invariant into Lightbox; the parity ledger records the corresponding test.

use std::time::{Duration, Instant};

use anyhow::ensure;

use crate::scenarios::Scenario;
use crate::support::{self, TempRoot};

mod console;
mod hub;
mod lifecycle;

/// The migrated legacy scenario registry.
pub fn registry() -> Vec<Scenario> {
    let mut scenarios = vec![(
        "legacy-control-status",
        control_status as fn() -> anyhow::Result<()>,
    )];
    scenarios.extend(console::registry());
    scenarios.extend(hub::registry());
    scenarios.extend(lifecycle::registry());
    scenarios
}

/// A fresh production service answers the real control endpoint with no extension or sessions.
fn control_status() -> anyhow::Result<()> {
    let tmp = TempRoot::new("legacy-control-status")?;
    let endpoint = support::unique_endpoint("control-status");
    let _service = support::spawn_service(&endpoint, tmp.path())?;

    let deadline = Instant::now() + Duration::from_secs(15);
    let reply = loop {
        if let Some(reply) = ghostlight_transport::ipc::query_status(&endpoint) {
            break reply;
        }
        ensure!(
            Instant::now() < deadline,
            "the control status request never answered"
        );
        std::thread::sleep(Duration::from_millis(100));
    };
    ensure!(reply.hub == ghostlight_transport::handshake::HUB_PROTO);
    ensure!(!reply.extension_connected);
    ensure!(reply.live_sessions == 0);
    Ok(())
}
